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

    let min = digest.min().unwrap();
    let max = digest.max().unwrap();
    let estimates: Vec<f64> = [0.0, 0.5, 1.0]
        .iter()
        .map(|q| digest.estimate_quantile(*q).unwrap())
        .collect();
    assert!(estimates.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(estimates.iter().all(|estimate| *estimate >= min && *estimate <= max));
});
