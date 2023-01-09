# T-Digest algorithm in rust

[![Build Status](https://travis-ci.com/MnO2/t-digest.svg?branch=master)](https://travis-ci.com/MnO2/t-digest)
[![codecov](https://codecov.io/gh/MnO2/t-digest/branch/master/graph/badge.svg)](https://codecov.io/gh/MnO2/t-digest)

This implementation is following Facebook folly's [implementation](https://github.com/facebook/folly/blob/master/folly/stats/TDigest.cpp)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tdigest = "0.2"
```

then you are good to go. If you are using Rust 2015 you have to ``extern crate tdigest`` to your crate root as well.

## Example

```rust
use tdigest::TDigest;

let t = TDigest::new_with_size(100);
let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();
let t = t.merge_sorted(values);
let ans = t.estimate_quantile(0.99);
let expected: f64 = 990_000.0;
let percentage: f64 = (expected - ans).abs() / expected;

assert!(percentage < 0.01);
```

Or, if you want to report with controlled memory and do not already have
all of your values collected in a vector:
```rust
use tdigest::online::OnlineTdigest;

let t = OnlineTdigest::default();

// You can record observations on any thread. The amortized cost
// is a few tens of nanoseconds.
(1..1_000_000).for_each(|i| t.observe(i))

// You can get() or reset() at any time. This gives you a TDigest.
let digest = t.get();
```
