# Architecture and Internals

This document explains how the `tdigest` crate is structured, how the t-digest algorithm works, and how the key operations are implemented.

## What is a t-digest?

A t-digest is a compact data structure for estimating quantiles (percentiles) of a dataset. Its retained state grows with the configured compression factor instead of the number of samples. It allocates finer resolution to the extreme tails (e.g. p99, p99.9), which is useful for latency monitoring and anomaly detection. Accuracy depends on the input distribution and ingestion pattern; this implementation does not promise a distribution-independent error bound.

The algorithm was introduced by Ted Dunning and Otmar Ertl in their paper [*Computing Extremely Accurate Quantiles Using t-Digests*](https://arxiv.org/abs/1902.04023). This implementation follows Facebook's [folly TDigest](https://github.com/facebook/folly/blob/master/folly/stats/TDigest.cpp) variant.

## Core data structures

The entire library lives in a single file: `src/lib.rs`. There are two public types.

### `Centroid`

A centroid represents a cluster of nearby values, stored as a weighted mean:

```text
Centroid {
    mean: f64,    // weighted mean of the values in this cluster
    weight: f64,  // number of values absorbed into this cluster
}
```

Centroids are ordered by `mean`, then by `weight` to break ties, using
`f64::total_cmp`. Equality follows the same total ordering. Non-finite inputs
are rejected by `debug_assert` at public ingestion boundaries and in the
centroid constructor.

The `add` method merges additional weight into a centroid, updating its mean incrementally:

```text
new_mean = (old_weight * old_mean + added_sum) / (old_weight + added_weight)
```

Internal compression and trimmed means interpolate centroid means using
normalized weights instead of forming a full weighted sum first. Interpolation
also avoids subtracting opposite-sign extremes directly. `Centroid::add` uses
this path when the added sum divided by its positive weight is finite; its
returned sum can still overflow.

### `TDigest`

The digest itself holds a sorted vector of centroids plus summary statistics:

```text
TDigest {
    centroids: Vec<Centroid>,  // sorted by mean
    max_size: usize,           // positive compression target
    sum: f64,                  // sum of all values added
    count: f64,                // total number of values added
    max: Option<f64>,          // maximum value seen (None if empty)
    min: Option<f64>,          // minimum value seen (None if empty)
    buffer: Vec<f64>,          // pending values from mutable ingestion
}
```

Both types are marked `#[non_exhaustive]`, so new fields can be added in future versions without breaking downstream code.

`count`, `sum`, `min`, and `max` include pending buffered values. The centroids
include them only after `flush`. `mean` normally divides the stored sum by count.
When the sum is non-finite, it falls back to an approximate weighted mean of the
centroids and buffered samples. The sum itself can overflow even when every
input is finite, and integer counts cease to be exact above `2^53`.

For ordinary ingestion, use `new_with_size` or `default`. The lower-level `new`
constructor imports weighted centroids and caller-supplied summary statistics;
those statistics must describe the same samples as the centroids. It sorts the
centroids and recompresses them at the requested size when needed. Zero is not
a valid compression size.

## The compression function: `k_to_q`

At the heart of the t-digest is a *scale function* that maps centroid positions to quantile space. This function determines how many values each centroid is allowed to absorb, and it is what gives t-digests their characteristic accuracy profile.

The implementation uses a quadratic scale function:

```rust
fn k_to_q(k: f64, d: f64) -> f64 {
    let k_div_d = k / d;
    if k_div_d >= 0.5 {
        let base = 1.0 - k_div_d;
        1.0 - 2.0 * base * base
    } else {
        2.0 * k_div_d * k_div_d
    }
}
```

Where:
- `k` is the centroid index (1, 2, 3, ...)
- `d` is the `max_size` (compression factor)

This produces a non-linear mapping: centroids near the tails (q close to 0 or 1) are allocated **less** quantile space, meaning they represent fewer values and give finer-grained resolution. Centroids near the median are allocated more space and can absorb more values.

**Visual intuition:**

```text
Quantile:  0.0          0.5          1.0
           |--- fine ---|-- coarse ---|--- fine ---|
           Few values    Many values    Few values
           per centroid  per centroid   per centroid
```

This is why t-digests are especially accurate at the extremes.

## Key operations

### `merge_sorted` -- Adding data to a digest

This is the primary method for building a digest from raw values. It implements a single-pass streaming merge:

1. **Initialize** a new result digest. Compute new `count`, `min`, and `max`.
2. **Set up two iterators**: one over the existing centroids, one over the incoming sorted values.
3. **Merge in sorted order**: at each step, pick whichever iterator has the smaller value. This is a classic two-way merge (like merge sort's merge step).
4. **Compress on the fly**: maintain a running weight. As long as the accumulated weight stays below the threshold from `k_to_q`, absorb the next value into the current centroid. When the threshold is exceeded, finalize the current centroid and start a new one.
5. **Finalize**: sort the compressed centroids (they may be slightly out of order due to mean updates) and store them.

```text
                          ┌──────────────────────┐
  existing centroids ────>│                      │
                          │  two-way sorted merge │──> compressed centroids
  new sorted values ─────>│  with k_to_q budget  │
                          └──────────────────────┘
```

Compression retains at most `max_size` centroids. The final bucket absorbs all
remaining weight, including any difference caused by floating-point summation
order. The size is an upper bound rather than a promised centroid count or an
exact memory budget.

### `merge_unsorted` -- Adding unsorted data

Simply sorts the input using `f64::total_cmp`, then delegates to `merge_sorted`. This is a convenience method.

### `merge_digests` -- Combining multiple digests

Used for distributed or parallel computation. The algorithm:

1. **Collect** all centroids from all input digests into a single vector, tracking the global `min`, `max`, and `count`.
2. **Sort** all centroids by mean.
3. **Compress** using the same `k_to_q` budgeting as `merge_sorted`, walking through the sorted centroids and merging adjacent ones that fit within the budget.

The result uses the largest `max_size` among all inputs, including empty
digests. An empty input vector returns an empty digest with the default size of
100. Recompression is approximate: splitting the same samples into batches or
changing merge order can change the resulting centroids and estimates.

### `estimate_quantile` -- Querying

Given a quantile `q` in [0.0, 1.0], estimate the corresponding value:

1. **Edge cases**: return `None` for an empty digest; return `min` for q=0; return `max` for q=1.
2. **Locate the centroid**: compute `rank = q * count`, then walk the centroids to find which one contains that rank. For `q > 0.5`, walk from the right (high values) for better numerical accuracy; for `q <= 0.5`, walk from the left.
3. **Interpolate between rank midpoints**: place each centroid's mean at the center of its cumulative-weight interval, then interpolate toward the adjacent centroid center on the requested side. At the tails, interpolate toward `min` or `max`. This keeps the estimate continuous across centroid boundaries even when neighboring weights or value gaps differ.

```text
                   centroid[pos-1]    centroid[pos]    centroid[pos+1]
                        |                 |                 |
    value:         ─────●─────────────────●─────────────────●─────
                                          ^
                                    interpolated value
```

Quantiles outside `[0, 1]` return the nearest endpoint. For several quantiles,
`quantiles` builds cumulative centroid weights once and uses binary search for
each requested rank. Inputs need not be sorted, and outputs retain input order.
It uses the same interpolation rule as `estimate_quantile`. Empty query slices
return immediately, and a single query uses the scalar path without allocating
a cumulative-weight vector.

### `estimate_rank` and `trimmed_mean`

`estimate_rank` approximates the CDF by interpolating between centroid ranks and
the observed endpoints. It returns a fraction in `[0, 1]`, or `None` for an empty
digest. This is a smoothed estimate rather than an exact empirical count of
samples less than or equal to the query value, especially at repeated values.
Values at or below the minimum return zero, and values at or above the maximum
return one. When all samples are equal, the rank at that constant value is one.

`trimmed_mean(lo, hi)` walks the centroids and weights each centroid's mean by
its overlap with the requested quantile interval. It clamps bounds to `[0, 1]`
and returns `None` for an empty digest, NaN bounds, or an empty interval.
Partial-centroid contributions assume that the centroid mean represents the
retained portion, so this is also approximate.

## Ingestion APIs

The batch methods (`merge_sorted`, `merge_unsorted`) borrow `&self` and return a
new `TDigest`. They consume the supplied value vector. Assign the return value
to retain the new samples, and use `merge_sorted` only with ascending input:

```rust
use tdigest::TDigest;

let batches = [vec![1.0, 2.0], vec![3.0, 4.0]];
let mut t = TDigest::new_with_size(100);
for batch in batches {
    t = t.merge_sorted(batch);
}
```

For one-at-a-time ingestion, `push` and `extend_values` retain values in a
buffer sized at five times the compression factor. The buffer is allocated on
the first mutable insertion and retained across flushes. Summary statistics update
immediately; when the buffer fills, it is sorted and compressed with the existing
centroids. `Extend<f64>` uses the same path. Call `flush` before
`estimate_quantile`, `quantiles`, `estimate_rank`, `trimmed_mean`, `centroids`,
immutable merges, or serialization. Queries and immutable merges use debug
assertions to catch an unflushed buffer. Serialization does not check it, and
omits pending values while retaining their summary statistics. Violating the
flush contract can therefore produce incomplete or inconsistent data.

`FromIterator<f64>` creates a default-size digest, ingests the iterator through
the buffer, and flushes before returning. `flush` is a no-op when no values are
pending. Query methods take `&self` and do not flush automatically; the type
retains `Send` and `Sync` without interior mutability.

## Time and space costs

Let `m` be `max_size`, `c = O(m)` the retained centroid count, `n` the new batch
size, `C` the total centroid count across input digests, and `k` the number of
requested quantiles. The final centroid sort is included in these bounds.

| Operation | Time | Additional working storage |
|-----------|------|----------------------------|
| `merge_sorted` | `O(n + c log c)` | `O(c)` result, plus the supplied `O(n)` vector |
| `merge_unsorted` | `O(n log n + c log c)` | As above; input sorting is in place |
| `push` / `extend_values` | Amortized `O(log m)` per value | `O(m)` buffer and compression result |
| `flush` | `O(m log m)` when the buffer is full | `O(m)` compression result; reuses the buffer |
| `merge_digests` | `O(C log C)` | `O(C + m)` collected centroids and result |
| `estimate_quantile` / `estimate_rank` / `trimmed_mean` | `O(c)` | `O(1)` |
| `quantiles` | `O(c + k log c)` | `O(c + k)` cumulative weights and output |
| `mean` | `O(1)` normally; `O(c + m)` if the sum is non-finite | `O(1)` |
| Other summary accessors | `O(1)` | `O(1)` |

The amortized ingestion bound assumes the buffer is allowed to fill. Flushing
after every insertion repeatedly recompresses the existing centroids. Batch
queries amortize the cumulative-weight pass when requesting many percentiles;
a single `estimate_quantile` avoids that allocation.

Each centroid holds 16 bytes of payload. A full mutable ingestion buffer adds
`5 * m` values, or `40 * m` bytes; batch-only digests do not allocate it.
Vector capacity, the digest struct, allocation overhead, input vectors, and
temporary merge storage are additional costs. See
the [README memory table](../README.md#compression-factor) for representative
payload sizes.

## Measured accuracy

Run `cargo run --release --example accuracy` to compare estimates for one million
deterministic samples from six distributions. The reference is the sorted sample
at index `round(q * (n - 1))`. The `single` mode ingests one sorted batch;
`streamed` merges 100 independently built digests, rather than using `push`.

Central quantiles on smooth distributions can have much smaller relative error
than extreme tails. For example, at `max_size = 100`, the harness's fixed-seed
lognormal batch has about 0.054% relative error at the median and 8.6% at q=0.9999.
These are observations for that dataset, not error guarantees. Merging partial
digests can improve some estimates and worsen others. Near zero, consult
absolute error because relative error exaggerates small differences; inspect
the bimodal median to see the effect of interpolation across a sparse region.

## Serde support

When the `serde` feature is enabled, both `TDigest` and `Centroid` support
`Serialize` and `Deserialize`. Serialization is derived, and deserialization
validates the data. The deprecated `use_serde` feature is a compatibility alias.
Serde uses its `alloc` support and also works when the default `std` feature is
disabled.

The ingestion buffer is skipped and defaults to empty when deserialized.
Serialize only after flushing. Deserialization rejects invalid compression
sizes, non-finite or negative counts, inconsistent empty states, invalid
extrema, nonpositive digest-centroid weights, centroid means outside the
extrema, unsorted centroids, and count/weight mismatches beyond floating-point
tolerance. A standalone `Centroid` also requires a finite mean and finite,
nonnegative weight; a zero-weight accumulator is allowed outside a digest.

The sum of a nonempty digest may be non-finite after valid finite ingestion, so
deserialization permits that state when the format can represent it. JSON
cannot round-trip an infinite or NaN sum; for exact finite-float round trips
with `serde_json`, enable its `float_roundtrip` feature. Validation checks structural
consistency; it does not reconstruct the original samples or prove that a
supplied sum matches them.

In 1.0.0, `min`/`max` changed from `f64` with a NaN sentinel to `Option<f64>`.
Compatibility with older serialized data depends on the format; binary formats
that encode the option discriminator require migration.

## `no_std` support

The library is always compiled with `#![no_std]` and uses `alloc::vec::Vec` for
storage. The default `std` feature preserves the conventional host build and
enables serde's `std` support when serde is active. Building with
`--no-default-features` requires only an allocator.

## Error handling

- Queries that need samples return `Option<f64>`, yielding `None` for empty
  digests. `quantiles` returns one `None` per requested quantile for an empty
  digest; `count` and `sum` return zero.
- A compression size of zero is rejected by constructors in every build.
- Ingestion requires finite values. Debug assertions catch NaN and infinity,
  along with several state invariants, but release builds omit these checks.
  Estimates from contract-violating inputs are unspecified.
- `estimate_quantile`, `quantiles`, and `estimate_rank` require non-NaN query
  arguments. `trimmed_mean` explicitly returns `None` for NaN bounds.
- Centroid queries and immutable merges require a flushed ingestion buffer in
  all builds, even though only debug builds check the precondition.

## File layout

```text
t-digest/
├── src/
│   └── lib.rs           # Library code, unit tests, and property tests
├── tests/               # Public API regression tests
├── docs/
│   └── architecture.md  # This file
├── benches/
│   └── tdigest.rs       # Criterion ingestion, merge, and query benchmarks
├── examples/
│   └── accuracy.rs      # Deterministic distribution accuracy comparison
├── fuzz/
│   └── fuzz_targets/
│       └── merge.rs     # cargo-fuzz merge and query invariants
├── Cargo.toml           # Package metadata, features, dependencies
├── CHANGELOG.md         # Version history
├── LICENSE              # Apache-2.0
├── rustfmt.toml         # Formatting config (max_width = 120)
└── .github/
    └── workflows/
        ├── CI.yml       # Tests, MSRV, no_std, lint, docs, semver, coverage
        └── fuzz.yml     # Manually triggered nightly fuzz smoke run
```
