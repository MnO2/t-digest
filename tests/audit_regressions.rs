use tdigest::{Centroid, TDigest};

#[test]
fn subnormal_positive_weights_have_finite_quantiles() {
    let weight = f64::from_bits(1);
    let digest = TDigest::new(
        vec![Centroid::new(0.0, weight), Centroid::new(1.0, weight)],
        weight,
        weight + weight,
        Some(1.0),
        Some(0.0),
        100,
    );
    let quantiles = [0.0, 0.25, 0.5, 0.75, 1.0];
    let scalar: Vec<_> = quantiles.iter().map(|q| digest.estimate_quantile(*q)).collect();
    let bulk = digest.quantiles(&quantiles);

    assert!(scalar.iter().chain(&bulk).all(|estimate| {
        let value = estimate.expect("a nonempty digest has a quantile");
        value.is_finite() && (0.0..=1.0).contains(&value)
    }));
    assert_eq!(scalar, bulk);
    assert_eq!(digest.estimate_quantile(0.5), Some(0.5));
}
