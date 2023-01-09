use std::sync::Mutex;

use crate::TDigest;

/// For use with monitoring, when you are recording a single value at a time.
/// Handles amortizing the merge into your tdigest to reduce the latency per
/// observation.
///
/// You should use this instead of a bare TDigest when you need your
/// observations to be as quick as possible, and you only occasionally read
/// the digest. (e.g., to periodically log or emit to a metrics backend)
///
/// Sharable between concurrent threads, the critical internal section is
/// protected via a mutex. You do not need mut to modify the internal state,
/// but mut will reduce the cost slightly.
///
/// Once in a while it will collapse your observations into the backing digest.
/// This merge is a more expensive observation, but it's less common. Reducing
/// the amortization size 25% to 24 costs 20% additional latency. Increasing
/// may reduce further, but we're around 43ns per observation with amortization
/// 32. This is a fine improvement over the baseline of 850ns doing a
/// per-observation merge.
///
/// The extra bookkeeping in this struct costs about 20ns. By deferring the merge,
/// the cost of each merge to each observation is about 20ns as well, for a budget
/// of 40-50ns. Your server might be faster than this.
#[derive(Default)]
pub struct OnlineTdigest {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    current: TDigest,
    amortized_observations: [f64; 32],
    i: u8,
}

impl OnlineTdigest {
    /// Get the current tdigest, merging any outstanding observations.
    pub fn get(&self) -> TDigest {
        let mut state = self.state.lock().expect("lock should never fail");
        get_snapshot(&mut state)
    }

    /// Get the current tdigest, merging any outstanding observations.
    pub fn get_mut(&mut self) -> TDigest {
        let state = self
            .state
            .get_mut()
            .expect("with &mut self the mutex should be unlocked");
        get_snapshot(state)
    }

    /// Retrieves the current tdigest, merging any outstanding observations and resetting.
    pub fn reset(&self) -> TDigest {
        let mut state = self.state.lock().expect("lock should never fail");
        let snapshot = get_snapshot(&mut state);
        state.current = Default::default();
        snapshot
    }

    /// Retrieves the current tdigest, merging any outstanding observations and resetting.
    pub fn reset_mut(&mut self) -> TDigest {
        let state = self
            .state
            .get_mut()
            .expect("with &mut self the mutex should be unlocked");
        get_snapshot_and_reset(state)
    }

    /// Record 1 occurrence of a value, to be merged into a
    /// tdigest later.
    #[inline] // saves 2-3 nanoseconds according to benchmark results
    pub fn observe(&self, observation: impl Into<f64>) {
        let mut state = self.state.lock().expect("lock should never fail");
        record_observation(&mut state, observation.into());
    }

    /// Record 1 occurrence of a value, to be merged into a
    /// tdigest later.
    /// Use this when you can, as the observe() function costs **around
    /// 30% more (9-11ns more on my laptop)** than using &mut.
    #[inline]
    pub fn observe_mut(&mut self, observation: impl Into<f64>) {
        let state = self
            .state
            .get_mut()
            .expect("with &mut self the mutex should be unlocked");
        record_observation(state, observation.into());
    }
}

#[inline]
fn get_snapshot_and_reset(state: &mut State) -> TDigest {
    let snapshot = get_snapshot(state);
    state.current = Default::default();
    snapshot
}

#[inline]
fn get_snapshot(state: &mut State) -> TDigest {
    flush_state(state);
    state.current.clone()
}

#[inline]
fn record_observation(mut state: &mut State, observation: f64) {
    let index = state.i as usize;
    state.amortized_observations[index] = observation;
    state.i += 1;
    if state.i == state.amortized_observations.len() as u8 {
        flush_state(state)
    }
}

#[inline]
fn flush_state(state: &mut State) {
    if state.i < 1 {
        return;
    }
    let new = state
        .current
        .merge_unsorted(Vec::from(&state.amortized_observations[0..state.i as usize]));
    state.current = new;
    state.i = 0;
}

#[cfg(test)]
mod tests {
    use super::OnlineTdigest;

    #[test]
    fn p999() {
        let digester = OnlineTdigest::default();
        for i in 0..10_001 {
            digester.observe(i as f64);
        }
        let digest = digester.reset();
        assert_eq!(0.0, digest.min());
        assert_eq!(10_000.0, digest.max());
        assert_eq!(10_001.0, digest.count());
        let error = 9_990.0 - digest.estimate_quantile(0.999);
        assert!(-1.0 < error && error < 1.0);
    }

    #[test]
    fn reset() {
        let digester = OnlineTdigest::default();
        digester.observe(1.23);
        digester.reset();
        let digest = digester.reset();
        assert_eq!(0.0, digest.count());
    }

    #[test]
    fn directly_observable_types() {
        let digester = OnlineTdigest::default();
        // Anything that can be turned Into<u64> can be passed straight in.
        digester.observe(1.23);
        digester.observe(1);
        digester.observe(1.23_f32);
        digester.observe(1.23_f64);
        digester.observe(1_i32);
        digester.observe(1_u32);
        // 64 bit integers will require a manual cast to f64
        (1..100).for_each(|i| digester.observe(i))
    }
}
