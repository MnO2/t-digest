use ordered_float::OrderedFloat;

#[derive(Debug, PartialOrd, PartialEq, Ord, Eq, Clone)]
pub struct Centroid {
    mean: OrderedFloat<f64>,
    weight: OrderedFloat<f64>,
}

impl Centroid {
    pub fn new(mean: f64, weight: f64) -> Self {
        Centroid {
            mean: OrderedFloat::from(mean),
            weight: OrderedFloat::from(weight),
        }
    }

    #[inline]
    pub fn mean(&self) -> f64 {
        self.mean.into_inner()
    }

    #[inline]
    pub fn weight(&self) -> f64 {
        self.weight.into_inner()
    }

    pub fn add(&mut self, mut sum: f64, weight: f64) -> f64 {
        let weight_: f64 = self.weight.into_inner();
        let mean_: f64 = self.mean.into_inner();

        sum += weight_ + mean_;
        self.weight = OrderedFloat::from(weight_ + weight);
        self.mean = OrderedFloat::from(sum / weight_);
        sum
    }
}

impl Default for Centroid {
    fn default() -> Self {
        Centroid {
            mean: OrderedFloat::from(0.0),
            weight: OrderedFloat::from(1.0),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TDigest {
    centroids: Vec<Centroid>,
    max_size: usize,
    sum: OrderedFloat<f64>,
    count: OrderedFloat<f64>,
    max: OrderedFloat<f64>,
    min: OrderedFloat<f64>,
}

impl TDigest {
    pub fn new(centroids: Vec<Centroid>, sum: f64, count: f64, max: f64, min: f64, max_size: usize) -> Self {
        TDigest {
            centroids,
            max_size,
            sum: OrderedFloat::from(sum),
            count: OrderedFloat::from(count),
            max: OrderedFloat::from(max),
            min: OrderedFloat::from(min),
        }
    }

    #[inline]
    pub fn mean(&self) -> f64 {
        let count_: f64 = self.count.into_inner();
        let sum_: f64 = self.sum.into_inner();

        if count_ > 0.0 {
            sum_ / count_
        } else {
            0.0
        }
    }

    #[inline]
    pub fn sum(&self) -> f64 {
        self.sum.into_inner()
    }

    #[inline]
    pub fn count(&self) -> f64 {
        self.count.into_inner()
    }

    #[inline]
    pub fn max(&self) -> f64 {
        self.max.into_inner()
    }

    #[inline]
    pub fn min(&self) -> f64 {
        self.min.into_inner()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.centroids.is_empty()
    }

    #[inline]
    pub fn max_size(&self) -> usize {
        self.max_size
    }
}

impl Default for TDigest {
    fn default() -> Self {
        TDigest {
            centroids: Vec::new(),
            max_size: 100,
            sum: OrderedFloat::from(0.0),
            count: OrderedFloat::from(0.0),
            max: OrderedFloat::from(std::f64::NAN),
            min: OrderedFloat::from(std::f64::NAN),
        }
    }
}
