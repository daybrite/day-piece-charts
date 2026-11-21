// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Drawing a [`Resolved`] chart onto a `Draw` display list.
//!
//! Every mark is positioned in **unit plot space** and handed to the [`Coordinate`] to be placed,
//! so there is one geometry path rather than one per coordinate system. A bar in polar space comes
//! out as a wedge because that is what a bar is in polar space; the renderer does not special-case
//! it, and nothing here knows the words "pie chart".

use day_geometry::{Affine, Angle};
use day_pieces::{Draw, PathBuilder, TextStyle};
use day_spec::props::TextAlign;
use day_spec::{
    BlendMode, Color, CornerRadii, FillRule, Ink, LinearGradient, Paint, Point, Rect, Shadow,
    Shape, StrokeStyle, TextAnchor, TextVAlign,
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
    /// What a ridge hides the rows behind it with ([`Mark::ridge`](crate::Mark::ridge)). `None`
    /// erases them, so the chart sits on whatever is behind its canvas; a drawing with a ground
    /// of its own (an export's white sheet, a plot background) names it, because erasing would
    /// cut through that too.
    pub ground: Option<Ink>,
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
            .map_or("", std::string::String::as_str);
        (paint.series_colors)(p.series_index, name)
    });
    let alpha = p.mark.style.opacity.clamp(0.0, 1.0);
    if alpha >= 1.0 {
        base
    } else {
        base.with_alpha(base.a * alpha)
    }
}

/// The full band one x position owns, in device points.
///
/// A discrete scale says so itself. A continuous one has no bands, so a bar there takes the
/// smallest gap between any two bars (`Resolved::x_gap`) less the scale's inner padding, which
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
pub(crate) fn band_of(p: &Placed, r: &Resolved) -> f64 {
    if let Some((_, w)) = p.x_band {
        return w;
    }
    let slot = full_band(r) / p.dodge_count.max(1) as f64;
    p.mark.width.resolve(slot)
}

/// The center of a mark along x, in device points, offset into its dodging slot.
pub(crate) fn center_x(p: &Placed, r: &Resolved) -> Option<f64> {
    if let Some((x, _)) = p.x_band {
        return Some(x);
    }
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

/// `ink` drawn at `alpha` of its own opacity: a tick's.
fn faded(ink: Ink, alpha: f64) -> Ink {
    if alpha >= 1.0 {
        ink
    } else {
        ink.map(|c| c.with_alpha(c.a * alpha.max(0.0)))
    }
}

/// Whether a device coordinate lies on the plot along one axis, with half a point to spare for
/// the gridlines drawn exactly on its edges. A tick sliding off an axis in a transition is
/// dropped at the edge rather than drawn over the neighbouring chrome.
fn within(v: f64, origin: f64, extent: f64) -> bool {
    v >= origin - 0.5 && v <= origin + extent + 0.5
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
    // Marks draw in z order, and a stable sort keeps declaration order inside a z, the contract
    // stacking already relies on.
    let mut order: Vec<usize> = (0..r.marks.len()).collect();
    order.sort_by(|a, b| {
        r.marks[*a]
            .mark
            .z
            .partial_cmp(&r.marks[*b].mark.z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Line and area marks are connected marks: they describe a series, not a datum, so they are
    // gathered by series and drawn as one path. Every other kind draws per mark.
    let mut connected: Vec<(MarkKind, usize)> = Vec::new();
    for i in &order {
        let p = &r.marks[*i];
        // A ridge is connected along its own row, and drawn by `ridges`.
        if matches!(p.mark.kind, MarkKind::Line | MarkKind::Area) && p.mark.rise.is_none() {
            let key = (p.mark.kind, p.series_index);
            if !connected.contains(&key) {
                connected.push(key);
            }
        }
    }
    // A pinned domain is the one case a mark can lie outside the plot: the app said where the
    // axis ends, and the data did not agree. Clip to the plot along each pinned axis only, so a
    // fat point at the last sample of an inferred axis keeps its far half.
    let clip = r.x.explicit_domain || r.y.explicit_domain || r.clip_plot;
    if clip {
        let far = 1.0e5;
        let plot = r.plot;
        let (cx, cw) = if r.x.explicit_domain || r.clip_plot {
            (plot.origin.x, plot.size.width)
        } else {
            (plot.origin.x - far, plot.size.width + 2.0 * far)
        };
        let (cy, chh) = if r.y.explicit_domain || r.clip_plot {
            (plot.origin.y, plot.size.height)
        } else {
            (plot.origin.y - far, plot.size.height + 2.0 * far)
        };
        d.save();
        d.clip(Shape::Rect(Rect::new(cx, cy, cw, chh)));
    }
    // A sweeping entrance uncovers the marks from the leading edge. The clip reaches a little
    // past the plot on both sides, so a point on the first or last sample is not cut in half.
    let revealing = r.reveal < 1.0;
    if revealing {
        let (far, overhang) = (1.0e5, 8.0);
        d.save();
        d.clip(Shape::Rect(Rect::new(
            r.plot.origin.x - overhang,
            r.plot.origin.y - far,
            (r.plot.size.width + 2.0 * overhang) * r.reveal.max(0.0),
            r.plot.size.height + 2.0 * far,
        )));
    }
    // Ridges before everything else: each one hides what is behind it, and a rule or a band
    // drawn over the range should stay on top.
    ridges(d, r, paint);
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
            MarkKind::Line | MarkKind::Area | MarkKind::Point => {}
        }
    }
    for (kind, si) in connected.iter().filter(|(k, _)| *k == MarkKind::Line) {
        connected_path(d, r, paint, *kind, *si);
    }
    // Point marks are batched: every datum sharing a symbol, a size, a color and a stroke width
    // is one stamp rather than one op each (docs/canvas.md "Stamping"). A scatter is the case the
    // batched op exists for: fifty thousand points drawn individually are fifty thousand ops to
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
        draw_symbols(d, look.symbol, &at, look.radius, look.color, look.width);
    }
    if revealing {
        d.restore();
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
            if let Some(y) = r.y.project(&Datum::Number(t.value))
                && t.alpha > 0.01
                && within(y, plot.origin.y, plot.size.height)
            {
                d.stroke(
                    Shape::Line(
                        Point::new(plot.origin.x, y),
                        Point::new(plot.origin.x + plot.size.width, y),
                    ),
                    faded(paint.chrome.grid_line, t.alpha),
                    1.0,
                );
            }
        }
    }
    if paint.x_axis.grid && !paint.x_axis.hidden {
        // Dashed, and along the category axis they fall between the categories rather than
        // through them, because where one band ends and the next begins is what a reader traces
        // a bar down to. Swift Charts draws both the same way.
        let dash = StrokeStyle::dashed(1.0, vec![4.0, 4.0]);
        let xs: Vec<(f64, f64)> = if r.x.kind.is_discrete() {
            let step = r.x.step();
            let mut xs = Vec::with_capacity(r.x.categories.len() + 1);
            for c in &r.x.categories {
                if let Some(x) = r.x.project(&Datum::Category(c.clone())) {
                    xs.push((x - step / 2.0, 1.0));
                }
            }
            if let Some((last, _)) = xs.last().copied() {
                xs.push((last + step, 1.0));
            }
            xs
        } else {
            r.x_ticks
                .iter()
                .filter(|t| t.alpha > 0.01)
                .filter_map(|t| Some((r.x.project(&Datum::Number(t.value))?, t.alpha)))
                .collect()
        };
        let (lo, hi) = (plot.origin.x - 0.5, plot.origin.x + plot.size.width + 0.5);
        for (x, alpha) in xs {
            if x < lo || x > hi {
                continue;
            }
            d.stroke_styled(
                Shape::Line(
                    Point::new(x, plot.origin.y),
                    Point::new(x, plot.origin.y + plot.size.height),
                ),
                faded(paint.chrome.grid_line, alpha),
                dash.clone(),
            );
        }
    }
}

fn axes(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>) {
    let plot = r.plot;
    // One measurement for the whole axis: these are font metrics, the same for every label, so
    // vertical labels reuse these metrics; horizontal collision checks measure their widths.
    let m = day_core::measure_text("0", paint.label_size, &paint.font);
    let line_h = m.height;
    // How far a tick label's line box must shift up so its cap box straddles the gridline instead.
    // An axis label is digits, which use none of the descender room the line box reserves, so
    // centering by the line box sits every label visibly low, by half the difference between the
    // descent and nothing. The cap middle sits `ascent - cap/2` below the line box top, and the
    // line middle at `height/2`; the gap between them is the correction.
    let cap_lift = m.height / 2.0 - (m.ascent - m.cap_height / 2.0);
    let label = |color: Ink, anchor: TextAnchor| TextStyle {
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
        if paint.x_axis.rule {
            d.stroke(
                Shape::Line(
                    Point::new(plot.origin.x, y),
                    Point::new(plot.origin.x + plot.size.width, y),
                ),
                paint.chrome.axis_line,
                1.0,
            );
        }
        let mut occupied = Vec::new();
        for t in &r.x_ticks {
            let x = t.at.or_else(|| r.x.project(&Datum::Number(t.value)));
            let Some(x) = x else { continue };
            if t.alpha <= 0.01 || !within(x, plot.origin.x, plot.size.width) {
                continue;
            }
            if paint.x_axis.ticks {
                d.stroke(
                    Shape::Line(Point::new(x, y), Point::new(x, y + 4.0 * away)),
                    faded(paint.chrome.tick, t.alpha),
                    1.0,
                );
            }
            if paint.x_axis.labels
                && !t.label.is_empty()
                && reserve_label(
                    &mut occupied,
                    x,
                    day_core::measure_text(&t.label, paint.label_size, &paint.font).width,
                    4.0,
                )
            {
                d.text(
                    &t.label,
                    Point::new(x, y + 6.0 * away),
                    label(
                        faded(paint.chrome.label, t.alpha),
                        TextAnchor {
                            h: TextAlign::Center,
                            v: v_anchor,
                        },
                    ),
                );
            }
        }
        if let Some(title) = axis_title(paint.x_axis) {
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
        if paint.y_axis.rule {
            d.stroke(
                Shape::Line(
                    Point::new(x, plot.origin.y),
                    Point::new(x, plot.origin.y + plot.size.height),
                ),
                paint.chrome.axis_line,
                1.0,
            );
        }
        let mut occupied = Vec::new();
        for t in &r.y_ticks {
            let y = t.at.or_else(|| r.y.project(&Datum::Number(t.value)));
            let Some(y) = y else { continue };
            if t.alpha <= 0.01 || !within(y, plot.origin.y, plot.size.height) {
                continue;
            }
            if paint.y_axis.ticks {
                d.stroke(
                    Shape::Line(Point::new(x + 4.0 * away, y), Point::new(x, y)),
                    faded(paint.chrome.tick, t.alpha),
                    1.0,
                );
            }
            if paint.y_axis.labels
                && !t.label.is_empty()
                && reserve_label(&mut occupied, y, line_h, 2.0)
            {
                // Aligned against the axis and centered on the tick. The anchor says that
                // outright, so nothing here has to measure the label first; the backend already
                // holds the width it is about to draw with (docs/canvas.md "Text"). The lift is
                // what turns "centered line box" into "centered digits".
                d.text(
                    &t.label,
                    Point::new(x + 8.0 * away, y - cap_lift),
                    label(
                        faded(paint.chrome.label, t.alpha),
                        TextAnchor {
                            h: h_anchor,
                            v: TextVAlign::Middle,
                        },
                    ),
                );
            }
        }
        if let Some(title) = axis_title(paint.y_axis) {
            // Rotated a quarter turn, which is the only way a y title fits a narrow margin. The
            // rotation is about the text's own anchor, so the transform is translate-rotate.
            let cx = x + (widest(&r.y_ticks, paint) + 14.0 + line_h / 2.0) * away;
            let cy = plot.origin.y + plot.size.height / 2.0;
            d.transformed(
                Affine::rotate(-Angle::QUARTER).then(Affine::translate(cx, cy)),
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

// Keep categorical and explicitly pinned axes legible as their viewport shrinks. Marks and
// gridlines retain every category; only colliding labels are omitted. Symmetric intervals
// work with reversed axes, RTL placement and caller-supplied ticks in any order.
fn reserve_label(occupied: &mut Vec<(f64, f64)>, center: f64, extent: f64, gap: f64) -> bool {
    let start = center - extent / 2.0 - gap / 2.0;
    let end = center + extent / 2.0 + gap / 2.0;
    if occupied.iter().any(|&(a, b)| start < b && end > a) {
        return false;
    }
    occupied.push((start, end));
    true
}

fn widest(ticks: &[crate::ticks::Tick], paint: &Paint2<'_>) -> f64 {
    ticks
        .iter()
        .map(|t| day_core::measure_text(&t.label, paint.label_size, &paint.font).width)
        .fold(0.0f64, f64::max)
}

fn axis_title(spec: &AxisSpec) -> Option<&str> {
    spec.title.as_deref().filter(|t| !t.is_empty())
}

// ---------------------------------------------------------------------------
// Marks
// ---------------------------------------------------------------------------

fn bar_or_rect(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    // In polar space a bar is a wedge: the grammar's answer, not a special case.
    if r.coordinate.is_polar() {
        sector(d, r, paint, p);
        return;
    }
    let plot = r.plot;
    // Which edges are rounded, as (top, bottom, left, right). Only the end a bar grew to is
    // rounded: the edge standing on the axis, and the edge a stacked segment shares with the one
    // below it, stay square, so a bar reads as rising from its baseline and a stack as one bar.
    // A range with no baseline (a candle's body, a Gantt bar) rounds both of its ends, and a
    // heat-map cell all four corners.
    let mut round = (true, true, true, true);
    let (top, bottom) = if p.mark.y.is_none() {
        // A band between two x values spans the plot.
        (plot.origin.y, plot.origin.y + plot.size.height)
    } else if r.y.kind.is_discrete() {
        // A categorical y (a heat map's rows, a horizontal bar's category): the mark is centered
        // on its band, as tall as the band less the mark's height dimension.
        let Some((cy, h)) = p.y_band else { return };
        (cy - h / 2.0, cy + h / 2.0)
    } else {
        let (Some(y0), Some(y1)) = (
            r.y.project(&Datum::Number(p.v0)),
            r.y.project(&Datum::Number(p.v1)),
        ) else {
            return;
        };
        // Clamped to the plot: a bar's baseline is zero, and zero has no position on a log axis;
        // it projects to an enormous negative and the bar runs off the pane. Clamping puts the
        // baseline on the axis floor, which is what a bar on a log scale means.
        let (y0, y1) = (r.y.clamp_to_range(y0), r.y.clamp_to_range(y1));
        let (start, end) = if p.mark.y_end.is_some() {
            // An explicit range: rounded at both ends, unless it starts on the baseline.
            (p.v0 != 0.0, true)
        } else {
            // From the baseline, or stacked on the segment below: the value end is rounded when
            // it is the outer end of its whole stack, which for a loss is the lowest segment.
            (false, if p.v1 >= p.v0 { p.stack_hi } else { p.stack_lo })
        };
        // Which end landed on which device edge: the value axis can be inverted, so this is read
        // off the projection rather than assumed.
        round = if y1 < y0 {
            (end, start, true, true)
        } else {
            (start, end, true, true)
        };
        (y0.min(y1), y0.max(y1))
    };
    // An explicit x span (a histogram bin, a horizontal bar, a Gantt bar) is the rectangle's
    // edges; without one the mark is centered on its position and as wide as its band.
    let (left, right) = if let (Some(a), Some(b)) = (&p.mark.x, &p.mark.x_end) {
        match (r.x.project(&a.datum), r.x.project(&b.datum)) {
            (Some(xa), Some(xb)) => {
                if r.y.kind.is_discrete() {
                    // A horizontal bar grows along x from its start: square on the axis when it
                    // starts at zero, rounded at both ends when it floats (a Gantt bar).
                    let starts_on_axis = a.datum.as_continuous() == Some(0.0);
                    let (start, end) = (!starts_on_axis, true);
                    let (left_end, right_end) = if xb >= xa { (start, end) } else { (end, start) };
                    round = (true, true, left_end, right_end);
                }
                (xa.min(xb), xa.max(xb))
            }
            _ => return,
        }
    } else if p.mark.x.is_none() {
        // A band between two y values spans the plot.
        (plot.origin.x, plot.origin.x + plot.size.width)
    } else {
        let Some(cx) = center_x(p, r) else { return };
        let w = band_of(p, r);
        (cx - w / 2.0, cx + w / 2.0)
    };
    let rect = Rect::new(
        left + p.mark.offset.0,
        top + p.mark.offset.1,
        (right - left).max(0.0),
        (bottom - top).max(0.0),
    );
    let color = color_of(p, r, paint);
    let radius = p.mark.style.corner_radius;
    // A corner is rounded when both edges meeting at it are.
    let (t, b, l, rt) = round;
    let at = |yes: bool| if yes { radius } else { 0.0 };
    let radii = CornerRadii {
        top_left: at(t && l),
        top_right: at(t && rt),
        bottom_right: at(b && rt),
        bottom_left: at(b && l),
    };
    // Fitted by `rounded_rect`: a radius wider than the bar degrades to a rounded end rather than
    // inverting the shape.
    fill_mark(d, Shape::rounded_rect(rect, radii), rect, p, color);
}

/// Fill `shape` the way the mark says: hatched, with its gradient, or solid in `color`.
/// `bounds` is the shape's bounding box, which a hatch has to cover.
fn fill_mark(d: &mut Draw, shape: Shape, bounds: Rect, p: &Placed, color: Color) {
    match p.mark.style.hatch {
        Some(spacing) => d.clipped(shape, |d| hatch(d, bounds, spacing, color)),
        None => d.fill(shape, fill_paint(p, color)),
    }
}

/// Diagonal lines `spacing` points apart across `bounds`, rising to the right, as one path.
///
/// The lines sit on a lattice fixed to the canvas, not to `bounds`: two hatched marks side by
/// side continue each other's lines, and a mark that moves or grows in a transition uncovers
/// lines that stay where they are.
fn hatch(d: &mut Draw, bounds: Rect, spacing: f64, color: Color) {
    let (left, top) = (bounds.origin.x, bounds.origin.y);
    let (right, bottom) = (left + bounds.size.width, top + bounds.size.height);
    // A line is the points where `x + y` is one constant; neighbors differ by the spacing
    // measured along an axis.
    let step = spacing.max(2.0) * std::f64::consts::SQRT_2;
    let mut c = ((left + top) / step).floor() * step;
    let mut path = PathBuilder::new();
    let mut any = false;
    while c <= right + bottom {
        path = path
            .move_to(Point::new(c - bottom, bottom))
            .line_to(Point::new(c - top, top));
        any = true;
        c += step;
    }
    if any {
        d.stroke(path.build(), color, 1.0);
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
            color,
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
            color,
            style,
        );
    }
}

fn stroke_style(p: &Placed) -> StrokeStyle {
    let mut s = StrokeStyle::width(p.mark.style.line_width);
    s.cap = p.mark.style.line_cap;
    s.join = p.mark.style.line_join;
    if !p.mark.style.dash.is_empty() {
        s.dash.clone_from(&p.mark.style.dash);
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
        d.stroke_styled(path, color, stroke_style(proto));
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
    fill_area(d, shape, proto, color);
}

/// Fill an area's shape: hatched, with its gradient, or in its color a little short of opaque,
/// so a gridline behind it still reads.
fn fill_area(d: &mut Draw, shape: Shape, proto: &Placed, color: Color) {
    if let Some(spacing) = proto.mark.style.hatch {
        let bounds = shape.bounds();
        d.clipped(shape, |d| hatch(d, bounds, spacing, color));
        return;
    }
    let paint = match proto.mark.style.gradient {
        Some(_) => fill_paint(proto, color),
        None => color.with_alpha(color.a * 0.85).into(),
    };
    d.fill(shape, paint);
}

/// Where a ridge mark's value sits, in device points: its row's center line less its height.
fn ridge_y(p: &Placed, r: &Resolved) -> Option<f64> {
    p.mark.rise.as_ref()?;
    Some(p.y_band?.0 - p.v1 * r.ridge_unit)
}

/// Draw the ridges ([`Mark::ridge`](crate::Mark::ridge)): row by row from the back, each row's
/// areas and then its lines, every area first hiding what is behind it.
fn ridges(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>) {
    if r.ridge_unit <= 0.0 {
        return;
    }
    let row_of = |p: &Placed| crate::resolve::key_of(&p.mark.y);
    let mut rows: Vec<(f64, String)> = Vec::new();
    for p in r.marks.iter().filter(|p| p.mark.rise.is_some()) {
        if let Some((base, _)) = p.y_band {
            let row = row_of(p);
            if !rows.iter().any(|(_, seen)| *seen == row) {
                rows.push((base, row));
            }
        }
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for (base, row) in &rows {
        for kind in [MarkKind::Area, MarkKind::Line] {
            let members = |si: usize| {
                r.marks.iter().filter(move |p| {
                    p.mark.kind == kind
                        && p.mark.rise.is_some()
                        && p.series_index == si
                        && row_of(p) == *row
                })
            };
            let mut series: Vec<usize> = Vec::new();
            for p in &r.marks {
                if p.mark.kind == kind
                    && p.mark.rise.is_some()
                    && row_of(p) == *row
                    && !series.contains(&p.series_index)
                {
                    series.push(p.series_index);
                }
            }
            for si in series {
                let Some(proto) = members(si).next() else {
                    continue;
                };
                let mut top: Vec<Point> = members(si)
                    .filter_map(|p| {
                        let x = center_x(p, r)? + p.mark.offset.0;
                        let y = ridge_y(p, r)? + p.mark.offset.1;
                        (x.is_finite() && y.is_finite()).then_some(Point::new(x, y))
                    })
                    .collect();
                top.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
                let (Some(first), Some(last)) = (top.first().copied(), top.last().copied()) else {
                    continue;
                };
                if top.len() < 2 {
                    continue;
                }
                let color = color_of(proto, r, paint);
                let interp = proto.mark.style.interpolation;
                if kind == MarkKind::Line {
                    d.stroke_styled(curve(&top, interp, false), color, stroke_style(proto));
                    continue;
                }
                let shape = append(
                    PathBuilder::new().rule(FillRule::NonZero),
                    &top,
                    interp,
                    true,
                )
                .line_to(Point::new(last.x, *base))
                .line_to(Point::new(first.x, *base))
                .close()
                .build();
                match paint.ground {
                    Some(ground) => d.fill(shape.clone(), ground),
                    None => d.blended(BlendMode::Erase, |d| d.fill(shape.clone(), Color::BLACK)),
                }
                fill_area(d, shape, proto, color);
            }
        }
    }
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
                let mx = f64::midpoint(w[0].x, w[1].x);
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
/// tangent, then clamp each tangent into the circle of radius 3 around the neighbouring secants.
/// That clamp is the whole theorem: it is what guarantees the spline cannot overshoot the data,
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
            m[i] = f64::midpoint(secant[i - 1], secant[i]);
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

/// Everything about a point mark's appearance. Two marks agreeing on all of it are
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
        // arithmetic on the same inputs, so equal values are bit-equal, and a degenerate NaN
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
    // On a categorical y (a dot plot, a dumbbell's ends) the point sits on its row.
    let y = match p.y_band {
        Some((center, _)) => center,
        None => r.y.project(&Datum::Number(p.v1))?,
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
    // symbol_size is an area, so the radius is its square root: the encoding only holds if
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

/// A symbol's geometry, authored around the origin: the shapes it fills and the shapes it strokes.
///
/// Around the origin rather than at a point, because that is what a [`Draw::stamp`] template is:
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
                    let a = f64::from(i) * std::f64::consts::PI / 3.0;
                    Shape::Line(
                        Point::new(-r * a.cos(), -r * a.sin()),
                        Point::new(r * a.cos(), r * a.sin()),
                    )
                })
                .collect(),
        ),
    }
}

/// One symbol at every one of `at`: a stamp per part, whatever the count.
fn draw_symbols(d: &mut Draw, sym: Symbol, at: &[Point], r: f64, color: Color, w: f64) {
    if at.is_empty() {
        return;
    }
    let (filled, stroked) = symbol_parts(sym, r);
    for shape in filled {
        d.stamp(shape, at.to_owned(), color);
    }
    for shape in stroked {
        d.stamp_styled(shape, at.to_owned(), color, StrokeStyle::width(w));
    }
}

/// A wedge between two radii, with its own angular span at each of them.
///
/// The two spans differ whenever an angular inset is in force; [`sector`] explains why the inner
/// one is the narrower.
///
/// Built from cubic beziers rather than an arc primitive so the donut hole, the angular inset and
/// the corner radius all compose: an arc op would draw the outer edge and leave the join to the
/// rasterizer's own arc-to-line rule, which differs between backends.
fn wedge(center: Point, r0: f64, r1: f64, outer: (f64, f64), inner: (f64, f64)) -> Shape {
    // Out along the far edge, back along the near one: one closed contour. The geometry here is
    // in radians. A zero inner radius and a zero-width inner span both collapse to a single
    // point, which `arc_to` reaches with a plain line: a pie's apex, or the blunt tip an inset
    // wedge ends in.
    let rad = Angle::rad;
    PathBuilder::new()
        .arc_to(center, r1, rad(outer.0), rad(outer.1 - outer.0))
        .arc_to(center, r0, rad(inner.1), rad(inner.0 - inner.1))
        .close()
        .build()
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
    // of "a pie is a normalized stack"; there is no separate percentage arithmetic.
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
    // The whole radius the plot allows. There was a 0.98 here, which cost the pie 2% of its size
    // against a Swift Charts twin in the same pane for no stated reason; a mark that wants to sit
    // inside the circle says so with `outer_radius`.
    let r_max = coord.radius(plot);
    let hole = match coord {
        Coordinate::Polar { hole, .. } => hole,
        Coordinate::Cartesian => 0.0,
    };
    let outer = r_max * p.mark.outer_radius;
    let inner = (r_max * hole).max(r_max * p.mark.inner_radius);
    let Some(w) = inset_wedge(p.mark.angular_inset, a0, a1, inner, outer) else {
        return;
    };
    let color = color_of(p, r, paint);
    d.fill(
        wedge(center, w.inner, outer, w.outer_span, w.inner_span),
        color,
    );
}

/// The most of its outer radius a wedge will spend on an angular inset. Measured off Swift
/// Charts, which stops widening `angularInset` here (at two plot radii 68pt and 130pt it capped
/// at 4.93% and 4.99%), and the ceiling is worth having on its own: it keeps an inset chosen for
/// a full-page chart from swallowing the same chart in a phone-width pane.
const MAX_INSET_FRACTION: f64 = 0.05;

/// One wedge's geometry once its angular inset is applied.
struct InsetWedge {
    /// The inner radius the inset pushes the wedge out to: its hole, or the blunt tip where
    /// the two inset edges converge, whichever is farther from the center.
    inner: f64,
    /// The angular span the wedge keeps at its outer radius, and at [`InsetWedge::inner`]. The
    /// inner one is the narrower, and collapses to a single angle at a converged tip.
    outer_span: (f64, f64),
    inner_span: (f64, f64),
}

/// Apply an angular inset to one wedge. `None` when the inset has eaten the slice whole.
///
/// The inset is a distance, and it is held perpendicular to the edge, so the channel between two
/// neighbors keeps an even width the whole way in. The angle that distance costs therefore grows as
/// the radius shrinks: an edge held `gap` away from its radial ray sits `asin(gap / r)` off it.
/// Spending a single angle on the whole wedge instead (the outer radius' share, say) tapers the
/// channel shut and lets every slice meet at the center.
fn inset_wedge(gap: f64, a0: f64, a1: f64, inner: f64, outer: f64) -> Option<InsetWedge> {
    let gap = gap.clamp(0.0, outer * MAX_INSET_FRACTION);
    let span = a1 - a0;
    let half = span.abs() / 2.0;
    let sign = if span < 0.0 { -1.0 } else { 1.0 };
    // Where the two inset edges converge: short of the center, and short of the hole when the hole
    // is the smaller. Past a half-circle they never converge, and `gap` itself is as close to the
    // center as either one comes.
    // A zero-width slice divides by `sin(0)` here and comes back infinite, which is the right
    // answer: two coincident edges cross the moment either one is inset, so nothing is left to
    // draw.
    let r_tip = if gap > 0.0 {
        gap / half.min(std::f64::consts::FRAC_PI_2).sin()
    } else {
        0.0
    };
    let r_in = inner.max(r_tip);
    if outer <= r_in {
        // The gap has eaten the slice whole: a sliver too thin to survive its inset.
        return None;
    }
    let inset_at = |r: f64| {
        if gap <= 0.0 {
            0.0
        } else if r > gap {
            (gap / r).asin()
        } else {
            std::f64::consts::FRAC_PI_2
        }
    };
    let (i_out, i_in) = (inset_at(outer), inset_at(r_in));
    Some(InsetWedge {
        inner: r_in,
        outer_span: (a0 + sign * i_out, a1 - sign * i_out),
        inner_span: (a0 + sign * i_in, a1 - sign * i_in),
    })
}

fn annotation(d: &mut Draw, r: &Resolved, paint: &Paint2<'_>, p: &Placed) {
    use crate::mark::AnnotationPosition as AP;
    let Some(a) = &p.mark.annotation else { return };
    if a.text.is_empty() {
        return;
    }
    // A mark's opacity is the whole mark's, its label included: a bar fading out of a
    // transition takes its value with it. A hatched band is the exception, because its
    // opacity thins the hatching and the name over it still has to read.
    let opacity = if p.mark.style.hatch.is_some() {
        1.0
    } else {
        p.mark.style.opacity.clamp(0.0, 1.0)
    };
    if opacity <= 0.01 {
        return;
    }
    let line = day_core::measure_text(&a.text, paint.label_size, &paint.font);
    // A pill is wider and taller than its text by its padding, and stands that much further
    // off the mark.
    let (pad_x, pad_y) = if a.pill { (7.0, 2.5) } else { (0.0, 0.0) };
    let (w, h) = (line.width + 2.0 * pad_x, line.height + 2.0 * pad_y);
    let spanning = p.mark.kind == MarkKind::Rule
        || p.mark.kind == MarkKind::Rectangle && (p.mark.x.is_none() || p.mark.y.is_none());
    let at = if spanning {
        let Some(at) = span_annotation(r, p, a.position, (w, h)) else {
            return;
        };
        at
    } else {
        let Some(x) = center_x(p, r) else { return };
        // On a categorical y the mark IS the band, so the annotation keys off the category, not
        // the stacked value it does not have.
        let y = if r.y.kind.is_discrete() {
            ridge_y(p, r).or(p.y_band.map(|(y, _)| y))
        } else {
            r.y.project(&Datum::Number(p.v1))
        };
        let Some(y) = y else { return };
        // Text laid over a bar or a cell has to fit inside it: a heat map's numbers on a
        // phone-width grid would otherwise spill into their neighbours and read as one smear.
        // Not drawing a label is the right answer there; the color still carries the value.
        if a.position == AP::Overlay && matches!(p.mark.kind, MarkKind::Bar | MarkKind::Rectangle) {
            let width = match (&p.mark.x, &p.mark.x_end) {
                (Some(a), Some(b)) => match (r.x.project(&a.datum), r.x.project(&b.datum)) {
                    (Some(a), Some(b)) => (a - b).abs(),
                    _ => band_of(p, r),
                },
                _ => band_of(p, r),
            };
            if w > width - 2.0 {
                return;
            }
        }
        // A point's label clears its symbol, however large that is drawn.
        let reach = if p.mark.kind == MarkKind::Point {
            (p.mark.style.symbol_size / std::f64::consts::PI).sqrt()
        } else {
            0.0
        };
        match a.position {
            // The automatic position puts the label clear of the mark on the side the value
            // grew toward, which is above for a positive bar and below for a negative one.
            AP::Automatic | AP::Top => Point::new(x, y - line.height * 0.7 - pad_y - reach),
            AP::Bottom => Point::new(x, y + line.height * 0.7 + pad_y + reach),
            AP::Leading => Point::new(x - w / 2.0 - 6.0 - reach, y),
            AP::Trailing => Point::new(x + w / 2.0 + 6.0 + reach, y),
            AP::Overlay => Point::new(x, y),
        }
    };
    let color = a.color.map_or(
        if a.pill {
            paint.chrome.title
        } else {
            paint.chrome.label
        },
        Ink::from,
    );
    if a.pill {
        let capsule = Shape::RoundedRect(Rect::new(at.x - w / 2.0, at.y - h / 2.0, w, h), h / 2.0);
        let surface = paint.chrome.surface;
        d.shadowed(
            Shadow::new(faded(paint.chrome.shadow, opacity), 5.0).offset(0.0, 1.5),
            |d| d.fill(capsule, faded(surface, opacity)),
        );
    }
    d.text(
        &a.text,
        at,
        TextStyle {
            size: paint.label_size,
            color: faded(color, opacity),
            anchor: TextAnchor::CENTERED,
            font: paint.font.clone(),
        },
    );
}

/// Where the annotation of a mark that spans the plot goes: a rule, or a band between two
/// values. `size` is the label's, and the answer is its center.
///
/// A horizontal rule carries its label at an end (`Leading`, `Trailing`), sitting on the line
/// just inside the plot, or over or under its middle. A vertical rule or a band between two x
/// values carries it at the top or the bottom, inside the plot, or beside itself.
fn span_annotation(
    r: &Resolved,
    p: &Placed,
    position: crate::mark::AnnotationPosition,
    (w, h): (f64, f64),
) -> Option<Point> {
    use crate::mark::AnnotationPosition as AP;
    let plot = r.plot;
    let (left, right) = (plot.origin.x, plot.origin.x + plot.size.width);
    let (top, bottom) = (plot.origin.y, plot.origin.y + plot.size.height);
    let project = |scale: &crate::scale::Scale, v: &Option<crate::data::Value>, or: f64| {
        v.as_ref()
            .and_then(|v| scale.project(&v.datum))
            .unwrap_or(or)
    };
    // The extent the mark covers along each axis: a rule's is a line, a band's a strip.
    let vertical = p.mark.x.is_some() && (p.mark.kind != MarkKind::Rule || p.mark.x_end.is_none());
    let (x0, x1, y0, y1) = if vertical && p.mark.kind == MarkKind::Rule {
        let x = project(&r.x, &p.mark.x, f64::NAN);
        let (a, b) = match (&p.mark.y, &p.mark.y_end) {
            (Some(_), Some(_)) => (
                project(&r.y, &p.mark.y, top),
                project(&r.y, &p.mark.y_end, bottom),
            ),
            _ => (top, bottom),
        };
        (x, x, a.min(b), a.max(b))
    } else if p.mark.kind == MarkKind::Rule {
        let y = project(&r.y, &p.mark.y, f64::NAN);
        let (a, b) = match (&p.mark.x, &p.mark.x_end) {
            (Some(_), Some(_)) => (
                project(&r.x, &p.mark.x, left),
                project(&r.x, &p.mark.x_end, right),
            ),
            _ => (left, right),
        };
        (a.min(b), a.max(b), y, y)
    } else if p.mark.y.is_none() {
        let (a, b) = (
            project(&r.x, &p.mark.x, left),
            project(&r.x, &p.mark.x_end, right),
        );
        (a.min(b), a.max(b), top, bottom)
    } else {
        let (a, b) = (
            project(&r.y, &p.mark.y, top),
            project(&r.y, &p.mark.y_end, bottom),
        );
        (left, right, a.min(b), a.max(b))
    };
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) {
        return None;
    }
    let (mid_x, mid_y) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    let (gap, edge) = (4.0, 3.0);
    let at = if p.mark.kind == MarkKind::Rule && !vertical {
        // A horizontal rule.
        match position {
            AP::Trailing => Point::new(x1 - w / 2.0 - edge, mid_y),
            AP::Leading => Point::new(x0 + w / 2.0 + edge, mid_y),
            AP::Automatic | AP::Top => Point::new(mid_x, mid_y - h / 2.0 - gap),
            AP::Bottom => Point::new(mid_x, mid_y + h / 2.0 + gap),
            AP::Overlay => Point::new(mid_x, mid_y),
        }
    } else {
        // A vertical rule, or a band: `x0..x1` is zero wide for the rule.
        match position {
            AP::Automatic | AP::Top => Point::new(mid_x, y0 + h / 2.0 + edge),
            AP::Bottom => Point::new(mid_x, y1 - h / 2.0 - edge),
            AP::Leading => Point::new(x0 - w / 2.0 - gap, y0 + h / 2.0 + edge),
            AP::Trailing => Point::new(x1 + w / 2.0 + gap, y0 + h / 2.0 + edge),
            AP::Overlay => Point::new(mid_x, mid_y),
        }
    };
    // Kept on the plot sideways, so a label at the first or last sample is whole.
    let half = w / 2.0;
    Some(Point::new(
        at.x.clamp(left + half, (right - half).max(left + half)),
        at.y,
    ))
}

// ---------------------------------------------------------------------------
// Selection: the hit model a draw leaves behind, and the guides drawn for one
// ---------------------------------------------------------------------------

/// Every mark's device position and labels, for the pointer to hit
/// (README.md "Selection").
///
/// Built from the same `Resolved` the marks were just drawn from, so a selection can only ever
/// name something on screen, and a pointer move costs a scan of this rather than a re-resolve.
pub fn hit_model(r: &Resolved, paint: &Paint2<'_>) -> crate::select::HitModel {
    let polar = r.coordinate.is_polar();
    // The axis's tick step decides how many decimals a value reads with, so a selected value
    // is rounded exactly like the axis label above it: "58.2", not "58.15339133". The exact
    // number is still on `SelectedValue::value` for an app that wants full precision.
    let step_of = |ticks: &[crate::ticks::Tick]| match ticks {
        [a, b, ..] => (b.value - a.value).abs(),
        _ => 0.0,
    };
    let (x_step, y_step) = (step_of(&r.x_ticks), step_of(&r.y_ticks));
    // A ridge has no axis to borrow a precision from, so its values read to three figures of
    // the tallest one.
    let tallest_ridge = r
        .marks
        .iter()
        .filter(|p| p.mark.rise.is_some())
        .map(|p| p.v1.abs())
        .fold(0.0_f64, f64::max);
    let ridge_step = if tallest_ridge > 0.0 {
        10f64.powf(tallest_ridge.log10().floor() - 2.0)
    } else {
        0.0
    };
    let marks = r
        .marks
        .iter()
        .filter_map(|p| {
            if p.mark.kind == MarkKind::Sector
                || polar && matches!(p.mark.kind, MarkKind::Bar | MarkKind::Rectangle)
            {
                let total: f64 = r
                    .marks
                    .iter()
                    .filter(|q| q.mark.kind == p.mark.kind)
                    .map(|q| (q.v1 - q.v0).abs())
                    .sum();
                if total <= 0.0 {
                    return None;
                }
                let coord = if polar {
                    r.coordinate
                } else {
                    Coordinate::polar()
                };
                let center = coord.center(r.plot);
                let radius = coord.radius(r.plot);
                let hole = match coord {
                    Coordinate::Polar { hole, .. } => hole,
                    Coordinate::Cartesian => 0.0,
                };
                let outer = radius * p.mark.outer_radius;
                let inner = (radius * hole).max(radius * p.mark.inner_radius);
                let start = coord.angle(p.v0 / total);
                let end = coord.angle(p.v1 / total);
                let w = inset_wedge(p.mark.angular_inset, start, end, inner, outer)?;
                let mid = f64::midpoint(start, end);
                let rr = f64::midpoint(outer, w.inner);
                let at = Point::new(center.x + rr * mid.cos(), center.y + rr * mid.sin());
                let number = p.mark.y.as_ref()?.datum.as_continuous()?;
                return Some(crate::select::HitMark {
                    legend: false,
                    record: crate::interaction::Record::from_mark(&p.mark),
                    link: p.mark.link.clone(),
                    region: crate::select::HitRegion::Sector {
                        center,
                        inner: w.inner,
                        outer,
                        start,
                        end,
                        gap: p.mark.angular_inset.clamp(0.0, outer * MAX_INSET_FRACTION),
                    },
                    at,
                    color: color_of(p, r, paint),
                    series: r.series.get(p.series_index).cloned().unwrap_or_default(),
                    x_label: p
                        .mark
                        .series
                        .as_ref()
                        .map_or_default(|v| v.datum.to_string()),
                    y_label: p.mark.selection_value.as_ref().map_or_else(
                        || crate::resolve::label_for(number, &r.y, paint.y_axis, y_step),
                        |v| v.label.clone(),
                    ),
                    value: number,
                });
            }
            // A band across the plot marks a range of an axis; it is not a datum to select.
            if p.mark.kind == MarkKind::Rectangle && (p.mark.x.is_none() || p.mark.y.is_none()) {
                return None;
            }
            let ridge = ridge_y(p, r);
            let horizontal = p.y_band.is_some() && p.mark.x_end.is_some() && ridge.is_none();
            let x = if horizontal {
                r.x.project(&p.mark.x_end.as_ref()?.datum)?
            } else {
                center_x(p, r)?
            };
            let y = if let Some(y) = ridge {
                y
            } else if let Some((center, _)) = p.y_band {
                center
            } else {
                r.y.project(&Datum::Number(p.v1))?
            };
            let (position, label, measured) = if ridge.is_some() {
                // A ridge's value is its height; the y scale is its rows.
                (
                    x_label_of(p, r, paint, x_step),
                    crate::ticks::format_value(p.v1, ridge_step),
                    p.v1,
                )
            } else if horizontal {
                (
                    p.mark
                        .y
                        .as_ref()
                        .and_then(|v| match &v.datum {
                            Datum::Category(c) => Some(c.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    crate::resolve::label_for(
                        p.mark.x_end.as_ref()?.datum.as_continuous()?,
                        &r.x,
                        paint.x_axis,
                        x_step,
                    ),
                    p.mark.x_end.as_ref()?.datum.as_continuous()?,
                )
            } else {
                (
                    x_label_of(p, r, paint, x_step),
                    crate::resolve::label_for(p.v1, &r.y, paint.y_axis, y_step),
                    p.v1,
                )
            };
            let mut series = r.series.get(p.series_index).cloned().unwrap_or_default();
            if ridge.is_some()
                && series.is_empty()
                && let Some(row) = p.mark.y.as_ref().and_then(|v| v.datum.as_category())
            {
                // With no color channel a ridge is named by its row.
                series = row.to_string();
            }
            let at = device(r, x + p.mark.offset.0, y + p.mark.offset.1, polar);
            let region = if !polar && matches!(p.mark.kind, MarkKind::Bar | MarkKind::Rectangle) {
                let (left, right) = if let Some(end) = &p.mark.x_end {
                    let a = r.x.project(&p.mark.x.as_ref()?.datum)?;
                    let b = r.x.project(&end.datum)?;
                    (a.min(b), a.max(b))
                } else {
                    let cx = center_x(p, r)?;
                    let w = band_of(p, r);
                    (cx - w / 2.0, cx + w / 2.0)
                };
                let (top, bottom) = if let Some((cy, h)) = p.y_band {
                    (cy - h / 2.0, cy + h / 2.0)
                } else {
                    let a = r.y.clamp_to_range(r.y.project(&Datum::Number(p.v0))?);
                    let b = r.y.clamp_to_range(r.y.project(&Datum::Number(p.v1))?);
                    (a.min(b), a.max(b))
                };
                crate::select::HitRegion::Rect(Rect::new(
                    left + p.mark.offset.0,
                    top + p.mark.offset.1,
                    right - left,
                    bottom - top,
                ))
            } else {
                crate::select::HitRegion::Point(
                    at,
                    (p.mark.style.symbol_size / std::f64::consts::PI)
                        .sqrt()
                        .max(6.0),
                )
            };
            Some(crate::select::HitMark {
                legend: false,
                record: crate::interaction::Record::from_mark(&p.mark),
                link: p.mark.link.clone(),
                region,
                at: device(r, x + p.mark.offset.0, y + p.mark.offset.1, polar),
                color: color_of(p, r, paint),
                x_label: position,
                y_label: p
                    .mark
                    .selection_value
                    .as_ref()
                    .map_or(label, |v| v.label.clone()),
                value: p
                    .mark
                    .selection_value
                    .as_ref()
                    .and_then(|v| v.datum.as_continuous())
                    .unwrap_or(measured),
                series,
            })
        })
        .collect();
    crate::select::HitModel {
        x_scale: Some(r.x.clone()),
        y_scale: Some(r.y.clone()),
        plot: r.plot,
        marks,
    }
}

/// A mark's x as a label: its category on a discrete axis, its value formatted by the axis's
/// formatter otherwise, so the guide's label reads exactly like the axis under it.
fn x_label_of(p: &Placed, r: &Resolved, paint: &Paint2<'_>, step: f64) -> String {
    match p.mark.x.as_ref().map(|v| &v.datum) {
        // A category the axis's formatter left blank (an axis that numbers every fourth week)
        // is still named when it is selected.
        Some(Datum::Category(c)) => paint
            .x_axis
            .format
            .as_ref()
            .map(|f| f(&Datum::Category(c.clone())))
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| c.clone()),
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
    // A position with no label of its own (an axis formatted to number only some of its
    // categories) takes no line.
    let mut lines: Vec<(String, Ink)> = Vec::new();
    if !sel.x_label.is_empty() {
        lines.push((sel.x_label.clone(), paint.chrome.title));
    }
    for v in &sel.values {
        let text = if v.series.is_empty() {
            v.label.clone()
        } else {
            format!("{}  {}", v.series, v.label)
        };
        lines.push((text, v.color.into()));
    }
    let line_h = day_core::measure_text("0", size, &paint.font).height;
    let w = lines
        .iter()
        .map(|(t, _)| day_core::measure_text(t, size, &paint.font).width)
        .fold(0.0f64, f64::max);
    let pad = 6.0;
    let (bw, bh) = (w + pad * 2.0, line_h * lines.len() as f64 + pad * 2.0);
    // Right of the rule where it fits, left otherwise, then clamped, so an extreme selection
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
    // A raised box: the chrome's surface with a soft shadow under it and a hairline around it,
    // so it separates from the marks it covers on a light ground and on a dark one.
    let frame = Shape::RoundedRect(Rect::new(bx, by, bw, bh), 6.0);
    d.shadowed(
        Shadow::new(paint.chrome.shadow, 10.0).offset(0.0, 3.0),
        |d| d.fill(frame.clone(), paint.chrome.surface),
    );
    d.stroke(frame, paint.chrome.grid_line, 1.0);
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

#[cfg(test)]
mod inset_tests {
    use super::inset_wedge;
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    /// How far the edge at `angle` sits from the ray at `ray`, measured perpendicular to the ray,
    /// at radius `r`. This is the distance the inset is supposed to hold constant.
    fn offset(r: f64, angle: f64, ray: f64) -> f64 {
        r * (angle - ray).sin()
    }

    #[test]
    fn no_inset_leaves_the_wedge_alone() {
        let w = inset_wedge(0.0, 0.0, 1.0, 30.0, 100.0).unwrap();
        assert_eq!(w.inner, 30.0);
        assert_eq!(w.outer_span, (0.0, 1.0));
        assert_eq!(w.inner_span, (0.0, 1.0));
    }

    #[test]
    fn the_channel_between_neighbors_keeps_its_width_at_every_radius() {
        // Two adjacent slices of a five-slice donut, the arrangement from the bug report. The gap
        // is kept under 5% of the outer radius so the cap does not enter into it.
        let (gap, step) = (4.0, TAU / 5.0);
        let a = inset_wedge(gap, 0.0, step, 40.0, 120.0).unwrap();
        let b = inset_wedge(gap, step, 2.0 * step, 40.0, 120.0).unwrap();
        assert_eq!(a.inner, 40.0, "a 40pt hole is wider than this inset's tip");
        assert_eq!(b.inner, 40.0);
        // Each edge sits `gap` from its ray, outside and inside alike, so the channel between
        // the two slices is `2 * gap` wide at both radii, and at every radius between them.
        let pairs = [
            (120.0, a.outer_span.1, b.outer_span.0),
            (a.inner, a.inner_span.1, b.inner_span.0),
        ];
        for (r, near, far) in pairs {
            assert!(
                (offset(r, near, step).abs() - gap).abs() < 1e-9,
                "{r}: {near}"
            );
            assert!(
                (offset(r, far, step).abs() - gap).abs() < 1e-9,
                "{r}: {far}"
            );
        }
    }

    #[test]
    fn a_pie_with_a_gap_stops_short_of_the_center() {
        // No hole at all: the old code let every slice converge on the center, closing the gap.
        let (gap, step) = (4.0, TAU / 5.0);
        let w = inset_wedge(gap, 0.0, step, 0.0, 120.0).unwrap();
        let expected = gap / (step / 2.0).sin();
        assert!((w.inner - expected).abs() < 1e-9, "tip radius {}", w.inner);
        assert!(
            w.inner > gap,
            "the tip clears the center by at least the gap"
        );
        // The two inset edges have met, so the wedge ends in a point rather than an inner arc.
        let inn = w.inner_span;
        assert!((inn.0 - inn.1).abs() < 1e-9, "{inn:?}");
    }

    #[test]
    fn a_counter_clockwise_sweep_insets_inward_too() {
        let out = inset_wedge(4.0, 1.0, 0.0, 30.0, 120.0).unwrap().outer_span;
        assert!(out.0 < 1.0 && out.1 > 0.0, "{out:?}");
        assert!(out.0 > out.1, "the span keeps its direction: {out:?}");
    }

    #[test]
    fn a_sliver_too_thin_for_its_own_inset_is_dropped() {
        assert!(inset_wedge(4.0, 0.0, 0.02, 0.0, 120.0).is_none());
        assert!(inset_wedge(4.0, 0.0, 0.0, 0.0, 120.0).is_none());
        // Even clamped to 5% of the radius, an inset can outgrow a thin enough slice.
        assert!(inset_wedge(200.0, 0.0, 0.1, 0.0, 120.0).is_none());
    }

    #[test]
    fn the_inset_is_capped_at_a_share_of_the_radius() {
        // Swift Charts stops widening `angularInset` at 5% of the outer radius; past that, asking
        // for more gap changes nothing. Below it the number is spent in full.
        let below = inset_wedge(4.0, 0.0, 1.0, 0.0, 120.0).unwrap();
        assert!((offset(120.0, below.outer_span.0, 0.0) - 4.0).abs() < 1e-9);
        for asked in [6.0, 8.0, 40.0] {
            let w = inset_wedge(asked, 0.0, 1.0, 0.0, 120.0).unwrap();
            let spent = offset(120.0, w.outer_span.0, 0.0);
            assert!((spent - 6.0).abs() < 1e-9, "asked {asked}, spent {spent}");
        }
        // The cap follows the radius, so a pane half the size gets half the gap.
        let small = inset_wedge(8.0, 0.0, 1.0, 0.0, 60.0).unwrap();
        assert!((offset(60.0, small.outer_span.0, 0.0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_wedge_past_a_half_circle_never_converges() {
        // `asin` tops out at a right angle, so edges this far apart stay apart; the closest either
        // comes to the center is the gap itself.
        let w = inset_wedge(4.0, 0.0, PI * 1.5, 0.0, 120.0).unwrap();
        assert!((w.inner - 4.0).abs() < 1e-9, "{}", w.inner);
        let inn = w.inner_span;
        assert!((inn.0 - FRAC_PI_2).abs() < 1e-9, "{inn:?}");
    }
}

#[cfg(test)]
mod corner_tests {
    //! Bars round only the end they grew to (README "What it does carefully").

    use super::*;
    use crate::data::value;
    use crate::mark::{Mark, Stacking, bar, rect};
    use crate::resolve::{Config, resolve};
    use crate::scale::ScaleSpec;
    use day_spec::{DrawOp, PathSeg, Size};

    const MARKED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Draw `marks` and return the shapes filled in `color`.
    fn filled(marks: Vec<Mark>, color: Color) -> Vec<Shape> {
        let (xs, ys) = (ScaleSpec::default(), ScaleSpec::default());
        let (xa, ya) = (AxisSpec::default(), AxisSpec::default());
        let colors = |i: usize, _: &str| crate::style::categorical(i);
        let cfg = Config {
            x_scale: &xs,
            y_scale: &ys,
            x_axis: &xa,
            y_axis: &ya,
            coordinate: crate::coord::Coordinate::Cartesian,
            series_colors: &colors,
            label_size: 11.0,
            font: day_spec::CanvasFont::default(),
            legend_insets: crate::layout::Insets::default(),
            plot_insets: Some(crate::layout::Insets::default()),
            ridge_overlap: 1.5,
        };
        let resolved = resolve(marks, Size::new(400.0, 300.0), &cfg);
        let chrome = Chrome::for_dark(false);
        let paint = Paint2 {
            chrome: &chrome,
            label_size: 11.0,
            font: day_spec::CanvasFont::default(),
            x_axis: &xa,
            y_axis: &ya,
            series_colors: &colors,
            ground: None,
        };
        let mut d = Draw::new();
        draw(&mut d, &resolved, &paint);
        d.ops()
            .iter()
            .filter_map(|op| match op {
                DrawOp::Fill(shape, Paint::Solid(c)) if *c == color.into() => Some(shape.clone()),
                _ => None,
            })
            .collect()
    }

    /// Which corners of a filled rectangle are rounded: top left, top right, bottom right,
    /// bottom left.
    fn rounded(shape: &Shape) -> [bool; 4] {
        match shape {
            Shape::Rect(_) => [false; 4],
            Shape::RoundedRect(_, r) => [*r > 0.0; 4],
            Shape::Path(p) => {
                let b = p.bounds();
                let (x0, y0) = (b.origin.x, b.origin.y);
                let (x1, y1) = (x0 + b.size.width, y0 + b.size.height);
                let pts: Vec<Point> = p
                    .segs
                    .iter()
                    .filter_map(|s| match s {
                        PathSeg::Move(q) | PathSeg::Line(q) | PathSeg::Cubic(_, _, q) => Some(*q),
                        _ => None,
                    })
                    .collect();
                let sharp = |x: f64, y: f64| {
                    pts.iter()
                        .any(|q| (q.x - x).abs() < 1e-9 && (q.y - y).abs() < 1e-9)
                };
                [
                    !sharp(x0, y0),
                    !sharp(x1, y0),
                    !sharp(x1, y1),
                    !sharp(x0, y1),
                ]
            }
            other => panic!("not a rectangle: {other:?}"),
        }
    }

    fn one(marks: Vec<Mark>) -> [bool; 4] {
        let shapes = filled(marks, MARKED);
        assert_eq!(shapes.len(), 1, "{shapes:?}");
        rounded(&shapes[0])
    }

    #[test]
    fn a_bar_is_rounded_on_top_and_square_on_its_axis() {
        let got = one(vec![
            bar(value("Month", "Jan"), value("Revenue", 10.0))
                .corner_radius(4.0)
                .foreground(MARKED),
        ]);
        assert_eq!(got, [true, true, false, false]);
    }

    #[test]
    fn a_loss_hangs_from_its_axis_rounded_at_the_bottom() {
        let got = one(vec![
            bar(value("Month", "Jan"), value("Revenue", 10.0)),
            bar(value("Month", "Feb"), value("Revenue", -10.0))
                .corner_radius(4.0)
                .foreground(MARKED),
        ]);
        assert_eq!(got, [false, false, true, true]);
    }

    #[test]
    fn a_stack_is_rounded_only_at_its_outer_end() {
        let (bottom, top) = (MARKED, Color::rgb(0.0, 1.0, 1.0));
        let marks = vec![
            bar(value("Month", "Jan"), value("Revenue", 10.0))
                .by_series(value("Region", "North"))
                .corner_radius(4.0)
                .foreground(bottom),
            bar(value("Month", "Jan"), value("Revenue", 6.0))
                .by_series(value("Region", "South"))
                .corner_radius(4.0)
                .foreground(top),
        ];
        let lower = filled(marks.clone(), bottom);
        let upper = filled(marks, top);
        assert_eq!(rounded(&lower[0]), [false; 4], "the segment on the axis");
        assert_eq!(
            rounded(&upper[0]),
            [true, true, false, false],
            "the stack's top"
        );
    }

    #[test]
    fn a_horizontal_bar_is_rounded_at_its_far_end() {
        let got = one(vec![
            bar(value("Share", 5.0), value("Language", "Rust"))
                .x_range(value("Share", 0.0), value("Share", 5.0))
                .corner_radius(4.0)
                .foreground(MARKED),
        ]);
        assert_eq!(got, [false, true, true, false]);
    }

    #[test]
    fn a_floating_range_and_a_heat_map_cell_keep_every_corner() {
        let range = one(vec![
            bar(value("Day", "Mon"), value("Price", 3.0))
                .y_range(value("Price", 3.0), value("Price", 8.0))
                .stacking(Stacking::Unstacked)
                .corner_radius(4.0)
                .foreground(MARKED),
        ]);
        assert_eq!(range, [true; 4]);
        let cell = one(vec![
            rect(value("Hour", "09"), value("Day", "Mon"))
                .corner_radius(4.0)
                .foreground(MARKED),
        ]);
        assert_eq!(cell, [true; 4]);
    }
}

#[cfg(test)]
mod label_collision_tests {
    use super::reserve_label;

    #[test]
    fn dense_ticks_leave_separated_labels_in_both_axis_directions() {
        for direction in [-1.0, 1.0] {
            let mut occupied = Vec::new();
            let accepted = (0..7)
                .filter(|i| {
                    reserve_label(&mut occupied, direction * f64::from(*i) * 10.0, 16.0, 4.0)
                })
                .count();
            assert_eq!(accepted, 4);
            for (i, &(a, b)) in occupied.iter().enumerate() {
                for &(c, d) in &occupied[i + 1..] {
                    assert!(b <= c || d <= a);
                }
            }
        }
    }

    #[test]
    fn out_of_order_ticks_do_not_overlap_an_earlier_label() {
        let mut occupied = Vec::new();
        assert!(reserve_label(&mut occupied, 0.0, 20.0, 4.0));
        assert!(reserve_label(&mut occupied, 100.0, 20.0, 4.0));
        assert!(!reserve_label(&mut occupied, 10.0, 20.0, 4.0));
        assert!(reserve_label(&mut occupied, 50.0, 20.0, 4.0));
        assert_eq!(occupied.len(), 3);
    }
}
