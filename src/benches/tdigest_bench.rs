use criterion::{black_box, Criterion};

pub fn bench(c: &mut Criterion) {
    // For online recording, 1 observation is typically encountered at a time.
    c.bench_function("observe 1 via merge", |b| {
        let mut digest = tdigest::TDigest::default();
        let mut i = 0.0;
        b.iter(|| {
            digest = digest.merge_sorted(black_box(vec![i]));
            i += 1.0;
        })
    });

    // For online recording, 1 observation is typically encountered at a time.
    c.bench_function("observe 1 via online wrapper", |b| {
        let digest = tdigest::online::OnlineTdigest::default();
        let mut i = 0.0;
        b.iter(|| {
            digest.observe(black_box(i));
            i += 1.0;
        })
    });

    c.bench_function("observe 1 via online wrapper &mut", |b| {
        let mut digest = tdigest::online::OnlineTdigest::default();
        let mut i = 0.0;
        b.iter(|| {
            digest.observe_mut(black_box(i));
            i += 1.0;
        })
    });
}
