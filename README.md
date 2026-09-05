# t-digest

[![CI](https://github.com/MnO2/t-digest/actions/workflows/CI.yml/badge.svg)](https://github.com/MnO2/t-digest/actions/workflows/CI.yml)
[![codecov](https://codecov.io/gh/MnO2/t-digest/branch/master/graph/badge.svg)](https://codecov.io/gh/MnO2/t-digest)
[![crates.io](https://img.shields.io/crates/v/tdigest.svg)](https://crates.io/crates/tdigest)
[![docs.rs](https://docs.rs/tdigest/badge.svg)](https://docs.rs/tdigest)
[![License: Apache-2.0](https://img.shields.io/crates/l/tdigest.svg)](LICENSE)

A Rust implementation of the [t-digest](https://arxiv.org/abs/1902.04023) data structure for accurate online accumulation of rank-based statistics such as quantiles and trimmed means, using a variant of 1-dimensional k-means clustering.

This implementation follows Facebook's [folly TDigest](https://github.com/facebook/folly/blob/master/folly/stats/TDigest.cpp).

## Features

- **Approximate quantiles** with finer resolution at the tails (p99, p99.9)
- **Mergeable** -- combine digests computed on different machines or threads
- **Compact** -- retained state scales with the compression factor, rather than the number of samples
- **No dependencies** by default (optional `serde` support behind a feature flag)
- **`no_std` + `alloc` support**, including serde without its `std` feature

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tdigest = "1.0"
```

### Optional features

| Feature | Description |
|-------------|------------------------------------------------|
| `std` | Enabled by default; disable it for `no_std` builds |
| `serde` | Enables `Serialize`/`Deserialize` for `TDigest` and `Centroid` |
| `use_serde` | Deprecated compatibility alias for `serde` |

```toml
[dependencies]
tdigest = { version = "1.0", features = ["serde"] }
```

## Quick start

```rust
use tdigest::TDigest;

// Create a digest with a compression factor of 100
let t = TDigest::new_with_size(100);

// Feed it one million values
let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();
let t = t.merge_sorted(values);

// Estimate quantiles
let p99 = t.estimate_quantile(0.99).unwrap();
let expected = 990_000.0;
assert!((expected - p99).abs() / expected < 0.01);
```

## Usage

### Creating a digest

```rust
use tdigest::TDigest;

// With explicit compression factor (controls accuracy vs. memory)
let t = TDigest::new_with_size(100);

// With default settings (max_size = 100)
let t = TDigest::default();
```

### Adding data

```rust
use tdigest::TDigest;

let t = TDigest::new_with_size(100);

// Pre-sorted data (fastest)
let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
let t = t.merge_sorted(sorted);

// Unsorted data (sorts internally)
let unsorted = vec![5.0, 3.0, 1.0, 4.0, 2.0];
let t = t.merge_unsorted(unsorted);
```

`merge_sorted` requires ascending input. Use `merge_unsorted` when the order is
unknown. Both methods borrow the existing digest and return a new one; keep the
returned value to retain the inserted data.

For streaming ingestion, use the buffered mutable API and flush before a
centroid-based query:

```rust
use tdigest::TDigest;

let mut t = TDigest::new_with_size(100);
for value in [5.0, 3.0, 1.0, 4.0, 2.0] {
    t.push(value);
}
t.flush();
assert_eq!(t.estimate_quantile(0.5), Some(3.0));
```

`push`, `extend_values`, and the standard `Extend<f64>` implementation update
`count`, `sum`, `mean`, `min`, `max`, and `is_empty` immediately. Call `flush`
before `estimate_quantile`, `quantiles`, `estimate_rank`, `trimmed_mean`,
`centroids`, immutable merges, or serialization. Automatic compression happens
when the buffer fills, so a partially filled buffer still needs an explicit flush.

Collecting an iterator creates a digest with the default compression factor and
flushes it before returning:

```rust
use tdigest::TDigest;

let t: TDigest = [5.0, 1.0, 3.0].into_iter().collect();
assert_eq!(t.estimate_quantile(0.5), Some(3.0));
```

### Querying

```rust
use tdigest::TDigest;

let t = TDigest::new_with_size(100);
let t = t.merge_sorted(vec![1.0, 2.0, 3.0, 4.0, 5.0]);

// Returns None for empty digests, Some(value) otherwise
let median = t.estimate_quantile(0.5);  // ~3.0
let min    = t.min();                    // Some(1.0)
let max    = t.max();                    // Some(5.0)
let mean   = t.mean();                   // Some(3.0)
let count  = t.count();                  // 5.0
let sum    = t.sum();                    // 15.0

// Estimate several quantiles in one call; input order is preserved
let percentiles = t.quantiles(&[0.5, 0.9, 0.99]);

// Approximate CDF and the mean of the middle 80% of the distribution
let rank = t.estimate_rank(3.0);
let middle_mean = t.trimmed_mean(0.1, 0.9);
```

Quantiles at or below 0 return the minimum; quantiles at or above 1 return the
maximum. `trimmed_mean` clamps its bounds to `[0, 1]` and returns `None` for an
empty interval or NaN bound. Rank and trimmed-mean queries also return `None`
for empty digests. These queries interpolate compressed data and need not match
an exact sample percentile or empirical CDF.

### Merging digests (distributed / parallel use)

```rust
use tdigest::TDigest;

// Build digests independently (e.g. on different threads)
let data: Vec<f64> = (1..=10_000).map(f64::from).collect();
let mut digests = Vec::new();
for chunk in data.chunks(1000) {
    let t = TDigest::new_with_size(100);
    let t = t.merge_unsorted(chunk.to_vec());
    digests.push(t);
}

// Merge into a single digest
let combined = TDigest::merge_digests(digests);
let p99 = combined.estimate_quantile(0.99);
```

Flush any digests built with mutable ingestion before merging. The result uses
the largest input `max_size`; an empty input vector produces an empty digest
with the default size of 100. Repeated merging can change estimates, so measure
accuracy with the merge pattern used by your application.

### Serialization (serde)

Enable the `serde` feature, then use any serde-compatible format. The deprecated
`use_serde` alias remains available for existing users.

```rust
use tdigest::TDigest;

let t = TDigest::new_with_size(100);
let t = t.merge_sorted(vec![1.0, 2.0, 3.0]);

let json = serde_json::to_string(&t).unwrap();
let restored: TDigest = serde_json::from_str(&json).unwrap();
```

To run this JSON example with exact finite-float round trips, add
`serde_json = { version = "1.0", features = ["float_roundtrip"] }` to your
dependencies. Without that feature, JSON parsing can introduce small rounding
differences in centroid values.

The pending ingestion buffer is omitted from serialization. Always call `flush`
after mutable ingestion: serializing without it succeeds but loses pending
samples while retaining their summary statistics. Deserialization validates
centroid ordering, weights, extrema, and count consistency, and rejects
inconsistent payloads such as an omitted buffer that leaves a count mismatch.
The 1.0 format uses `Option<f64>` for `min` and `max`; data written by older
versions may need migration, depending on the serialization format.

## Compression factor

The positive `max_size` parameter controls the trade-off between accuracy and
memory; zero is rejected. The default is 100.

| `max_size` | Centroid payload (approx.) | Full ingestion buffer |
|------------|----------------------------|-----------------------|
| 50         | 0.8 KB                     | 2 KB                  |
| 100        | 1.6 KB                     | 4 KB                  |
| 200        | 3.2 KB                     | 8 KB                  |
| 500        | 8 KB                       | 20 KB                 |

Each centroid holds two `f64` values (16 bytes); the streaming buffer holds up to
`5 * max_size` values (8 bytes each), allocated on the first mutable insertion
and reused after flushing. Batch-only digests do not allocate this buffer.
These are payload estimates in decimal KB, excluding the digest struct, spare
vector capacity, allocator overhead, input vectors, and temporary storage during
merges. `max_size` is a compression target, not an exact byte budget.

Larger values generally improve accuracy at the cost of more memory and merge
work. Start with the default of 100, then measure on your own distribution.

## Input and numeric limits

Inserted values must be finite. NaN and positive or negative infinity violate
the ingestion contract: debug builds reject them, while release-build estimates
are unspecified. NaN also violates the contract for `estimate_quantile`,
`quantiles`, and `estimate_rank`; `trimmed_mean` instead returns `None` for a NaN
bound.

Counts and sums use `f64`. Integer counts are exact only through `2^53`, and a
sum of finite values can overflow to infinity or NaN. `mean` normally computes
`sum / count`; when the sum is non-finite, it falls back to an approximate
weighted mean of the centroids and pending values. This fallback takes linear
time in the retained state. `sum` remains non-finite: formats such as JSON cannot
round-trip that value. Scale inputs when you need a finite sum or JSON storage.

## Minimum Supported Rust Version (MSRV)

Rust **1.62** -- verified in CI.

## `no_std`

Disable default features to use the crate with `alloc` but without `std`:

```toml
[dependencies]
tdigest = { version = "1.0", default-features = false }
```

Serde also works in this configuration by adding `features = ["serde"]`.

## Accuracy

Run `cargo run --release --example accuracy` to compare estimated and exact
quantiles across deterministic uniform, normal, lognormal, exponential, bimodal,
and adversarial distributions. See [architecture and internals](docs/architecture.md#measured-accuracy)
for a summary of the measured error profile.

The crate does not provide a distribution-independent error bound. Tail
resolution improves with compression size, but skew, repeated values, gaps in
the distribution, batch boundaries, and merge order can affect estimates.

## Benchmarks

Run `cargo bench` to measure sorted and unsorted batch ingestion, incremental
ingestion, digest merging, quantile queries, and compression-size sensitivity.
The benchmark inputs use fixed random seeds so results can be compared across
changes. Run `cargo run --release --example accuracy` for the accuracy harness.

## Documentation

- [API docs on docs.rs](https://docs.rs/tdigest)
- [Architecture and internals](docs/architecture.md)

## License

Apache-2.0 -- see [LICENSE](LICENSE) for details.
