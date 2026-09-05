use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tdigest::TDigest;

const MAX_SIZE: usize = 100;

fn values(len: usize, seed: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..len).map(|_| rng.gen_range(-1_000_000.0..1_000_000.0)).collect()
}

fn sorted_values(len: usize, seed: u64) -> Vec<f64> {
    let mut values = values(len, seed);
    values.sort_by(f64::total_cmp);
    values
}

fn batch_ingest(c: &mut Criterion) {
    let mut sorted = c.benchmark_group("batch_ingest_sorted");
    sorted.sample_size(10).measurement_time(Duration::from_secs(2));
    for size in [10_000, 100_000, 1_000_000] {
        let input = sorted_values(size, size as u64);
        sorted.throughput(Throughput::Elements(size as u64));
        sorted.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| TDigest::new_with_size(MAX_SIZE).merge_sorted(black_box(input.clone())))
        });
    }
    sorted.finish();

    let mut unsorted = c.benchmark_group("batch_ingest_unsorted");
    unsorted.sample_size(10).measurement_time(Duration::from_secs(2));
    for size in [10_000, 100_000, 1_000_000] {
        let input = values(size, size as u64);
        unsorted.throughput(Throughput::Elements(size as u64));
        unsorted.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| TDigest::new_with_size(MAX_SIZE).merge_unsorted(black_box(input.clone())))
        });
    }
    unsorted.finish();
}

fn incremental_ingest(c: &mut Criterion) {
    let input = values(10_000, 10_000);
    let mut group = c.benchmark_group("incremental_ingest");
    group.sample_size(10).measurement_time(Duration::from_secs(2));
    group.throughput(Throughput::Elements(input.len() as u64));
    group.bench_function("immutable_single_value", |b| {
        b.iter(|| {
            let mut digest = TDigest::new_with_size(MAX_SIZE);
            for value in &input {
                digest = digest.merge_unsorted(vec![black_box(*value)]);
            }
            digest
        })
    });
    group.bench_function("buffered_push", |b| {
        b.iter(|| {
            let mut digest = TDigest::new_with_size(MAX_SIZE);
            for value in &input {
                digest.push(black_box(*value));
            }
            digest.flush();
            digest
        })
    });
    group.finish();
}

fn small_batch_ingest(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_batch_ingest");
    group.sample_size(30).measurement_time(Duration::from_secs(2));
    for size in [1, 10, 100] {
        let input = sorted_values(size, 50_000 + size as u64);
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| TDigest::new_with_size(MAX_SIZE).merge_sorted(black_box(input.clone())))
        });
    }
    group.finish();
}

fn buffered_flush(c: &mut Criterion) {
    let seed = TDigest::new_with_size(MAX_SIZE).merge_unsorted(values(10_000, 60_000));
    let mut group = c.benchmark_group("buffered_flush");
    group.sample_size(30).measurement_time(Duration::from_secs(2));
    for size in [1, 100, 499] {
        let mut pending = seed.clone();
        pending.extend_values(values(size, 60_000 + size as u64));
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &pending, |b, pending| {
            b.iter_batched(
                || pending.clone(),
                |mut digest| {
                    digest.flush();
                    digest
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn merge_digests(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_digests");
    group.sample_size(10).measurement_time(Duration::from_secs(2));
    for digest_count in [10, 100] {
        let digests: Vec<_> = (0..digest_count)
            .map(|index| TDigest::new_with_size(MAX_SIZE).merge_unsorted(values(1_000, 20_000 + index as u64)))
            .collect();
        group.throughput(Throughput::Elements((digest_count * 1_000) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(digest_count), &digests, |b, digests| {
            b.iter(|| TDigest::merge_digests(black_box(digests.clone())))
        });
    }
    group.finish();
}

fn estimate_quantile(c: &mut Criterion) {
    let digest = TDigest::new_with_size(MAX_SIZE).merge_unsorted(values(100_000, 30_000));
    let mut group = c.benchmark_group("estimate_quantile");
    group.bench_function("p50", |b| b.iter(|| digest.estimate_quantile(black_box(0.5))));
    group.bench_function("p99", |b| b.iter(|| digest.estimate_quantile(black_box(0.99))));
    let quantiles: Vec<f64> = (0..100).map(|q| f64::from(q) / 99.0).collect();
    group.bench_function("100_queries", |b| {
        b.iter(|| {
            quantiles
                .iter()
                .map(|q| digest.estimate_quantile(black_box(*q)))
                .collect::<Vec<_>>()
        })
    });
    group.bench_function("bulk_100_queries", |b| {
        b.iter(|| digest.quantiles(black_box(&quantiles)))
    });
    group.bench_function("bulk_empty", |b| b.iter(|| digest.quantiles(black_box(&[]))));
    group.bench_function("bulk_p50", |b| b.iter(|| digest.quantiles(black_box(&[0.5]))));
    group.bench_function("bulk_p99", |b| b.iter(|| digest.quantiles(black_box(&[0.99]))));
    group.finish();
}

fn max_size_sensitivity(c: &mut Criterion) {
    let input = sorted_values(100_000, 40_000);
    let mut group = c.benchmark_group("max_size_sensitivity");
    group.sample_size(10).measurement_time(Duration::from_secs(2));
    group.throughput(Throughput::Elements(input.len() as u64));
    for max_size in [50, 100, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(max_size), &max_size, |b, max_size| {
            b.iter(|| TDigest::new_with_size(*max_size).merge_sorted(black_box(input.clone())))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    batch_ingest,
    incremental_ingest,
    small_batch_ingest,
    buffered_flush,
    merge_digests,
    estimate_quantile,
    max_size_sensitivity
);
criterion_main!(benches);
