// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! From a bag of marks to everything a renderer needs: the position adjustments, the scales, the
//! ticks, and the plot rectangle.
//!
//! This is the whole pipeline, and it is deliberately pure — no canvas, no side effects, one
//! function from (marks, configuration, size) to a [`Resolved`]. That is what lets the interesting
//! parts be unit-tested headlessly: stacking arithmetic, domain inference, and the measured layout
//! are all decided here, and [`crate::render`] only draws what this produced.
//!
//! The order matters and follows the grammar: **variables → position adjustment (stacking, dodging)
//! → scales → guides → coordinates.** Stacking happens in DATA space before any scale exists,
//! which is the only order that works: a stacked domain is the domain of the sums, not of the
//! values, and inferring the scale first would clip every stack at the tallest single contribution.

use std::collections::BTreeMap;

use day_spec::{Color, Rect, Size};

use crate::axis::{AxisPosition, AxisSpec};
use crate::coord::Coordinate;
use crate::data::{Datum, Interval};
use crate::layout::Insets;
use crate::mark::{Mark, MarkKind, Stacking};
use crate::scale::{Scale, ScaleKind, ScaleSpec, infer};
use crate::ticks::{self, LabelFit, Tick};

/// One mark with its position adjustment applied.
#[derive(Clone, Debug)]
pub struct Placed {
    pub mark: Mark,
    /// The mark's extent along the value axis, in DATA space, after stacking. For an unstacked
    /// mark these are the baseline and the value.
    pub v0: f64,
    pub v1: f64,
    /// Index into [`Resolved::series`], which is what picks the color.
    pub series_index: usize,
    /// Position within the dodging group, and how many there are.
    pub dodge_index: usize,
    pub dodge_count: usize,
    /// Whether this mark sits at the low and high ends of its own stack. Both are true for
    /// anything that did not stack, so an unstacked bar or a heat-map cell is its own two ends.
    /// A bar rounds only the corners at an end that is true, which is what lets a stack read as
    /// one bar rather than a column of separate lozenges.
    pub stack_lo: bool,
    pub stack_hi: bool,
}

/// Everything the renderer needs.
pub struct Resolved {
    pub plot: Rect,
    pub x: Scale,
    pub y: Scale,
    /// The color-channel domain, in first-seen order.
    pub series: Vec<String>,
    pub x_ticks: Vec<Tick>,
    pub y_ticks: Vec<Tick>,
    pub marks: Vec<Placed>,
    pub coordinate: Coordinate,
    /// The legend's entries, empty when there is nothing to explain.
    pub legend: Vec<(String, Color)>,
    /// Titles resolved from the data columns when the app named none.
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    /// The smallest gap between two distinct bar positions on a CONTINUOUS x axis, in data units
    /// — what decides how wide a bar on a time axis may be before it overlaps its neighbour.
    /// `None` when there is no such pair.
    pub x_gap: Option<f64>,
}

/// What the chart was configured with, gathered so the pipeline takes one argument.
pub struct Config<'a> {
    pub x_scale: &'a ScaleSpec,
    pub y_scale: &'a ScaleSpec,
    pub x_axis: &'a AxisSpec,
    pub y_axis: &'a AxisSpec,
    pub coordinate: Coordinate,
    pub series_colors: &'a dyn Fn(usize, &str) -> Color,
    pub label_size: f64,
    pub font: day_spec::CanvasFont,
    /// Space the legend has already claimed, so the plot does not draw under it.
    pub legend_insets: Insets,
    /// Fixed margins in place of the measured ones (`Chart::plot_insets`). The legend's space is
    /// added on top either way.
    pub plot_insets: Option<Insets>,
}

/// A stable key for "the same x position", used to group marks for stacking and dodging.
fn key_of(v: &Option<crate::data::Value>) -> String {
    match v {
        Some(v) => match &v.datum {
            Datum::Category(s) => s.clone(),
            Datum::Number(n) | Datum::Time(n) => format!("{n:?}"),
        },
        None => String::new(),
    }
}

/// Whether this mark kind participates in stacking at all.
fn stackable(kind: MarkKind) -> bool {
    matches!(kind, MarkKind::Bar | MarkKind::Area | MarkKind::Sector)
}

/// Apply the position adjustments: stacking along the value axis, dodging across the band.
///
/// Both are computed in data space and in declaration order. Declaration order is the contract —
/// the first mark of a series is the bottom of every stack — because any other rule (sorted by
/// value, say) would make a stack reorder itself as the data changed, which no reader can follow.
fn place(marks: Vec<Mark>, series: &[String]) -> Vec<Placed> {
    // Running totals per (kind, x-key), positive and negative accumulated separately so a series
    // with mixed signs stacks away from the baseline in both directions instead of cancelling.
    let mut pos_top: BTreeMap<(u8, String), f64> = BTreeMap::new();
    let mut neg_top: BTreeMap<(u8, String), f64> = BTreeMap::new();
    // The dodging groups, discovered in order.
    let mut dodge_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in &marks {
        if let Some(d) = &m.dodge {
            let g = dodge_groups.entry(key_of(&m.x)).or_default();
            let k = d.datum.to_string();
            if !g.contains(&k) {
                g.push(k);
            }
        }
    }
    // Every dodging group is given the SAME number of slots — the union across the chart — so a
    // category missing one series leaves a gap where that series would be rather than widening its
    // neighbours. Bars that change width by which data happened to arrive are unreadable.
    let mut all_dodge: Vec<String> = Vec::new();
    for g in dodge_groups.values() {
        for k in g {
            if !all_dodge.contains(k) {
                all_dodge.push(k.clone());
            }
        }
    }

    let mut out = Vec::with_capacity(marks.len());
    for m in marks {
        let series_index = m
            .series
            .as_ref()
            .and_then(|s| series.iter().position(|c| *c == s.datum.to_string()))
            .unwrap_or(0);
        let (dodge_index, dodge_count) = match &m.dodge {
            Some(d) => (
                all_dodge
                    .iter()
                    .position(|k| *k == d.datum.to_string())
                    .unwrap_or(0),
                all_dodge.len().max(1),
            ),
            None => (0, 1),
        };

        // The value channel: y for a vertical mark, and the whole quantity for a sector.
        let value =
            m.y.as_ref()
                .and_then(|v| v.datum.as_continuous())
                .unwrap_or(0.0);
        let explicit_end = m.y_end.as_ref().and_then(|v| v.datum.as_continuous());

        let (v0, v1) = if let Some(end) = explicit_end {
            // An explicit range opts out of stacking entirely: the app said where both edges go.
            (value, end)
        } else if stackable(m.kind) && m.stacking != Stacking::Unstacked && value.is_finite() {
            let key = (m.kind as u8, key_of(&m.x));
            if value >= 0.0 {
                let base = *pos_top.get(&key).unwrap_or(&0.0);
                pos_top.insert(key, base + value);
                (base, base + value)
            } else {
                let base = *neg_top.get(&key).unwrap_or(&0.0);
                neg_top.insert(key, base + value);
                (base, base + value)
            }
        } else {
            (0.0, value)
        };

        out.push(Placed {
            mark: m,
            v0,
            v1,
            series_index,
            dodge_index,
            dodge_count,
            stack_lo: true,
            stack_hi: true,
        });
    }

    // Normalized and centred stacking are second passes, because both need the group's total,
    // which is only known once every mark in it has been seen.
    let normalized: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, p)| p.mark.stacking == Stacking::Normalized && stackable(p.mark.kind))
        .map(|(i, _)| i)
        .collect();
    if !normalized.is_empty() {
        for i in normalized {
            let key = (out[i].mark.kind as u8, key_of(&out[i].mark.x));
            let total = pos_top.get(&key).copied().unwrap_or(0.0)
                - neg_top.get(&key).copied().unwrap_or(0.0);
            if total.abs() > f64::EPSILON {
                out[i].v0 /= total;
                out[i].v1 /= total;
            }
        }
    }
    let centred: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, p)| p.mark.stacking == Stacking::Center && stackable(p.mark.kind))
        .map(|(i, _)| i)
        .collect();
    for i in centred {
        let key = (out[i].mark.kind as u8, key_of(&out[i].mark.x));
        let total = pos_top.get(&key).copied().unwrap_or(0.0);
        out[i].v0 -= total / 2.0;
        out[i].v1 -= total / 2.0;
    }

    // Now that every segment's extent is final, find the ends of each stack. Only marks that
    // actually stacked take part: an unstacked bar, an explicit range and a heat-map cell each
    // keep both ends, which is what the `true` they were pushed with already says. A dodged slot
    // is its own column, so the group key carries the slot index.
    let mut ends: BTreeMap<(u8, String, usize), (usize, usize)> = BTreeMap::new();
    for i in 0..out.len() {
        let p = &out[i];
        if !stackable(p.mark.kind)
            || p.mark.stacking == Stacking::Unstacked
            || p.mark.y_end.is_some()
        {
            continue;
        }
        let key = (p.mark.kind as u8, key_of(&p.mark.x), p.dodge_index);
        let (v_lo, v_hi) = (p.v0.min(p.v1), p.v0.max(p.v1));
        match ends.get(&key).copied() {
            None => {
                ends.insert(key, (i, i));
            }
            Some((lo, hi)) => {
                let low = if v_lo < out[lo].v0.min(out[lo].v1) {
                    i
                } else {
                    lo
                };
                let high = if v_hi > out[hi].v0.max(out[hi].v1) {
                    i
                } else {
                    hi
                };
                ends.insert(key, (low, high));
                // Whichever of the three is neither end any more is interior, and stays interior:
                // an end only ever moves outward, so nothing here can be reinstated later.
                for j in [lo, hi, i] {
                    if j != low && j != high {
                        out[j].stack_lo = false;
                        out[j].stack_hi = false;
                    }
                }
            }
        }
    }
    // Each winner is an end on ONE side, unless its stack turned out to be a single segment.
    for (lo, hi) in ends.into_values() {
        if lo != hi {
            out[lo].stack_hi = false;
            out[hi].stack_lo = false;
        }
    }
    out
}

/// The color-channel domain: every distinct series value, in first-seen order.
fn series_domain(marks: &[Mark]) -> Vec<String> {
    let mut out = Vec::new();
    for m in marks {
        if let Some(s) = &m.series {
            let k = s.datum.to_string();
            if !out.contains(&k) {
                out.push(k);
            }
        }
    }
    out
}

/// Run the whole pipeline.
pub fn resolve(marks: Vec<Mark>, size: Size, cfg: &Config<'_>) -> Resolved {
    let series = series_domain(&marks);
    let x_title = marks
        .iter()
        .find_map(|m| m.x.as_ref().map(|v| v.label.clone()));
    let y_title = marks
        .iter()
        .find_map(|m| m.y.as_ref().map(|v| v.label.clone()));
    let placed = place(marks, &series);

    // The columns each scale sees. The value axis sees the STACKED bounds, not the raw values —
    // see the module note.
    let x_data: Vec<Datum> = placed
        .iter()
        .flat_map(|p| {
            p.mark
                .x
                .iter()
                .chain(p.mark.x_end.iter())
                .map(|v| v.datum.clone())
                .collect::<Vec<_>>()
        })
        .filter(|d| d.is_finite())
        .collect();
    let mut y_data: Vec<Datum> = Vec::new();
    for p in &placed {
        if p.mark.kind == MarkKind::Sector {
            continue; // a sector's quantity is angular; it never sizes the y axis
        }
        // A categorical y — the rows of a heat map — is a band like a categorical x: the
        // category itself is the datum, and the stacked bounds (which are zero for it) must not
        // reach the scale, or the inferred band would gain a phantom "0" row.
        if let Some(c) = p.mark.y.as_ref().and_then(|v| v.datum.as_category()) {
            y_data.push(Datum::Category(c.to_string()));
            continue;
        }
        // A mark that measures from a baseline — a bar, an area, anything with an explicit y
        // span — sizes the axis with both of its edges. A line or a point has no baseline: its
        // `v0` is a placeholder zero, and letting it reach the scale would pin every line chart
        // to a zero-based axis whatever the data does.
        let has_baseline = stackable(p.mark.kind) || p.mark.y_end.is_some();
        if has_baseline && p.v0.is_finite() {
            y_data.push(Datum::Number(p.v0));
        }
        if p.v1.is_finite() {
            y_data.push(Datum::Number(p.v1));
        }
    }
    let x_gap = smallest_gap(
        placed
            .iter()
            .filter(|p| matches!(p.mark.kind, MarkKind::Bar | MarkKind::Rectangle))
            .filter_map(|p| p.mark.x.as_ref()?.datum.as_continuous()),
    );
    // A rule with no y is a vertical rule: it spans the axis and must not size it.
    let any_cartesian_value = placed
        .iter()
        .any(|p| p.mark.kind != MarkKind::Sector && p.mark.y.is_some());
    if !any_cartesian_value {
        y_data.clear();
    }

    // Bars and areas measure from a baseline, so their axis has to include it or the picture
    // exaggerates every difference. Swift Charts does the same, and it is the single most common
    // way a chart misleads.
    let baseline_kinds = placed
        .iter()
        .any(|p| matches!(p.mark.kind, MarkKind::Bar | MarkKind::Area));
    let mut y_spec = cfg.y_scale.clone();
    if baseline_kinds && cfg.y_scale.domain.is_none() {
        y_spec.include_zero = true;
    }

    // --- Pass one: guess the margins, so there is an axis length to search ticks against ---
    let em = cfg.label_size;
    let guess = Insets {
        top: cfg.legend_insets.top + em,
        leading: cfg.legend_insets.leading + em * 3.5,
        bottom: cfg.legend_insets.bottom + em * 2.2,
        trailing: cfg.legend_insets.trailing + em,
    };
    let provisional = guess.apply(size);
    let mut x_spec = cfg.x_scale.clone();
    let (x1, y1) = scales(&placed, &x_data, &y_data, &x_spec, &y_spec, provisional);
    let xt1 = axis_ticks(&x1, cfg.x_axis, provisional.size.width, cfg, true);
    let yt1 = axis_ticks(&y1, cfg.y_axis, provisional.size.height, cfg, false);
    if let Some(spec) = niced(&x1, &x_spec) {
        x_spec = spec;
    }
    if let Some(spec) = niced(&y1, &y_spec) {
        y_spec = spec;
    }

    // --- Pass two: measure those labels and settle ---
    // The same rule `render::axis_title` draws by: a title the app set, and nothing else. The
    // inset has to reserve space on exactly that condition, or a title is drawn into whatever sits
    // under the axis — on a legend-at-the-bottom chart, straight through the legend.
    let titled = |spec: &AxisSpec| spec.title.as_deref().is_some_and(|t| !t.is_empty());
    let x_titled = titled(cfg.x_axis);
    let y_titled = titled(cfg.y_axis);
    let measure = |s: &str| day_core::measure_text(s, cfg.label_size, &cfg.font).width;
    let widest_y = yt1.iter().map(|t| measure(&t.label)).fold(0.0f64, f64::max);
    let line = day_core::measure_text("0", cfg.label_size, &cfg.font).height;
    let insets = if let Some(fixed) = cfg.plot_insets {
        // The app decided; only the legend's own room is added.
        Insets {
            top: cfg.legend_insets.top + fixed.top,
            leading: cfg.legend_insets.leading + fixed.leading,
            bottom: cfg.legend_insets.bottom + fixed.bottom,
            trailing: cfg.legend_insets.trailing + fixed.trailing,
        }
    } else if cfg.coordinate.is_polar() {
        // A polar chart has no axes to leave room for; it wants the whole pane so the circle is
        // as large as it can be.
        Insets {
            top: cfg.legend_insets.top + 4.0,
            leading: cfg.legend_insets.leading + 4.0,
            bottom: cfg.legend_insets.bottom + 4.0,
            trailing: cfg.legend_insets.trailing + 4.0,
        }
    } else {
        // The room each axis needs on ITS side, then placed on whichever edge it was put.
        let y_side = if cfg.y_axis.hidden || !cfg.y_axis.labels {
            line * 0.5
        } else {
            widest_y + 10.0 + if y_titled { line + 4.0 } else { 0.0 }
        };
        let x_side = if cfg.x_axis.hidden || !cfg.x_axis.labels {
            line * 0.5
        } else {
            // The title is drawn 8pt past the plot plus 1.9 line heights, CENTRED, so it needs
            // half a line more than that beyond it. Allocating one line put it into whatever sat
            // under the axis — on a legend-at-the-bottom chart, the legend.
            line + 10.0 + if x_titled { line * 1.6 + 6.0 } else { 0.0 }
        };
        let y_trailing = cfg.y_axis.position == AxisPosition::Trailing;
        let x_top = cfg.x_axis.position == AxisPosition::Top;
        // The first and last x labels hang half their width past the plot's ends; whichever end
        // has no y axis to hide behind reserves that overhang itself.
        let x_labelled = !cfg.x_axis.hidden && cfg.x_axis.labels;
        let overhang = |t: Option<&Tick>| {
            if x_labelled {
                x_overhang(t, &measure)
            } else {
                line * 0.5
            }
        };
        Insets {
            top: cfg.legend_insets.top + if x_top { x_side } else { line * 0.75 },
            bottom: cfg.legend_insets.bottom + if x_top { line * 0.5 } else { x_side },
            leading: cfg.legend_insets.leading
                + if y_trailing {
                    overhang(xt1.first())
                } else {
                    y_side
                },
            trailing: cfg.legend_insets.trailing
                + if y_trailing {
                    y_side
                } else {
                    overhang(xt1.last())
                },
        }
    };
    let plot = insets.apply(size);
    let (x, y) = scales(&placed, &x_data, &y_data, &x_spec, &y_spec, plot);
    let x_ticks = axis_ticks(&x, cfg.x_axis, plot.size.width, cfg, true);
    let y_ticks = axis_ticks(&y, cfg.y_axis, plot.size.height, cfg, false);

    let legend = series
        .iter()
        .enumerate()
        .map(|(i, s)| (s.clone(), (cfg.series_colors)(i, s)))
        .collect();

    Resolved {
        plot,
        x,
        y,
        series,
        x_ticks,
        y_ticks,
        marks: placed,
        coordinate: cfg.coordinate,
        legend,
        x_title,
        y_title,
        x_gap,
    }
}

/// Half of an end label hangs past the plot's edge; reserve it so it is not clipped.
fn x_overhang(tick: Option<&Tick>, measure: &dyn Fn(&str) -> f64) -> f64 {
    tick.map(|t| measure(&t.label) / 2.0 + 4.0).unwrap_or(4.0)
}

/// The smallest positive difference between any two of the values, or `None` with fewer than two
/// distinct ones.
fn smallest_gap(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut xs: Vec<f64> = values.filter(|v| v.is_finite()).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs.windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g > 0.0)
        .fold(None, |acc: Option<f64>, g| {
            Some(acc.map_or(g, |a| a.min(g)))
        })
}

fn scales(
    placed: &[Placed],
    x_data: &[Datum],
    y_data: &[Datum],
    x_spec: &ScaleSpec,
    y_spec: &ScaleSpec,
    plot: Rect,
) -> (Scale, Scale) {
    // A bar chart's x is discrete unless the data says otherwise; a line chart's is continuous.
    // The default only decides between Linear and Band when the data itself is ambiguous.
    let x_default = if placed.iter().any(|p| p.mark.kind == MarkKind::Bar) {
        ScaleKind::Band
    } else {
        ScaleKind::Linear
    };
    let x = infer(
        x_spec,
        x_data,
        (plot.origin.x, plot.origin.x + plot.size.width),
        x_default,
    );
    // y's range runs from the BOTTOM of the plot upward, which is the flip that makes larger
    // values sit higher without every mark restating it.
    let y = infer(
        y_spec,
        y_data,
        (plot.origin.y + plot.size.height, plot.origin.y),
        ScaleKind::Linear,
    );
    (x, y)
}

/// The ticks one axis shows, by the rule its scale kind implies.
fn axis_ticks(
    scale: &Scale,
    spec: &AxisSpec,
    axis_length: f64,
    cfg: &Config<'_>,
    horizontal: bool,
) -> Vec<Tick> {
    if spec.hidden {
        return Vec::new();
    }
    // A discrete axis labels every category: there is nothing to search for, and dropping one
    // would leave a bar unlabelled.
    if scale.kind.is_discrete() {
        return scale
            .categories
            .iter()
            .map(|c| Tick {
                value: 0.0,
                label: match &spec.format {
                    Some(f) => f(&Datum::Category(c.clone())),
                    None => c.clone(),
                },
            })
            .collect();
    }

    if let Some(values) = &spec.values {
        return values
            .iter()
            .map(|v| Tick {
                value: *v,
                label: label_for(*v, scale, spec, 0.0),
            })
            .collect();
    }

    // Legibility is judged against the direction the labels actually extend: their width on a
    // horizontal axis, their line height on a vertical one. Both the searched labelling and the
    // log axis's decade thinning are decided against it, so it is built before the match.
    let font = cfg.font.clone();
    let size = cfg.label_size;
    let measure_h = move |s: &str| day_core::measure_text(s, size, &font).width;
    let line = day_core::measure_text("0", cfg.label_size, &cfg.font).height;
    let measure_v = move |_: &str| line;
    let fit = if horizontal {
        LabelFit {
            axis_length,
            gap: cfg.label_size * 0.75,
            measure: &measure_h,
        }
    } else {
        LabelFit {
            axis_length,
            gap: cfg.label_size * 0.9,
            measure: &measure_v,
        }
    };
    let labelling = match &scale.kind {
        ScaleKind::Log { base } => {
            crate::ticks::log_ticks(scale.domain, *base, spec.desired_count, &fit)
        }
        ScaleKind::Time => crate::ticks::time_ticks(scale.domain, spec.desired_count),
        _ => ticks::extended(scale.domain, spec.desired_count, &fit),
    };
    labelling
        .values
        .iter()
        .filter(|v| **v >= scale.domain.lo - 1e-9 && **v <= scale.domain.hi + 1e-9)
        .map(|v| Tick {
            value: *v,
            label: label_for(*v, scale, spec, labelling.step),
        })
        .collect()
}

/// The "nice numbers" an axis bound is allowed to land on, within each power of ten.
///
/// A fixed ladder is what makes the bound MONOTONE in the data: the smallest rung at or above a
/// value can only rise as that value rises. Deriving the bound from the tick step instead cannot
/// promise that, because the step is itself chosen from the data — a stacked bar reaching 152 got
/// a step of 50 and an axis to 200, and the same bar reaching 158 got a step of 40 and an axis to
/// 160, so growing the data made the axis SHRINK and every bar jump upward.
const NICE: [f64; 10] = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];

/// The smallest nice number at or above `v`, and the largest at or below it. Both are
/// non-decreasing in `v`, which is the property the axis inherits.
fn nice_bound(v: f64, up: bool) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    if v < 0.0 {
        // Mirrored: rounding a negative bound "up" moves it toward zero, which is the SMALLER
        // magnitude, so the direction flips with the sign.
        return -nice_bound(-v, !up);
    }
    let decade = 10f64.powf(v.log10().floor());
    let m = v / decade;
    // A bound already sitting on a rung stays there: the epsilon keeps its own rounding error
    // from pushing it a whole rung further out.
    let rung = if up {
        NICE.iter().find(|r| **r >= m - 1e-9)
    } else {
        NICE.iter().rev().find(|r| **r <= m + 1e-9)
    };
    rung.copied().unwrap_or(m) * decade
}

/// Round an inferred domain outward to the nearest nice bounds.
///
/// A domain that stops at the data puts the tallest mark against the frame with the top label well
/// below it. Rounding out gives the marks headroom and puts a labelled gridline at each end — and
/// because [`nice_bound`] reads only the data, the axis can grow as the data grows but never fall
/// back, which is what a reader watching a live chart needs: bars that shrink when nothing shrank
/// are worse than bars with too much headroom.
///
/// Only an INFERRED linear domain moves. An app that pinned one said what it wanted, and a log or
/// time axis rounds to its own kind of bound — a decade, a calendar boundary — not to a rung.
fn niced(scale: &Scale, spec: &ScaleSpec) -> Option<ScaleSpec> {
    if spec.domain.is_some() || !matches!(scale.kind, ScaleKind::Linear) {
        return None;
    }
    // Only a domain that REACHES ZERO is rounded, because the ladder is anchored there. A bar or
    // area chart is exactly that case and it is the one that needed the headroom. A floating range
    // is left alone: 980..1000 is a price chart, and the nearest rung below 980 is 800, which
    // would flatten the whole line into a ribbon along the top of the plot.
    if scale.domain.lo > 0.0 || scale.domain.hi < 0.0 {
        return None;
    }
    let lo = nice_bound(scale.domain.lo, false);
    let hi = nice_bound(scale.domain.hi, true);
    if hi <= lo || (lo >= scale.domain.lo - 1e-9 && hi <= scale.domain.hi + 1e-9) {
        return None;
    }
    let mut out = spec.clone();
    out.domain = Some(Interval::new(lo, hi));
    Some(out)
}

pub(crate) fn label_for(v: f64, scale: &Scale, spec: &AxisSpec, step: f64) -> String {
    if let Some(f) = &spec.format {
        let d = match scale.kind {
            ScaleKind::Time => Datum::Time(v),
            _ => Datum::Number(v),
        };
        return f(&d);
    }
    match scale.kind {
        ScaleKind::Time => ticks::time_label(v, scale.domain.span()),
        ScaleKind::Log { .. } => {
            // A log axis labels its decades as written numbers while they are short, and switches
            // to exponent form when they stop being readable. Grouped, a decade stays readable a
            // long way further than it used to: `1,000,000` is a number, `1000000` is a puzzle.
            if (0.000_001..1_000_000_000.0).contains(&v) {
                let d = if v >= 1.0 {
                    0
                } else {
                    (-v.log10().floor()) as usize
                };
                day_l10n::format_decimal(v, d)
            } else {
                format!("1e{}", v.log10().round() as i64)
            }
        }
        _ => ticks::format_value(
            v,
            if step > 0.0 {
                step
            } else {
                scale.domain.span() / 10.0
            },
        ),
    }
}

/// The domain a continuous axis would infer, exposed for a caller that needs it before rendering.
pub fn domain_of(data: &[Datum]) -> Interval {
    let mut it = data.iter().filter_map(|d| d.as_continuous());
    match it.next() {
        Some(first) => {
            let mut iv = Interval::new(first, first);
            for v in it {
                iv.extend(v);
            }
            iv
        }
        None => Interval::new(0.0, 1.0),
    }
}
