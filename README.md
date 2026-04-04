# T-Digest algorithm in rust

[![CI](https://github.com/MnO2/t-digest/actions/workflows/CI.yml/badge.svg)](https://github.com/MnO2/t-digest/actions/workflows/CI.yml)
[![codecov](https://codecov.io/gh/MnO2/t-digest/branch/master/graph/badge.svg)](https://codecov.io/gh/MnO2/t-digest)

This implementation is following Facebook folly's [implementation](https://github.com/facebook/folly/blob/master/folly/stats/TDigest.cpp)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tdigest = "1.0"
```

## Example

```rust
use tdigest::TDigest;

let t = TDigest::new_with_size(100);
let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();
let t = t.merge_sorted(values);
let ans = t.estimate_quantile(0.99).unwrap();
let expected: f64 = 990_000.0;
let percentage: f64 = (expected - ans).abs() / expected;

assert!(percentage < 0.01);
```
