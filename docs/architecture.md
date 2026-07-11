# Architecture and Internals

This document explains how the `tdigest` crate is structured, how the t-digest algorithm works, and how the key operations are implemented.

## What is a t-digest?

A t-digest is a compact data structure for estimating quantiles (percentiles) of a dataset. It can handle billions of values while using only a few kilobytes of memory, and it provides especially accurate estimates at the extreme tails (e.g. p99, p99.9) -- precisely where accuracy matters most for latency monitoring and anomaly detection.

The algorithm was introduced by Ted Dunning and Otmar Ertl in their paper [*Computing Extremely Accurate Quantiles Using t-Digests*](https://arxiv.org/abs/1902.04023). This implementation follows Facebook's [folly TDigest](https://github.com/facebook/folly/blob/master/folly/stats/TDigest.cpp) variant.

## Core data structures

The entire library lives in a single file: `src/lib.rs`. There are two public types.

### `Centroid`

A centroid represents a cluster of nearby values, stored as a weighted mean:

```
Centroid {
    mean: f64,    // weighted mean of the values in this cluster
    weight: f64,  // number of values absorbed into this cluster
}
```

Centroids are ordered by `mean` using `f64::total_cmp`, which gives a total ordering over all `f64` values. Non-finite inputs are rejected by `debug_assert` at public ingestion boundaries and in the centroid constructor.

The `add` method merges additional weight into a centroid, updating its mean incrementally:

```
new_mean = (old_weight * old_mean + added_sum) / (old_weight + added_weight)
```

### `TDigest`

The digest itself holds a sorted vector of centroids plus summary statistics:

```
TDigest {
    centroids: Vec<Centroid>,  // sorted by mean
    max_size: usize,           // compression factor (max centroids to keep)
    sum: f64,                  // sum of all values added
    count: f64,                // total number of values added
    max: Option<f64>,          // maximum value seen (None if empty)
    min: Option<f64>,          // minimum value seen (None if empty)
}
```

Both types are marked `#[non_exhaustive]`, so new fields can be added in future versions without breaking downstream code.

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

```
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

```
                          ┌──────────────────────┐
  existing centroids ────>│                      │
                          │  two-way sorted merge │──> compressed centroids
  new sorted values ─────>│  with k_to_q budget  │
                          └──────────────────────┘
```

The result is always bounded by `max_size` centroids.

### `merge_unsorted` -- Adding unsorted data

Simply sorts the input using `f64::total_cmp`, then delegates to `merge_sorted`. This is a convenience method.

### `merge_digests` -- Combining multiple digests

Used for distributed or parallel computation. The algorithm:

1. **Collect** all centroids from all input digests into a single vector, tracking the global `min`, `max`, and `count`.
2. **Sort** all centroids by mean.
3. **Compress** using the same `k_to_q` budgeting as `merge_sorted`, walking through the sorted centroids and merging adjacent ones that fit within the budget.

This is what makes t-digests "mergeable" -- you can build independent digests on different machines/threads and combine them without loss of accuracy guarantees.

### `estimate_quantile` -- Querying

Given a quantile `q` in [0.0, 1.0], estimate the corresponding value:

1. **Edge cases**: return `None` for an empty digest; return `min` for q=0; return `max` for q=1.
2. **Locate the centroid**: compute `rank = q * count`, then walk the centroids to find which one contains that rank. For `q > 0.5`, walk from the right (high values) for better numerical accuracy; for `q <= 0.5`, walk from the left.
3. **Interpolate within the centroid**: use linear interpolation between the centroid's mean and its neighbors to estimate the exact value at the desired rank. The result is clamped between the neighboring centroids' means (or `min`/`max` for edge centroids).

```
                   centroid[pos-1]    centroid[pos]    centroid[pos+1]
                        |                 |                 |
    value:         ─────●─────────────────●─────────────────●─────
                                          ^
                                    interpolated value
```

## Ingestion APIs

The original batch methods (`merge_sorted`, `merge_unsorted`) consume `&self` and return a new `TDigest`. This makes the API naturally thread-safe for read-heavy workloads and simplifies reasoning about state:

```rust
let mut t = TDigest::new_with_size(100);
for batch in batches {
    t = t.merge_sorted(batch);
}
```

For one-at-a-time ingestion, `push` and `extend_values` retain values in a
buffer sized at five times the compression factor. Summary statistics update
immediately; when the buffer fills, it is sorted and compressed with the existing
centroids. Call `flush` before `estimate_quantile`, `estimate_rank`,
`trimmed_mean`, `centroids`, immutable merges, or serialization. Those operations
use a debug assertion to catch an unflushed buffer without adding interior
mutability or weakening `Sync`.

## Measured accuracy

Run `cargo run --release --example accuracy` to compare estimates with exact
quantiles for one million deterministic samples from six distributions. With
`max_size = 100`, central quantiles on smooth distributions generally have
relative error around 1e-4. At the extreme 0.0001 and 0.9999 quantiles, skewed
distributions can reach errors on the order of 1e-2 to 1e-1. Merging 100 partial
digests increases some extreme-tail errors; near zero, consult absolute error
because relative error exaggerates very small differences. A deliberately
bimodal distribution also exposes interpolation across the gap around its
median, which is useful as a regression signal for future interpolation work.

## Serde support

When the `use_serde` feature is enabled, both `TDigest` and `Centroid` derive `Serialize` and `Deserialize`. This allows digests to be stored, transmitted over the network, or checkpointed. The `serde` dependency is compiled with `std` support to avoid issues with `f64` serialization.

Note: the wire format changed in 1.0.0 (`min`/`max` changed from `f64` with NaN sentinel to `Option<f64>`). Data from older versions requires migration.

## Error handling

The library avoids panicking in normal operation:

- `estimate_quantile`, `mean`, `min`, and `max` return `Option<f64>`, yielding `None` for empty digests.
- Constructors and ingestion methods use `debug_assert` to catch non-finite values and invalid states during development, but these are compiled out in release builds.

## File layout

```
t-digest/
├── src/
│   └── lib.rs           # All library code and tests
├── docs/
│   └── architecture.md  # This file
├── Cargo.toml           # Package metadata, features, dependencies
├── CHANGELOG.md         # Version history
├── LICENSE              # Apache-2.0
├── rustfmt.toml         # Formatting config (max_width = 120)
└── .github/
    └── workflows/
        └── CI.yml       # GitHub Actions: check, test, MSRV, coverage, fmt
```
