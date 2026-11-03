//! Sampling of simulation series (G09).
//!
//! Results are stored as one `time` vector plus one vector per variable, so asking for
//! the value at an arbitrary instant is a lookup plus an interpolation. The plot window
//! (G10) and the Variable Browser share this.

/// Value of `values` at time `t`.
///
/// `time` is non-decreasing: an event can emit the same instant twice (before and after
/// the event), and then the **last** sample wins, which is the value after the event.
/// Outside the sampled range the nearest end value is returned; in between the value is
/// interpolated linearly. `None` when there is nothing to sample.
pub fn value_at(time: &[f64], values: &[f64], t: f64) -> Option<f64> {
    let n = time.len().min(values.len());
    if n == 0 || !t.is_finite() {
        return None;
    }
    let last = n - 1;
    if t <= time[0] {
        return Some(values[0]);
    }
    if t >= time[last] {
        return Some(values[last]);
    }
    // First sample strictly after `t`; the one before it is the last sample at or before
    // `t`, so a duplicated timestamp resolves to the post-event sample.
    let hi = time[..n].partition_point(|&x| x <= t);
    let lo = hi - 1;
    let (t0, t1) = (time[lo], time[hi]);
    if t1 <= t0 {
        return Some(values[lo]);
    }
    let f = (t - t0) / (t1 - t0);
    Some(values[lo] + (values[hi] - values[lo]) * f)
}

/// The time range of a result, or `None` when it has no samples.
pub fn time_range(time: &[f64]) -> Option<(f64, f64)> {
    let first = *time.first()?;
    let last = *time.last()?;
    (first.is_finite() && last.is_finite()).then_some((first, last))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_between_samples() {
        let time = [0.0, 1.0, 2.0];
        let values = [0.0, 10.0, 20.0];
        assert_eq!(value_at(&time, &values, 0.5), Some(5.0));
        assert_eq!(value_at(&time, &values, 1.25), Some(12.5));
        assert_eq!(value_at(&time, &values, 1.0), Some(10.0));
    }

    #[test]
    fn clamps_outside_the_range() {
        let time = [0.0, 1.0, 2.0];
        let values = [3.0, 10.0, 20.0];
        assert_eq!(value_at(&time, &values, -5.0), Some(3.0));
        assert_eq!(value_at(&time, &values, 99.0), Some(20.0));
    }

    #[test]
    fn a_duplicated_instant_uses_the_post_event_sample() {
        // An event at t = 1 s: the state jumps from 10 to 20.
        let time = [0.0, 1.0, 1.0, 2.0];
        let values = [0.0, 10.0, 20.0, 30.0];
        assert_eq!(value_at(&time, &values, 1.0), Some(20.0));
        assert_eq!(value_at(&time, &values, 0.5), Some(5.0));
        assert_eq!(value_at(&time, &values, 1.5), Some(25.0));
    }

    #[test]
    fn empty_or_unusable_series_have_no_value() {
        assert_eq!(value_at(&[], &[], 0.0), None);
        assert_eq!(value_at(&[0.0, 1.0], &[], 0.0), None);
        assert_eq!(value_at(&[0.0, 1.0], &[1.0, 2.0], f64::NAN), None);
        // A single sample is both ends of the range.
        assert_eq!(value_at(&[1.0], &[7.0], 0.0), Some(7.0));
        assert_eq!(value_at(&[1.0], &[7.0], 5.0), Some(7.0));
    }

    #[test]
    fn time_ranges_skip_unusable_ends() {
        assert_eq!(time_range(&[0.0, 1.0, 2.0]), Some((0.0, 2.0)));
        assert_eq!(time_range(&[]), None);
        assert_eq!(time_range(&[f64::NAN, 1.0]), None);
    }
}
