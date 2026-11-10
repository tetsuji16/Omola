//! Plot diagrams and CSV export (G10).
//!
//! A plot window holds one or more diagrams stacked vertically; they share the time
//! axis but each keeps its own y zoom.

use crate::app::PlotSignal;

/// One diagram of the plot window: the curves it shows and its own y range.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotDiagram {
    pub signals: Vec<PlotSignal>,
    /// y range picked by a drag zoom; `None` = fit the curves of this diagram.
    pub y_view: Option<(f64, f64)>,
    pub style: PlotStyle,
}

impl PlotDiagram {
    /// Reset manual axis ranges so the plot fits its data again.
    pub fn rescale(&mut self) {
        self.y_view = None;
        self.style.x_view = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPos {
    #[default]
    Top,
    Bottom,
    Hidden,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlotStyle {
    pub grid: bool,
    pub legend: LegendPos,
    pub line_width: f32,
    pub x_view: Option<(f64, f64)>,
}

impl Default for PlotStyle {
    fn default() -> Self {
        Self {
            grid: true,
            legend: LegendPos::Top,
            line_width: 1.0,
            x_view: None,
        }
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;

    #[test]
    fn plot_style_has_expected_defaults() {
        assert_eq!(
            PlotStyle::default(),
            PlotStyle {
                grid: true,
                legend: LegendPos::Top,
                line_width: 1.0,
                x_view: None,
            }
        );
    }

    #[test]
    fn rescale_clears_both_manual_axis_ranges() {
        let mut diagram = PlotDiagram {
            y_view: Some((-1.0, 2.0)),
            style: PlotStyle {
                x_view: Some((0.4, 0.8)),
                ..PlotStyle::default()
            },
            ..PlotDiagram::default()
        };
        diagram.rescale();
        assert_eq!(diagram.y_view, None);
        assert_eq!(diagram.style.x_view, None);
    }
}

/// CSV of the plotted curves, shaped like `omola results export` writes it: a `time`
/// column first, then one column per curve, every value printed the way Rust prints
/// an `f64` (shortest round-trip).
///
/// Rows are the samples of the run; an event makes two rows share one instant, which
/// `omola results inspect` accepts (`time` must be monotonic non-decreasing). A curve
/// shorter than the others stops the export at the shortest one, so no row is empty.
pub fn plotted_csv(time: &[f64], columns: &[(&str, &[f64])]) -> String {
    let mut out = String::new();
    let mut header = vec!["time".to_owned()];
    header.extend(columns.iter().map(|(name, _)| (*name).to_owned()));
    write_record(&mut out, &header);
    let rows = columns
        .iter()
        .map(|(_, values)| values.len())
        .min()
        .unwrap_or(0)
        .min(time.len());
    for i in 0..rows {
        let mut row = vec![format!("{}", time[i])];
        row.extend(columns.iter().map(|(_, values)| format!("{}", values[i])));
        write_record(&mut out, &row);
    }
    out
}

/// One CSV record: fields with a comma, quote or line break are quoted, quotes doubled.
fn write_record(out: &mut String, fields: &[String]) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        if field.contains([',', '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&field.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(field);
        }
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_starts_with_time_and_prints_round_trip_values() {
        let csv = plotted_csv(
            &[0.0, 0.5, 1.0],
            &[
                ("height", &[1.0, 0.1, -0.25]),
                ("velocity", &[0.0, -0.5, -1.0]),
            ],
        );
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "time,height,velocity");
        assert_eq!(lines[1], "0,1,0");
        assert_eq!(lines.len(), 4);
        // Every number must parse back to the value it came from.
        let value: f64 = lines[2].split(',').nth(1).unwrap().parse().unwrap();
        assert_eq!(value, 0.1);
    }

    #[test]
    fn csv_quotes_names_with_a_comma_or_a_quote() {
        let csv = plotted_csv(&[0.0], &[("a,b", &[1.0]), ("say \"hi\"", &[2.0])]);
        assert_eq!(
            csv.lines().next().unwrap(),
            "time,\"a,b\",\"say \"\"hi\"\"\""
        );
    }

    #[test]
    fn csv_stops_at_the_shortest_curve() {
        let csv = plotted_csv(&[0.0, 1.0, 2.0], &[("a", &[1.0, 2.0])]);
        assert_eq!(csv.lines().count(), 3, "header plus two rows");
        assert!(
            csv.lines().all(|line| !line.ends_with(',')),
            "no empty field"
        );
    }

    #[test]
    fn csv_without_curves_has_only_the_time_header() {
        assert_eq!(plotted_csv(&[0.0], &[]), "time\n");
    }

    #[test]
    fn duplicate_instants_are_kept_as_separate_rows() {
        // An event gives two samples at t = 1; `results inspect` accepts both.
        let csv = plotted_csv(&[0.0, 1.0, 1.0], &[("x", &[0.0, 1.0, 2.0])]);
        assert_eq!(csv.lines().count(), 4);
        assert_eq!(csv.lines().last().unwrap(), "1,2");
    }
}
