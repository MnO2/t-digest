# Plan: testing — property tests, fuzzing, tighter accuracy assertions

**Goal:** current tests check ~1% relative error at a handful of quantiles on
uniform-ish data. Add property-based tests, a fuzz target, and tighter accuracy
bounds so regressions in the merge/interpolation logic actually fail CI.

Best executed **after** `correctness.md` (so the NaN policy is settled and
`estimate_rank` exists to test), but does not strictly depend on it — skip the
rank properties if that method doesn't exist yet.

## 1. Property tests (proptest)

- Add `proptest = "1"` to `[dev-dependencies]`. Check MSRV 1.62 compatibility;
  if the latest proptest requires a newer rustc, pin the newest version that
  supports 1.62 (CI has an MSRV job that will catch this).
- Property tests live in the existing inline `#[cfg(test)]` module at the bottom
  of `src/lib.rs` (project convention: single file, inline tests). Put them in a
  nested `mod proptests`.
- Value strategy: `prop::collection::vec(-1e9f64..1e9, 1..2000)` — finite,
  non-NaN, per the crate's documented input contract.

Properties to encode:

1. **Monotonicity:** for any input vec and sorted quantile list
   `[0.0, 0.01, ..., 1.0]`, `estimate_quantile` results are non-decreasing.
2. **Bounds:** every estimate lies within `[min(), max()]`; `estimate_quantile(0.0)
   == min()` and `estimate_quantile(1.0) == max()`.
3. **Count/sum conservation:** after any sequence of `merge_unsorted` calls
   (split the input into random chunks), `count()` equals total inserted count
   exactly, and `sum()` is within 1e-6 relative of the true sum.
4. **Merge-path insensitivity:** digest built from the whole vec vs. digest built
   by chunking + `merge_digests` — median estimates within 10% relative (loose on
   purpose; this guards against gross divergence like the k_limit off-by-one bug,
   see `test_merge_digests_matches_merge_sorted`).
5. **Rank/quantile round trip** (if `estimate_rank` exists):
   `estimate_rank(estimate_quantile(q))` within 0.1 of `q` for q ∈ 0.05..=0.95.
6. **Serde round trip** (gate with `#[cfg(feature = "use_serde")]`): serialize +
   deserialize gives equal quantile estimates at several q.

Keep default case counts modest (e.g. `ProptestConfig { cases: 64, .. }` for the
expensive ones) so `cargo test` stays fast.

## 2. Fuzz target (cargo-fuzz)

- `cargo fuzz init`; target `fuzz/fuzz_targets/merge.rs`.
- The `fuzz/` directory is a separate cargo package (standard cargo-fuzz layout);
  this does not violate the single-file-library rule. Add `fuzz/target` and
  `fuzz/corpus`/`fuzz/artifacts` to `.gitignore` per cargo-fuzz defaults.
- Target: interpret arbitrary bytes as `(max_size: u8 clamped to 1..=200, Vec<f64>)`
  via the `arbitrary` crate; filter NaN from inputs (contract); build a digest,
  split-and-merge via `merge_digests`, query quantiles 0.0/0.5/1.0, assert
  monotonic + within min/max + no panic.
- Fuzzing is nightly-only and slow; do **not** add it to the main CI matrix. Add a
  separate manually-triggered (`workflow_dispatch`) GitHub Actions workflow that
  runs each target for ~60 seconds, plus a README note in `fuzz/` on running
  locally.

## 3. Tighter accuracy assertions in existing tests

Current uniform-distribution tests assert <1% relative error with `max_size=100`
over 1M values — a correct implementation achieves far better at the tails, so 1%
hides regressions.

- First **measure** actual error (use the accuracy harness from `benchmarks.md` if
  it exists, otherwise print in-test), then set bounds at ~3× observed error:
  expect roughly: q=0.01/0.99 well under 0.1%, q=0.5 under 0.5% for uniform.
  Do not guess bounds — derive them from measurement, with headroom for platform
  float variation.
- Add missing edge-case unit tests:
  - all-identical values (1000 × `42.0`): every quantile returns exactly 42.0;
  - `max_size = 1` and `max_size = 2` digests over 10k values: no panic, estimates
    within `[min, max]`, monotonic;
  - two-value digest quantile interpolation;
  - large-count documentation test: `count` is `f64` — add a doc note in
    `lib.rs` that exact counting is limited to 2^53 (no test needed, just docs).

## Acceptance criteria

- `cargo test --all-features` green and completes in reasonable time (<60s).
- Property tests are deterministic-friendly (proptest persistence files
  `proptest-regressions/` committed if any failures were found and fixed).
- Fuzz workflow runs green on manual dispatch; any panics found are fixed in the
  same PR (or filed as issues with the failing input committed to the corpus).
- MSRV job still green.
