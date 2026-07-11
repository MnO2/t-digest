# Plan: buffered mutable ingestion API

**Goal:** the crate's only ingestion path is immutable — `merge_sorted`/`merge_unsorted`
take `&self`, clone-and-compress, and return a new `TDigest`. For streaming use
(insert one value at a time) this is O(centroids) work **per value** — see the
"incremental ingest" bench in `benchmarks.md`, which is the baseline this plan must
beat by orders of magnitude. Add a buffered mutable API alongside, without breaking
the immutable one. This is how folly's TDigest is actually used in production and
how DataFusion's fork works.

## Design

Additive only — the existing `TDigest` struct and immutable methods do not change.
This is a semver-minor release.

Add fields and methods to `TDigest` in `src/lib.rs`:

```rust
pub struct TDigest {
    // existing fields unchanged ...
    #[cfg_attr(feature = "use_serde", serde(skip, default))]
    buffer: Vec<f64>,   // unmerged values, capacity BUFFER_FACTOR * max_size
}

impl TDigest {
    /// Insert a single value. Buffered; compression happens automatically
    /// when the internal buffer fills.
    pub fn push(&mut self, value: f64);

    /// Insert many values. Buffered like `push`.
    pub fn extend_values(&mut self, values: impl IntoIterator<Item = f64>);

    /// Merge any buffered values into the centroids. Called automatically by
    /// queries; public so callers can control when compression cost is paid.
    pub fn flush(&mut self);
}

impl Extend<f64> for TDigest { /* delegates to extend_values */ }
impl FromIterator<f64> for TDigest { /* default size 100, extend, flush */ }
```

### Semantics

- **Buffer size:** `BUFFER_FACTOR * max_size` with `BUFFER_FACTOR = 5` (folly uses
  a multiple of max_size; pick 5 and note it's tunable later — do not add a config
  knob now). When the buffer reaches capacity, `push`/`extend_values` trigger a flush.
- **Flush** sorts the buffer (`sort_unstable_by(f64::total_cmp)`) and runs the
  existing `merge_sorted` compression logic. Refactor: extract the core of
  `merge_sorted` (src/lib.rs:236-316) into a private helper that both the immutable
  method and `flush` call, rather than duplicating the merge loop. `flush` should
  reuse allocations where practical (`std::mem::take` the buffer, clear and reuse).
- **Queries must see buffered data.** The cleanest contract: `estimate_quantile`,
  `mean`, `min`, `max`, `count`, `centroids`, `is_empty` reflect *all* inserted
  values. Since those take `&self`, have `push`-users call `flush` first OR make
  queries flush lazily. Decision: **keep queries `&self` and document that `flush`
  must be called before querying if `push` was used; add `debug_assert!(self.buffer.is_empty())`
  in queries.** Interior mutability (RefCell) is not worth the loss of `Sync`.
  Exception: `count()` and `min()`/`max()` are cheap to keep exact — update
  `count`, `min`, `max` eagerly in `push` so only `sum`/`centroids`-dependent
  queries need a flush. `sum` should also be updated eagerly (it's one addition).
  That leaves only `estimate_quantile`/`centroids` requiring a flush; assert there.
- **Immutable methods on a digest with a non-empty buffer:** `merge_sorted`/
  `merge_unsorted`/`merge_digests` should `debug_assert` the buffer is empty
  (mixing the two styles mid-stream is a programming error) — or, simpler and
  friendlier: clone-and-flush internally. Pick the debug_assert; document it.
- **Serde:** buffer is `serde(skip)`. Document that serializing without flushing
  drops buffered values; `debug_assert` buffer-empty in a custom check is not
  possible with derive — just document it loudly on the struct.
- **`PartialEq`/`Clone`/`Debug`:** derived impls now include `buffer`; that is
  acceptable (two digests with different pending buffers are not equal).

### NaN

`push` must `debug_assert!(!value.is_nan())`, consistent with existing convention.
If the correctness workstream (`correctness.md`) lands a stricter NaN policy first,
follow that policy instead.

## Tests (inline `#[cfg(test)]`, bottom of `src/lib.rs`)

1. `push` × 1M uniform values, flush, quantile estimates match the tolerances used
   in `test_merge_sorted_against_uniform_distro`.
2. Equivalence: digest built via `push` per value ≈ digest built via one
   `merge_sorted` on the same data (quantiles within 1% at 0.01/0.5/0.99).
3. Buffer boundary: insert exactly `BUFFER_FACTOR * max_size` values, then one
   more; count/min/max exact at every point without explicit flush.
4. `extend_values`, `Extend`, `FromIterator` smoke tests.
5. `count`/`min`/`max`/`mean` exact with unflushed buffer (eager-update contract).
6. Serde round-trip after flush still passes; document-drop behavior when
   unflushed is at least exercised (round-trip after flush equals original).

## Benchmarks

Extend `benches/tdigest.rs` (from `benchmarks.md`) with `push`-based incremental
ingest of 10k values; it must be orders of magnitude faster per element than the
`merge_unsorted(vec![v])` baseline bench. Quote the ratio in the PR description.

## Acceptance criteria

- All existing tests pass unchanged (immutable API untouched).
- New tests above pass; `cargo test --all-features` green.
- No new runtime dependencies.
- Rustdoc on every new public item, with a streaming example on `push`.
- README gains a short streaming example next to the existing batch example.
