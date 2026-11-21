// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Headless tests for the marks that annotate and layer a chart: bands across the plot, hatched
//! fills, ridges on a categorical axis, points on a row, labelled rules, and how each of them
//! enters.
//!
//! Each test resolves a chart and reads either the resolved geometry or the display list the
//! renderer records, so none of it needs a toolkit.

use day_piece_charts::animate::blend;
use day_piece_charts::resolve::{Config, Resolved, resolve};
use day_piece_charts::scale::ScaleSpec;
use day_piece_charts::{self as ch, *};
use day_pieces::Draw;
use day_spec::{BlendMode, Color, DrawOp, Paint, Rect, Shape, Size};

const SIZE: Size = Size::new(400.0, 300.0);

const MARKED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

fn resolved(marks: Vec<Mark>, overlap: f64) -> Resolved {
    let (xs, ys) = (ScaleSpec::default(), ScaleSpec::default());
    let (xa, ya) = (AxisSpec::default(), AxisSpec::default());
    let colors = |i: usize, _: &str| categorical(i);
    let cfg = Config {
        x_scale: &xs,
        y_scale: &ys,
        x_axis: &xa,
        y_axis: &ya,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: day_spec::CanvasFont::default(),
        legend_insets: Insets::default(),
        plot_insets: None,
        ridge_overlap: overlap,
    };
    resolve(marks, SIZE, &cfg)
}

/// The display list `r` records, on `ground` when it has one.
fn ops(r: &Resolved, ground: Option<Color>) -> Vec<DrawOp> {
    let (xa, ya) = (AxisSpec::default(), AxisSpec::default());
    let colors = |i: usize, _: &str| categorical(i);
    let chrome = Chrome::for_dark(false);
    let paint = render::Paint2 {
        chrome: &chrome,
        label_size: 11.0,
        font: day_spec::CanvasFont::default(),
        x_axis: &xa,
        y_axis: &ya,
        series_colors: &colors,
        ground: ground.map(Into::into),
    };
    let mut d = Draw::new();
    render::draw(&mut d, r, &paint);
    d.ops().to_vec()
}

fn series(values: &[f64]) -> Vec<Mark> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| ch::line(value("Minute", i as f64), value("Flux", *v)))
        .collect()
}

#[test]
fn a_band_and_a_vertical_rule_span_the_axis_without_sizing_it() {
    // A floating domain: a line from 980 to 1000. Neither a band between two x values nor a
    // vertical rule has a y, and neither may drag the axis down to zero.
    let mut marks = series(&[980.0, 1000.0, 990.0]);
    marks.push(band_x(value("Minute", 0.5), value("Minute", 1.5)).foreground(MARKED));
    marks.push(rule_x(value("Minute", 1.0)));
    let r = resolved(marks, 1.5);
    assert!(r.y.domain.lo >= 980.0 - 1e-9, "y domain {:?}", r.y.domain);

    // The band is the plot's whole height between its two x positions.
    let band = ops(&r, None)
        .into_iter()
        .find_map(|op| match op {
            DrawOp::Fill(Shape::Rect(rect), Paint::Solid(c)) if c == MARKED.into() => Some(rect),
            _ => None,
        })
        .expect("the band is filled");
    let (x0, x1) = (
        r.x.project(&Datum::Number(0.5)).unwrap(),
        r.x.project(&Datum::Number(1.5)).unwrap(),
    );
    assert!((band.origin.x - x0).abs() < 1e-9 && (band.size.width - (x1 - x0)).abs() < 1e-9);
    assert!((band.origin.y - r.plot.origin.y).abs() < 1e-9);
    assert!((band.size.height - r.plot.size.height).abs() < 1e-9);
}

#[test]
fn a_band_between_two_y_values_spans_the_plot_sideways() {
    let mut marks = series(&[10.0, 30.0, 20.0]);
    marks.push(band_y(value("Flux", 15.0), value("Flux", 25.0)).foreground(MARKED));
    let r = resolved(marks, 1.5);
    let band = ops(&r, None)
        .into_iter()
        .find_map(|op| match op {
            DrawOp::Fill(Shape::Rect(rect), Paint::Solid(c)) if c == MARKED.into() => Some(rect),
            _ => None,
        })
        .expect("the band is filled");
    assert!((band.origin.x - r.plot.origin.x).abs() < 1e-9);
    assert!((band.size.width - r.plot.size.width).abs() < 1e-9);
    let (y_hi, y_lo) = (
        r.y.project(&Datum::Number(25.0)).unwrap(),
        r.y.project(&Datum::Number(15.0)).unwrap(),
    );
    assert!((band.origin.y - y_hi).abs() < 1e-9 && (band.size.height - (y_lo - y_hi)).abs() < 1e-9);
}

#[test]
fn a_hatched_band_is_lines_clipped_to_it_and_no_fill() {
    let mut marks = series(&[10.0, 30.0, 20.0]);
    marks.push(
        band_x(value("Minute", 0.5), value("Minute", 1.5))
            .foreground(MARKED)
            .hatched(6.0),
    );
    let r = resolved(marks, 1.5);
    let recorded = ops(&r, None);
    assert!(
        !recorded
            .iter()
            .any(|op| matches!(op, DrawOp::Fill(_, Paint::Solid(c)) if *c == MARKED.into())),
        "a hatched mark has no solid fill"
    );
    let clip = recorded
        .iter()
        .position(|op| matches!(op, DrawOp::Clip(Shape::Rect(_))))
        .expect("the hatching is clipped to the band");
    let DrawOp::Clip(Shape::Rect(band)) = &recorded[clip] else {
        unreachable!()
    };
    let DrawOp::Stroke(Shape::Path(path), Paint::Solid(color), _) = &recorded[clip + 1] else {
        panic!(
            "the hatching follows its clip as one path: {:?}",
            recorded[clip + 1]
        );
    };
    assert_eq!(*color, MARKED.into());
    // One move and one line per hatch line, enough of them to cross the band at this spacing.
    let lines = path.segs.len() / 2;
    let expected = (band.size.width + band.size.height) / (6.0 * std::f64::consts::SQRT_2);
    assert!(
        (lines as f64 - expected).abs() <= 2.0,
        "{lines} lines for a band needing about {expected}"
    );
}

fn ridge_marks() -> Vec<Mark> {
    let mut marks = Vec::new();
    for (row, peak) in [("O", 40.0), ("G", 100.0), ("M", 60.0)] {
        for (i, share) in [0.2, 1.0, 0.4].iter().enumerate() {
            marks.push(
                area(value("Wavelength", i as f64), value("Class", row))
                    .ridge(value("Flux", peak * share)),
            );
        }
    }
    marks
}

/// Which row each `Blend(Erase)` in `recorded` belongs to, read from the baseline of the shape
/// filled right after it.
fn erased_baselines(recorded: &[DrawOp]) -> Vec<f64> {
    recorded
        .windows(2)
        .filter_map(|pair| match pair {
            [DrawOp::Blend(BlendMode::Erase), DrawOp::Fill(shape, _)] => {
                let b = shape.bounds();
                Some(b.origin.y + b.size.height)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn ridges_stand_on_evenly_spaced_rows_and_the_tallest_spans_the_overlap() {
    let overlap = 2.0;
    let r = resolved(ridge_marks(), overlap);
    let base = |row: &str| r.y.project(&Datum::Category(row.to_string())).unwrap();
    let step = base("G") - base("O");
    assert!(step > 0.0 && ((base("M") - base("G")) - step).abs() < 1e-9);
    // The last row stands on the plot's floor, and the first leaves room for a full ridge.
    let (top, bottom) = (r.plot.origin.y, r.plot.origin.y + r.plot.size.height);
    assert!(
        (base("M") - bottom).abs() < 1e-9,
        "{} vs {bottom}",
        base("M")
    );
    assert!(((base("O") - top) - overlap * step).abs() < 1e-9);
    // The tallest value in the chart (100) is `overlap` rows high.
    assert!((r.ridge_unit * 100.0 - overlap * step).abs() < 1e-9);
    // The rows are the y axis; the heights never reach a scale.
    assert_eq!(r.y.categories, ["O", "G", "M"]);
    // Each ridge is its own series along its row: one hidden shape per row, back to front.
    let baselines = erased_baselines(&ops(&r, None));
    assert_eq!(baselines.len(), 3);
    for (got, row) in baselines.iter().zip(["O", "G", "M"]) {
        assert!(
            (got - base(row)).abs() < 1e-6,
            "{row}: {got} vs {}",
            base(row)
        );
    }
}

#[test]
fn a_ridge_erases_what_is_behind_it_or_paints_the_ground_it_is_given() {
    let r = resolved(ridge_marks(), 1.5);
    let live = ops(&r, None);
    assert_eq!(
        live.iter()
            .filter(|op| matches!(op, DrawOp::Blend(BlendMode::Erase)))
            .count(),
        3
    );
    // On a ground of its own (an export's white sheet) nothing is erased: each ridge paints
    // the ground and then itself.
    let still = ops(&r, Some(Color::WHITE));
    assert!(!still.iter().any(|op| matches!(op, DrawOp::Blend(_))));
    assert_eq!(
        still
            .iter()
            .filter(
                |op| matches!(op, DrawOp::Fill(_, Paint::Solid(c)) if *c == Color::WHITE.into())
            )
            .count(),
        3
    );
}

#[test]
fn a_ridge_is_selectable_at_its_crest_and_named_by_its_row() {
    let r = resolved(ridge_marks(), 1.5);
    let (xa, ya) = (AxisSpec::default(), AxisSpec::default());
    let colors = |i: usize, _: &str| categorical(i);
    let chrome = Chrome::for_dark(false);
    let paint = render::Paint2 {
        chrome: &chrome,
        label_size: 11.0,
        font: day_spec::CanvasFont::default(),
        x_axis: &xa,
        y_axis: &ya,
        series_colors: &colors,
        ground: None,
    };
    let hits = render::hit_model(&r, &paint);
    let peak = hits
        .marks
        .iter()
        .find(|m| m.series == "G" && (m.value - 100.0).abs() < 1e-9)
        .expect("the tallest sample");
    let base = r.y.project(&Datum::Category("G".to_string())).unwrap();
    assert!((peak.at.y - (base - 100.0 * r.ridge_unit)).abs() < 1e-9);
    assert_eq!(peak.y_label, "100");
}

#[test]
fn a_point_on_a_categorical_y_sits_on_its_row() {
    let r = resolved(
        vec![
            point(value("Throughput", 60.0), value("Filter", "g")).foreground(MARKED),
            point(value("Throughput", 80.0), value("Filter", "r")),
        ],
        1.5,
    );
    let row = r.y.project(&Datum::Category("g".to_string())).unwrap();
    let at = ops(&r, None)
        .into_iter()
        .find_map(|op| match op {
            DrawOp::Stamp(stamp) if stamp.paint == Paint::Solid(MARKED.into()) => {
                stamp.at.first().copied()
            }
            _ => None,
        })
        .expect("the point is stamped");
    assert!((at.y - row).abs() < 1e-9, "{} vs {row}", at.y);
    assert!((at.x - r.x.project(&Datum::Number(60.0)).unwrap()).abs() < 1e-9);
}

/// The capsule a pill annotation fills, and the text drawn on it.
fn pill(recorded: &[DrawOp]) -> Option<(Rect, String)> {
    let shadow = recorded
        .iter()
        .position(|op| matches!(op, DrawOp::Shadow(Some(_))))?;
    let DrawOp::Fill(Shape::RoundedRect(capsule, radius), _) = &recorded[shadow + 1] else {
        return None;
    };
    assert!(
        (radius - capsule.size.height / 2.0).abs() < 1e-9,
        "a capsule"
    );
    let text = recorded[shadow..].iter().find_map(|op| match op {
        DrawOp::Text { text, .. } => Some(text.clone()),
        _ => None,
    })?;
    Some((*capsule, text))
}

#[test]
fn a_rule_carries_its_label_in_a_pill_at_its_trailing_end() {
    let mut marks = series(&[10.0, 30.0, 20.0]);
    marks.push(
        rule_y(value("Flux", 20.0))
            .annotation(AnnotationPosition::Trailing, "avg 20")
            .annotation_pill(),
    );
    let r = resolved(marks, 1.5);
    let (capsule, text) = pill(&ops(&r, None)).expect("a pill is drawn");
    assert_eq!(text, "avg 20");
    let y = r.y.project(&Datum::Number(20.0)).unwrap();
    let right = r.plot.origin.x + r.plot.size.width;
    assert!(
        (capsule.origin.y + capsule.size.height / 2.0 - y).abs() < 1e-9,
        "centered on the rule"
    );
    assert!(
        capsule.origin.x + capsule.size.width <= right,
        "inside the plot"
    );
    assert!(
        capsule.origin.x + capsule.size.width > right - 6.0,
        "at its trailing end"
    );
}

#[test]
fn a_band_is_named_at_its_top_and_keeps_its_name_inside_the_plot() {
    let mut marks = series(&[10.0, 30.0, 20.0]);
    // A band at the very start of the axis: its centered label would hang off the plot.
    marks.push(
        band_x(value("Minute", 0.0), value("Minute", 0.1))
            .hatched(6.0)
            .annotation(AnnotationPosition::Top, "cloud passing")
            .annotation_pill(),
    );
    let r = resolved(marks, 1.5);
    let (capsule, text) = pill(&ops(&r, None)).expect("a pill is drawn");
    assert_eq!(text, "cloud passing");
    assert!(capsule.origin.x >= r.plot.origin.x - 1e-9);
    assert!(capsule.origin.y >= r.plot.origin.y);
    assert!(
        capsule.origin.y < r.plot.origin.y + 8.0,
        "at the top of the plot"
    );
}

fn dumbbell(before: f64, after: f64) -> Vec<Mark> {
    vec![
        rule_y(value("Filter", "g")).x_range(value("Before", before), value("After", after)),
        point(value("Before", before), value("Filter", "g")).key("before"),
        point(value("After", after), value("Filter", "g")).key("after"),
    ]
}

#[test]
fn a_range_is_drawn_out_from_its_start_and_its_end_point_rides_it() {
    let to = resolved(dumbbell(40.0, 90.0), 1.5);
    let mut from = to.clone();
    from.marks.clear();
    let x_of = |r: &Resolved, i: usize| r.marks[i].mark.x.as_ref().unwrap().datum.as_continuous();
    let end_of = |r: &Resolved, i: usize| {
        r.marks[i]
            .mark
            .x_end
            .as_ref()
            .unwrap()
            .datum
            .as_continuous()
    };

    // At the start the range is collapsed onto its near end, and both points are there.
    let start = blend(&from, &to, 0.0);
    assert_eq!(end_of(&start, 0), Some(40.0));
    assert_eq!(x_of(&start, 1), Some(40.0));
    assert_eq!(
        x_of(&start, 2),
        Some(40.0),
        "the far point starts on the near end"
    );
    assert_eq!(
        start.marks[0].mark.style.opacity, 1.0,
        "a range does not fade"
    );

    // Halfway, the range has reached halfway and its end point is on its tip.
    let mid = blend(&from, &to, 0.5);
    assert_eq!(end_of(&mid, 0), Some(65.0));
    assert_eq!(x_of(&mid, 2), Some(65.0));
    assert_eq!(x_of(&mid, 1), Some(40.0), "the near point stays");

    let end = blend(&from, &to, 1.0);
    assert_eq!(end.marks, to.marks);
}

#[test]
fn a_rule_across_the_plot_fades_in() {
    let mut marks = series(&[10.0, 30.0]);
    marks.push(rule_y(value("Flux", 20.0)));
    let to = resolved(marks, 1.5);
    let mut from = to.clone();
    from.marks.clear();
    let rule = |r: &Resolved| {
        r.marks
            .iter()
            .find(|p| p.mark.kind == MarkKind::Rule)
            .unwrap()
            .mark
            .style
            .opacity
    };
    assert_eq!(rule(&blend(&from, &to, 0.0)), 0.0);
    assert_eq!(rule(&blend(&from, &to, 1.0)), 1.0);
}

#[test]
fn a_sweep_uncovers_the_marks_from_the_leading_edge() {
    let to = resolved(series(&[10.0, 30.0, 20.0]), 1.5);
    assert_eq!(to.reveal, 1.0);
    let mut from = to.clone();
    from.reveal = 0.0;
    let mid = blend(&from, &to, 0.5);
    assert_eq!(mid.reveal, 0.5);
    assert_eq!(mid.marks, to.marks, "nothing moves; the chart is uncovered");
    // Half revealed, the marks are clipped to the leading half of the plot (and its overhang).
    let reveal_clip = |r: &Resolved| {
        ops(r, None).into_iter().find_map(|op| match op {
            DrawOp::Clip(Shape::Rect(rect)) if rect.size.height > 1.0e4 => Some(rect),
            _ => None,
        })
    };
    let clip = reveal_clip(&mid).expect("a clip while sweeping");
    let covered = to.plot.size.width + 2.0 * 8.0;
    assert!((clip.size.width - covered / 2.0).abs() < 1e-9);
    assert!(clip.origin.x < to.plot.origin.x);
    assert!(
        reveal_clip(&to).is_none(),
        "the finished chart is not clipped"
    );
    // A spring's overshoot never reveals more than the whole chart.
    assert_eq!(blend(&from, &to, 1.2).reveal, 1.0);
}

#[test]
fn a_ridge_rises_from_its_own_row() {
    let to = resolved(ridge_marks(), 1.5);
    let mut from = to.clone();
    from.marks.clear();
    let start = blend(&from, &to, 0.0);
    assert!(start.marks.iter().all(|p| p.v1 == 0.0), "flat at the start");
    let mid = blend(&from, &to, 0.5);
    for (got, want) in mid.marks.iter().zip(&to.marks) {
        assert!((got.v1 - want.v1 / 2.0).abs() < 1e-9);
        assert_eq!(got.y_band, want.y_band, "a ridge stays on its row");
    }
}

#[test]
fn a_rounded_out_domain_does_not_clip_and_a_pinned_one_does() {
    // Bars from zero: the pipeline rounds the y domain out to nice bounds. That domain holds
    // the data by construction, so it must not count as one the app pinned.
    let bars = vec![
        bar(value("Month", "Jan"), value("Sales", 37.0)),
        bar(value("Month", "Feb"), value("Sales", 52.0)),
    ];
    let r = resolved(bars, 1.5);
    assert!(r.y.domain.hi > 52.0, "rounded out: {:?}", r.y.domain);
    assert!(!r.y.explicit_domain && !r.x.explicit_domain);
    assert!(
        !ops(&r, None).iter().any(|op| matches!(op, DrawOp::Clip(_))),
        "nothing is clipped"
    );
}

#[test]
fn a_points_label_clears_its_symbol() {
    let labelled = |area: f64| {
        let r = resolved(
            vec![
                point(value("Throughput", 60.0), value("Filter", "g"))
                    .symbol_size(area)
                    .annotation(AnnotationPosition::Trailing, "+14"),
                point(value("Throughput", 90.0), value("Filter", "r")),
            ],
            1.5,
        );
        let x = r.x.project(&Datum::Number(60.0)).unwrap();
        let text = ops(&r, None)
            .into_iter()
            .find_map(|op| match op {
                DrawOp::Text { text, at, .. } if text == "+14" => Some(at.x),
                _ => None,
            })
            .expect("the label is drawn");
        text - x
    };
    let radius = |area: f64| (area / std::f64::consts::PI).sqrt();
    let (small, large) = (labelled(20.0), labelled(200.0));
    assert!(
        ((large - small) - (radius(200.0) - radius(20.0))).abs() < 1e-9,
        "a larger symbol pushes its label out by the difference in radius"
    );
}

#[test]
fn a_category_its_axis_left_blank_is_still_named_when_selected() {
    let (xs, ys) = (ScaleSpec::default(), ScaleSpec::default());
    let ya = AxisSpec::default();
    // An axis that labels only the even weeks.
    let xa = AxisSpec {
        format: Some(std::rc::Rc::new(|d: &Datum| match d {
            Datum::Category(week) if week.parse::<u32>().is_ok_and(|n| n % 2 == 0) => week.clone(),
            _ => String::new(),
        })),
        ..AxisSpec::default()
    };
    let colors = |i: usize, _: &str| categorical(i);
    let cfg = Config {
        x_scale: &xs,
        y_scale: &ys,
        x_axis: &xa,
        y_axis: &ya,
        coordinate: Coordinate::Cartesian,
        series_colors: &colors,
        label_size: 11.0,
        font: day_spec::CanvasFont::default(),
        legend_insets: Insets::default(),
        plot_insets: None,
        ridge_overlap: 1.5,
    };
    let r = resolve(
        vec![
            rect(value("Week", "14"), value("Day", "Mon")),
            rect(value("Week", "15"), value("Day", "Mon")),
        ],
        SIZE,
        &cfg,
    );
    let chrome = Chrome::for_dark(false);
    let paint = render::Paint2 {
        chrome: &chrome,
        label_size: 11.0,
        font: day_spec::CanvasFont::default(),
        x_axis: &xa,
        y_axis: &ya,
        series_colors: &colors,
        ground: None,
    };
    let hits = render::hit_model(&r, &paint);
    let labels: Vec<&str> = hits.marks.iter().map(|m| m.x_label.as_str()).collect();
    assert_eq!(labels, ["14", "15"]);
}
