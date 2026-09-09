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

use crate::axis::AxisSpec;
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
        if p.v0.is_finite() {
            y_data.push(Datum::Number(p.v0));
        }
        if p.v1.is_finite() {
            y_data.push(Datum::Number(p.v1));
        }
    }
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
    let (x1, y1) = scales(&placed, &x_data, &y_data, cfg, &y_spec, provisional);
    let xt1 = axis_ticks(&x1, cfg.x_axis, provisional.size.width, cfg, true);
    let yt1 = axis_ticks(&y1, cfg.y_axis, provisional.size.height, cfg, false);

    // --- Pass two: measure those labels and settle ---
    // A title is drawn whenever the axis has one OR the data column carries a label to fall back
    // on (`axis_title` in render.rs). The inset has to reserve space on the same condition, or the
    // fallback title is drawn into whatever sits under the axis — on a legend-at-the-bottom chart,
    // straight through the legend.
    let x_titled = cfg.x_axis.title.is_some() || x_title.as_deref().is_some_and(|t| !t.is_empty());
    let y_titled = cfg.y_axis.title.is_some() || y_title.as_deref().is_some_and(|t| !t.is_empty());
    let measure = |s: &str| day_core::measure_text(s, cfg.label_size, &cfg.font).width;
    let widest_y = yt1.iter().map(|t| measure(&t.label)).fold(0.0f64, f64::max);
    let line = day_core::measure_text("0", cfg.label_size, &cfg.font).height;
    // A polar chart has no axes to leave room for; it wants the whole pane so the circle is as
    // large as it can be.
    let insets = if cfg.coordinate.is_polar() {
        Insets {
            top: cfg.legend_insets.top + 4.0,
            leading: cfg.legend_insets.leading + 4.0,
            bottom: cfg.legend_insets.bottom + 4.0,
            trailing: cfg.legend_insets.trailing + 4.0,
        }
    } else {
        Insets {
            top: cfg.legend_insets.top + line * 0.75,
            leading: cfg.legend_insets.leading
                + if cfg.y_axis.hidden || !cfg.y_axis.labels {
                    line * 0.5
                } else {
                    widest_y + 10.0 + if y_titled { line + 4.0 } else { 0.0 }
                },
            bottom: cfg.legend_insets.bottom
                + if cfg.x_axis.hidden || !cfg.x_axis.labels {
                    line * 0.5
                } else {
                    // The title is drawn 8pt below the plot plus 1.9 line heights, CENTRED, so it
                    // needs half a line more than that beneath it. Allocating one line put it into
                    // whatever sat under the axis — on a legend-at-the-bottom chart, the legend.
                    line + 10.0 + if x_titled { line * 1.6 + 6.0 } else { 0.0 }
                },
            trailing: cfg.legend_insets.trailing + widest_x_overhang(&xt1, &measure),
        }
    };
    let plot = insets.apply(size);
    let (x, y) = scales(&placed, &x_data, &y_data, cfg, &y_spec, plot);
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
    }
}

/// Half the last x label hangs past the plot's right edge; reserve it so it is not clipped.
fn widest_x_overhang(ticks: &[Tick], measure: &dyn Fn(&str) -> f64) -> f64 {
    ticks
        .last()
        .map(|t| measure(&t.label) / 2.0 + 4.0)
        .unwrap_or(4.0)
}

fn scales(
    placed: &[Placed],
    x_data: &[Datum],
    y_data: &[Datum],
    cfg: &Config<'_>,
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
        cfg.x_scale,
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

    let labelling = match &scale.kind {
        ScaleKind::Log { base } => crate::ticks::log_ticks(scale.domain, *base, spec.desired_count),
        ScaleKind::Time => crate::ticks::time_ticks(scale.domain, spec.desired_count),
        _ => {
            // Legibility is judged against the direction the labels actually extend: their width
            // on a horizontal axis, their line height on a vertical one.
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
            ticks::extended(scale.domain, spec.desired_count, &fit)
        }
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

fn label_for(v: f64, scale: &Scale, spec: &AxisSpec, step: f64) -> String {
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
            // to exponent form when they stop being readable.
            if (0.001..100_000.0).contains(&v) {
                let d = if v >= 1.0 {
                    0
                } else {
                    (-v.log10().floor()) as usize
                };
                format!("{v:.d$}")
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
