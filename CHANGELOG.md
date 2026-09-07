# Changelog

All notable changes to this project will be documented in this file.

## [1.0.1] - 2026-09-07

### Fixed

- Preserve the requested compression size and sort centroids in `TDigest::new`; reject zero compression sizes.
- Keep compression within `max_size` even when fractional centroid weights and the stored count differ by roundoff.
- Make quantile interpolation monotonic across uneven centroid gaps, with finite results for extreme values and subnormal weights.
- Interpolate ranks for single-centroid digests that represent varying values and avoid overflow across extreme endpoints.
- Keep centroid means and trimmed means finite when weighted sums overflow; compute `mean()` from retained data if its sum overflows.
- Preserve centroids when adding zero sum and zero weight, including empty accumulators.
- Make centroid equality consistent with its total ordering, including weights and signed zeros.
- Reject invalid serde states before they can cause query or merge panics, preserving the existing field layout for valid digests.
- Restrict debug-assertion tests to debug builds so the release test suite can run.

### Performance

- Allocate the ingestion buffer on first use and reuse it across flushes.
- Bound compression-vector reservations by the available input size.
- Avoid building cumulative weights for empty or single-element bulk quantile queries.

### Documentation and validation

- Clarify memory usage, flush requirements, serialization limits, query semantics, and distribution-dependent accuracy.
- Add constructor, numeric, and serde regression tests; extend fuzzing to buffered ingestion and interior quantiles.
- Cover optimized tests and Rust 1.62 `no_std` configurations in CI.

## [1.0.0] - 2026-07-12

### Added

- Deterministic Criterion benchmarks and an accuracy harness covering six distributions.
- Buffered mutable ingestion with `push`, `extend_values`, `flush`, `Extend`, and `FromIterator`.
- `estimate_rank`, `trimmed_mean`, and bulk `quantiles` queries.
- Property tests, a cargo-fuzz target, and a manual fuzzing workflow.
- `no_std` + `alloc` support behind a default `std` feature.
- Clippy, rustdoc-warning, no-std, and semver CI gates.

### Changed

- `merge_digests` now uses the largest input `max_size`.
- Finite-input checks cover public ingestion boundaries in debug builds.
- Unsorted ingestion and buffered flushes use unstable sorting.
- Documentation warnings are denied for public API items.

### Fixed

- Stable centroid averaging prevents intermediate overflow from breaking quantile monotonicity for large finite values.

### Deprecated

- `use_serde` remains as a compatibility alias; new users should enable `serde`.

### Breaking Changes

- `min()` / `max()` now return `Option<f64>` instead of `f64`. Use `.unwrap()` or `.unwrap_or(f64::NAN)` to recover old behavior.
- `estimate_quantile()` now returns `Option<f64>` instead of `f64`. Use `.unwrap()` or `.unwrap_or(0.0)` to recover old behavior.
- `mean()` now returns `Option<f64>` instead of `f64`. Use `.unwrap()` or `.unwrap_or(0.0)` to recover old behavior.
- `new()` now takes `Option<f64>` for `min` and `max` parameters. Wrap existing values in `Some(...)`.
- `Eq` removed from `TDigest`. Use `PartialEq` only.
- `#[non_exhaustive]` added to `TDigest` and `Centroid`. Use constructors or `Default` instead of struct literals.
- The `ordered-float` dependency has been removed. No action needed by consumers.
- **Serde wire format changed:** The `min` and `max` fields are now `Option<f64>` (serialized as `null` when empty) instead of `f64` (which used `NaN` as a sentinel). Data serialized with 0.2.x cannot be deserialized by 1.0.0 without manual migration.

### Bug Fixes

- Fixed `merge_digests` k_limit off-by-one: the first centroid bucket was absorbing more weight than intended because `k_limit` was not incremented after the initial `q_limit_times_count` computation.
- Fixed hardcoded `100` in `new()` re-compression: when centroids exceeded `max_size`, the constructor ignored the caller-supplied `max_size`.
- Fixed `merge_digests` empty-input losing `max_size`: previously returned `TDigest::default()` (max_size=100) instead of preserving the caller's max_size.

### Improvements

- Dropped `ordered-float` dependency; uses `f64::total_cmp` (stable since Rust 1.62).
- Replaced custom `clamp` function with `f64::clamp()`.
- Replaced `external_merge` (~90 lines of custom merge sort) with `centroids.sort()`.
- Simplified `merge_unsorted` to sort in-place without intermediate allocation.
- Added `centroids()` accessor to expose `&[Centroid]`.
- Added `#[must_use]` annotations to methods returning new values.
- Added `debug_assert` validation in constructors for NaN and missing min/max.

### Infrastructure

- Removed Travis CI configuration.
- Updated GitHub Actions CI: replaced deprecated `actions-rs/*` with `dtolnay/rust-toolchain`, added MSRV check job.
- Declared MSRV as Rust 1.62 via `rust-version` in `Cargo.toml`.
- Removed `Cargo.lock` from version control (library convention).
