# Plan: performance micro-optimizations

**Goal:** a set of small, low-risk speedups in `src/lib.rs`. Each item must be
validated with the criterion suite from `benchmarks.md` — land that workstream
first, take a baseline, and quote before/after numbers per item in the PR.
If an item shows no measurable win, drop it rather than adding churn.

## Items, in order

### 1. `sort_unstable_by` in `merge_unsorted` (src/lib.rs:231)

`sort_by` is a stable merge sort that allocates a temp buffer; `f64` values need
no stability. Change to `sorted_values.sort_unstable_by(f64::total_cmp)`.
Expected: measurable win on the unsorted-ingest bench, zero risk.

### 2. Make `Centroid` `Copy`

`Centroid` is two `f64`s. Add `Copy` to its derives, then remove the now-redundant
`.clone()` calls in the merge loops (src/lib.rs:267, 283, 302, 376, 384). Additive
trait impl — not a breaking change. Mostly a readability win; the codegen win is
likely small — keep it even if benches are flat, since it simplifies the loops.

### 3. Remove or guard the trailing re-sort of `compressed`

`merge_sorted` and `merge_digests` sort their output at the end
(src/lib.rs:312 and :386) even though `compressed` is produced by consuming an
ascending stream — the output should already be sorted, because each emitted
centroid's mean is a weighted average of a contiguous ascending run.

**Caveat:** weighted averaging can in principle produce a mean that ties or, with
float rounding, slips a hair below the previous emitted centroid. So do this
carefully:

1. Add a temporary `debug_assert!(compressed.windows(2).all(|w| w[0] <= w[1]))`
   before the sort and run the full test suite + accuracy harness + proptests
   (from `testing.md`) to see whether unsortedness ever occurs.
2. If it never fires: replace `sort()` with that debug_assert.
3. If it fires: replace the full sort with `if !compressed.is_sorted() { compressed.sort(); }`
   — note `slice::is_sorted` is stabilized after MSRV 1.62, so write the windows
   check manually or via a small helper.

**Execution result:** the ordering assertion passed the unit tests, proptests,
and accuracy harness, but removing the sort did not produce a reproducible
benchmark improvement. The sort was retained per this plan's no-churn rule.

### 4. Drop `shrink_to_fit` on the compressed vec (src/lib.rs:311, 385)

It can force a realloc+copy on every merge to save at most a few hundred bytes.
Remove both calls; verify no test asserts on capacity (none do today).

**Execution result:** restoring `shrink_to_fit` was neutral at smaller sizes and
about 2% slower on the 1M sorted-ingest case, so the removal was retained.

### 5. Reserve exact capacity in `merge_sorted`

`compressed` is created `with_capacity(self.max_size)` but `result.centroids`
replaces a `Vec::new()`; fine. Check instead that `merge_digests`'s
`Vec::with_capacity(max_size)` (src/lib.rs:354) is not undersized when
`compressed.len()` can exceed `max_size` by 1 (final push after loop). If it can,
`with_capacity(max_size + 1)` avoids a doubling realloc. Trivial; verify by
asserting `compressed.len() <= max_size + 1` in tests.

### 6. Bulk quantile API: `quantiles(&self, qs: &[f64]) -> Vec<Option<f64>>`

`estimate_quantile` does an O(centroids) scan per call. For callers requesting
many quantiles (the common monitoring case: p50/p90/p99/p999):

- New public method that computes the cumulative-weight prefix array once, then
  answers each q via binary search (`partition_point`) + the same interpolation
  as `estimate_quantile`.
- Extract the interpolation block (src/lib.rs:442-461) into a private helper both
  methods share — do not duplicate it.
- Does **not** require sorted `qs`; per-element semantics identical to calling
  `estimate_quantile` in a loop (assert this equivalence in a test across
  q ∈ {0.0, 0.001, 0.5, 0.999, 1.0} and on an empty digest).
- `#[must_use]`, rustdoc with example.
- Benchmark: 100 quantile queries via loop vs. `quantiles` on a 100-centroid
  digest.

## Non-goals

- Algorithmic changes (scale functions, interpolation) — see `correctness.md`
  and future work.
- SIMD, unsafe, or parallelism. The crate has no unsafe and should stay that way.
- Changing any existing public signature.

## Acceptance criteria

- Before/after criterion numbers for items 1, 3, 4, 6 in the PR description.
- Item 3 only lands with the assert-first evidence described above.
- All tests (including proptests if present) and the accuracy harness show no
  accuracy change — these are pure performance changes.
- `cargo fmt` / `cargo check --all-features` / MSRV job green.
