#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tdigest::TDigest;

#[derive(Arbitrary, Debug)]
struct Input {
    max_size: u8,
    split: usize,
    values: Vec<f64>,
}

fuzz_target!(|input: Input| {
    let max_size = usize::from(input.max_size).clamp(1, 200);
    let values: Vec<f64> = input.values.into_iter().filter(|value| value.is_finite()).collect();
    if values.is_empty() {
        return;
    }

    let split = input.split.min(values.len());
    let left = TDigest::new_with_size(max_size).merge_unsorted(values[..split].to_vec());
    let right = TDigest::new_with_size(max_size).merge_unsorted(values[split..].to_vec());
    let digest = TDigest::merge_digests(vec![left, right]);

    let mut buffered = TDigest::new_with_size(max_size);
    buffered.extend_values(values);
    buffered.flush();

    for digest in [digest, buffered] {
        let min = digest.min().unwrap();
        let max = digest.max().unwrap();
        let qs = [0.0, 0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 0.999, 1.0];
        let estimates: Vec<f64> = digest.quantiles(&qs).into_iter().map(Option::unwrap).collect();
        assert!(estimates.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(estimates
            .iter()
            .all(|estimate| estimate.is_finite() && *estimate >= min && *estimate <= max));
        for (q, estimate) in qs.iter().zip(&estimates) {
            assert_eq!(digest.estimate_quantile(*q), Some(*estimate));
            let rank = digest.estimate_rank(*estimate).unwrap();
            assert!((0.0..=1.0).contains(&rank));
        }
        for mean in [digest.mean().unwrap(), digest.trimmed_mean(0.1, 0.9).unwrap()] {
            assert!(mean.is_finite());
            // Means can differ from an endpoint by rounding in their summary sum.
            assert!(mean >= min || (mean - min).abs() <= min.abs() * 1e-12);
            assert!(mean <= max || (mean - max).abs() <= max.abs() * 1e-12);
        }
    }
});
