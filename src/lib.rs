//! T-Digest algorithm in rust
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tdigest = "1.0"
//! ```
//!
//! ## Example
//!
//! ```rust
//! use tdigest::TDigest;
//!
//! let t = TDigest::new_with_size(100);
//! let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();
//!
//! let t = t.merge_sorted(values);
//!
//! let ans = t.estimate_quantile(0.99).unwrap();
//! let expected: f64 = 990_000.0;
//!
//! let percentage: f64 = (expected - ans).abs() / expected;
//! assert!(percentage < 0.01);
//! ```
//!
//! ## Handling of non-finite values
//!
//! NaN and positive or negative infinity are contract violations. Public
//! ingestion methods check this in debug builds, but estimates are unspecified
//! if non-finite values are supplied to a release build.

use std::cmp::Ordering;

#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};

/// Centroid implementation to the cluster mentioned in the paper.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub struct Centroid {
    mean: f64,
    weight: f64,
}

impl PartialEq for Centroid {
    fn eq(&self, other: &Self) -> bool {
        self.mean == other.mean && self.weight == other.weight
    }
}

impl PartialOrd for Centroid {
    fn partial_cmp(&self, other: &Centroid) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Centroid {
    fn cmp(&self, other: &Centroid) -> Ordering {
        self.mean.total_cmp(&other.mean)
    }
}

impl Eq for Centroid {}

impl Centroid {
    pub fn new(mean: f64, weight: f64) -> Self {
        debug_assert!(mean.is_finite() && weight.is_finite(), "mean and weight must be finite");
        Centroid { mean, weight }
    }

    #[inline]
    pub fn mean(&self) -> f64 {
        self.mean
    }

    #[inline]
    pub fn weight(&self) -> f64 {
        self.weight
    }

    pub fn add(&mut self, sum: f64, weight: f64) -> f64 {
        let new_sum: f64 = sum + self.weight * self.mean;
        let new_weight: f64 = self.weight + weight;
        self.weight = new_weight;
        self.mean = new_sum / new_weight;
        new_sum
    }
}

impl Default for Centroid {
    fn default() -> Self {
        Centroid { mean: 0.0, weight: 1.0 }
    }
}

/// T-Digest to be operated on.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub struct TDigest {
    centroids: Vec<Centroid>,
    max_size: usize,
    sum: f64,
    count: f64,
    max: Option<f64>,
    min: Option<f64>,
}

impl TDigest {
    #[must_use]
    pub fn new_with_size(max_size: usize) -> Self {
        TDigest {
            centroids: Vec::new(),
            max_size,
            sum: 0.0,
            count: 0.0,
            max: None,
            min: None,
        }
    }

    #[must_use]
    pub fn new(
        centroids: Vec<Centroid>,
        sum: f64,
        count: f64,
        max: Option<f64>,
        min: Option<f64>,
        max_size: usize,
    ) -> Self {
        debug_assert!(
            centroids.is_empty() || (min.is_some() && max.is_some()),
            "non-empty digest must have min and max"
        );
        debug_assert!(min.map_or(true, f64::is_finite), "min must be finite");
        debug_assert!(max.map_or(true, f64::is_finite), "max must be finite");

        if centroids.len() <= max_size {
            TDigest {
                centroids,
                max_size,
                sum,
                count,
                max,
                min,
            }
        } else {
            let sz = centroids.len();
            let digests: Vec<TDigest> = vec![
                TDigest::new_with_size(max_size),
                TDigest::new(centroids, sum, count, max, min, sz),
            ];

            Self::merge_digests(digests)
        }
    }

    #[inline]
    #[must_use]
    pub fn mean(&self) -> Option<f64> {
        if self.count > 0.0 {
            Some(self.sum / self.count)
        } else {
            None
        }
    }

    #[inline]
    pub fn sum(&self) -> f64 {
        self.sum
    }

    #[inline]
    pub fn count(&self) -> f64 {
        self.count
    }

    #[inline]
    pub fn max(&self) -> Option<f64> {
        self.max
    }

    #[inline]
    pub fn min(&self) -> Option<f64> {
        self.min
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.centroids.is_empty()
    }

    #[inline]
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    #[inline]
    #[must_use]
    pub fn centroids(&self) -> &[Centroid] {
        &self.centroids
    }
}

impl Default for TDigest {
    fn default() -> Self {
        TDigest {
            centroids: Vec::new(),
            max_size: 100,
            sum: 0.0,
            count: 0.0,
            max: None,
            min: None,
        }
    }
}

impl TDigest {
    fn k_to_q(k: f64, d: f64) -> f64 {
        let k_div_d = k / d;
        if k_div_d >= 0.5 {
            let base = 1.0 - k_div_d;
            1.0 - 2.0 * base * base
        } else {
            2.0 * k_div_d * k_div_d
        }
    }

    #[must_use]
    pub fn merge_unsorted(&self, unsorted_values: Vec<f64>) -> TDigest {
        debug_assert!(
            unsorted_values.iter().all(|value| value.is_finite()),
            "input values must be finite"
        );
        let mut sorted_values = unsorted_values;
        sorted_values.sort_by(f64::total_cmp);
        self.merge_sorted(sorted_values)
    }

    #[must_use]
    pub fn merge_sorted(&self, sorted_values: Vec<f64>) -> TDigest {
        debug_assert!(
            sorted_values.iter().all(|value| value.is_finite()),
            "input values must be finite"
        );
        if sorted_values.is_empty() {
            return self.clone();
        }

        let mut result = TDigest::new_with_size(self.max_size());
        result.count = self.count() + (sorted_values.len() as f64);

        let maybe_min = *sorted_values.first().unwrap();
        let maybe_max = *sorted_values.last().unwrap();

        if self.count() > 0.0 {
            result.min = Some(self.min.unwrap().min(maybe_min));
            result.max = Some(self.max.unwrap().max(maybe_max));
        } else {
            result.min = Some(maybe_min);
            result.max = Some(maybe_max);
        }

        let mut compressed: Vec<Centroid> = Vec::with_capacity(self.max_size);

        let mut k_limit: f64 = 1.0;
        let mut q_limit_times_count: f64 = Self::k_to_q(k_limit, self.max_size as f64) * result.count;
        k_limit += 1.0;

        let mut iter_centroids = self.centroids.iter().peekable();
        let mut iter_sorted_values = sorted_values.iter().peekable();

        let mut curr: Centroid = if let Some(c) = iter_centroids.peek() {
            let curr = **iter_sorted_values.peek().unwrap();
            if c.mean() < curr {
                iter_centroids.next().unwrap().clone()
            } else {
                Centroid::new(*iter_sorted_values.next().unwrap(), 1.0)
            }
        } else {
            Centroid::new(*iter_sorted_values.next().unwrap(), 1.0)
        };

        let mut weight_so_far: f64 = curr.weight();

        let mut sums_to_merge: f64 = 0.0;
        let mut weights_to_merge: f64 = 0.0;

        while iter_centroids.peek().is_some() || iter_sorted_values.peek().is_some() {
            let next: Centroid = if let Some(c) = iter_centroids.peek() {
                if iter_sorted_values.peek().is_none() || c.mean() < **iter_sorted_values.peek().unwrap() {
                    iter_centroids.next().unwrap().clone()
                } else {
                    Centroid::new(*iter_sorted_values.next().unwrap(), 1.0)
                }
            } else {
                Centroid::new(*iter_sorted_values.next().unwrap(), 1.0)
            };

            let next_sum: f64 = next.mean() * next.weight();
            weight_so_far += next.weight();

            if weight_so_far <= q_limit_times_count {
                sums_to_merge += next_sum;
                weights_to_merge += next.weight();
            } else {
                result.sum += curr.add(sums_to_merge, weights_to_merge);
                sums_to_merge = 0.0;
                weights_to_merge = 0.0;

                compressed.push(curr.clone());
                q_limit_times_count = Self::k_to_q(k_limit, self.max_size as f64) * result.count;
                k_limit += 1.0;
                curr = next;
            }
        }

        result.sum += curr.add(sums_to_merge, weights_to_merge);
        compressed.push(curr);
        compressed.shrink_to_fit();
        compressed.sort();

        result.centroids = compressed;
        result
    }

    /// Merge several digests into one.
    ///
    /// The result uses the largest `max_size` among the inputs. With no inputs,
    /// this returns a digest with the default size of 100.
    #[must_use]
    pub fn merge_digests(digests: Vec<TDigest>) -> TDigest {
        let max_size = digests.iter().map(TDigest::max_size).max().unwrap_or(100);
        let n_centroids: usize = digests.iter().map(|d| d.centroids.len()).sum();
        if n_centroids == 0 {
            return TDigest::new_with_size(max_size);
        }

        let mut centroids: Vec<Centroid> = Vec::with_capacity(n_centroids);

        let mut count: f64 = 0.0;
        let mut min: Option<f64> = None;
        let mut max: Option<f64> = None;

        for digest in digests.into_iter() {
            let curr_count: f64 = digest.count();
            if curr_count > 0.0 {
                min = Some(match min {
                    Some(v) => v.min(digest.min.unwrap()),
                    None => digest.min.unwrap(),
                });
                max = Some(match max {
                    Some(v) => v.max(digest.max.unwrap()),
                    None => digest.max.unwrap(),
                });
                count += curr_count;
                for centroid in digest.centroids {
                    centroids.push(centroid);
                }
            }
        }

        centroids.sort();

        let mut result = TDigest::new_with_size(max_size);
        let mut compressed: Vec<Centroid> = Vec::with_capacity(max_size);

        let mut k_limit: f64 = 1.0;
        let mut q_limit_times_count: f64 = Self::k_to_q(k_limit, max_size as f64) * count;
        k_limit += 1.0;

        let mut iter_centroids = centroids.iter_mut();
        let mut curr = iter_centroids.next().unwrap();
        let mut weight_so_far: f64 = curr.weight();
        let mut sums_to_merge: f64 = 0.0;
        let mut weights_to_merge: f64 = 0.0;

        for centroid in iter_centroids {
            weight_so_far += centroid.weight();

            if weight_so_far <= q_limit_times_count {
                sums_to_merge += centroid.mean() * centroid.weight();
                weights_to_merge += centroid.weight();
            } else {
                result.sum += curr.add(sums_to_merge, weights_to_merge);
                sums_to_merge = 0.0;
                weights_to_merge = 0.0;
                compressed.push(curr.clone());
                q_limit_times_count = Self::k_to_q(k_limit, max_size as f64) * count;
                k_limit += 1.0;
                curr = centroid;
            }
        }

        result.sum += curr.add(sums_to_merge, weights_to_merge);
        compressed.push(curr.clone());
        compressed.shrink_to_fit();
        compressed.sort();

        result.count = count;
        result.min = min;
        result.max = max;
        result.centroids = compressed;
        result
    }

    /// Estimate the value at quantile `q` (0.0 to 1.0).
    /// Returns `None` if the digest is empty.
    #[must_use]
    pub fn estimate_quantile(&self, q: f64) -> Option<f64> {
        if self.centroids.is_empty() {
            return None;
        }

        let count = self.count;
        let rank: f64 = q * count;

        let mut pos: usize;
        let mut t: f64;
        if q > 0.5 {
            if q >= 1.0 {
                return self.max;
            }

            pos = 0;
            t = count;

            for (k, centroid) in self.centroids.iter().enumerate().rev() {
                t -= centroid.weight();

                if rank >= t {
                    pos = k;
                    break;
                }
            }
        } else {
            if q <= 0.0 {
                return self.min;
            }

            pos = self.centroids.len() - 1;
            t = 0.0;

            for (k, centroid) in self.centroids.iter().enumerate() {
                if rank < t + centroid.weight() {
                    pos = k;
                    break;
                }

                t += centroid.weight();
            }
        }

        let mut delta = 0.0;
        let mut min = self.min.unwrap();
        let mut max = self.max.unwrap();

        if self.centroids.len() > 1 {
            if pos == 0 {
                delta = self.centroids[pos + 1].mean() - self.centroids[pos].mean();
                max = self.centroids[pos + 1].mean();
            } else if pos == (self.centroids.len() - 1) {
                delta = self.centroids[pos].mean() - self.centroids[pos - 1].mean();
                min = self.centroids[pos - 1].mean();
            } else {
                delta = (self.centroids[pos + 1].mean() - self.centroids[pos - 1].mean()) / 2.0;
                min = self.centroids[pos - 1].mean();
                max = self.centroids[pos + 1].mean();
            }
        }

        let value = self.centroids[pos].mean() + ((rank - t) / self.centroids[pos].weight() - 0.5) * delta;
        Some(value.clamp(min, max))
    }

    /// Estimate the rank (CDF) of `value`: the fraction of inserted values less
    /// than or equal to it.
    ///
    /// Returns `None` if the digest is empty.
    #[must_use]
    pub fn estimate_rank(&self, value: f64) -> Option<f64> {
        debug_assert!(!value.is_nan(), "value must not be NaN");
        if self.centroids.is_empty() {
            return None;
        }

        let min = self.min.unwrap();
        let max = self.max.unwrap();
        if self.centroids.len() == 1 || min == max {
            return Some(if value >= max { 1.0 } else { 0.0 });
        }
        if value <= min {
            return Some(0.0);
        }
        if value >= max {
            return Some(1.0);
        }

        let first = &self.centroids[0];
        if value < first.mean() {
            let width = first.mean() - min;
            let rank = if width > 0.0 {
                (value - min) / width * first.weight() / 2.0
            } else {
                0.0
            };
            return Some((rank / self.count).clamp(0.0, 1.0));
        }

        let mut weight_before = 0.0;
        for pair in self.centroids.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            let left_rank = weight_before + left.weight() / 2.0;
            let right_rank = weight_before + left.weight() + right.weight() / 2.0;
            if value <= right.mean() {
                let width = right.mean() - left.mean();
                let rank = if width > 0.0 {
                    left_rank + (value - left.mean()) / width * (right_rank - left_rank)
                } else {
                    right_rank
                };
                return Some((rank / self.count).clamp(0.0, 1.0));
            }
            weight_before += left.weight();
        }

        let last = self.centroids.last().unwrap();
        let last_rank = self.count - last.weight() / 2.0;
        let width = max - last.mean();
        let rank = if width > 0.0 {
            last_rank + (value - last.mean()) / width * (self.count - last_rank)
        } else {
            self.count
        };
        Some((rank / self.count).clamp(0.0, 1.0))
    }

    /// Estimate the mean of values between quantiles `lo` and `hi`.
    ///
    /// Quantiles are clamped to `[0.0, 1.0]`. Returns `None` if the digest is
    /// empty, either bound is NaN, or the resulting interval is empty.
    #[must_use]
    pub fn trimmed_mean(&self, lo: f64, hi: f64) -> Option<f64> {
        if self.centroids.is_empty() || lo.is_nan() || hi.is_nan() || lo >= hi {
            return None;
        }

        let lower = lo.clamp(0.0, 1.0) * self.count;
        let upper = hi.clamp(0.0, 1.0) * self.count;
        if lower >= upper {
            return None;
        }

        let mut cumulative = 0.0;
        let mut included_weight = 0.0;
        let mut included_sum = 0.0;
        for centroid in &self.centroids {
            let start = cumulative;
            let end = start + centroid.weight();
            let overlap = end.min(upper) - start.max(lower);
            if overlap > 0.0 {
                included_weight += overlap;
                included_sum += centroid.mean() * overlap;
            }
            cumulative = end;
            if cumulative >= upper {
                break;
            }
        }

        if included_weight > 0.0 {
            Some(included_sum / included_weight)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centroid_addition_regression() {
        //https://github.com/MnO2/t-digest/pull/1

        let vals = vec![1.0, 1.0, 1.0, 2.0, 1.0, 1.0];
        let mut t = TDigest::new_with_size(10);

        for v in vals {
            t = t.merge_unsorted(vec![v]);
        }

        let ans = t.estimate_quantile(0.5).unwrap();
        let expected: f64 = 1.0;
        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.95).unwrap();
        let expected: f64 = 2.0;
        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);
    }

    #[test]
    fn test_merge_sorted_against_uniform_distro() {
        let t = TDigest::new_with_size(100);
        let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();

        let t = t.merge_sorted(values);

        let ans = t.estimate_quantile(1.0).unwrap();
        let expected: f64 = 1_000_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.99).unwrap();
        let expected: f64 = 990_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.01).unwrap();
        let expected: f64 = 10_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.0).unwrap();
        let expected: f64 = 1.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.5).unwrap();
        let expected: f64 = 500_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);
    }

    #[test]
    fn test_merge_unsorted_against_uniform_distro() {
        let t = TDigest::new_with_size(100);
        let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();

        let t = t.merge_unsorted(values);

        let ans = t.estimate_quantile(1.0).unwrap();
        let expected: f64 = 1_000_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.99).unwrap();
        let expected: f64 = 990_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.01).unwrap();
        let expected: f64 = 10_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.0).unwrap();
        let expected: f64 = 1.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.5).unwrap();
        let expected: f64 = 500_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);
    }

    #[test]
    fn test_merge_sorted_against_skewed_distro() {
        let t = TDigest::new_with_size(100);
        let mut values: Vec<f64> = (1..=600_000).map(f64::from).collect();
        for _ in 0..400_000 {
            values.push(1_000_000.0);
        }

        let t = t.merge_sorted(values);

        let ans = t.estimate_quantile(0.99).unwrap();
        let expected: f64 = 1_000_000.0;
        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.01).unwrap();
        let expected: f64 = 10_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.5).unwrap();
        let expected: f64 = 500_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);
    }

    #[test]
    fn test_merge_unsorted_against_skewed_distro() {
        let t = TDigest::new_with_size(100);
        let mut values: Vec<f64> = (1..=600_000).map(f64::from).collect();
        for _ in 0..400_000 {
            values.push(1_000_000.0);
        }

        let t = t.merge_unsorted(values);

        let ans = t.estimate_quantile(0.99).unwrap();
        let expected: f64 = 1_000_000.0;
        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.01).unwrap();
        let expected: f64 = 10_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.5).unwrap();
        let expected: f64 = 500_000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);
    }

    #[test]
    fn test_merge_digests() {
        let mut digests: Vec<TDigest> = Vec::new();

        for _ in 1..=100 {
            let t = TDigest::new_with_size(100);
            let values: Vec<f64> = (1..=1_000).map(f64::from).collect();
            let t = t.merge_sorted(values);
            digests.push(t)
        }

        let t = TDigest::merge_digests(digests);

        let ans = t.estimate_quantile(1.0).unwrap();
        let expected: f64 = 1000.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.99).unwrap();
        let expected: f64 = 990.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.01).unwrap();
        let expected: f64 = 10.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.2);

        let ans = t.estimate_quantile(0.0).unwrap();
        let expected: f64 = 1.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);

        let ans = t.estimate_quantile(0.5).unwrap();
        let expected: f64 = 500.0;

        let percentage: f64 = (expected - ans).abs() / expected;
        assert!(percentage < 0.01);
    }

    #[test]
    fn test_merge_digests_matches_merge_sorted() {
        // Regression test for k_limit off-by-one in merge_digests.
        // The bug caused the first centroid bucket to absorb more weight
        // than intended because k_limit was not incremented after the
        // initial q_limit_times_count computation.
        let mut digests: Vec<TDigest> = Vec::new();
        let all_values: Vec<f64> = (1..=10_000).map(f64::from).collect();

        // Build 10 small digests
        for chunk in all_values.chunks(1000) {
            let t = TDigest::new_with_size(100);
            let t = t.merge_sorted(chunk.to_vec());
            digests.push(t);
        }

        let merged = TDigest::merge_digests(digests);

        // Build one big digest from all values
        let single = TDigest::new_with_size(100);
        let single = single.merge_sorted(all_values);

        // Compare the weight of the first centroid. The off-by-one bug
        // causes merge_digests to use k_limit=1 twice, so its first
        // bucket absorbs more weight than merge_sorted's first bucket.
        let merged_first_weight = merged.centroids().first().unwrap().weight();
        let single_first_weight = single.centroids().first().unwrap().weight();

        let weight_ratio = merged_first_weight / single_first_weight;
        assert!(
            weight_ratio < 1.5,
            "First centroid weight divergence too high: merge_digests={}, merge_sorted={}, ratio={:.2}",
            merged_first_weight,
            single_first_weight,
            weight_ratio
        );

        // Also verify quantile estimates are close
        for q in &[0.1, 0.25, 0.5, 0.75, 0.9, 0.99] {
            let merged_est = merged.estimate_quantile(*q).unwrap();
            let single_est = single.estimate_quantile(*q).unwrap();
            let pct = (merged_est - single_est).abs() / single_est;
            assert!(
                pct < 0.05,
                "Quantile {} divergence too high: merge_digests={}, merge_sorted={}, diff={:.2}%",
                q,
                merged_est,
                single_est,
                pct * 100.0
            );
        }
    }

    #[test]
    fn test_empty_digest() {
        let t = TDigest::new_with_size(100);
        assert!(t.is_empty());
        assert_eq!(t.count(), 0.0);
        assert_eq!(t.sum(), 0.0);
        assert_eq!(t.min(), None);
        assert_eq!(t.max(), None);
        assert_eq!(t.mean(), None);
        assert_eq!(t.estimate_quantile(0.5), None);
        assert_eq!(t.centroids().len(), 0);
    }

    #[test]
    fn test_single_value() {
        let t = TDigest::new_with_size(100);
        let t = t.merge_sorted(vec![42.0]);
        assert!(!t.is_empty());
        assert_eq!(t.count(), 1.0);
        assert_eq!(t.min(), Some(42.0));
        assert_eq!(t.max(), Some(42.0));
        assert_eq!(t.mean(), Some(42.0));
        assert_eq!(t.estimate_quantile(0.0), Some(42.0));
        assert_eq!(t.estimate_quantile(0.5), Some(42.0));
        assert_eq!(t.estimate_quantile(1.0), Some(42.0));
    }

    #[test]
    fn test_negative_values() {
        let t = TDigest::new_with_size(100);
        let values: Vec<f64> = (-500..=500).map(f64::from).collect();
        let t = t.merge_sorted(values);

        assert_eq!(t.min(), Some(-500.0));
        assert_eq!(t.max(), Some(500.0));

        let median = t.estimate_quantile(0.5).unwrap();
        assert!((median - 0.0).abs() < 10.0, "Median should be near 0, got {}", median);
    }

    #[test]
    fn test_quantile_monotonicity() {
        let t = TDigest::new_with_size(100);
        let values: Vec<f64> = (1..=10_000).map(f64::from).collect();
        let t = t.merge_sorted(values);

        let quantiles = [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0];
        let estimates: Vec<f64> = quantiles.iter().map(|q| t.estimate_quantile(*q).unwrap()).collect();

        for i in 1..estimates.len() {
            assert!(
                estimates[i] >= estimates[i - 1],
                "Quantile estimates not monotonic: q={} -> {}, q={} -> {}",
                quantiles[i - 1],
                estimates[i - 1],
                quantiles[i],
                estimates[i]
            );
        }
    }

    #[test]
    #[should_panic(expected = "non-empty digest must have min and max")]
    fn test_new_panics_on_missing_min_max() {
        let centroids = vec![Centroid::new(1.0, 1.0)];
        let _ = TDigest::new(centroids, 1.0, 1.0, None, None, 100);
    }

    #[test]
    fn test_merge_digests_empty_preserves_max_size() {
        let digests: Vec<TDigest> = vec![TDigest::new_with_size(200), TDigest::new_with_size(200)];
        let result = TDigest::merge_digests(digests);
        assert_eq!(result.max_size(), 200);
    }

    #[test]
    #[should_panic(expected = "input values must be finite")]
    fn test_merge_unsorted_rejects_nan_in_debug_builds() {
        let _ = TDigest::default().merge_unsorted(vec![1.0, f64::NAN]);
    }

    #[test]
    #[should_panic(expected = "input values must be finite")]
    fn test_merge_sorted_rejects_infinity_in_debug_builds() {
        let _ = TDigest::default().merge_sorted(vec![1.0, f64::INFINITY]);
    }

    #[test]
    fn test_estimate_rank_uniform_and_round_trip() {
        let values: Vec<f64> = (1..=1_000_000).map(f64::from).collect();
        let digest = TDigest::new_with_size(100).merge_sorted(values);
        assert!((digest.estimate_rank(500_000.0).unwrap() - 0.5).abs() < 0.01);
        assert_eq!(digest.estimate_rank(1.0), Some(0.0));
        assert_eq!(digest.estimate_rank(1_000_000.0), Some(1.0));

        for q in [0.01, 0.25, 0.5, 0.75, 0.99] {
            let value = digest.estimate_quantile(q).unwrap();
            let rank = digest.estimate_rank(value).unwrap();
            assert!((rank - q).abs() < 0.03, "q={q}, rank={rank}");
        }
    }

    #[test]
    fn test_estimate_rank_single_value() {
        let digest = TDigest::default().merge_sorted(vec![42.0]);
        assert_eq!(digest.estimate_rank(41.0), Some(0.0));
        assert_eq!(digest.estimate_rank(42.0), Some(1.0));
        assert_eq!(digest.estimate_rank(43.0), Some(1.0));
        assert_eq!(TDigest::default().estimate_rank(42.0), None);
    }

    #[test]
    fn test_trimmed_mean() {
        let values: Vec<f64> = (1..=100_000).map(f64::from).collect();
        let digest = TDigest::new_with_size(100).merge_sorted(values);
        let exact = 50_000.5;
        let trimmed = digest.trimmed_mean(0.1, 0.9).unwrap();
        assert!((trimmed - exact).abs() / exact < 0.01);
        assert!((digest.trimmed_mean(0.0, 1.0).unwrap() - digest.mean().unwrap()).abs() < 1e-9);
        assert_eq!(digest.trimmed_mean(0.5, 0.5), None);
        assert_eq!(TDigest::default().trimmed_mean(0.0, 1.0), None);
    }

    #[test]
    fn test_merge_digests_uses_largest_max_size() {
        let small = TDigest::new_with_size(100).merge_sorted((1..=1_000).map(f64::from).collect());
        let large = TDigest::new_with_size(500).merge_sorted((1_001..=2_000).map(f64::from).collect());
        let merged = TDigest::merge_digests(vec![small, large]);
        assert_eq!(merged.max_size(), 500);
        assert!(merged.centroids().len() <= 500);
    }

    #[cfg(feature = "use_serde")]
    #[test]
    fn test_serde_round_trip() {
        let t = TDigest::new_with_size(100);
        let values: Vec<f64> = (1..=1_000).map(f64::from).collect();
        let t = t.merge_sorted(values);

        let serialized = serde_json::to_string(&t).unwrap();
        let deserialized: TDigest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(t.count(), deserialized.count());
        assert_eq!(t.min(), deserialized.min());
        assert_eq!(t.max(), deserialized.max());
        assert_eq!(t.centroids().len(), deserialized.centroids().len());

        for q in &[0.1, 0.5, 0.9, 0.99] {
            assert_eq!(t.estimate_quantile(*q), deserialized.estimate_quantile(*q));
        }
    }
}
