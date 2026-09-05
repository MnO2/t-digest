# Fuzzing

Install a nightly toolchain and `cargo-fuzz`, then run from the repository root:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
cargo +nightly fuzz run merge
```

The target filters out non-finite input values, chooses a compression size from
1 through 200, builds two partial digests with `merge_unsorted`, and combines
them with `merge_digests`. It also builds a digest through buffered ingestion
and flushes it. Both paths are checked for:

- Monotonic, finite, bounded estimates at 11 quantiles spanning the tails and center.
- Agreement between scalar and bulk quantile queries.
- Estimated ranks within `[0, 1]`.
- Finite means and trimmed means within the observed bounds, allowing rounding error.

Finite inputs can span the full `f64` range. Estimates from the two ingestion
paths need not be identical because their compression histories differ.

For a bounded smoke run, matching the manual GitHub Actions workflow:

```bash
cargo +nightly fuzz run merge -- -max_total_time=60
```

Fuzzing runs separately from `cargo test`. New corpus inputs are stored in
`fuzz/corpus/merge/`, and failures are written under `fuzz/artifacts/merge/`.
Replay a failure by passing its path to `cargo +nightly fuzz run merge`. Turn a
confirmed failure into a deterministic regression test in `tests/` or the
inline `src/lib.rs` tests before fixing it.
