# CLAUDE.md

## Project overview

This is `tdigest`, a Rust implementation of the t-digest data structure for accurate quantile estimation. It follows Facebook's folly TDigest implementation. The crate is published on crates.io as `tdigest`.

## Build and test commands

```bash
cargo build                  # Build with default features
cargo build --all-features   # Build with serde support
cargo test --all-features    # Run all tests including serde round-trip
cargo test --release --all-features # Test optimized builds without debug assertions
cargo test --features serde  # Test the preferred serde feature
cargo test --features use_serde # Test the compatibility alias
cargo fmt --all -- --check   # Check formatting
cargo check --all-features   # Type-check only
cargo check --no-default-features # Verify no_std + alloc
cargo check --no-default-features --features serde # Verify serde with alloc
cargo clippy --all-features --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo bench                  # Criterion performance benchmarks
cargo run --release --example accuracy # Deterministic accuracy comparison
```

## Project structure

- `src/lib.rs` -- entire library: `Centroid`, `TDigest`, all methods, unit tests, and property tests
- `tests/` -- integration regression tests for public API behavior
- `Cargo.toml` -- package metadata, MSRV 1.62, default `std`, optional `serde`, and compatibility `use_serde` features
- `docs/architecture.md` -- algorithm and architecture documentation
- `benches/tdigest.rs` -- deterministic Criterion ingestion, merge, and query benchmarks
- `examples/accuracy.rs` -- exact versus estimated quantiles across six distributions
- `fuzz/` -- standalone cargo-fuzz package; see `fuzz/README.md` for nightly commands
- `rustfmt.toml` -- max_width = 120

## Code conventions

- Single-file library (`src/lib.rs`), with inline `#[cfg(test)]` tests and public API regression tests in `tests/`
- Batch API: `merge_sorted` and `merge_unsorted` take `&self` and return a new `TDigest`; the sorted method requires ascending input
- Mutable API: `push`, `extend_values`, and `Extend<f64>` buffer data; summary statistics update immediately
- Call `flush` before centroid queries, immutable merges, or serialization; `FromIterator<f64>` flushes before returning
- `Option<f64>` for queries that require samples (`min`, `max`, `mean`, `estimate_quantile`, `estimate_rank`, `trimmed_mean`); `quantiles` returns a vector of optional estimates
- `#[must_use]` on methods returning new values
- `#[non_exhaustive]` on public structs
- Require a positive compression size in constructors; `max_size` is a compression target, not an exact byte budget
- `debug_assert` for ingestion contracts (finite values) and state invariants; these checks are absent in release builds
- Serde skips the pending buffer; never assume serialization flushes or rejects pending data
- Deserialization validates centroid order, weights, extrema, and count consistency; nonempty sums may be non-finite after valid ingestion
- Compression's final bucket absorbs remaining weight so count roundoff cannot exceed `max_size`
- Keep library code compatible with Rust 1.62 without `std`; verify this configuration separately from all-features builds
- No external dependencies by default; serde is behind the `serde` feature, with `use_serde` as a deprecated alias
- The library is `no_std` and uses `alloc`; the default `std` feature preserves existing behavior
- Formatting: rustfmt with max_width=120

## CI

GitHub Actions (`.github/workflows/CI.yml`): check, test (ubuntu/macos/windows), MSRV (1.62), no_std, clippy, rustdoc warnings, semver checks, code coverage (tarpaulin + codecov), and rustfmt. Fuzzing is a separate manual workflow.
