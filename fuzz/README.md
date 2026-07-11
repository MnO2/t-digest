# Fuzzing

Install `cargo-fuzz` and run the merge target with a nightly toolchain:

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run merge
```

The target accepts only finite values, builds two partial digests, merges them,
and checks quantile monotonicity and bounds.
