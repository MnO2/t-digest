# Plan: benchmark suite and accuracy harness

**Goal:** the crate currently has zero benchmarks. Add (a) a criterion micro-benchmark
suite so speed changes are measurable, and (b) an accuracy harness so interpolation
and scale-function changes are measurable. Every other performance/quality workstream
depends on this existing first.

## Part A — criterion suite

### Setup

- Add to `Cargo.toml`:
  ```toml
  [dev-dependencies]
  # Criterion 0.5 requires Rust 1.64; keep the crate's Rust 1.62 MSRV.
  criterion = "=0.4.0"
  rand = "0.8"

  [[bench]]
  name = "tdigest"
  harness = false
  ```
- Create `benches/tdigest.rs`. Benches live outside `src/lib.rs`; the single-file
  rule applies to library code only.
- Pin RNG seeds (`StdRng::seed_from_u64`) so runs are comparable.

### Benchmarks to include

All with `max_size = 100` unless noted (that is the crate default and README example).

1. **Batch ingest, sorted** — `merge_sorted` of 10k / 100k / 1M pre-sorted values
   into an empty digest. Use `Throughput::Elements`.
2. **Batch ingest, unsorted** — `merge_unsorted` same sizes (measures sort overhead).
3. **Incremental ingest (worst case)** — 10k values inserted one at a time via
   `merge_unsorted(vec![v])`. This documents the pathological cost of the current
   immutable-only API and is the baseline the mutable-API workstream
   (`mutable-api.md`) must beat. Expect this to be dramatically slower per element.
4. **merge_digests** — merge 10 / 100 digests of 1k values each.
5. **estimate_quantile** — single query at q=0.5 and q=0.99 on a digest built from
   100k values. Also a "100 quantile queries" variant (motivates a future bulk API).
6. **max_size sensitivity** — batch ingest 100k values at `max_size` 50 / 100 / 500.

### Acceptance criteria (Part A)

- `cargo bench` runs green.
- Bench code is seeded/deterministic (no `thread_rng`).
- README gains a short "Benchmarks" section: how to run, one-line summary of what
  is measured (do not paste absolute numbers that rot; describe how to reproduce).

## Part B — accuracy harness

### Setup

- Create `examples/accuracy.rs`, run with `cargo run --release --example accuracy`.
  Using an example (not a test) keeps it out of the test suite's runtime while
  keeping it compiled by CI (`cargo check --all-features` covers examples via
  `cargo check --examples` — add `--examples` to the CI check step if absent).
- `rand` is already a dev-dependency from Part A; add `rand_distr = "0.4"` for
  lognormal/exponential.

### What it does

For each distribution × each quantile, build a digest from N = 1,000,000 samples,
compare `estimate_quantile(q)` against the exact quantile (sort the sample, index
into it), and print a table of relative error (and absolute error where the exact
value is ~0, e.g. the median of a symmetric distribution).

- **Distributions:** uniform, standard normal, lognormal(0, 1), exponential(1),
  bimodal (mixture of two normals), adversarial (sorted ramp + heavy spike of one
  repeated value, like `test_merge_sorted_against_skewed_distro`).
- **Quantiles:** 0.0001, 0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 0.999, 0.9999.
- **Modes:** single `merge_sorted` build, and a "streamed" build (100 chunks merged
  via `merge_digests`) — tail accuracy typically degrades under merging and we want
  that visible.
- Output: plain-text aligned table to stdout. No plotting dependencies.

### Acceptance criteria (Part B)

- `cargo run --release --example accuracy` prints the table in under ~30s.
- Seeded/deterministic output.
- `docs/architecture.md` gains a short "Measured accuracy" section summarizing the
  observed error magnitudes at mid vs. tail quantiles (orders of magnitude, not
  exact figures).

## Out of scope

- CI benchmark regression tracking (codspeed/bencher.dev) — note it as a follow-up
  in the PR description, don't build it here.
- Comparative benchmarks against other crates.
