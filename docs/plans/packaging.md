# Plan: packaging — serde feature rename, no_std, docs, CI hardening

**Goal:** ecosystem-convention and infrastructure improvements. Four independent
items; each can be its own small PR.

## 1. Rename feature `use_serde` → `serde` (keep alias)

The ecosystem convention is a feature literally named `serde`. Users will type
`features = ["serde"]` first and hit a confusing error.

With Rust ≥ 1.60 (`dep:` syntax, MSRV 1.62 allows it):

```toml
[features]
serde = ["dep:serde", "serde/derive", "serde/std"]
use_serde = ["serde"]   # deprecated alias, remove in 2.0
```

- Change all `#[cfg(feature = "use_serde")]` / `#[cfg_attr(feature = "use_serde", ...)]`
  in `src/lib.rs` to `feature = "serde"`. The alias feature enables the `serde`
  feature, so old users keep working.
- Update README, CHANGELOG (`Added: serde feature; Deprecated: use_serde alias`),
  and `CLAUDE.md`'s build commands if they reference the feature name.
- Test: `cargo test --features serde` and `cargo test --features use_serde` both
  green (run both in CI once, or at least locally and note it in the PR).

## 2. `no_std` + `alloc` support

The crate has zero mandatory dependencies and uses only `Vec`, `std::cmp::Ordering`,
and `f64` methods. `f64::total_cmp` is in `core`. Float math used (`min`, `max`,
`clamp`, arithmetic) is all core-compatible — **verify**: if anything pulls in
std-only float intrinsics (e.g. `powf`, none currently used), gate accordingly.

- Add `#![no_std]` with `extern crate alloc;`, import `alloc::vec::Vec`.
- Add a `std` feature, **on by default**, so existing users are unaffected:
  ```toml
  [features]
  default = ["std"]
  std = []
  serde = ["dep:serde", "serde/derive"]        # drop serde/std from the base
  ```
  Make `serde` work in no_std (serde with `default-features = false` + `alloc`);
  if that fights with the existing `serde/std` requirement, gate: `serde` feature
  requires `std` for now and note it — don't burn time on serde-no_std.
- Tests keep using std (`#[cfg(test)]` can `extern crate std`).
- CI: add a build-only check `cargo check --no-default-features` (and a
  `thumbv7em-none-eabihf` target check if cheap: `rustup target add` + `cargo
  check --no-default-features --target thumbv7em-none-eabihf`).
- Confirm MSRV 1.62 still holds.

## 3. Documentation

- Crate root: add `#![deny(missing_docs)]` (or `#![warn]` escalated by CI) and fill
  in the gaps it exposes — `Centroid` methods, `TDigest::new`, `merge_digests`,
  `sum`, `count`, `max_size`, `centroids` currently lack doc comments.
- Per-method rustdoc examples for `merge_sorted`, `merge_unsorted`, `merge_digests`,
  `estimate_quantile` (and new methods if other workstreams landed).
- Document `Option` semantics (`None` = empty digest) in one place at the type
  level and link to it.
- Document that `count` is `f64` and exact only up to 2^53.
- README: add docs.rs badge and crates.io badge if missing; add an "Accuracy"
  section pointing at the harness (`benchmarks.md` Part B) once it exists.
- `cargo doc --all-features --no-deps` must build without warnings
  (`RUSTDOCFLAGS="-D warnings"`).

## 4. CI hardening (`.github/workflows/CI.yml`)

Add jobs/steps (keep the existing matrix intact):

- **clippy:** `cargo clippy --all-features --all-targets -- -D warnings`. Fix any
  existing lints in the same PR (expect a handful: e.g. `needless_range_loop`-style
  or manual `map_or` patterns).
- **docs:** `RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps`.
- **semver:** `cargo-semver-checks` action (`obi1kenobi/cargo-semver-checks-action`)
  — the crate just cut 1.0, so catching accidental breakage matters.
- **no_std check** (if item 2 landed): `cargo check --no-default-features`.
- Feature matrix sanity: one job runs `cargo test --no-default-features` and
  `cargo test --all-features` (partially exists; verify).

## Acceptance criteria

- Each item lands with green CI including the new jobs.
- No breaking change for existing users: `use_serde` still works; default
  features unchanged in behavior.
- CHANGELOG.md updated per item.
- `CLAUDE.md` updated where its commands/conventions are affected (feature name,
  new CI jobs list).
