# Improvement plans

Execution-ready plans for improving the `tdigest` crate. Each file is a self-contained
workstream that one agent can pick up and execute end to end.

## Workstreams

| # | Plan | Priority | Depends on |
|---|------|----------|------------|
| 1 | [benchmarks.md](benchmarks.md) — criterion suite + accuracy harness | High | none |
| 2 | [mutable-api.md](mutable-api.md) — buffered mutable ingestion API | High | 1 (to measure the win) |
| 3 | [correctness.md](correctness.md) — NaN policy, `estimate_rank`, edge cases | High | none |
| 4 | [testing.md](testing.md) — property tests, fuzzing, tighter assertions | Medium | none (best after 3) |
| 5 | [performance.md](performance.md) — micro-optimizations | Medium | 1 (measure before/after) |
| 6 | [packaging.md](packaging.md) — feature rename, no_std, docs, CI | Medium | none |

## Recommended order

1. **benchmarks** first — everything else becomes measurable.
2. **mutable-api** and **correctness** next (independent of each other, can run in parallel).
3. **testing**, **performance**, **packaging** in any order after that.

## Future work (no plan written yet)

Deliberately deferred — larger lifts that need the accuracy harness (workstream 1)
in place first to be evaluable:

- **Interpolation refinement:** the current folly-style interpolation treats
  weight-1 centroids like heavy ones; Dunning's reference implementation
  special-cases singletons at the tails for better extreme-quantile (p99.9+)
  accuracy. Measure with the accuracy harness before and after.
- **Selectable scale functions (k0/k1/k2/k3):** `k_to_q` hardcodes one scale
  function; the t-digest paper's alternatives trade tail accuracy vs. centroid
  budget. API design question (const generic? enum field?) — needs a plan.
- **CI benchmark regression tracking** (codspeed / bencher.dev).
- **Comparative benchmarks** vs. the `tdigests` crate and an exact baseline.
- **`Centroid::add` API cleanup** (leaks internal running-sum detail) — semver
  break, batch for 2.0.

## Ground rules for all workstreams

These come from `CLAUDE.md` and existing conventions — do not deviate:

- Single-file library: all library code stays in `src/lib.rs`; tests are inline
  `#[cfg(test)]` at the bottom.
- Existing immutable API (`merge_sorted`, `merge_unsorted` taking `&self` and
  returning a new `TDigest`) must not break. This crate is 1.0 — no semver-major
  changes unless a plan explicitly says the change is additive.
- `Option<f64>` for fallible queries, never NaN sentinels.
- `#[must_use]` on methods returning new values; `#[non_exhaustive]` on public structs.
- `debug_assert` for invariant checks unless a plan says otherwise.
- No new runtime dependencies by default. Dev-dependencies (criterion, proptest,
  rand) are fine.
- Format with rustfmt (`max_width = 120`); run `cargo fmt --all -- --check`,
  `cargo test --all-features`, and `cargo check --all-features` before finishing.
- MSRV is 1.62 — verify any new syntax/APIs against it (CI has an MSRV job).
