# CLAUDE.md

## Project overview

This is `tdigest`, a Rust implementation of the t-digest data structure for accurate quantile estimation. It follows Facebook's folly TDigest implementation. The crate is published on crates.io as `tdigest`.

## Build and test commands

```bash
cargo build                  # Build with default features
cargo build --all-features   # Build with serde support
cargo test --all-features    # Run all tests including serde round-trip
cargo fmt --all -- --check   # Check formatting
cargo check --all-features   # Type-check only
```

## Project structure

- `src/lib.rs` -- entire library: `Centroid`, `TDigest`, all methods, and all tests
- `Cargo.toml` -- package metadata, MSRV 1.62, optional `use_serde` feature
- `docs/architecture.md` -- algorithm and architecture documentation
- `rustfmt.toml` -- max_width = 120

## Code conventions

- Single-file library (`src/lib.rs`), tests are inline `#[cfg(test)]` at the bottom
- Immutable API: data methods (`merge_sorted`, `merge_unsorted`) take `&self` and return a new `TDigest`
- `Option<f64>` for fallible queries (`min`, `max`, `mean`, `estimate_quantile`) -- never NaN sentinels
- `#[must_use]` on methods returning new values
- `#[non_exhaustive]` on public structs
- `debug_assert` for invariant checks (NaN rejection, min/max presence)
- No external dependencies by default; `serde` is behind the `use_serde` feature flag
- Formatting: rustfmt with max_width=120

## CI

GitHub Actions (`.github/workflows/CI.yml`): check, test (ubuntu/macos/windows), MSRV (1.62), code coverage (tarpaulin + codecov), rustfmt.
