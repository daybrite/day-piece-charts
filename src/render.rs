// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Drawing a [`Resolved`] chart onto a `Draw` display list.
//!
//! Every mark is positioned in **unit plot space** and handed to the [`Coordinate`] to be placed,
//! so there is one geometry path rather than one per coordinate system. A bar in polar space comes
//! out as a wedge because that is what a bar IS in polar space — the renderer does not special-case
//! it, and nothing here knows the words "pie chart".

use day_geometry::Affine;
use day_pieces::{Draw, PathBuilder, TextStyle};
use day_spec::props::TextAlign;
use day_spec::{
    Color, FillRule, LinearGradient, Paint, Point, Rect, Shape, StrokeStyle, TextAnchor, TextVAlign,
};

use crate::axis::{AxisPosition, AxisSpec};
use crate::coord::Coordinate;
use crate::data::Datum;
use crate::mark::{Interpolation, MarkKind, Symbol};
use crate::resolve::{Placed, Resolved};
use crate::style::Chrome;

/// How the chart's chrome is drawn, gathered so `draw` takes one argument.
pub struct Paint2<'a> {
    pub chrome: &'a Chrome,
    pub label_size: f64,
    pub font: day_spec::CanvasFont,
    pub x_axis: &'a AxisSpec,
    pub y_axis: &'a AxisSpec,
    pub series_colors: &'a dyn Fn(usize, &str) -> Color,
}

/// Normalized position of a device x within the plot.
fn u_of(plot: Rect, device_x: f64) -> f64 {
    if plot.size.width <= 0.0 {
        return 0.0;
    }
    (device_x - plot.origin.x) / plot.size.width
}

/// Normalized position of a device y within the plot, measured UPWARD.
fn v_of(plot: Rect, device_y: f64) -> f64 {
    if plot.size.height <= 0.0 {
        return 0.0;
    }
    1.0 - (device_y - plot.origin.y) / plot.size.height
}

/// The color a placed mark draws in.
fn color_of(p: &Placed, r: &Resolved, paint: &Paint2<'_>) -> Color {
    let base = p.mark.style.fill.unwrap_or_else(|| {
        let name = r
            .series
            .get(p.series_index)
            .map(|s| s.as_str())
            .unwrap_or("");
        (paint.series_colors)(p.series_index, name)
    });
    if p.mark.style.opacity >= 1.0 {
        base
    } else {
        base.with_alpha(base.a * p.mark.style.opacity)
    }
}

/// The full band one x position owns, in device points.
///
/// A discrete scale says so itself. A continuous one has no bands, so a bar there takes the
/// smallest gap between any two bars (`Resolved::x_gap`) less the scale's inner padding — which
/// is what lets a year of daily volume draw as 250 bars that touch nothing, where a fixed share
/// of the axis would have stacked them on top of one another. With a single bar there is no gap
/// to measure, and it gets a twenty-fourth of the axis.
fn full_band(r: &Resolved) -> f64 {
    let x = &r.x;
    if x.kind.is_discrete() {
        return x.band_width();
    }
    let extent = (x.range.1 - x.range.0).abs();
    let Some(gap) = r.x_gap else {
        return extent / 24.0;
    };
    // Measured at both ends of the domain: on a log or power axis equal data gaps are not
    // equal device gaps, and the narrower end is the one that must not overlap.
    let device_gap = |from: f64| match (
        x.project(&Datum::Number(from)),
        x.project(&Datum::Number(from + gap)),
    ) {
        (Some(a), Some(b)) => (b - a).abs(),
        _ => extent / 24.0,
    };
    let narrowest = device_gap(x.domain.lo).min(device_gap(x.domain.hi - gap));
    (narrowest * (1.0 - x.padding.inner)).min(extent)
}

/// The band a mark occupies along x, in device points, narrowed by its dodging slot.
fn band_of(p: &Placed, r: &Resolved) -> f64 {
    let slot = full_band(r) / p.dodge_count.max(1) as f64;
    p.mark.width.resolve(slot)
}

/// The center of a mark along x, in device points, offset into its dodging slot.
fn center_x(p: &Placed, r: &Resolved) -> Option<f64> {
    let base = r.x.project(&p.mark.x.as_ref()?.datum)?;
    if p.dodge_count <= 1 {
        return Some(base);
    }
    let full = full_band(r);
    let slot = full / p.dodge_count as f64;
    Some(base - full / 2.0 + slot * (p.dodge_index as f64 + 0.5))
}

/// The paint a filled mark draws with: its gradient when it has one, else its color. Opacity
/// applies to both stops, so a faded gradient fades evenly.
fn fill_paint(p: &Placed, color: Color) -> Paint {
    match p.mark.style.gradient {
        Some((top, bottom)) => {
            let o = p.mark.style.opacity.clamp(0.0, 1.0);
            LinearGradient::vertical(top.with_alpha(top.a * o), bottom.with_alpha(bottom.a * o))
                .into()
        }
        None => color.into(),
    }
}

/// Draw the whole chart.
pub fn draw(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>) {
    if r.plot.size.width <= 1.0 || r.plot.size.height <= 1.0 {
        return;
    }
    if let Some(bg) = paint.chrome.plot_background {
        d.fill(Shape::Rect(r.plot), bg);
    }
    if !r.coordinate.is_polar() {
        grid(d, r, paint);
    }
    // Marks draw in z order, and a stable sort keeps declaration order inside a z — the contract
    // stacking already relies on.
    let mut order: Vec<usize> = (0..r.marks.len()).collect();
    order.sort_by(|a, b| {
        r.marks[*a]
            .mark
            .z
            .partial_cmp(&r.marks[*b].mark.z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Line and area marks are CONNECTED marks: they describe a series, not a datum, so they are
    // gathered by series and drawn as one path. Every other kind draws per mark.
    let mut connected: Vec<(MarkKind, usize)> = Vec::new();
    for i in &order {
        let p = &r.marks[*i];
        if matches!(p.mark.kind, MarkKind::Line | MarkKind::Area) {
            let key = (p.mark.kind, p.series_index);
            if !connected.contains(&key) {
                connected.push(key);
            }
        }
    }
    // A pinned domain is the one case a mark can lie outside the plot — the app said where the
    // axis ends, and the data did not agree. Clip to the plot along each pinned axis only, so a
    // fat point at the last sample of an inferred axis keeps its far half.
    let clip = r.x.explicit_domain || r.y.explicit_domain;
    if clip {
        let far = 1.0e5;
        let plot = r.plot;
        let (cx, cw) = if r.x.explicit_domain {
            (plot.origin.x, plot.size.width)
        } else {
            (plot.origin.x - far, plot.size.width + 2.0 * far)
        };
        let (cy, chh) = if r.y.explicit_domain {
            (plot.origin.y, plot.size.height)
        } else {
            (plot.origin.y - far, plot.size.height + 2.0 * far)
        };
        d.save();
        d.clip(Shape::Rect(Rect::new(cx, cy, cw, chh)));
    }
    // Areas first so lines and points read on top of them.
    for (kind, si) in connected.iter().filter(|(k, _)| *k == MarkKind::Area) {
        connected_path(d, r, paint, *kind, *si);
    }
    for i in &order {
        let p = &r.marks[*i];
        match p.mark.kind {
            MarkKind::Bar | MarkKind::Rectangle => bar_or_rect(d, r, paint, p),
            MarkKind::Rule => rule(d, r, paint, p),
            MarkKind::Sector => sector(d, r, paint, p),
            MarkKind::Line | MarkKind::Area => {}
            MarkKind::Point => {}
        }
    }
    for (kind, si) in connected.iter().filter(|(k, _)| *k == MarkKind::Line) {
        connected_path(d, r, paint, *kind, *si);
    }
    // Point marks are BATCHED: every datum sharing a symbol, a size, a color and a stroke width
    // is one stamp rather than one op each (docs/canvas.md "Stamping"). A scatter is the case the
    // batched op exists for — fifty thousand points drawn individually are fifty thousand ops to
    // build, compare and clone on every frame that re-records, and a scatter re-records whenever
    // any control moves. Grouped, it is one op per distinct appearance, which for a chart is one
    // per series.
    //
    // Insertion order is preserved so the z-order `order` established still holds between groups;
    // within a group every mark looks identical, so their relative order cannot be seen.
    let mut groups: Vec<(PointLook, Vec<Point>)> = Vec::new();
    for i in &order {
        let p = &r.marks[*i];
        if p.mark.kind == MarkKind::Point
            && let Some((look, at)) = point_mark(r, paint, p)
        {
            match groups.iter_mut().find(|(l, _)| *l == look) {
                Some((_, pts)) => pts.push(at),
                None => groups.push((look, vec![at])),
            }
        }
    }
    for (look, at) in groups {
        draw_symbols(d, look.symbol, at, look.radius, look.color, look.width);
    }
    if clip {
        d.restore();
    }
    for i in &order {
        annotation(d, r, paint, &r.marks[*i]);
    }
    if !r.coordinate.is_polar() {
        axes(d, r, paint);
    }
}

// ---------------------------------------------------------------------------
// Guides
// ---------------------------------------------------------------------------

fn grid(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>) {
    let plot = r.plot;
    if paint.y_axis.grid && !paint.y_axis.hidden {
        for t in &r.y_ticks {
            if let Some(y) = r.y.project(&Datum::Number(t.value)) {
                d.stroke(
                    Shape::Line(
                        Point::new(plot.origin.x, y),
                        Point::new(plot.origin.x + plot.size.width, y),
                    ),
                    paint.chrome.grid_line,
                    1.0,
                );
            }
        }
    }
    if paint.x_axis.grid && !paint.x_axis.hidden && !r.x.kind.is_discrete() {
        for t in &r.x_ticks {
            if let Some(x) = r.x.project(&Datum::Number(t.value)) {
                d.stroke(
                    Shape::Line(
                        Point::new(x, plot.origin.y),
                        Point::new(x, plot.origin.y + plot.size.height),
                    ),
                    paint.chrome.grid_line,
                    1.0,
                );
            }
        }
    }
}

fn axes(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>) {
    let plot = r.plot;
    // One measurement for the whole axis: these are FONT metrics, the same for every label, so
    // nothing below measures per tick (docs/fonts.md).
    let m = day_core::measure_text("0", paint.label_size, &paint.font);
    let line_h = m.height;
    // How far a tick label's line box must shift up so its CAP box straddles the gridline instead.
    // An axis label is digits, which use none of the descender room the line box reserves, so
    // centering by the line box sits every label visibly low — half the difference between the
    // descent and nothing. The cap middle sits `ascent - cap/2` below the line box top, and the
    // line middle at `height/2`; the gap between them is the correction.
    let cap_lift = m.height / 2.0 - (m.ascent - m.cap_height / 2.0);
    let label = |color: Color, anchor: TextAnchor| TextStyle {
        size: paint.label_size,
        color,
        anchor,
        font: paint.font.clone(),
    };

    // --- x ---
    if !paint.x_axis.hidden {
        // `away` points out of the plot: down from a bottom axis, up from a top one, so every
        // tick, label and title below is placed by one rule.
        let top = paint.x_axis.position == AxisPosition::Top;
        let (y, away) = if top {
            (plot.origin.y, -1.0)
        } else {
            (plot.origin.y + plot.size.height, 1.0)
        };
        let v_anchor = if top {
            TextVAlign::Bottom
        } else {
            TextVAlign::Top
        };
        d.stroke(
            Shape::Line(
                Point::new(plot.origin.x, y),
                Point::new(plot.origin.x + plot.size.width, y),
            ),
            paint.chrome.axis_line,
            1.0,
        );
        for (i, t) in r.x_ticks.iter().enumerate() {
            let x = if r.x.kind.is_discrete() {
                r.x.project(&Datum::Category(t.label.clone())).or_else(|| {
                    r.x.categories
                        .get(i)
                        .and_then(|c| r.x.project(&Datum::Category(c.clone())))
                })
            } else {
                r.x.project(&Datum::Number(t.value))
            };
            let Some(x) = x else { continue };
            if paint.x_axis.ticks {
                d.stroke(
                    Shape::Line(Point::new(x, y), Point::new(x, y + 4.0 * away)),
                    paint.chrome.tick,
                    1.0,
                );
            }
            if paint.x_axis.labels {
                d.text(
                    &t.label,
                    Point::new(x, y + 6.0 * away),
                    label(
                        paint.chrome.label,
                        TextAnchor {
                            h: TextAlign::Center,
                            v: v_anchor,
                        },
                    ),
                );
            }
        }
        if let Some(title) = axis_title(paint.x_axis, r.x_title.as_deref()) {
            d.text(
                title,
                Point::new(
                    plot.origin.x + plot.size.width / 2.0,
                    y + (10.0 + line_h * 1.4) * away,
                ),
                label(
                    paint.chrome.title,
                    TextAnchor {
                        h: TextAlign::Center,
                        v: v_anchor,
                    },
                ),
            );
        }
    }

    // --- y ---
    if !paint.y_axis.hidden {
        // The same rule sideways: `away` points out of the plot, and the labels hang toward it.
        let trailing = paint.y_axis.position == AxisPosition::Trailing;
        let (x, away) = if trailing {
            (plot.origin.x + plot.size.width, 1.0)
        } else {
            (plot.origin.x, -1.0)
        };
        let h_anchor = if trailing {
            TextAlign::Leading
        } else {
            TextAlign::Trailing
        };
        d.stroke(
            Shape::Line(
                Point::new(x, plot.origin.y),
                Point::new(x, plot.origin.y + plot.size.height),
            ),
            paint.chrome.axis_line,
            1.0,
        );
        for (i, t) in r.y_ticks.iter().enumerate() {
            let y = if r.y.kind.is_discrete() {
                r.y.categories
                    .get(i)
                    .and_then(|c| r.y.project(&Datum::Category(c.clone())))
            } else {
                r.y.project(&Datum::Number(t.value))
            };
            let Some(y) = y else { continue };
            if paint.y_axis.ticks {
                d.stroke(
                    Shape::Line(Point::new(x + 4.0 * away, y), Point::new(x, y)),
                    paint.chrome.tick,
                    1.0,
                );
            }
            if paint.y_axis.labels {
                // Aligned against the axis and centered on the tick. The anchor says that
                // outright, so nothing here has to measure the label first — the backend already
                // holds the width it is about to draw with (docs/canvas.md "Text"). The lift is
                // what turns "centered line box" into "centered digits".
                d.text(
                    &t.label,
                    Point::new(x + 8.0 * away, y - cap_lift),
                    label(
                        paint.chrome.label,
                        TextAnchor {
                            h: h_anchor,
                            v: TextVAlign::Middle,
                        },
                    ),
                );
            }
        }
        if let Some(title) = axis_title(paint.y_axis, r.y_title.as_deref()) {
            // Rotated a quarter turn, which is the only way a y title fits a narrow margin. The
            // rotation is about the text's own anchor, so the transform is translate-rotate.
            let cx = x + (widest(&r.y_ticks, paint) + 14.0 + line_h / 2.0) * away;
            let cy = plot.origin.y + plot.size.height / 2.0;
            d.transformed(
                Affine::rotate(-std::f64::consts::FRAC_PI_2).then(Affine::translate(cx, cy)),
                |d| {
                    d.text(
                        title,
                        Point::new(0.0, 0.0),
                        TextStyle {
                            size: paint.label_size,
                            color: paint.chrome.title,
                            anchor: TextAnchor::CENTERED,
                            font: paint.font.clone(),
                        },
                    );
                },
            );
        }
    }
}

fn widest(ticks: &[crate::ticks::Tick], paint: &Paint2<'_>) -> f64 {
    ticks
        .iter()
        .map(|t| day_core::measure_text(&t.label, paint.label_size, &paint.font).width)
        .fold(0.0f64, f64::max)
}

fn axis_title<'a>(spec: &'a AxisSpec, fallback: Option<&'a str>) -> Option<&'a str> {
    spec.title.as_deref().or(fallback).filter(|t| !t.is_empty())
}

// ---------------------------------------------------------------------------
// Marks
// ---------------------------------------------------------------------------

fn bar_or_rect(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    // In polar space a bar is a wedge — the grammar's own answer, not a special case.
    if r.coordinate.is_polar() {
        sector(d, r, paint, p);
        return;
    }
    let Some(cx) = center_x(p, r) else { return };
    let (top, bottom) = if r.y.kind.is_discrete() {
        // A categorical y — a heat map's rows: the cell is centered on its band, as tall as the
        // band less the mark's own height dimension.
        let Some(cy) = p.mark.y.as_ref().and_then(|v| r.y.project(&v.datum)) else {
            return;
        };
        let h = p.mark.height.resolve(r.y.band_width());
        (cy - h / 2.0, cy + h / 2.0)
    } else {
        let (Some(y0), Some(y1)) = (
            r.y.project(&Datum::Number(p.v0)),
            r.y.project(&Datum::Number(p.v1)),
        ) else {
            return;
        };
        // Clamped to the plot: a bar's baseline is zero, and zero has no position on a log axis
        // — it projects to an enormous negative and the bar runs off the pane. Clamping puts the
        // baseline on the axis floor, which is what a bar on a log scale actually means.
        let (y0, y1) = (r.y.clamp_to_range(y0), r.y.clamp_to_range(y1));
        (y0.min(y1), y0.max(y1))
    };
    // An explicit x span — a histogram bin, a Gantt bar — is the rectangle's own edges; without
    // one the mark is centered on its position and as wide as its band.
    let (left, right) = match (&p.mark.x, &p.mark.x_end) {
        (Some(a), Some(b)) => match (r.x.project(&a.datum), r.x.project(&b.datum)) {
            (Some(a), Some(b)) => (a.min(b), a.max(b)),
            _ => return,
        },
        _ => {
            let w = band_of(p, r);
            (cx - w / 2.0, cx + w / 2.0)
        }
    };
    let rect = Rect::new(
        left + p.mark.offset.0,
        top + p.mark.offset.1,
        (right - left).max(0.0),
        (bottom - top).max(0.0),
    );
    let paint = fill_paint(p, color_of(p, r, paint));
    let radius = p
        .mark
        .style
        .corner_radius
        // A radius wider than half the bar is not a rounded bar, it is a lozenge; clamp so a
        // generous radius degrades instead of inverting the shape.
        .min(rect.size.width / 2.0)
        .min(rect.size.height / 2.0);
    if radius > 0.0 {
        d.fill(Shape::RoundedRect(rect, radius), paint);
    } else {
        d.fill(Shape::Rect(rect), paint);
    }
}

fn rule(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    let color = color_of(p, r, paint);
    let style = stroke_style(p);
    let plot = r.plot;
    // A rule is vertical when it has an x and no x span, horizontal when it has a y and no y
    // span: `rule_y(v).x_range(a, b)` carries all three of x, x_end and y, and the span is what
    // says which way it runs.
    let vertical = p.mark.x.is_some() && p.mark.x_end.is_none();
    if let (true, Some(v)) = (vertical, &p.mark.x) {
        let Some(x) = r.x.project(&v.datum) else {
            return;
        };
        let (y0, y1) = match (&p.mark.y, &p.mark.y_end) {
            (Some(a), Some(b)) => (
                r.y.project(&a.datum).unwrap_or(plot.origin.y),
                r.y.project(&b.datum)
                    .unwrap_or(plot.origin.y + plot.size.height),
            ),
            _ => (plot.origin.y, plot.origin.y + plot.size.height),
        };
        d.stroke_styled(
            Shape::Line(Point::new(x, y0), Point::new(x, y1)),
            Paint::Solid(color),
            style,
        );
    } else if let Some(v) = &p.mark.y {
        let Some(y) = r.y.project(&v.datum) else {
            return;
        };
        let (x0, x1) = match (&p.mark.x, &p.mark.x_end) {
            (Some(a), Some(b)) => (
                r.x.project(&a.datum).unwrap_or(plot.origin.x),
                r.x.project(&b.datum)
                    .unwrap_or(plot.origin.x + plot.size.width),
            ),
            _ => (plot.origin.x, plot.origin.x + plot.size.width),
        };
        d.stroke_styled(
            Shape::Line(Point::new(x0, y), Point::new(x1, y)),
            Paint::Solid(color),
            style,
        );
    }
}

fn stroke_style(p: &Placed) -> StrokeStyle {
    let mut s = StrokeStyle::width(p.mark.style.line_width);
    s.cap = p.mark.style.line_cap;
    s.join = p.mark.style.line_join;
    if !p.mark.style.dash.is_empty() {
        s.dash = p.mark.style.dash.clone();
    }
    s
}

/// The points of one connected series, in x order, as device coordinates.
fn series_points(r: &Resolved, kind: MarkKind, series_index: usize) -> Vec<(f64, f64, f64)> {
    let mut pts: Vec<(f64, f64, f64)> = r
        .marks
        .iter()
        .filter(|p| p.mark.kind == kind && p.series_index == series_index)
        .filter_map(|p| {
            let x = center_x(p, r)?;
            let y1 = r.y.project(&Datum::Number(p.v1))?;
            let y0 = r.y.project(&Datum::Number(p.v0))?;
            Some((x, y1, y0))
        })
        .filter(|(x, y1, y0)| x.is_finite() && y1.is_finite() && y0.is_finite())
        .collect();
    // A line is a function of x; drawing it in declaration order would let unsorted data draw a
    // scribble that looks like a valid series.
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    pts
}

fn connected_path(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, kind: MarkKind, si: usize) {
    let pts = series_points(r, kind, si);
    if pts.len() < 2 {
        // A single point is still a datum: draw the degenerate case as a dot rather than nothing.
        if let Some((x, y, _)) = pts.first()
            && let Some(p) = r
                .marks
                .iter()
                .find(|p| p.mark.kind == kind && p.series_index == si)
        {
            let c = color_of(p, r, paint);
            d.fill(circle(Point::new(*x, *y), p.mark.style.line_width), c);
        }
        return;
    }
    let Some(proto) = r
        .marks
        .iter()
        .find(|p| p.mark.kind == kind && p.series_index == si)
    else {
        return;
    };
    let color = color_of(proto, r, paint);
    let interp = proto.mark.style.interpolation;
    let polar = r.coordinate.is_polar();

    let top: Vec<Point> = pts
        .iter()
        .map(|(x, y, _)| device(r, *x, *y, polar))
        .collect();
    if kind == MarkKind::Line {
        let path = curve(&top, interp, polar);
        d.stroke_styled(path, Paint::Solid(color), stroke_style(proto));
        return;
    }
    // Area: the top edge, then back along the baseline.
    let base: Vec<Point> = pts
        .iter()
        .rev()
        .map(|(x, _, y0)| device(r, *x, *y0, polar))
        .collect();
    let mut b = PathBuilder::new().rule(FillRule::NonZero);
    b = append(b, &top, interp, true);
    b = append(b, &base, interp, false);
    let shape = b.close().build();
    let paint = match proto.mark.style.gradient {
        Some(_) => fill_paint(proto, color),
        None => color.with_alpha(color.a * 0.85).into(),
    };
    d.fill(shape, paint);
}

fn device(r: &Resolved, x: f64, y: f64, polar: bool) -> Point {
    if polar {
        r.coordinate
            .project(r.plot, u_of(r.plot, x), v_of(r.plot, y))
    } else {
        Point::new(x, y)
    }
}

fn curve(pts: &[Point], interp: Interpolation, closed: bool) -> Shape {
    let b = append(PathBuilder::new(), pts, interp, true);
    if closed { b.close().build() } else { b.build() }
}

/// Append `pts` to a path under `interp`. `start` opens a new contour; a continuation lines to the
/// first point instead, which is what closes an area's baseline back onto its top edge.
fn append(mut b: PathBuilder, pts: &[Point], interp: Interpolation, start: bool) -> PathBuilder {
    if pts.is_empty() {
        return b;
    }
    b = if start {
        b.move_to(pts[0])
    } else {
        b.line_to(pts[0])
    };
    match interp {
        Interpolation::Linear => {
            for p in &pts[1..] {
                b = b.line_to(*p);
            }
        }
        Interpolation::StepStart => {
            for w in pts.windows(2) {
                b = b.line_to(Point::new(w[0].x, w[1].y)).line_to(w[1]);
            }
        }
        Interpolation::StepEnd => {
            for w in pts.windows(2) {
                b = b.line_to(Point::new(w[1].x, w[0].y)).line_to(w[1]);
            }
        }
        Interpolation::StepCenter => {
            for w in pts.windows(2) {
                let mx = (w[0].x + w[1].x) / 2.0;
                b = b
                    .line_to(Point::new(mx, w[0].y))
                    .line_to(Point::new(mx, w[1].y))
                    .line_to(w[1]);
            }
        }
        Interpolation::CatmullRom | Interpolation::Cardinal => {
            // Catmull-Rom as cubic beziers. Tension 0 is the plain spline; the cardinal family
            // slackens the tangents by (1 - tension).
            let k = if interp == Interpolation::Cardinal {
                0.5
            } else {
                1.0
            };
            for i in 0..pts.len() - 1 {
                let p0 = pts[i.saturating_sub(1)];
                let p1 = pts[i];
                let p2 = pts[i + 1];
                let p3 = pts[(i + 2).min(pts.len() - 1)];
                let c1 = Point::new(
                    p1.x + k * (p2.x - p0.x) / 6.0,
                    p1.y + k * (p2.y - p0.y) / 6.0,
                );
                let c2 = Point::new(
                    p2.x - k * (p3.x - p1.x) / 6.0,
                    p2.y - k * (p3.y - p1.y) / 6.0,
                );
                b = b.cubic_to(c1, c2, p2);
            }
        }
        Interpolation::Monotone => {
            for (c1, c2, p) in monotone(pts) {
                b = b.cubic_to(c1, c2, p);
            }
        }
    }
    b
}

/// Fritsch–Carlson monotone cubic interpolation, as bezier control points.
///
/// The construction is the 1980 paper's: take the secant slopes, average them for an interior
/// tangent, then CLAMP each tangent into the circle of radius 3 around the neighbouring secants.
/// That clamp is the whole theorem — it is what guarantees the spline cannot overshoot the data,
/// so a series of non-negative values never dips below zero on the way between two of them.
fn monotone(pts: &[Point]) -> Vec<(Point, Point, Point)> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n.saturating_sub(1));
    if n < 2 {
        return out;
    }
    let dx: Vec<f64> = (0..n - 1).map(|i| pts[i + 1].x - pts[i].x).collect();
    let secant: Vec<f64> = (0..n - 1)
        .map(|i| {
            if dx[i].abs() < f64::EPSILON {
                0.0
            } else {
                (pts[i + 1].y - pts[i].y) / dx[i]
            }
        })
        .collect();
    let mut m = vec![0.0; n];
    m[0] = secant[0];
    m[n - 1] = secant[n - 2];
    for i in 1..n - 1 {
        if secant[i - 1] * secant[i] <= 0.0 {
            // A local extremum: a zero tangent is the only value that cannot overshoot.
            m[i] = 0.0;
        } else {
            m[i] = (secant[i - 1] + secant[i]) / 2.0;
        }
    }
    for i in 0..n - 1 {
        if secant[i].abs() < f64::EPSILON {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let a = m[i] / secant[i];
            let b = m[i + 1] / secant[i];
            let s = a * a + b * b;
            if s > 9.0 {
                let t = 3.0 / s.sqrt();
                m[i] = t * a * secant[i];
                m[i + 1] = t * b * secant[i];
            }
        }
    }
    for i in 0..n - 1 {
        let h = dx[i];
        out.push((
            Point::new(pts[i].x + h / 3.0, pts[i].y + m[i] * h / 3.0),
            Point::new(pts[i + 1].x - h / 3.0, pts[i + 1].y - m[i + 1] * h / 3.0),
            pts[i + 1],
        ));
    }
    out
}

/// Everything about a point mark's APPEARANCE — two marks agreeing on all of it are
/// indistinguishable, which is what lets them share one stamp.
///
/// The two f64s compare by bit pattern rather than by value: they come from the same arithmetic on
/// the same inputs for every mark in a series, so equal values are bit-equal, and a NaN radius
/// (which `==` would never match, splitting a group per datum) lands in one group instead.
#[derive(Clone, Copy)]
struct PointLook {
    symbol: Symbol,
    radius: f64,
    width: f64,
    color: Color,
}

impl PartialEq for PointLook {
    fn eq(&self, other: &Self) -> bool {
        // Bit patterns, not values: every mark in a series reaches these through the same
        // arithmetic on the same inputs, so equal values are bit-equal — and a degenerate NaN
        // radius groups with itself instead of splitting the batch one op per datum, which is
        // exactly the case `==` would get wrong.
        let bits = |a: f64, b: f64| a.to_bits() == b.to_bits();
        self.symbol == other.symbol
            && bits(self.radius, other.radius)
            && bits(self.width, other.width)
            && bits(self.color.r, other.color.r)
            && bits(self.color.g, other.color.g)
            && bits(self.color.b, other.color.b)
            && bits(self.color.a, other.color.a)
    }
}

impl PointLook {
    fn new(symbol: Symbol, radius: f64, width: f64, color: Color) -> Self {
        PointLook {
            symbol,
            radius,
            width,
            color,
        }
    }
}

/// Where one point mark goes and what it looks like, or `None` if it does not project.
fn point_mark(r: &Resolved, paint: &Paint2<'_>, p: &Placed) -> Option<(PointLook, Point)> {
    let x = center_x(p, r)?;
    let y = r.y.project(&Datum::Number(p.v1))?;
    let at = device(
        r,
        x + p.mark.offset.0,
        y + p.mark.offset.1,
        r.coordinate.is_polar(),
    );
    let color = color_of(p, r, paint);
    let sym = p.mark.style.symbol.unwrap_or_else(|| {
        if p.mark.symbol_by.is_some() {
            Symbol::CYCLE[p.series_index % Symbol::CYCLE.len()]
        } else {
            Symbol::Circle
        }
    });
    // symbol_size is an AREA, so the radius is its square root — the encoding is only honest if
    // twice the value is twice the ink.
    let radius = (p.mark.style.symbol_size / std::f64::consts::PI).sqrt();
    Some((
        PointLook::new(sym, radius, p.mark.style.line_width.max(1.5), color),
        at,
    ))
}

fn circle(at: Point, r: f64) -> Shape {
    Shape::Ellipse(Rect::new(at.x - r, at.y - r, r * 2.0, r * 2.0))
}

/// A symbol's geometry, authored around the ORIGIN: the shapes it fills and the shapes it strokes.
///
/// Around the origin rather than at a point, because that is what a [`Draw::stamp`] template is —
/// one symbol becomes one op however many data points wear it (docs/canvas.md "Stamping").
fn symbol_parts(sym: Symbol, r: f64) -> (Vec<Shape>, Vec<Shape>) {
    let poly = |n: usize, rot: f64| {
        Shape::Polygon(
            (0..n)
                .map(|i| {
                    let a = rot + i as f64 * std::f64::consts::TAU / n as f64;
                    Point::new(r * a.cos(), r * a.sin())
                })
                .collect(),
        )
    };
    let up = -std::f64::consts::FRAC_PI_2;
    match sym {
        Symbol::Circle => (
            vec![Shape::Ellipse(Rect::new(-r, -r, r * 2.0, r * 2.0))],
            vec![],
        ),
        Symbol::Square => (
            vec![Shape::Rect(Rect::new(-r, -r, r * 2.0, r * 2.0))],
            vec![],
        ),
        Symbol::Triangle => (vec![poly(3, up)], vec![]),
        Symbol::Diamond => (vec![poly(4, up)], vec![]),
        Symbol::Pentagon => (vec![poly(5, up)], vec![]),
        Symbol::Cross => (
            vec![],
            vec![
                Shape::Line(Point::new(-r, -r), Point::new(r, r)),
                Shape::Line(Point::new(-r, r), Point::new(r, -r)),
            ],
        ),
        Symbol::Plus => (
            vec![],
            vec![
                Shape::Line(Point::new(-r, 0.0), Point::new(r, 0.0)),
                Shape::Line(Point::new(0.0, -r), Point::new(0.0, r)),
            ],
        ),
        Symbol::Asterisk => (
            vec![],
            (0..3)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::PI / 3.0;
                    Shape::Line(
                        Point::new(-r * a.cos(), -r * a.sin()),
                        Point::new(r * a.cos(), r * a.sin()),
                    )
                })
                .collect(),
        ),
    }
}

/// One symbol at every one of `at` — a stamp per part, whatever the count.
fn draw_symbols(d: &mut Draw, sym: Symbol, at: Vec<Point>, r: f64, color: Color, w: f64) {
    if at.is_empty() {
        return;
    }
    let (filled, stroked) = symbol_parts(sym, r);
    for shape in filled {
        d.stamp(shape, at.clone(), color);
    }
    for shape in stroked {
        d.stamp_styled(shape, at.clone(), color, StrokeStyle::width(w));
    }
}

/// A wedge between two angles and two radii, as a path.
///
/// Built from cubic beziers rather than an arc primitive so the donut hole, the angular inset and
/// the corner radius all compose: an arc op would draw the outer edge and leave the join to the
/// rasterizer's own arc-to-line rule, which differs between backends.
#[allow(clippy::too_many_arguments)]
fn wedge(center: Point, r0: f64, r1: f64, a0: f64, a1: f64) -> Shape {
    let mut b = PathBuilder::new();
    let arc = |b: PathBuilder, r: f64, from: f64, to: f64, first: bool| {
        let mut b = b;
        // A cubic approximates a circular arc well up to a quarter turn; beyond that the error is
        // visible, so long sweeps are split.
        let steps = (((to - from).abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).max(1);
        let d = (to - from) / steps as f64;
        let mut a = from;
        if first {
            b = b.move_to(Point::new(center.x + r * a.cos(), center.y + r * a.sin()));
        } else {
            b = b.line_to(Point::new(center.x + r * a.cos(), center.y + r * a.sin()));
        }
        for _ in 0..steps {
            let next = a + d;
            // The exact tangent length for a circular arc of this sweep.
            let k = 4.0 / 3.0 * (d / 4.0).tan();
            let p0 = Point::new(center.x + r * a.cos(), center.y + r * a.sin());
            let p1 = Point::new(center.x + r * next.cos(), center.y + r * next.sin());
            let c1 = Point::new(p0.x - k * r * a.sin(), p0.y + k * r * a.cos());
            let c2 = Point::new(p1.x + k * r * next.sin(), p1.y - k * r * next.cos());
            b = b.cubic_to(c1, c2, p1);
            a = next;
        }
        b
    };
    b = arc(b, r1, a0, a1, true);
    if r0 > 0.5 {
        b = arc(b, r0, a1, a0, false);
    } else {
        b = b.line_to(center);
    }
    b.close().build()
}

fn sector(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    let plot = r.plot;
    let coord = if r.coordinate.is_polar() {
        r.coordinate
    } else {
        // A sector in a cartesian chart still wants a circle; give it one rather than refusing.
        Coordinate::polar()
    };
    // The angular extent comes from the stacked bounds, normalized by the total. That is the whole
    // of "a pie is a normalized stack" — no separate percentage arithmetic.
    let total: f64 = r
        .marks
        .iter()
        .filter(|q| q.mark.kind == p.mark.kind)
        .map(|q| (q.v1 - q.v0).abs())
        .sum();
    if total <= 0.0 {
        return;
    }
    let u0 = p.v0 / total;
    let u1 = p.v1 / total;
    let a0 = coord.angle(u0);
    let a1 = coord.angle(u1);
    let center = coord.center(plot);
    let r_max = coord.radius(plot) * 0.98;
    let hole = match coord {
        Coordinate::Polar { hole, .. } => hole,
        _ => 0.0,
    };
    let outer = r_max * p.mark.outer_radius;
    let inner = (r_max * hole).max(r_max * p.mark.inner_radius);
    // The inset is an arc LENGTH, so converting it to an angle depends on the radius; using the
    // outer radius keeps the visible gap even rather than tapering toward the center.
    let inset = if outer > 0.0 {
        p.mark.angular_inset / outer
    } else {
        0.0
    };
    let (a0, a1) = if (a1 - a0).abs() > 2.0 * inset {
        (a0 + inset, a1 - inset)
    } else {
        (a0, a1)
    };
    let color = color_of(p, r, paint);
    d.fill(wedge(center, inner, outer, a0, a1), color);
}

fn annotation(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    let Some(a) = &p.mark.annotation else { return };
    if a.text.is_empty() {
        return;
    }
    use crate::mark::AnnotationPosition as AP;
    let Some(x) = center_x(p, r) else { return };
    // On a categorical y the mark IS the band, so the annotation keys off the category, not the
    // stacked value it does not have.
    let y = if r.y.kind.is_discrete() {
        p.mark.y.as_ref().and_then(|v| r.y.project(&v.datum))
    } else {
        r.y.project(&Datum::Number(p.v1))
    };
    let Some(y) = y else { return };
    let line = day_core::measure_text(&a.text, paint.label_size, &paint.font);
    // Text laid OVER a bar or a cell has to fit inside it: a heat map's numbers on a phone-width
    // grid would otherwise spill into their neighbours and read as one smear. Not drawing a
    // label is the honest answer there; the color still carries the value.
    if a.position == AP::Overlay && matches!(p.mark.kind, MarkKind::Bar | MarkKind::Rectangle) {
        let width = match (&p.mark.x, &p.mark.x_end) {
            (Some(a), Some(b)) => match (r.x.project(&a.datum), r.x.project(&b.datum)) {
                (Some(a), Some(b)) => (a - b).abs(),
                _ => band_of(p, r),
            },
            _ => band_of(p, r),
        };
        if line.width > width - 2.0 {
            return;
        }
    }
    let at = match a.position {
        // The automatic position puts the label clear of the mark on the side the value grew
        // toward, which is above for a positive bar and below for a negative one.
        AP::Automatic | AP::Top => Point::new(x, y - line.height * 0.7),
        AP::Bottom => Point::new(x, y + line.height * 0.7),
        AP::Leading => Point::new(x - line.width / 2.0 - 6.0, y),
        AP::Trailing => Point::new(x + line.width / 2.0 + 6.0, y),
        AP::Overlay => Point::new(x, y),
    };
    d.text(
        &a.text,
        at,
        TextStyle {
            size: paint.label_size,
            color: a.color.unwrap_or(paint.chrome.label),
            anchor: TextAnchor::CENTERED,
            font: paint.font.clone(),
        },
    );
}

// ---------------------------------------------------------------------------
// Selection: the hit model a draw leaves behind, and the guides drawn for one
// ---------------------------------------------------------------------------

/// Every mark's device position and labels, for the pointer to hit
/// (README.md "Selection").
///
/// Built from the SAME `Resolved` the marks were just drawn from, so a selection can only ever
/// name something on screen, and a pointer move costs a scan of this rather than a re-resolve.
pub fn hit_model(r: &Resolved, paint: &Paint2<'_>) -> crate::select::HitModel {
    let polar = r.coordinate.is_polar();
    // The axis's own tick step decides how many decimals a value reads with, so a selected value
    // is rounded exactly like the axis label above it — "58.2", not "58.15339133". The exact
    // number is still on `SelectedValue::value` for an app that wants full precision.
    let step_of = |ticks: &[crate::ticks::Tick]| match ticks {
        [a, b, ..] => (b.value - a.value).abs(),
        _ => 0.0,
    };
    let (x_step, y_step) = (step_of(&r.x_ticks), step_of(&r.y_ticks));
    let marks = r
        .marks
        .iter()
        .filter_map(|p| {
            let x = center_x(p, r)?;
            let y = r.y.project(&Datum::Number(p.v1))?;
            let series = r.series.get(p.series_index).cloned().unwrap_or_default();
            Some(crate::select::HitMark {
                at: device(r, x + p.mark.offset.0, y + p.mark.offset.1, polar),
                color: color_of(p, r, paint),
                x_label: x_label_of(p, r, paint, x_step),
                y_label: crate::resolve::label_for(p.v1, &r.y, paint.y_axis, y_step),
                value: p.v1,
                series,
            })
        })
        .collect();
    crate::select::HitModel {
        plot: r.plot,
        marks,
    }
}

/// A mark's own x as a label: its category on a discrete axis, its value formatted by the axis's
/// own formatter otherwise — so the guide's label reads exactly like the axis under it.
fn x_label_of(p: &Placed, r: &Resolved, paint: &Paint2<'_>, step: f64) -> String {
    match p.mark.x.as_ref().map(|v| &v.datum) {
        Some(Datum::Category(c)) => match &paint.x_axis.format {
            Some(f) => f(&Datum::Category(c.clone())),
            None => c.clone(),
        },
        Some(Datum::Number(v) | Datum::Time(v)) => {
            crate::resolve::label_for(*v, &r.x, paint.x_axis, step)
        }
        _ => String::new(),
    }
}

/// The rules, rings and label box for the current selection.
///
/// Drawn last, over the marks: a guide that a mark can cover is not a guide. The label box is
/// placed on whichever side of the rule has room and then clamped into the plot, so it never
/// hangs off the edge at either extreme of the axis.
pub fn draw_guides(
    d: &mut Draw,
    r: &Resolved,
    paint: &Paint2<'_>,
    sel: &crate::select::Selection,
    guides: crate::select::Guides,
) {
    let plot = r.plot;
    let rule = paint.chrome.label.with_alpha(0.55);
    if guides.vertical {
        d.stroke(
            Shape::Line(
                Point::new(sel.at.x, plot.origin.y),
                Point::new(sel.at.x, plot.origin.y + plot.size.height),
            ),
            rule,
            1.0,
        );
    }
    if guides.horizontal {
        d.stroke(
            Shape::Line(
                Point::new(plot.origin.x, sel.at.y),
                Point::new(plot.origin.x + plot.size.width, sel.at.y),
            ),
            rule,
            1.0,
        );
    }
    if guides.points {
        // A ring rather than a filled dot: the mark underneath stays visible, which is what says
        // "this one" instead of covering the thing being pointed at.
        for v in &sel.values {
            d.stroke(circle(v.at, 5.0), v.color, 2.0);
        }
    }
    if !guides.label {
        return;
    }
    // The box: the position on the first line, then one line per selected series.
    let size = paint.label_size;
    let mut lines: Vec<(String, Color)> = vec![(sel.x_label.clone(), paint.chrome.title)];
    for v in &sel.values {
        let text = if v.series.is_empty() {
            v.label.clone()
        } else {
            format!("{}  {}", v.series, v.label)
        };
        lines.push((text, v.color));
    }
    let line_h = day_core::measure_text("0", size, &paint.font).height;
    let w = lines
        .iter()
        .map(|(t, _)| day_core::measure_text(t, size, &paint.font).width)
        .fold(0.0f64, f64::max);
    let pad = 6.0;
    let (bw, bh) = (w + pad * 2.0, line_h * lines.len() as f64 + pad * 2.0);
    // Right of the rule where it fits, left otherwise — then clamped, so an extreme selection
    // keeps the whole box inside the plot instead of half of it outside.
    let mut bx = sel.at.x + 10.0;
    if bx + bw > plot.origin.x + plot.size.width {
        bx = sel.at.x - 10.0 - bw;
    }
    bx = bx.clamp(
        plot.origin.x,
        (plot.origin.x + plot.size.width - bw).max(plot.origin.x),
    );
    let by = (sel.at.y - bh / 2.0).clamp(
        plot.origin.y,
        (plot.origin.y + plot.size.height - bh).max(plot.origin.y),
    );
    d.fill(
        Shape::RoundedRect(Rect::new(bx, by, bw, bh), 5.0),
        // The plot's own ground where it has one, so the box reads as part of the chart; the
        // page's otherwise, which is what a chart with a transparent plot sits on.
        paint
            .chrome
            .plot_background
            .unwrap_or(if day_core::dark_mode() {
                Color::rgba(0.11, 0.11, 0.13, 1.0)
            } else {
                Color::rgba(1.0, 1.0, 1.0, 1.0)
            })
            .with_alpha(0.94),
    );
    d.stroke(
        Shape::RoundedRect(Rect::new(bx, by, bw, bh), 5.0),
        rule,
        1.0,
    );
    for (i, (text, color)) in lines.iter().enumerate() {
        d.text(
            text,
            Point::new(bx + pad, by + pad + line_h * i as f64),
            TextStyle {
                size,
                color: *color,
                anchor: TextAnchor::LEADING,
                font: paint.font.clone(),
            },
        );
    }
}
