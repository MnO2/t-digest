mod tdigest_bench;

use criterion::{criterion_group, criterion_main};
use tdigest_bench::bench;

criterion_group!(tdigest_bench, bench);
criterion_main!(tdigest_bench);
