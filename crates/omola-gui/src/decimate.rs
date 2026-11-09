//! Min/max decimation for drawing dense time series.

/// Keep the first, minimum, maximum and last sample in each pixel column.
///
/// The returned indices stay in source order. One finite sample immediately before
/// `x0` and immediately after `x1` is retained so clipped lines reach the frame edge.
pub fn min_max_indices(t: &[f64], v: &[f64], x0: f64, x1: f64, columns: usize) -> Vec<usize> {
    let n = t.len().min(v.len());
    if columns == 0 || n < 2 || x1 <= x0 {
        return (0..n)
            .filter(|&i| t[i].is_finite() && v[i].is_finite())
            .collect();
    }
    let mut in_range = Vec::new();
    let mut before = None;
    let mut after = None;
    for i in 0..n {
        if !t[i].is_finite() || !v[i].is_finite() {
            continue;
        }
        if t[i] < x0 {
            before = Some(i);
        } else if t[i] > x1 {
            after = Some(i);
            break;
        } else {
            in_range.push(i);
        }
    }
    if n <= columns.saturating_mul(4) {
        let mut indices = Vec::with_capacity(in_range.len() + 2);
        indices.extend(before);
        indices.extend(in_range);
        indices.extend(after);
        return indices;
    }

    #[derive(Clone, Copy)]
    struct Bucket {
        first: usize,
        min: usize,
        max: usize,
        last: usize,
    }
    let mut buckets: Vec<Option<Bucket>> = vec![None; columns];
    let width = x1 - x0;
    for &i in &in_range {
        let column = (((t[i] - x0) / width) * columns as f64)
            .floor()
            .clamp(0.0, (columns - 1) as f64) as usize;
        match &mut buckets[column] {
            Some(bucket) => {
                bucket.last = i;
                if v[i] < v[bucket.min] {
                    bucket.min = i;
                }
                if v[i] > v[bucket.max] {
                    bucket.max = i;
                }
            }
            slot @ None => {
                *slot = Some(Bucket {
                    first: i,
                    min: i,
                    max: i,
                    last: i,
                })
            }
        }
    }
    let mut indices = Vec::with_capacity(columns.saturating_mul(4).min(n) + 2);
    indices.extend(before);
    for bucket in buckets.into_iter().flatten() {
        let mut selected = [bucket.first, bucket.min, bucket.max, bucket.last];
        selected.sort_unstable();
        for index in selected {
            if indices.last() != Some(&index) {
                indices.push(index);
            }
        }
    }
    indices.extend(after);
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_series_keeps_all_in_range_samples() {
        let t: Vec<_> = (0..10).map(f64::from).collect();
        let v = vec![0.0; t.len()];
        assert_eq!(
            min_max_indices(&t, &v, 0.0, 9.0, 100),
            (0..10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn dense_series_keeps_extrema_within_four_points_per_column() {
        let t: Vec<_> = (0..100_000).map(|i| i as f64 / 100.0).collect();
        let v: Vec<_> = t
            .iter()
            .map(|t| (t * std::f64::consts::TAU).sin())
            .collect();
        let selected = min_max_indices(&t, &v, t[0], *t.last().unwrap(), 500);
        assert!(selected.len() <= 2_000);
        assert!(selected.contains(
            &v.iter()
                .position(|x| *x == v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
                .unwrap()
        ));
        assert!(selected.contains(
            &v.iter()
                .position(|x| *x == v.iter().copied().fold(f64::INFINITY, f64::min))
                .unwrap()
        ));
    }

    #[test]
    fn keeps_only_one_sample_outside_each_range_edge() {
        let t = [-2.0, -1.0, 0.0, 0.2, 0.8, 1.0, 1.1, 1.2, 2.0];
        let v = [0.0; 9];
        let selected = min_max_indices(&t, &v, 0.0, 1.0, 1);
        assert_eq!(selected.first(), Some(&1));
        assert_eq!(selected.last(), Some(&6));
        assert!(!selected.contains(&0));
        assert!(!selected.contains(&7));
        assert!(!selected.contains(&8));
    }

    #[test]
    fn duplicate_timestamps_keep_order_and_nan_is_skipped() {
        let t = [0.0, 1.0, 1.0, 2.0];
        let v = [0.0, 1.0, 2.0, 0.0];
        let indices = min_max_indices(&t, &v, 0.0, 2.0, 1);
        assert!(indices.windows(2).all(|w| w[0] < w[1]));
        let t = [0.0, 1.0, 2.0];
        let v = [0.0, f64::NAN, 1.0];
        assert_eq!(min_max_indices(&t, &v, 0.0, 2.0, 4), vec![0, 2]);
        assert!(min_max_indices(&[], &[], 0.0, 1.0, 10).is_empty());
    }
}
