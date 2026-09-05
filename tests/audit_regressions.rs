use tdigest::{Centroid, TDigest};

#[test]
fn constructor_recompresses_to_the_requested_size() {
    let centroids = (1..=1_000).map(|value| Centroid::new(f64::from(value), 1.0)).collect();
    let digest = TDigest::new(centroids, 500_500.0, 1_000.0, Some(1_000.0), Some(1.0), 10);

    assert_eq!(digest.max_size(), 10);
    assert!(digest.centroids().len() <= digest.max_size());
    assert_eq!(digest.count(), 1_000.0);
    assert_eq!(digest.sum(), 500_500.0);
    assert_eq!(digest.min(), Some(1.0));
    assert_eq!(digest.max(), Some(1_000.0));
}

#[test]
fn constructor_sorts_centroids_before_queries_and_merges() {
    let digest = TDigest::new(
        vec![
            Centroid::new(3.0, 1.0),
            Centroid::new(1.0, 1.0),
            Centroid::new(2.0, 1.0),
        ],
        6.0,
        3.0,
        Some(3.0),
        Some(1.0),
        100,
    );

    assert_eq!(
        digest.centroids().iter().map(Centroid::mean).collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );
    assert_eq!(digest.estimate_quantile(0.5), Some(2.0));
    let merged = digest.merge_sorted(vec![4.0]);
    assert_eq!(merged.estimate_quantile(0.5), Some(2.5));
}

#[test]
#[should_panic(expected = "max_size must be greater than zero")]
fn empty_constructor_rejects_zero_compression_size() {
    let _ = TDigest::new_with_size(0);
}

#[test]
#[should_panic(expected = "max_size must be greater than zero")]
fn centroid_constructor_rejects_zero_compression_size() {
    let _ = TDigest::new(vec![Centroid::new(1.0, 1.0)], 1.0, 1.0, Some(1.0), Some(1.0), 0);
}

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

#[cfg(feature = "serde")]
mod serde_validation {
    use super::*;
    use serde::Deserialize;
    use serde_json::{json, Value};

    fn single_centroid() -> Value {
        json!({
            "centroids": [{ "mean": 1.0, "weight": 1.0 }],
            "max_size": 100,
            "sum": 1.0,
            "count": 1.0,
            "min": 1.0,
            "max": 1.0
        })
    }

    #[test]
    fn rejects_zero_compression_size() {
        let mut value = single_centroid();
        value["max_size"] = json!(0);
        assert!(serde_json::from_value::<TDigest>(value).is_err());
    }

    #[test]
    fn rejects_nonempty_digest_without_extrema() {
        let mut value = single_centroid();
        value["min"] = Value::Null;
        value["max"] = Value::Null;
        assert!(serde_json::from_value::<TDigest>(value).is_err());
    }

    #[test]
    fn rejects_incoherent_digest_counts() {
        for count in [0.0, -1.0, 2.0] {
            let mut value = single_centroid();
            value["count"] = json!(count);
            assert!(
                serde_json::from_value::<TDigest>(value).is_err(),
                "accepted count {count}"
            );
        }
        let mut value = single_centroid();
        value["centroids"] = json!([]);
        assert!(serde_json::from_value::<TDigest>(value).is_err());
    }

    #[test]
    fn rejects_nonpositive_digest_centroid_weights() {
        for weight in [0.0, -1.0] {
            let mut value = single_centroid();
            value["centroids"][0]["weight"] = json!(weight);
            assert!(
                serde_json::from_value::<TDigest>(value).is_err(),
                "accepted weight {weight}"
            );
        }
    }

    #[test]
    fn rejects_invalid_digest_bounds() {
        for (min, max) in [(2.0, 1.0), (2.0, 3.0), (-2.0, -1.0)] {
            let mut value = single_centroid();
            value["min"] = json!(min);
            value["max"] = json!(max);
            assert!(serde_json::from_value::<TDigest>(value).is_err());
        }
    }

    #[test]
    fn rejects_unsorted_centroids() {
        let value = json!({
            "centroids": [{ "mean": 2.0, "weight": 1.0 }, { "mean": 1.0, "weight": 1.0 }],
            "max_size": 100,
            "sum": 3.0,
            "count": 2.0,
            "min": 1.0,
            "max": 2.0
        });
        assert!(serde_json::from_value::<TDigest>(value).is_err());
    }

    #[test]
    fn round_trip_preserves_fractional_weighted_digest() {
        let digest = TDigest::new(
            vec![Centroid::new(1.0, 0.1), Centroid::new(2.0, 0.2)],
            0.5,
            0.3,
            Some(2.0),
            Some(1.0),
            100,
        );
        let restored: TDigest = serde_json::from_str(&serde_json::to_string(&digest).unwrap()).unwrap();
        assert_eq!(restored, digest);
        assert_eq!(restored.quantiles(&[0.0, 0.5, 1.0]), digest.quantiles(&[0.0, 0.5, 1.0]));
    }

    #[test]
    fn tolerated_count_roundoff_remains_bounded_after_merging() {
        let value = json!({
            "centroids": [
                { "mean": 0.0, "weight": 1.0 },
                { "mean": 1.0, "weight": 1e-11 },
                { "mean": 2.0, "weight": 1e-11 },
                { "mean": 3.0, "weight": 1e-11 }
            ],
            "max_size": 1,
            "sum": 6e-11,
            "count": 1.0,
            "min": 0.0,
            "max": 3.0
        });
        let digest: TDigest = serde_json::from_value(value).unwrap();
        let merged = TDigest::merge_digests(vec![digest]);
        assert!(merged.centroids().len() <= merged.max_size());
        assert_eq!(merged.count(), 1.0);
        assert!(merged.estimate_quantile(0.5).unwrap().is_finite());
    }

    #[test]
    fn accepts_zero_weight_standalone_centroid() {
        let centroid: Centroid = serde_json::from_str(r#"{"mean":0.0,"weight":0.0}"#).unwrap();
        assert_eq!(centroid, Centroid::new(0.0, 0.0));
    }

    #[test]
    fn rejects_nonfinite_or_negative_centroid_fields() {
        for (mean, weight) in [
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (1.0, f64::NEG_INFINITY),
            (1.0, -1.0),
        ] {
            let deserializer = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(
                [("mean", mean), ("weight", weight)].into_iter(),
            );
            assert!(Centroid::deserialize(deserializer).is_err());
        }
    }
}
