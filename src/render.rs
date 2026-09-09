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
use day_spec::{Color, FillRule, Paint, Point, Rect, Shape, StrokeStyle, TextAnchor, TextVAlign};

use crate::axis::AxisSpec;
use crate::coord::Coordinate;
use crate::data::Datum;
use crate::mark::{Interpolation, MarkKind, Symbol};
use crate::resolve::{Placed, Resolved};
use crate::scale::Scale;
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

/// The band a mark occupies along x, in device points, narrowed by its dodging slot.
fn band_of(p: &Placed, x: &Scale) -> f64 {
    let full = if x.kind.is_discrete() {
        x.band_width()
    } else {
        // A continuous x has no band; give bars a share of the smallest gap between them so a
        // time-series bar chart still has bars rather than hairlines.
        (x.range.1 - x.range.0).abs() / 24.0
    };
    let slot = full / p.dodge_count.max(1) as f64;
    p.mark.width.resolve(slot)
}

/// The centre of a mark along x, in device points, offset into its dodging slot.
fn center_x(p: &Placed, x: &Scale) -> Option<f64> {
    let base = x.project(&p.mark.x.as_ref()?.datum)?;
    if p.dodge_count <= 1 {
        return Some(base);
    }
    let full = if x.kind.is_discrete() {
        x.band_width()
    } else {
        (x.range.1 - x.range.0).abs() / 24.0
    };
    let slot = full / p.dodge_count as f64;
    Some(base - full / 2.0 + slot * (p.dodge_index as f64 + 0.5))
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
    for i in &order {
        let p = &r.marks[*i];
        if p.mark.kind == MarkKind::Point {
            point_mark(d, r, paint, p);
        }
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
    let line_h = day_core::measure_text("0", paint.label_size, &paint.font).height;
    let label = |color: Color, anchor: TextAnchor| TextStyle {
        size: paint.label_size,
        color,
        anchor,
        font: paint.font.clone(),
    };

    // --- x ---
    if !paint.x_axis.hidden {
        let y = plot.origin.y + plot.size.height;
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
                    Shape::Line(Point::new(x, y), Point::new(x, y + 4.0)),
                    paint.chrome.tick,
                    1.0,
                );
            }
            if paint.x_axis.labels {
                d.text(
                    &t.label,
                    Point::new(x, y + 6.0),
                    label(
                        paint.chrome.label,
                        TextAnchor {
                            h: TextAlign::Center,
                            v: TextVAlign::Top,
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
                    y + 10.0 + line_h * 1.4,
                ),
                label(
                    paint.chrome.title,
                    TextAnchor {
                        h: TextAlign::Center,
                        v: TextVAlign::Top,
                    },
                ),
            );
        }
    }

    // --- y ---
    if !paint.y_axis.hidden {
        let x = plot.origin.x;
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
                    Shape::Line(Point::new(x - 4.0, y), Point::new(x, y)),
                    paint.chrome.tick,
                    1.0,
                );
            }
            if paint.y_axis.labels {
                // Right-aligned against the axis and centred on the tick. The anchor says that
                // outright, so nothing here has to measure the label first — the backend already
                // holds the width it is about to draw with (docs/canvas.md "Text").
                d.text(
                    &t.label,
                    Point::new(x - 8.0, y),
                    label(
                        paint.chrome.label,
                        TextAnchor {
                            h: TextAlign::Trailing,
                            v: TextVAlign::Middle,
                        },
                    ),
                );
            }
        }
        if let Some(title) = axis_title(paint.y_axis, r.y_title.as_deref()) {
            // Rotated a quarter turn, which is the only way a y title fits a narrow margin. The
            // rotation is about the text's own anchor, so the transform is translate-rotate.
            let cx = plot.origin.x - (widest(&r.y_ticks, paint) + 14.0 + line_h / 2.0);
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
    let Some(cx) = center_x(p, &r.x) else { return };
    let (Some(y0), Some(y1)) = (
        r.y.project(&Datum::Number(p.v0)),
        r.y.project(&Datum::Number(p.v1)),
    ) else {
        return;
    };
    let w = if p.mark.kind == MarkKind::Rectangle {
        p.mark.width.resolve(band_of(p, &r.x))
    } else {
        band_of(p, &r.x)
    };
    // Clamped to the plot: a bar's baseline is zero, and zero has no position on a log axis — it
    // projects to an enormous negative and the bar runs off the pane. Clamping puts the baseline on
    // the axis floor, which is what a bar on a log scale actually means.
    let (y0, y1) = (r.y.clamp_to_range(y0), r.y.clamp_to_range(y1));
    let (top, bottom) = (y0.min(y1), y0.max(y1));
    let rect = Rect::new(
        cx - w / 2.0 + p.mark.offset.0,
        top + p.mark.offset.1,
        w,
        (bottom - top).max(0.0),
    );
    let color = color_of(p, r, paint);
    let radius = p
        .mark
        .style
        .corner_radius
        // A radius wider than half the bar is not a rounded bar, it is a lozenge; clamp so a
        // generous radius degrades instead of inverting the shape.
        .min(rect.size.width / 2.0)
        .min(rect.size.height / 2.0);
    if radius > 0.0 {
        d.fill(Shape::RoundedRect(rect, radius), color);
    } else {
        d.fill(Shape::Rect(rect), color);
    }
}

fn rule(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    let color = color_of(p, r, paint);
    let style = stroke_style(p);
    let plot = r.plot;
    if let Some(v) = &p.mark.x {
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
            let x = center_x(p, &r.x)?;
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
    d.fill(shape, color.with_alpha(color.a * 0.85));
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

fn point_mark(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    let Some(x) = center_x(p, &r.x) else { return };
    let Some(y) = r.y.project(&Datum::Number(p.v1)) else {
        return;
    };
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
    draw_symbol(d, sym, at, radius, color, p.mark.style.line_width.max(1.5));
}

fn circle(at: Point, r: f64) -> Shape {
    Shape::Ellipse(Rect::new(at.x - r, at.y - r, r * 2.0, r * 2.0))
}

fn draw_symbol(d: &mut Draw, sym: Symbol, at: Point, r: f64, color: Color, w: f64) {
    let poly = |n: usize, rot: f64| {
        let pts: Vec<Point> = (0..n)
            .map(|i| {
                let a = rot + i as f64 * std::f64::consts::TAU / n as f64;
                Point::new(at.x + r * a.cos(), at.y + r * a.sin())
            })
            .collect();
        Shape::Polygon(pts)
    };
    match sym {
        Symbol::Circle => d.fill(circle(at, r), color),
        Symbol::Square => d.fill(
            Shape::Rect(Rect::new(at.x - r, at.y - r, r * 2.0, r * 2.0)),
            color,
        ),
        Symbol::Triangle => d.fill(poly(3, -std::f64::consts::FRAC_PI_2), color),
        Symbol::Diamond => d.fill(poly(4, -std::f64::consts::FRAC_PI_2), color),
        Symbol::Pentagon => d.fill(poly(5, -std::f64::consts::FRAC_PI_2), color),
        Symbol::Cross => {
            d.stroke(
                Shape::Line(
                    Point::new(at.x - r, at.y - r),
                    Point::new(at.x + r, at.y + r),
                ),
                color,
                w,
            );
            d.stroke(
                Shape::Line(
                    Point::new(at.x - r, at.y + r),
                    Point::new(at.x + r, at.y - r),
                ),
                color,
                w,
            );
        }
        Symbol::Plus => {
            d.stroke(
                Shape::Line(Point::new(at.x - r, at.y), Point::new(at.x + r, at.y)),
                color,
                w,
            );
            d.stroke(
                Shape::Line(Point::new(at.x, at.y - r), Point::new(at.x, at.y + r)),
                color,
                w,
            );
        }
        Symbol::Asterisk => {
            for i in 0..3 {
                let a = i as f64 * std::f64::consts::PI / 3.0;
                d.stroke(
                    Shape::Line(
                        Point::new(at.x - r * a.cos(), at.y - r * a.sin()),
                        Point::new(at.x + r * a.cos(), at.y + r * a.sin()),
                    ),
                    color,
                    w,
                );
            }
        }
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
    // outer radius keeps the visible gap even rather than tapering toward the centre.
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
    use crate::mark::AnnotationPosition as AP;
    let Some(x) = center_x(p, &r.x) else { return };
    let Some(y) = r.y.project(&Datum::Number(p.v1)) else {
        return;
    };
    let line = day_core::measure_text(&a.text, paint.label_size, &paint.font);
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
            color: paint.chrome.label,
            anchor: TextAnchor::CENTERED,
            font: paint.font.clone(),
        },
    );
}
