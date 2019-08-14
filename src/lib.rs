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
}
