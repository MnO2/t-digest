# Plan: correctness — NaN policy, `estimate_rank`, edge cases

**Goal:** three independent fixes/additions, all in `src/lib.rs`. They can land as
one PR or three small ones.

## 1. NaN and infinity policy

### Problem

- `Centroid::new` rejects NaN only via `debug_assert` (src/lib.rs:65). In release
  builds a NaN input silently poisons the digest.
- `merge_sorted`/`merge_unsorted` never check inputs. `sort_by(f64::total_cmp)`
  places NaNs **last**, so `sorted_values.last()` (src/lib.rs:245) makes
  `max = NaN`, which then propagates through every quantile clamp.
- Infinities are untested and undocumented.

### Decision

Filter is too magical and `Result` breaks the API. Policy:

- **Documented precondition + debug_assert at the boundary.** Add to `merge_sorted`,
  `merge_unsorted` (and `push` if `mutable-api.md` has landed):
  `debug_assert!(values.iter().all(|v| !v.is_nan()), "input must not contain NaN")`.
  For `merge_sorted` this is O(n) — acceptable in debug builds only, which is what
  `debug_assert` gives us.
- Crate-level rustdoc: a "## Handling of NaN and infinities" section stating:
  NaN input is a contract violation (checked in debug builds, undefined estimates
  in release); ±∞ values are accepted and behave as ordinary extreme values
  (verify with the test below; if they *don't* behave sanely, document them as
  also-rejected and extend the debug_asserts to `is_finite`).
- This matches the existing convention (debug_assert for invariants, no NaN
  sentinels, no Result-ification of a 1.0 API).

### Tests

- `#[should_panic]` (debug) test: `merge_unsorted(vec![1.0, f64::NAN])`.
- Infinity test: digest over `[f64::NEG_INFINITY, 1.0, 2.0, 3.0, f64::INFINITY]`;
  assert `min()`/`max()` are the infinities, `estimate_quantile(0.5)` is finite
  and sane. Adjust policy per the outcome as described above.

## 2. `estimate_rank` (CDF) and `trimmed_mean`

### `estimate_rank`

The inverse of `estimate_quantile` — given a value, estimate the fraction of the
distribution ≤ it. This is the other half of a t-digest's standard API surface.

```rust
/// Estimate the rank (CDF) of `value`: the fraction of inserted values ≤ `value`.
/// Returns `None` if the digest is empty.
#[must_use]
pub fn estimate_rank(&self, value: f64) -> Option<f64>
```

Semantics (folly's `cdf`, which this crate follows elsewhere):

- Empty digest → `None`.
- `value < min` → `Some(0.0)`; `value > max` → `Some(1.0)`.
- Otherwise walk centroids accumulating weight; linearly interpolate within the
  bracketing pair of centroid means, mirroring `estimate_quantile`'s interpolation
  (src/lib.rs:442-461). Clamp the result to `[0.0, 1.0]`.
- Single-centroid digest / `min == max`: `value >= max → 1.0`, else `0.0`.
- `debug_assert!(!value.is_nan())`.

Tests: uniform 1..=1M digest — `estimate_rank(500_000)` ≈ 0.5 within 1%,
rank(min)≈0, rank(max)=1; round-trip `estimate_rank(estimate_quantile(q)) ≈ q`
for q ∈ {0.01, 0.25, 0.5, 0.75, 0.99} within a few %; single-value digest cases.

### `trimmed_mean`

```rust
/// Mean of values between quantiles `lo` and `hi` (e.g. 0.05, 0.95).
/// Returns `None` if the digest is empty or `lo >= hi`.
#[must_use]
pub fn trimmed_mean(&self, lo: f64, hi: f64) -> Option<f64>
```

Walk centroids accumulating `mean * weight` for centroids whose cumulative-weight
span lies inside `[lo*count, hi*count]`, taking fractional weight for the two
boundary centroids. Test against the exact trimmed mean of uniform 1..=100k
(within 1%), and `trimmed_mean(0.0, 1.0) ≈ mean()`.

## 3. `merge_digests` mixed `max_size` and empty-input semantics

### Problem

`merge_digests` (src/lib.rs:319-326) takes the **first** digest's `max_size`, even
when digests disagree — and when all digests are empty but the vec is non-empty it
correctly preserves the first's size, yet `merge_digests(vec![])` invents 100.

### Decision (behavioral tweak, defensible as bug-fix minor)

- Use the **maximum** `max_size` across input digests — merging a size-500 digest
  into a size-100 one should not silently degrade the 500. Document this rule in
  the method's rustdoc.
- `merge_digests(vec![])` keeps returning the default (size 100); document it.
- Add rustdoc to `merge_digests` (it currently has none).

Tests: merge size-100 + size-500 digests → result `max_size() == 500` and
`centroids().len() <= 500`; existing `test_merge_digests_empty_preserves_max_size`
still passes.

## Acceptance criteria

- `cargo test --all-features` green, including new tests.
- New public methods have `#[must_use]`, rustdoc with examples, `Option<f64>`
  returns (never NaN sentinels).
- Crate-level docs gain the NaN/infinity section.
- No changes to existing public signatures.
