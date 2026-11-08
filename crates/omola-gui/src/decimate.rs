//! Pixel-width decimation for the plot window (G21).
//!
//! A result can hold hundreds of thousands of samples per curve. Converting every one
//! of them to a screen point every frame is what made the plot slow, and most of the
//! points land on the same pixel column anyway. For each column this keeps the first,
//! smallest, largest and last sample, which is the usual way to draw a line without
//! changing its visible shape.

/// Indices into `t`/`v` to draw, in ascending order.
///
/// `t` must be non-decreasing. Samples outside `[x0, x1]` are skipped except the one
/// just before `x0` and the one just after `x1`, so the line still reaches the frame
/// edge. Degenerate input (no columns, fewer than two samples, an empty or inverted
/// range) returns every index, and non-finite values are skipped instead of panicking.
pub fn min_max_indices(t: &[f64], v: &[f64], x0: f64, x1: f64, columns: usize) -> Vec<usize> {
    let n = t.len().min(v.len());
    if columns == 0 || n < 2 || x1 <= x0 || !x1.is_finite() {
        return (0..n).collect();
    }
    // `t` is non-decreasing, so the visible samples are a contiguous run.
    let first = t[..n].partition_point(|&x| x < x0);
    let last = t[..n].partition_point(|&x| x <= x1).saturating_sub(1);
    if first >= n || first > last {
        // Nothing inside the range: the two samples around it are enough.
        let mut out = Vec::with_capacity(2);
        if first > 0 {
            out.push(first - 1);
        }
        if first < n {
            out.push(first);
        }
        return out;
    }
    let start = first.saturating_sub(1);
    let end = (last + 1).min(n - 1);
    if last - first < columns * 4 {
        return (start..=end).collect();
    }

    let span = x1 - x0;
    let mut firsts = vec![usize::MAX; columns];
    let mut lasts = vec![usize::MAX; columns];
    let mut mins = vec![usize::MAX; columns];
    let mut maxs = vec![usize::MAX; columns];
    let mut min_v = vec![f64::INFINITY; columns];
    let mut max_v = vec![f64::NEG_INFINITY; columns];
    for i in first..=last {
        let (ti, vi) = (t[i], v[i]);
        if !ti.is_finite() || !vi.is_finite() {
            continue;
        }
        let scaled = ((ti - x0) / span) * columns as f64;
        let column = if scaled <= 0.0 {
            0
        } else {
            (scaled.floor() as usize).min(columns - 1)
        };
        if firsts[column] == usize::MAX {
            firsts[column] = i;
            lasts[column] = i;
            mins[column] = i;
            maxs[column] = i;
            min_v[column] = vi;
            max_v[column] = vi;
        } else {
            lasts[column] = i;
            if vi < min_v[column] {
                min_v[column] = vi;
                mins[column] = i;
            }
            if vi > max_v[column] {
                max_v[column] = vi;
                maxs[column] = i;
            }
        }
    }

    let mut out: Vec<usize> = Vec::with_capacity(columns * 4 + 2);
    if start < first {
        out.push(start);
    }
    for column in 0..columns {
        if firsts[column] == usize::MAX {
            continue;
        }
        let mut four = [firsts[column], mins[column], maxs[column], lasts[column]];
        four.sort_unstable();
        for i in four {
            if out.last() != Some(&i) {
                out.push(i);
            }
        }
    }
    if end > last {
        out.push(end);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> (Vec<f64>, Vec<f64>) {
        let t: Vec<f64> = (0..n).map(|i| i as f64 / (n - 1) as f64).collect();
        let v: Vec<f64> = t.iter().map(|x| x.sin()).collect();
        (t, v)
    }

    #[test]
    fn few_points_are_not_decimated() {
        let (t, v) = ramp(10);
        assert_eq!(
            min_max_indices(&t, &v, 0.0, 1.0, 100),
            (0..10).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_long_curve_fits_the_columns_and_keeps_its_extremes() {
        let (t, v) = ramp(100_000);
        let out = min_max_indices(&t, &v, 0.0, 1.0, 500);
        assert!(out.len() <= 2000, "{} points", out.len());
        let max_at = (0..t.len()).max_by(|a, b| v[*a].total_cmp(&v[*b])).unwrap();
        let min_at = (0..t.len()).min_by(|a, b| v[*a].total_cmp(&v[*b])).unwrap();
        assert!(out.contains(&max_at), "the peak was dropped");
        assert!(out.contains(&min_at), "the trough was dropped");
        assert!(
            out.windows(2).all(|w| w[0] < w[1]),
            "indices stay ascending"
        );
    }

    #[test]
    fn only_the_samples_next_to_the_range_are_kept_outside_it() {
        let t: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let v: Vec<f64> = t.iter().map(|x| x * 0.5).collect();
        // The range covers 40..=60; 39 and 61 are the neighbours.
        let out = min_max_indices(&t, &v, 40.0, 60.0, 2);
        assert_eq!(out.first(), Some(&39));
        assert_eq!(out.last(), Some(&61));
        assert!(out.iter().all(|&i| (39..=61).contains(&i)), "{out:?}");
    }

    #[test]
    fn duplicate_timestamps_stay_in_order() {
        let t = [0.0, 1.0, 1.0, 2.0];
        let v = [0.0, 5.0, -5.0, 1.0];
        let out = min_max_indices(&t, &v, 0.0, 2.0, 1);
        assert!(out.windows(2).all(|w| w[0] < w[1]), "{out:?}");
        assert!(out.contains(&1) && out.contains(&2), "{out:?}");
    }

    #[test]
    fn degenerate_input_does_not_panic() {
        assert!(min_max_indices(&[], &[], 0.0, 1.0, 10).is_empty());
        let t = [0.0, f64::NAN, 0.5, 1.0];
        let v = [0.0, 1.0, f64::NAN, 1.0];
        let out = min_max_indices(&t, &v, 0.0, 1.0, 100);
        assert!(out.windows(2).all(|w| w[0] < w[1]), "{out:?}");
        // An inverted or empty range returns everything.
        assert_eq!(min_max_indices(&t, &v, 1.0, 1.0, 2).len(), 4);
        assert_eq!(min_max_indices(&t, &v, 0.0, 1.0, 0).len(), 4);
    }
}
