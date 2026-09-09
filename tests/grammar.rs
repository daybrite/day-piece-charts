// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Headless tests for the parts of a chart that are arithmetic rather than pixels: tick selection,
//! scale geometry, the position adjustments, and the monotone spline's no-overshoot guarantee.
//!
//! None of this needs a toolkit. `day::measure_text` answers its documented approximation when no
//! tree is mounted, which is exactly what a unit test wants: a deterministic width that does not
//! depend on which machine runs it.

use day_piece_charts::axis::AxisPosition;
use day_piece_charts::data::Interval;
use day_piece_charts::mark::Stacking;
use day_piece_charts::scale::{BandPadding, ScaleKind, ScaleSpec, infer};
use day_piece_charts::ticks::{self, LabelFit, civil};
use day_piece_charts::*;

fn fit(len: f64) -> LabelFit<'static> {
    // A generous per-character estimate, so these tests exercise the SEARCH rather than the
    // legibility cutoff.
    LabelFit {
        axis_length: len,
        gap: 4.0,
        measure: &|s: &str| s.len() as f64 * 6.0,
    }
}

#[test]
fn extended_wilkinson_prefers_round_steps_and_covers_the_data() {
    let l = ticks::extended(Interval::new(0.0, 61.0), 5, &fit(400.0));
    assert!(l.values.len() >= 3, "too few labels: {:?}", l.values);
    // The step has to be one of the preferred multipliers times a power of ten — that is the
    // simplicity criterion, and it is what stops an axis labelled 0, 12.2, 24.4.
    let mantissa = l.step / 10f64.powf(l.step.log10().floor());
    assert!(
        [1.0, 2.0, 2.5, 3.0, 4.0, 5.0]
            .iter()
            .any(|q| (mantissa - q).abs() < 1e-6),
        "step {} is not a preferred multiple",
        l.step
    );
    // Coverage: the labels have to reach the data, or the plot has dead space at one end.
    assert!(*l.values.first().unwrap() <= 0.0 + 1e-9);
    assert!(*l.values.last().unwrap() >= 61.0 - l.step);
}

#[test]
fn a_domain_containing_zero_is_labelled_through_zero() {
    let l = ticks::extended(Interval::new(-40.0, 90.0), 6, &fit(400.0));
    assert!(
        l.values.iter().any(|v| v.abs() < 1e-9),
        "zero is missing from {:?} — the simplicity term rewards including it",
        l.values
    );
}

#[test]
fn a_short_axis_gets_fewer_labels_than_a_long_one() {
    // The legibility criterion is the whole reason the tick count follows the pane: the same
    // domain must not put ten labels on a 60-point axis.
    let long = ticks::extended(Interval::new(0.0, 1000.0), 8, &fit(800.0));
    let short = ticks::extended(Interval::new(0.0, 1000.0), 8, &fit(70.0));
    assert!(
        short.values.len() < long.values.len(),
        "short axis kept {} labels against the long axis's {}",
        short.values.len(),
        long.values.len()
    );
}

#[test]
fn labels_print_at_the_precision_the_step_implies() {
    assert_eq!(ticks::format_value(0.25, 0.25), "0.25");
    assert_eq!(ticks::format_value(4.0, 1.0), "4");
    // A negative zero is a real f64 and prints its sign, which reads as an error on an axis.
    assert_eq!(ticks::format_value(-0.0, 1.0), "0");
}

#[test]
fn log_ticks_are_decades() {
    let l = ticks::log_ticks(Interval::new(1.0, 10_000.0), 10.0, 6);
    assert_eq!(l.values, vec![1.0, 10.0, 100.0, 1000.0, 10_000.0]);
}

#[test]
fn civil_dates_round_trip_through_the_epoch_and_a_leap_day() {
    assert_eq!(civil::from_days(0), (1970, 1, 1));
    assert_eq!(civil::to_days(1970, 1, 1), 0);
    // 2024 is a leap year; the 29th has to exist and come back unchanged.
    let d = civil::to_days(2024, 2, 29);
    assert_eq!(civil::from_days(d), (2024, 2, 29));
    assert_eq!(
        civil::from_days(civil::to_days(1899, 12, 31)),
        (1899, 12, 31)
    );
}

#[test]
fn a_monthly_time_axis_lands_on_the_first_of_the_month() {
    let start = civil::to_days(2024, 1, 15) as f64 * 86_400.0;
    let end = civil::to_days(2024, 12, 20) as f64 * 86_400.0;
    let l = ticks::time_ticks(Interval::new(start, end), 6);
    assert!(!l.values.is_empty());
    for v in &l.values {
        let (_, _, day) = civil::from_days((v / 86_400.0).floor() as i64);
        assert_eq!(day, 1, "tick at {v} is not the first of a month");
    }
}

#[test]
fn band_geometry_matches_the_d3_definition() {
    let spec = ScaleSpec {
        padding: Some(BandPadding {
            inner: 0.0,
            outer: 0.0,
            align: 0.5,
        }),
        ..Default::default()
    };
    let data: Vec<Datum> = ["a", "b", "c", "d"]
        .iter()
        .map(|s| Datum::Category(s.to_string()))
        .collect();
    let s = infer(&spec, &data, (0.0, 100.0), ScaleKind::Band);
    // Four bands, no padding: each is a quarter of the range, centred in its slot.
    assert!((s.step() - 25.0).abs() < 1e-9);
    assert!((s.band_width() - 25.0).abs() < 1e-9);
    assert!((s.project(&Datum::Category("a".into())).unwrap() - 12.5).abs() < 1e-9);
    assert!((s.project(&Datum::Category("d".into())).unwrap() - 87.5).abs() < 1e-9);
    // And the inverse agrees, which is what a hit test depends on.
    assert_eq!(s.category_at(12.5), Some("a"));
    assert_eq!(s.category_at(87.5), Some("d"));
}

#[test]
fn continuous_scales_invert_what_they_project() {
    for kind in [
        ScaleKind::Linear,
        ScaleKind::Log { base: 10.0 },
        ScaleKind::Power { exponent: 0.5 },
    ] {
        let s = Scale::continuous(kind.clone(), Interval::new(1.0, 1000.0), (0.0, 500.0));
        for v in [1.0, 7.5, 100.0, 999.0] {
            let p = s.project(&Datum::Number(v)).unwrap();
            let back = s.invert(p).unwrap();
            assert!(
                (back - v).abs() < 1e-6 * v.max(1.0),
                "{kind:?}: {v} projected to {p} and inverted to {back}"
            );
        }
    }
}

#[test]
fn a_log_domain_reaching_zero_is_lifted_to_a_decade() {
    let spec = ScaleSpec {
        kind: Some(ScaleKind::Log { base: 10.0 }),
        ..Default::default()
    };
    let data = vec![Datum::Number(0.0), Datum::Number(4.0), Datum::Number(900.0)];
    let s = infer(&spec, &data, (0.0, 100.0), ScaleKind::Linear);
    assert!(s.domain.lo > 0.0, "a log scale cannot start at zero");
    assert!(
        (s.domain.lo - 1.0).abs() < 1e-9,
        "expected the decade below 4"
    );
}

#[test]
fn bars_stack_in_declaration_order_and_normalize_to_the_whole() {
    let marks = vec![
        bar(value("q", "Q1"), value("v", 30.0)).by_series(value("s", "A")),
        bar(value("q", "Q1"), value("v", 70.0)).by_series(value("s", "B")),
    ];
    let r = resolved(marks.clone(), Stacking::Standard);
    assert_eq!((r[0].v0, r[0].v1), (0.0, 30.0));
    assert_eq!((r[1].v0, r[1].v1), (30.0, 100.0));

    let r = resolved(marks, Stacking::Normalized);
    assert!(
        (r[1].v1 - 1.0).abs() < 1e-9,
        "a normalized stack fills the axis"
    );
    assert!((r[0].v1 - 0.3).abs() < 1e-9);
}

#[test]
fn negative_values_stack_away_from_the_baseline() {
    let marks = vec![
        bar(value("q", "Q1"), value("v", -20.0)).by_series(value("s", "A")),
        bar(value("q", "Q1"), value("v", 50.0)).by_series(value("s", "B")),
        bar(value("q", "Q1"), value("v", -10.0)).by_series(value("s", "C")),
    ];
    let r = resolved(marks, Stacking::Standard);
    // The positive one starts at zero; the negatives accumulate downward, not against them.
    assert_eq!((r[1].v0, r[1].v1), (0.0, 50.0));
    assert_eq!((r[0].v0, r[0].v1), (0.0, -20.0));
    assert_eq!((r[2].v0, r[2].v1), (-20.0, -30.0));
}

#[test]
fn dodged_bars_share_the_band_evenly_even_where_a_series_is_missing() {
    let marks = vec![
        bar(value("q", "Q1"), value("v", 1.0)).by_position(value("s", "A")),
        bar(value("q", "Q1"), value("v", 2.0)).by_position(value("s", "B")),
        // Q2 has no B: its A bar must stay the same width as Q1's, not double.
        bar(value("q", "Q2"), value("v", 3.0)).by_position(value("s", "A")),
    ];
    let r = resolved(marks, Stacking::Unstacked);
    assert_eq!(r[0].dodge_count, 2);
    assert_eq!(
        r[2].dodge_count, 2,
        "a missing series must leave a gap, not widen its neighbour"
    );
    assert_eq!(r[2].dodge_index, 0);
}

/// Run the pipeline far enough to read the position adjustments back.
fn resolved(marks: Vec<Mark>, stacking: Stacking) -> Vec<day_piece_charts::resolve::Placed> {
    let marks: Vec<Mark> = marks.into_iter().map(|m| m.stacking(stacking)).collect();
    let x = ScaleSpec::default();
    let y = ScaleSpec::default();
    let ax = AxisSpec::default();
    let colors = |i: usize, _: &str| day_piece_charts::categorical(i);
    let cfg = day_piece_charts::resolve::Config {
        x_scale: &x,
        y_scale: &y,
        x_axis: &ax,
        y_axis: &ax,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: Default::default(),
        legend_insets: Default::default(),
        plot_insets: None,
    };
    day_piece_charts::resolve::resolve(marks, day_spec::Size::new(400.0, 300.0), &cfg).marks
}

#[test]
fn a_bar_chart_always_includes_its_baseline() {
    // Data that never approaches zero still gets a zero-based axis: a bar chart that crops its
    // baseline exaggerates every difference, which is the most common way a chart misleads.
    let marks = vec![
        bar(value("q", "A"), value("v", 980.0)),
        bar(value("q", "B"), value("v", 1000.0)),
    ];
    let x = ScaleSpec::default();
    let y = ScaleSpec::default();
    let ax = AxisSpec::default();
    let colors = |i: usize, _: &str| day_piece_charts::categorical(i);
    let cfg = day_piece_charts::resolve::Config {
        x_scale: &x,
        y_scale: &y,
        x_axis: &ax,
        y_axis: &ax,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: Default::default(),
        legend_insets: Default::default(),
        plot_insets: None,
    };
    let r = day_piece_charts::resolve::resolve(marks, day_spec::Size::new(400.0, 300.0), &cfg);
    assert!(
        r.y.domain.lo <= 0.0,
        "y domain {:?} crops the baseline",
        r.y.domain
    );
}

#[test]
fn polar_projection_is_circular_and_starts_at_twelve_oclock() {
    let rect = day_spec::Rect::new(0.0, 0.0, 200.0, 200.0);
    let c = Coordinate::polar();
    let top = c.project(rect, 0.0, 1.0);
    assert!((top.x - 100.0).abs() < 1e-9, "u=0 is not at twelve o'clock");
    assert!(top.y < 5.0, "u=0 should be at the top, got y={}", top.y);
    let quarter = c.project(rect, 0.25, 1.0);
    assert!(
        quarter.x > 195.0,
        "a quarter turn should be at three o'clock"
    );
    // A non-square pane still draws a circle rather than an ellipse.
    let wide = day_spec::Rect::new(0.0, 0.0, 400.0, 100.0);
    assert!((c.radius(wide) - 50.0).abs() < 1e-9);
}

#[test]
fn sectors_stack_by_default_so_a_pie_sweeps_a_whole_turn() {
    // Unstacked, every wedge would start at twelve o'clock and lie on top of the last one — which
    // is what a pie looked like before sectors joined the stacking default.
    let marks = vec![
        sector(value("Share", 30.0)).by_series(value("Channel", "A")),
        sector(value("Share", 50.0)).by_series(value("Channel", "B")),
        sector(value("Share", 20.0)).by_series(value("Channel", "C")),
    ];
    let x = ScaleSpec::default();
    let y = ScaleSpec::default();
    let ax = AxisSpec::default();
    let colors = |i: usize, _: &str| day_piece_charts::categorical(i);
    let cfg = day_piece_charts::resolve::Config {
        x_scale: &x,
        y_scale: &y,
        x_axis: &ax,
        y_axis: &ax,
        coordinate: Coordinate::polar(),
        series_colors: &colors,
        label_size: 11.0,
        font: Default::default(),
        legend_insets: Default::default(),
        plot_insets: None,
    };
    let r = day_piece_charts::resolve::resolve(marks, day_spec::Size::new(300.0, 300.0), &cfg);
    let bounds: Vec<(f64, f64)> = r.marks.iter().map(|p| (p.v0, p.v1)).collect();
    assert_eq!(bounds, vec![(0.0, 30.0), (30.0, 80.0), (80.0, 100.0)]);
    // And the wedges tile the turn exactly: the last one ends where the first began.
    let total: f64 = r.marks.iter().map(|p| p.v1 - p.v0).sum();
    assert_eq!(r.marks.last().unwrap().v1, total);
}

/// The pipeline with the default configuration, for the tests that read the scales back.
fn resolve_default(marks: Vec<Mark>, size: day_spec::Size) -> day_piece_charts::resolve::Resolved {
    let x = ScaleSpec::default();
    let y = ScaleSpec::default();
    let ax = AxisSpec::default();
    let colors = |i: usize, _: &str| day_piece_charts::categorical(i);
    let cfg = day_piece_charts::resolve::Config {
        x_scale: &x,
        y_scale: &y,
        x_axis: &ax,
        y_axis: &ax,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: Default::default(),
        legend_insets: Default::default(),
        plot_insets: None,
    };
    day_piece_charts::resolve::resolve(marks, size, &cfg)
}

#[test]
fn an_iso_date_is_the_start_of_its_day_and_garbage_is_dropped() {
    let epoch = date("Date", "1970-01-01");
    assert_eq!(epoch.datum, Datum::Time(0.0));
    let leap = date("Date", "2024-02-29");
    assert_eq!(
        leap.datum,
        Datum::Time(civil::to_days(2024, 2, 29) as f64 * 86_400.0)
    );
    // A trailing clock time is tolerated; anything that is not a date is a non-finite instant,
    // which the pipeline drops rather than plotting at the epoch.
    assert_eq!(
        date("Date", "2024-02-29T15:30:00Z").datum,
        date("Date", "2024-02-29").datum
    );
    assert!(!date("Date", "yesterday").datum.is_finite());
    assert!(!date("Date", "2024-13-01").datum.is_finite());
}

#[test]
fn a_categorical_y_becomes_a_band_of_rows() {
    // A heat map: one cell per (month, region), the region a CATEGORY on y. The y scale has to be
    // a band of the two rows, in first-seen order, with no phantom numeric row from the stacked
    // bounds a rect does not have.
    let marks = vec![
        rect(value("Month", "Jan"), value("Region", "North")),
        rect(value("Month", "Feb"), value("Region", "North")),
        rect(value("Month", "Jan"), value("Region", "South")),
    ];
    let r = resolve_default(marks, day_spec::Size::new(400.0, 300.0));
    assert!(
        r.y.kind.is_discrete(),
        "y should be a band, got {:?}",
        r.y.kind
    );
    assert_eq!(
        r.y.categories,
        vec!["North".to_string(), "South".to_string()]
    );
    assert_eq!(r.y_ticks.len(), 2, "one label per row: {:?}", r.y_ticks);
    assert!(r.y.band_width() > 0.0);
    // And the first row is at the TOP, the way a table reads.
    let north = r.y.project(&Datum::Category("North".into())).unwrap();
    let south = r.y.project(&Datum::Category("South".into())).unwrap();
    assert!(
        north < south,
        "North ({north}) should sit above South ({south})"
    );
}

#[test]
fn bars_on_a_continuous_axis_are_as_wide_as_their_closest_pair_allows() {
    // Daily bars on a time axis: the gap is a day, whatever the axis spans.
    let day = 86_400.0;
    let marks: Vec<Mark> = (0..10)
        .map(|i| bar(time("Date", i as f64 * day), value("Volume", 5.0)))
        .collect();
    let r = resolve_default(marks, day_spec::Size::new(400.0, 300.0));
    assert!(!r.x.kind.is_discrete());
    assert_eq!(r.x_gap, Some(day));
    // A lone bar has no pair to measure against.
    let r = resolve_default(
        vec![bar(time("Date", 0.0), value("Volume", 5.0))],
        day_spec::Size::new(400.0, 300.0),
    );
    assert_eq!(r.x_gap, None);
}

#[test]
fn a_trailing_y_axis_moves_its_margin_to_the_trailing_edge() {
    let marks = || {
        vec![
            day_piece_charts::line(value("x", 0.0), value("Price", 1000.0)),
            day_piece_charts::line(value("x", 10.0), value("Price", 1200.0)),
        ]
    };
    let size = day_spec::Size::new(400.0, 300.0);
    let leading = resolve_default(marks(), size);
    let x = ScaleSpec::default();
    let y = ScaleSpec::default();
    let ax = AxisSpec::default();
    let ay = AxisSpec {
        position: AxisPosition::Trailing,
        ..Default::default()
    };
    let colors = |i: usize, _: &str| day_piece_charts::categorical(i);
    let cfg = day_piece_charts::resolve::Config {
        x_scale: &x,
        y_scale: &y,
        x_axis: &ax,
        y_axis: &ay,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: Default::default(),
        legend_insets: Default::default(),
        plot_insets: None,
    };
    let trailing = day_piece_charts::resolve::resolve(marks(), size, &cfg);
    // The label column is the same width either way; it just changes sides.
    assert!(
        trailing.plot.origin.x < leading.plot.origin.x,
        "trailing axis left {} of margin on the leading edge, the leading axis {}",
        trailing.plot.origin.x,
        leading.plot.origin.x
    );
    let right =
        |r: &day_piece_charts::resolve::Resolved| size.width - r.plot.origin.x - r.plot.size.width;
    assert!(right(&trailing) > right(&leading));
}

#[test]
fn fixed_insets_replace_the_measured_margins() {
    let marks = vec![
        day_piece_charts::line(value("x", 0.0), value("y", 1.0)),
        day_piece_charts::line(value("x", 1.0), value("y", 2.0)),
    ];
    let x = ScaleSpec::default();
    let y = ScaleSpec::default();
    let ax = AxisSpec::default();
    let colors = |i: usize, _: &str| day_piece_charts::categorical(i);
    let cfg = day_piece_charts::resolve::Config {
        x_scale: &x,
        y_scale: &y,
        x_axis: &ax,
        y_axis: &ax,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: Default::default(),
        legend_insets: Default::default(),
        plot_insets: Some(Insets::uniform(2.0)),
    };
    let r = day_piece_charts::resolve::resolve(marks, day_spec::Size::new(100.0, 50.0), &cfg);
    assert_eq!(r.plot, day_spec::Rect::new(2.0, 2.0, 96.0, 46.0));
}

#[test]
fn a_line_chart_is_not_zero_based() {
    // A line has no baseline, so its axis is the data's own extent. Bars and areas include zero
    // (tested above); a line that did too would flatten every price chart into a ribbon at
    // the top of the plot.
    let marks = vec![
        day_piece_charts::line(value("x", 0.0), value("Price", 980.0)),
        day_piece_charts::line(value("x", 1.0), value("Price", 1000.0)),
        point(value("x", 2.0), value("Price", 990.0)),
    ];
    let r = resolve_default(marks, day_spec::Size::new(400.0, 300.0));
    assert!(
        r.y.domain.lo >= 980.0 - 1e-9,
        "y domain {:?} reaches below the data",
        r.y.domain
    );
}
