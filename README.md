# t-digest

[![CI](https://github.com/MnO2/t-digest/actions/workflows/CI.yml/badge.svg)](https://github.com/MnO2/t-digest/actions/workflows/CI.yml)
[![codecov](https://codecov.io/gh/MnO2/t-digest/branch/master/graph/badge.svg)](https://codecov.io/gh/MnO2/t-digest)
[![crates.io](https://img.shields.io/crates/v/tdigest.svg)](https://crates.io/crates/tdigest)
[![docs.rs](https://docs.rs/tdigest/badge.svg)](https://docs.rs/tdigest)
[![License: Apache-2.0](https://img.shields.io/crates/l/tdigest.svg)](LICENSE)

A Rust implementation of the [t-digest](https://arxiv.org/abs/1902.04023) data structure for accurate online accumulation of rank-based statistics such as quantiles and trimmed means, using a variant of 1-dimensional k-means clustering.

This implementation follows Facebook's [folly TDigest](https://github.com/facebook/folly/blob/master/folly/stats/TDigest.cpp).

## Features

- **Accurate quantile estimation** with bounded error, especially at the tails (p99, p999)
- **Mergeable** -- combine digests computed on different machines or threads
- **Compact** -- fixed memory footprint regardless of input size
- **No dependencies** by default (optional `serde` support behind a feature flag)
- **`no_std`-friendly** when serde is disabled

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tdigest = "1.0"
```

### Optional features

| Feature | Description |
|-------------|------------------------------------------------|
| `use_serde` | Enables `Serialize`/`Deserialize` for `TDigest` and `Centroid` |

```toml
[dependencies]
tdigest = { version = "1.0", features = ["use_serde"] }
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
```

### Merging digests (distributed / parallel use)

```rust
use tdigest::TDigest;

// Build digests independently (e.g. on different threads)
let mut digests = Vec::new();
for chunk in data.chunks(1000) {
    let t = TDigest::new_with_size(100);
    let t = t.merge_sorted(chunk.to_vec());
    digests.push(t);
}

// Merge into a single digest
let combined = TDigest::merge_digests(digests);
let p99 = combined.estimate_quantile(0.99);
```

### Serialization (serde)

Enable the `use_serde` feature, then use any serde-compatible format:

```rust
use tdigest::TDigest;

let t = TDigest::new_with_size(100);
let t = t.merge_sorted(vec![1.0, 2.0, 3.0]);

let json = serde_json::to_string(&t).unwrap();
let restored: TDigest = serde_json::from_str(&json).unwrap();
```

## Compression factor

The `max_size` parameter controls the trade-off between accuracy and memory:

| `max_size` | Memory (approx.) | Accuracy |
|------------|-------------------|----------|
| 50         | ~1.2 KB           | Good for rough estimates |
| 100        | ~2.4 KB           | Good default |
| 200        | ~4.8 KB           | High accuracy |
| 500        | ~12 KB            | Very high accuracy |

Larger values produce more centroids, giving better accuracy at the cost of more memory and slightly slower merges. A value of 100 is sufficient for most use cases.

## Minimum Supported Rust Version (MSRV)

Rust **1.62** -- verified in CI.

## Documentation

- [API docs on docs.rs](https://docs.rs/tdigest)
- [Architecture and internals](docs/architecture.md)

## License

Apache-2.0 -- see [LICENSE](LICENSE) for details.
