use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Exp, LogNormal, Normal, StandardNormal};
use tdigest::TDigest;

const SAMPLE_COUNT: usize = 1_000_000;
const MAX_SIZE: usize = 100;
const QUANTILES: [f64; 11] = [0.0001, 0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 0.999, 0.9999];

fn samples(name: &str, seed: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    match name {
        "uniform" => (0..SAMPLE_COUNT).map(|_| rng.gen_range(0.0..1.0)).collect(),
        "normal" => (0..SAMPLE_COUNT).map(|_| StandardNormal.sample(&mut rng)).collect(),
        "lognormal" => {
            let distribution = LogNormal::new(0.0, 1.0).unwrap();
            (0..SAMPLE_COUNT).map(|_| distribution.sample(&mut rng)).collect()
        }
        "exponential" => {
            let distribution = Exp::new(1.0).unwrap();
            (0..SAMPLE_COUNT).map(|_| distribution.sample(&mut rng)).collect()
        }
        "bimodal" => {
            let left = Normal::new(-3.0, 1.0).unwrap();
            let right = Normal::new(3.0, 1.0).unwrap();
            (0..SAMPLE_COUNT)
                .map(|index| {
                    if index % 2 == 0 {
                        left.sample(&mut rng)
                    } else {
                        right.sample(&mut rng)
                    }
                })
                .collect()
        }
        "adversarial" => {
            let mut values: Vec<f64> = (1..=600_000).map(f64::from).collect();
            values.resize(SAMPLE_COUNT, 1_000_000.0);
            values
        }
        _ => unreachable!(),
    }
}

fn exact_quantile(sorted: &[f64], q: f64) -> f64 {
    let index = (q * (sorted.len() - 1) as f64).round() as usize;
    sorted[index]
}

fn streamed_digest(values: &[f64]) -> TDigest {
    let digests = values
        .chunks(SAMPLE_COUNT / 100)
        .map(|chunk| TDigest::new_with_size(MAX_SIZE).merge_unsorted(chunk.to_vec()))
        .collect();
    TDigest::merge_digests(digests)
}

fn report(name: &str, mode: &str, digest: &TDigest, sorted: &[f64]) {
    for q in QUANTILES {
        let exact = exact_quantile(sorted, q);
        let estimate = digest.estimate_quantile(q).unwrap();
        let absolute_error = (estimate - exact).abs();
        let relative_error = if exact.abs() > 1e-12 {
            absolute_error / exact.abs()
        } else {
            f64::NAN
        };
        println!(
            "{name:<12} {mode:<8} {q:>7.4} {exact:>14.6} {estimate:>14.6} {absolute_error:>12.6} {relative_error:>12.6}",
        );
    }
}

fn main() {
    println!("distribution mode    quantile          exact       estimate    abs_error    rel_error");
    println!("------------ -------- -------- -------------- -------------- ------------ ------------");

    for (index, name) in [
        "uniform",
        "normal",
        "lognormal",
        "exponential",
        "bimodal",
        "adversarial",
    ]
    .iter()
    .enumerate()
    {
        let values = samples(name, 0x5eed + index as u64);
        let mut sorted = values.clone();
        sorted.sort_by(f64::total_cmp);

        let single = TDigest::new_with_size(MAX_SIZE).merge_sorted(sorted.clone());
        report(name, "single", &single, &sorted);

        let streamed = streamed_digest(&values);
        report(name, "streamed", &streamed, &sorted);
    }
}
