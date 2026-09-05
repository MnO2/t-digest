use std::collections::BTreeSet;
use tdigest::{Centroid, TDigest};

#[test]
fn centroid_ordering_agrees_with_equality() {
    let centroids = [Centroid::new(1.0, 1.0), Centroid::new(1.0, 2.0)];
    assert!(!centroids[0].cmp(&centroids[1]).is_eq());
    let set: BTreeSet<_> = centroids.into_iter().collect();
    assert_eq!(set.len(), 2);
    let negative_zero = Centroid::new(-0.0, 1.0);
    let positive_zero = Centroid::new(0.0, 1.0);
    assert_eq!(
        negative_zero == positive_zero,
        negative_zero.cmp(&positive_zero).is_eq()
    );
}

#[test]
fn mean_stays_finite_when_the_sum_overflows() {
    let mut digest = TDigest::default();
    digest.extend_values([f64::MAX, f64::MAX]);
    assert_eq!(digest.mean(), Some(f64::MAX));
    digest.flush();
    assert_eq!(digest.mean(), Some(f64::MAX));
    let opposite = TDigest::default().merge_sorted(vec![-f64::MAX; 2]);
    let merged = TDigest::merge_digests(vec![digest, opposite]);
    assert!(merged.mean().unwrap().abs() / f64::MAX < 1e-15);
}

#[test]
fn centroid_add_keeps_a_representable_mean_when_sum_overflows() {
    let mut centroid = Centroid::new(f64::MAX, 1.0);
    assert_eq!(centroid.add(f64::MAX, 1.0), f64::INFINITY);
    assert_eq!(centroid.mean(), f64::MAX);
    assert_eq!(centroid.weight(), 2.0);
}

#[test]
fn adding_zero_sum_and_weight_preserves_a_centroid() {
    for original in [Centroid::new(0.0, 0.0), Centroid::new(f64::MAX, 2.0)] {
        let mut centroid = original;
        assert_eq!(centroid.add(0.0, 0.0), original.mean() * original.weight());
        assert_eq!(centroid, original);
    }
}

#[test]
fn quantiles_are_monotonic_across_uneven_centroid_gaps() {
    let digest = TDigest::default().merge_sorted(vec![0.0, 1.0, 2.0, 100.0]);
    let qs: Vec<_> = (0..=1000).map(|i| f64::from(i) / 1000.0).collect();
    let estimates = digest.quantiles(&qs);
    assert!(estimates.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(digest.estimate_quantile(0.5), Some(1.5));
    for (q, expected) in qs.iter().zip(estimates) {
        assert_eq!(digest.estimate_quantile(*q), expected);
    }
}

#[test]
fn extreme_finite_quantiles_and_ranks_interpolate_without_overflow() {
    let digest = TDigest::default().merge_sorted(vec![-f64::MAX, f64::MAX]);
    assert_eq!(digest.estimate_quantile(0.5), Some(0.0));
    assert_eq!(digest.estimate_rank(0.0), Some(0.5));
    for value in [-f64::MAX / 2.0, 0.0, f64::MAX / 2.0] {
        let rank = digest.estimate_rank(value).unwrap();
        assert!(rank.is_finite() && (0.0..=1.0).contains(&rank));
    }
}

#[test]
fn rank_interpolates_when_compression_retains_one_centroid() {
    let digest = TDigest::new_with_size(1).merge_sorted(vec![0.0, 10.0]);
    assert_eq!(digest.estimate_rank(5.0), Some(0.5));
    assert_eq!(digest.estimate_rank(2.5), Some(0.25));
    assert_eq!(digest.estimate_rank(7.5), Some(0.75));
}

#[test]
fn trimmed_mean_stays_finite_for_large_finite_values() {
    for values in [vec![f64::MAX; 10], vec![-f64::MAX; 10]] {
        let expected = values[0];
        let digest = TDigest::default().merge_sorted(values);
        assert_eq!(digest.trimmed_mean(0.0, 1.0), Some(expected));
        assert_eq!(digest.trimmed_mean(0.1, 0.9), Some(expected));
    }
}
