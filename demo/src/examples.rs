// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! The illustrative charts the crate's README documents, one function each, now with controls.
//!
//! Every entry carries its own data and reads as a complete chart. At first load each one draws
//! the data the README prints beside the screenshot `dayscript/gallery.yaml` captures here; the
//! controls above it then change the data or the chart's parameters, and every chart is
//! `.animated()` (README "Animation"), so each change is drawn as the data moving: bars grow and
//! trade places, a line's window widens and its axis rescales, a pie's wedges sweep, a scatter's
//! points travel, and a heat map's cells blend to their new colors.
//!
//! "Randomize data" advances a seed rather than reading a clock, so the same presses give the same
//! charts and a dayscript can photograph them.
//!
//! Most of them are interactive: `.select(sig)` reports what the pointer is over into a signal,
//! `.guides(..)` reads the same signal back to draw a rule, rings and a label box for it, and the
//! readout under the chart shows the same value in words, which is what an app does with a
//! selection it owns. Hover reports on a pointer; a tap or a drag reports on a touch screen.
//!
//! Each chart carries `.id(..)` before its sizing decorators, because an id belongs on the piece
//! itself: a dayscript `tap` emitted to a wrapper would be delivered to the wrapper and dropped.
//! The id is also the route that opens the page (`…/#lines`) and the name the screenshot is filed
//! under, so one name addresses a chart three ways.
//!
//! `line` is written qualified throughout, because `day::prelude` exports a `line` shape of its
//! own.

use day::prelude::*;
use day_piece_charts::select::{Guides, Selection, Snap};
use day_piece_charts::*;

use crate::{Page, res};

/// One gallery entry: the route that opens it, the row's label, and the chart.
pub(crate) struct Example {
    pub(crate) page: Page,
    pub(crate) title: fn() -> day::LocalizedText,
    pub(crate) build: fn() -> AnyPiece,
}

/// The gallery, in the order the sidebar lists it and the README documents it.
pub(crate) fn gallery() -> Vec<Example> {
    let e = |page, title: fn() -> day::LocalizedText, build: fn() -> AnyPiece| Example {
        page,
        title,
        build,
    };
    vec![
        e(Page::Lines, res::str::ex_line, || line_chart().any()),
        e(Page::LinesTarget, res::str::ex_line_series, || {
            line_series().any()
        }),
        e(Page::Area, res::str::ex_area, || area_chart().any()),
        e(Page::AreaStacked, res::str::ex_area_stacked, || {
            area_stacked().any()
        }),
        e(Page::Bars, res::str::ex_bar, || bar_chart().any()),
        e(Page::BarsGrouped, res::str::ex_bar_grouped, || {
            bar_grouped().any()
        }),
        e(Page::Bars100, res::str::ex_bar_normalized, || {
            bar_normalized().any()
        }),
        e(Page::BarsHorizontal, res::str::ex_bar_horizontal, || {
            bar_horizontal().any()
        }),
        e(Page::Pie, res::str::ex_pie, || pie_chart().any()),
        e(Page::Scatter, res::str::ex_scatter, || {
            scatter_chart().any()
        }),
        e(Page::Heatmap, res::str::ex_heatmap, || {
            heatmap_chart().any()
        }),
        e(Page::Time, res::str::ex_time, || time_chart().any()),
        e(Page::Sparkline, res::str::ex_sparkline, || {
            sparkline().any()
        }),
    ]
}

// ---------------------------------------------------------------------------
// Controls and data
// ---------------------------------------------------------------------------

/// A small deterministic generator for the "Randomize data" buttons: xorshift, seeded by the
/// button's press count, so a run is repeatable and a screenshot of it is too.
struct Rng(u64);

impl Rng {
    fn new(seed: u64, salt: u64) -> Self {
        Rng(
            (seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xD1B5_4A32_D192_ED03))
                | 1,
        )
    }
    /// A number in `0..1`.
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
    /// A normally distributed number (Box–Muller).
    fn normal(&mut self, mean: f64, sd: f64) -> f64 {
        let u = self.next().max(f64::MIN_POSITIVE);
        let v = self.next();
        mean + sd * (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
    /// An exponentially distributed number with the given mean.
    fn exponential(&mut self, mean: f64) -> f64 {
        -mean * (1.0 - self.next()).ln()
    }
    /// `n` values that wander between `lo` and `hi`, each within `step` of the last: a series that
    /// looks measured rather than thrown.
    fn walk(&mut self, n: usize, lo: f64, hi: f64, step: f64) -> Vec<f64> {
        let mut v = self.range(lo, hi);
        (0..n)
            .map(|_| {
                v = (v + self.range(-step, step)).clamp(lo, hi);
                (v * 10.0).round() / 10.0
            })
            .collect()
    }
}

/// The button every example carries. It advances `seed`, which the chart's data reads.
fn randomize(seed: Signal<u64>) -> impl Piece {
    button(res::str::ex_randomize())
        .action(move || seed.update(|s| *s += 1))
        .id("charts-randomize")
}

/// A row of controls above a chart.
fn controls(items: Vec<AnyPiece>) -> impl Piece {
    row(PieceVec(items)).spacing(12.0).align(VAlign::Center)
}

/// A segmented picker over `options`, labeled.
fn choice(
    title: day::LocalizedText,
    options: &[String],
    selected: Signal<usize>,
    id: &'static str,
) -> AnyPiece {
    row((
        label(title),
        picker(options.to_vec(), selected).segmented().id(id),
    ))
    .spacing(6.0)
    .align(VAlign::Center)
    .any()
}

/// A labeled switch.
fn switch(title: day::LocalizedText, on: Signal<bool>, id: &'static str) -> AnyPiece {
    row((toggle(on).id(id), label(title)))
        .spacing(6.0)
        .align(VAlign::Center)
        .any()
}

/// A labeled slider over `range`.
fn amount(
    title: day::LocalizedText,
    value: Signal<f64>,
    range: std::ops::RangeInclusive<f64>,
    id: &'static str,
) -> AnyPiece {
    row((label(title), slider(value).range(range).id(id).width(140.0)))
        .spacing(6.0)
        .align(VAlign::Center)
        .any()
}

/// A labeled slider over a whole number, with the number beside it.
fn count(
    title: day::LocalizedText,
    value: Signal<f64>,
    range: std::ops::RangeInclusive<f64>,
    id: &'static str,
    readout_id: &'static str,
) -> AnyPiece {
    row((
        label(title),
        slider(value).range(range).id(id).width(160.0),
        label(move || format!("{}", value.get().round() as usize))
            .id(readout_id)
            .width(36.0),
    ))
    .spacing(6.0)
    .align(VAlign::Center)
    .any()
}

/// A labeled stepper over `range`: the value between a − and a + that stop at its ends. The ids
/// are the decrement button, the value, and the increment button.
fn stepper(
    title: day::LocalizedText,
    value: Signal<usize>,
    range: std::ops::RangeInclusive<usize>,
    ids: [&'static str; 3],
) -> AnyPiece {
    let (lo, hi) = (*range.start(), *range.end());
    row((
        label(title),
        button("−")
            .action(move || value.update(|v| *v = v.saturating_sub(1).max(lo)))
            .enabled(move || value.get() > lo)
            .id(ids[0]),
        label(move || value.get().to_string()).id(ids[1]),
        button("+")
            .action(move || value.update(|v| *v = (*v + 1).min(hi)))
            .enabled(move || value.get() < hi)
            .id(ids[2]),
    ))
    .spacing(6.0)
    .align(VAlign::Center)
    .any()
}

/// How the scatter example draws its random points.
#[derive(Clone, Copy, PartialEq)]
enum Distribution {
    /// Strain rising with load, with even noise around the line: what the readings look like.
    Trend,
    /// Anywhere on the plot, equally likely.
    Uniform,
    /// A normal cloud around a center of the series' own.
    Normal,
    /// Loads bunched at the light end and thinning out, strain rising with them.
    Exponential,
    /// Strain swinging with load around a sine wave.
    Wave,
    /// A trend whose noise widens with load (heteroscedastic).
    Fan,
}

const DISTRIBUTIONS: [Distribution; 6] = [
    Distribution::Trend,
    Distribution::Uniform,
    Distribution::Normal,
    Distribution::Exponential,
    Distribution::Wave,
    Distribution::Fan,
];

/// One series' shape under every distribution, drawn once so a series keeps its character (its
/// slope, where its cloud sits, its wave's phase) as its points are added and re-drawn.
struct Traits {
    slope: f64,
    center: (f64, f64),
    phase: f64,
}

impl Distribution {
    /// A point on the plot: load in 8 to 50 kN, strain from 0.2 mm up. A draw whose load falls
    /// off the axis is drawn again rather than clamped, which would pile a tail's points into a
    /// column at the edge.
    fn sample(self, rng: &mut Rng, t: &Traits) -> (f64, f64) {
        const TRIES: usize = 16;
        let mut point = self.draw(rng, t);
        for _ in 1..TRIES {
            if (8.0..=50.0).contains(&point.0) {
                break;
            }
            point = self.draw(rng, t);
        }
        let (load, strain) = point;
        (
            (load.clamp(8.0, 50.0) * 10.0).round() / 10.0,
            strain.max(0.2),
        )
    }

    fn draw(self, rng: &mut Rng, t: &Traits) -> (f64, f64) {
        const LOAD: (f64, f64) = (8.0, 50.0);
        let noise = |rng: &mut Rng| rng.range(-0.6, 0.6);
        match self {
            Distribution::Trend => {
                let load = rng.range(LOAD.0, LOAD.1);
                (load, load * t.slope + noise(rng))
            }
            Distribution::Uniform => (rng.range(LOAD.0, LOAD.1), rng.range(0.2, 7.0)),
            Distribution::Normal => (rng.normal(t.center.0, 4.5), rng.normal(t.center.1, 0.6)),
            Distribution::Exponential => {
                let load = LOAD.0 + rng.exponential(8.0);
                (load, load * t.slope + noise(rng))
            }
            Distribution::Wave => {
                let load = rng.range(LOAD.0, LOAD.1);
                (
                    load,
                    3.5 + 2.2 * (load / 5.0 + t.phase).sin() + rng.normal(0.0, 0.25),
                )
            }
            Distribution::Fan => {
                let load = rng.range(LOAD.0, LOAD.1);
                (load, load * t.slope + rng.normal(0.0, 0.03 * load))
            }
        }
    }
}

fn distribution_menu(selected: Signal<usize>) -> AnyPiece {
    let names: Vec<String> = [
        res::str::ex_dist_trend(),
        res::str::ex_dist_uniform(),
        res::str::ex_dist_normal(),
        res::str::ex_dist_exponential(),
        res::str::ex_dist_wave(),
        res::str::ex_dist_fan(),
    ]
    .into_iter()
    .map(|t| t.format())
    .collect();
    row((
        label(res::str::ex_distribution()),
        picker(names, selected).menu().id("charts-distribution"),
    ))
    .spacing(6.0)
    .align(VAlign::Center)
    .any()
}

/// The interpolations a line or area can take, in the order the curve menu lists them.
const CURVES: [Interpolation; 7] = [
    Interpolation::Linear,
    Interpolation::Monotone,
    Interpolation::CatmullRom,
    Interpolation::Cardinal,
    Interpolation::StepStart,
    Interpolation::StepCenter,
    Interpolation::StepEnd,
];

/// The curve menu: every interpolation the crate draws. A curve is a way of drawing the same
/// data, not new data, so changing it redraws the path in place rather than animating values.
fn curve_menu(selected: Signal<usize>) -> AnyPiece {
    let names: Vec<String> = [
        res::str::ex_curve_linear(),
        res::str::ex_curve_monotone(),
        res::str::ex_curve_catmull(),
        res::str::ex_curve_cardinal(),
        res::str::ex_curve_step_start(),
        res::str::ex_curve_step_center(),
        res::str::ex_curve_step_end(),
    ]
    .into_iter()
    .map(|t| t.format())
    .collect();
    row((
        label(res::str::ex_curve()),
        picker(names, selected).menu().id("charts-curve"),
    ))
    .spacing(6.0)
    .align(VAlign::Center)
    .any()
}

fn curve(selected: Signal<usize>) -> Interpolation {
    CURVES[selected.get().min(CURVES.len() - 1)]
}

/// The current selection in words, under an interactive chart.
///
/// The chart draws its own guides from the signal; this is the same value as text, which is what
/// an app reads the signal for — a readout, a detail pane, a fetch for the selected row. Every
/// gallery readout carries one id, because the page shows one example at a time.
fn selection_readout(selected: Signal<Option<Selection>>) -> impl Piece {
    label(move || match selected.get() {
        Some(sel) => {
            let values: Vec<String> = sel
                .values
                .iter()
                .map(|v| {
                    if v.series.is_empty() {
                        v.label.clone()
                    } else {
                        format!("{}: {}", v.series, v.label)
                    }
                })
                .collect();
            format!("{} \u{b7} {}", sel.x_label, values.join(", "))
        }
        None => res::str::ex_hover().format(),
    })
    .font(Font::Footnote)
    .id("charts-example-value")
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The timeframe picker's options, and how many months each one shows.
fn timeframes() -> Vec<String> {
    vec![
        res::str::ex_tf_half().format(),
        res::str::ex_tf_year().format(),
    ]
}
fn months_in(frame: usize) -> usize {
    if frame == 0 { 6 } else { 12 }
}

/// A year of monthly revenue for `region` (0 or 1): the README's numbers at seed zero, extended
/// through the second half of the year, and a fresh walk for every press after that.
fn monthly(seed: u64, region: u64) -> Vec<f64> {
    const NORTH: [f64; 12] = [
        12.0, 18.0, 15.0, 22.0, 28.0, 24.0, 27.0, 31.0, 29.0, 34.0, 38.0, 36.0,
    ];
    const SOUTH: [f64; 12] = [
        8.0, 11.0, 14.0, 12.0, 19.0, 23.0, 21.0, 25.0, 24.0, 28.0, 26.0, 31.0,
    ];
    if seed == 0 {
        return if region == 0 { NORTH } else { SOUTH }.to_vec();
    }
    Rng::new(seed, 10 + region).walk(12, 6.0, 40.0, 7.0)
}

// ---------------------------------------------------------------------------
// Lines
// ---------------------------------------------------------------------------

/// A line through one series, with the samples marked.
///
/// The monotone spline is a curve, not a styling choice: it is constrained never to overshoot, so
/// the line cannot dip below a value the data never took. The timeframe picker adds and removes
/// months: the kept samples slide to their new bands, and the new ones grow out of the old line.
fn line_chart() -> impl Piece {
    let seed = Signal::new(0u64);
    let frame = Signal::new(0usize);
    let interp = Signal::new(1usize);
    let points = Signal::new(true);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(
                res::str::ex_timeframe(),
                &timeframes(),
                frame,
                "charts-timeframe",
            ),
            curve_menu(interp),
            switch(res::str::ex_points(), points, "charts-points"),
        ]),
        chart(move || {
            let revenue = monthly(seed.get(), 0);
            let (interpolation, dots) = (curve(interp), points.get());
            let mut marks = Vec::new();
            for (month, revenue) in MONTHS.iter().zip(revenue).take(months_in(frame.get())) {
                let x = value("Month", *month);
                let y = value("Revenue", revenue);
                marks.push(
                    { day_piece_charts::line(x.clone(), y.clone()) }
                        .interpolation(interpolation)
                        .line_width(2.0)
                        .rounded(),
                );
                if dots {
                    marks.push(point(x, y).symbol_size(40.0));
                }
            }
            marks
        })
        .y_label("Revenue (thousands)")
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("lines")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Two series and a target line: `by_series` splits the data and names the legend entries, and a
/// `rule_y` is a horizontal line at a value, dashed and annotated here. The target moves with the
/// data (the series' mean), so the rule glides along the rescaling axis too.
fn line_series() -> impl Piece {
    const REGIONS: [&str; 2] = ["North", "South"];
    let seed = Signal::new(0u64);
    let frame = Signal::new(0usize);
    let interp = Signal::new(0usize);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(
                res::str::ex_timeframe(),
                &timeframes(),
                frame,
                "charts-timeframe",
            ),
            curve_menu(interp),
        ]),
        chart(move || {
            let n = months_in(frame.get());
            let interpolation = curve(interp);
            let s = seed.get();
            let mut marks: Vec<Mark> = Vec::new();
            let mut sum = 0.0;
            for (r, region) in REGIONS.iter().enumerate() {
                for (month, revenue) in MONTHS.iter().zip(monthly(s, r as u64)).take(n) {
                    sum += revenue;
                    marks.push(
                        {
                            day_piece_charts::line(
                                value("Month", *month),
                                value("Revenue", revenue),
                            )
                        }
                        .by_series(value("Region", *region))
                        .interpolation(interpolation)
                        .line_width(2.0)
                        .rounded(),
                    );
                }
            }
            let target = if s == 0 && n == 6 {
                20.0
            } else {
                (sum / (2 * n) as f64).round()
            };
            marks.push(
                rule_y(value("Target", target))
                    .foreground(Color::rgb(0.55, 0.56, 0.6))
                    .dash([5.0, 4.0])
                    .annotation(AnnotationPosition::Top, "Target"),
            );
            marks
        })
        .y_label("Revenue (thousands)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("lines-target")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

// ---------------------------------------------------------------------------
// Areas
// ---------------------------------------------------------------------------

/// An area under a line, filled with a vertical gradient: `gradient` takes the color at the mark's
/// top edge and the one at its baseline, so the fill fades out of the plot rather than blocking it.
fn area_chart() -> impl Piece {
    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const SESSIONS: [f64; 7] = [320.0, 410.0, 385.0, 520.0, 610.0, 455.0, 390.0];
    let seed = Signal::new(0u64);
    let interp = Signal::new(1usize);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![randomize(seed).any(), curve_menu(interp)]),
        chart(move || {
            let s = seed.get();
            let interpolation = curve(interp);
            let sessions = if s == 0 {
                SESSIONS.to_vec()
            } else {
                Rng::new(s, 3).walk(7, 180.0, 720.0, 140.0)
            };
            let ink = categorical(0);
            DAYS.iter()
                .zip(sessions)
                .flat_map(|(day, sessions)| {
                    let x = value("Day", *day);
                    let y = value("Sessions", sessions);
                    [
                        area(x.clone(), y.clone())
                            .interpolation(interpolation)
                            .gradient(
                                Color::rgba(ink.r, ink.g, ink.b, 0.55),
                                Color::rgba(ink.r, ink.g, ink.b, 0.02),
                            ),
                        { day_piece_charts::line(x, y) }
                            .interpolation(interpolation)
                            .line_width(2.0)
                            .rounded(),
                    ]
                })
                .collect()
        })
        .y_label("Sessions")
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("area")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Three series stacked into a total. Areas stack by default, so the composition is the marks
/// themselves; `opacity` keeps the bands distinct where they meet. An animated stack is summed
/// again every frame, so the bands never cross while the data moves.
fn area_stacked() -> impl Piece {
    const QUARTERS: [&str; 8] = ["Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7", "Q8"];
    const CHANNELS: [(&str, [f64; 8]); 3] = [
        ("Direct", [18.0, 22.0, 25.0, 31.0, 36.0, 34.0, 39.0, 43.0]),
        ("Partner", [12.0, 14.0, 19.0, 21.0, 24.0, 27.0, 26.0, 30.0]),
        ("Online", [6.0, 11.0, 14.0, 20.0, 29.0, 33.0, 38.0, 45.0]),
    ];
    const COUNTS: [usize; 3] = [4, 5, 8];
    const STACKINGS: [Stacking; 4] = [
        Stacking::Standard,
        Stacking::Normalized,
        Stacking::Center,
        Stacking::Unstacked,
    ];
    let seed = Signal::new(0u64);
    let count = Signal::new(1usize);
    let stacking = Signal::new(0usize);
    let interp = Signal::new(1usize);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(
                res::str::ex_quarters(),
                &COUNTS.map(|n| n.to_string()),
                count,
                "charts-quarters",
            ),
            choice(
                res::str::ex_stacking(),
                &[
                    res::str::ex_stack_standard().format(),
                    res::str::ex_stack_normalized().format(),
                    res::str::ex_stack_center().format(),
                    res::str::ex_stack_none().format(),
                ],
                stacking,
                "charts-stacking",
            ),
            curve_menu(interp),
        ]),
        chart(move || {
            let s = seed.get();
            let n = COUNTS[count.get().min(COUNTS.len() - 1)];
            let mode = STACKINGS[stacking.get().min(STACKINGS.len() - 1)];
            let interpolation = curve(interp);
            // Overlapping areas need to see through each other; stacked ones do not.
            let opacity = if mode == Stacking::Unstacked {
                0.45
            } else {
                0.85
            };
            CHANNELS
                .iter()
                .enumerate()
                .flat_map(|(c, (channel, values))| {
                    let values = if s == 0 {
                        values.to_vec()
                    } else {
                        Rng::new(s, 20 + c as u64).walk(8, 4.0, 45.0, 9.0)
                    };
                    QUARTERS
                        .iter()
                        .zip(values)
                        .take(n)
                        .map(move |(quarter, revenue)| {
                            area(value("Quarter", *quarter), value("Revenue", revenue))
                                .by_series(value("Channel", *channel))
                                .stacking(mode)
                                .interpolation(interpolation)
                                .opacity(opacity)
                        })
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("area-stacked")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

// ---------------------------------------------------------------------------
// Bars
// ---------------------------------------------------------------------------

/// Bars with their values written above them. An annotation is attached to a mark rather than
/// placed on the plot, so it follows the bar as the data changes: sorted, the bars trade places
/// and carry their labels with them.
fn bar_chart() -> impl Piece {
    const REVENUE: [f64; 6] = [12.0, 18.0, 15.0, 22.0, 28.0, 24.0];
    let seed = Signal::new(0u64);
    let sorted = Signal::new(false);
    let radius = Signal::new(4.0f64);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            switch(res::str::ex_sort(), sorted, "charts-sort"),
            amount(res::str::ex_corners(), radius, 0.0..=16.0, "charts-corners"),
        ]),
        chart(move || {
            let s = seed.get();
            let values = if s == 0 {
                REVENUE.to_vec()
            } else {
                Rng::new(s, 30).walk(6, 5.0, 40.0, 12.0)
            };
            let mut rows: Vec<(&str, f64)> = MONTHS.iter().copied().zip(values).collect();
            if sorted.get() {
                rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            let r = radius.get();
            rows.into_iter()
                .map(|(month, revenue)| {
                    bar(value("Month", month), value("Revenue", revenue))
                        .corner_radius(r)
                        .annotation(AnnotationPosition::Top, format!("{revenue:.0}"))
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("bars")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Grouped bars: `by_series` colors them, `by_position` splits each month's band between the two
/// regions, and `Stacking::Unstacked` is what stops them stacking, which is what bars do by
/// default.
fn bar_grouped() -> impl Piece {
    const REGIONS: [&str; 2] = ["North", "South"];
    let seed = Signal::new(0u64);
    let frame = Signal::new(0usize);
    let stacked = Signal::new(false);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(
                res::str::ex_timeframe(),
                &timeframes(),
                frame,
                "charts-timeframe",
            ),
            switch(res::str::ex_stacked(), stacked, "charts-stacked-bars"),
        ]),
        chart(move || {
            let (s, n) = (seed.get(), months_in(frame.get()));
            let stacked = stacked.get();
            REGIONS
                .iter()
                .enumerate()
                .flat_map(|(r, region)| {
                    MONTHS
                        .iter()
                        .zip(monthly(s, r as u64))
                        .take(n)
                        .map(move |(month, revenue)| {
                            let b = bar(value("Month", *month), value("Revenue", revenue))
                                .by_series(value("Region", *region))
                                .corner_radius(3.0);
                            // Stacked, the regions share a column; grouped, `by_position` splits
                            // the band and nothing stacks. Either way a bar keeps its identity,
                            // so switching slides each one into its new place.
                            if stacked {
                                b
                            } else {
                                b.by_position(value("Region", *region))
                                    .stacking(Stacking::Unstacked)
                            }
                        })
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("bars-grouped")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Each column scaled to fill the axis: `Stacking::Normalized` turns magnitudes into shares, so
/// the axis is a percentage and the grid has nothing left to say.
fn bar_normalized() -> impl Piece {
    const QUARTERS: [&str; 4] = ["Q1", "Q2", "Q3", "Q4"];
    const CHANNELS: [(&str, [f64; 4]); 3] = [
        ("Direct", [18.0, 22.0, 25.0, 31.0]),
        ("Partner", [12.0, 14.0, 19.0, 21.0]),
        ("Online", [6.0, 11.0, 14.0, 20.0]),
    ];
    let seed = Signal::new(0u64);
    column((
        controls(vec![randomize(seed).any()]),
        chart(move || {
            let s = seed.get();
            CHANNELS
                .iter()
                .enumerate()
                .flat_map(|(c, (channel, values))| {
                    let values = if s == 0 {
                        values.to_vec()
                    } else {
                        Rng::new(s, 40 + c as u64).walk(4, 2.0, 40.0, 16.0)
                    };
                    QUARTERS.iter().zip(values).map(move |(quarter, revenue)| {
                        bar(value("Quarter", *quarter), value("Revenue", revenue))
                            .by_series(value("Channel", *channel))
                            .stacking(Stacking::Normalized)
                            .corner_radius(3.0)
                    })
                })
                .collect()
        })
        .y_format(|d| match d {
            Datum::Number(v) => format!("{:.0}%", v * 100.0),
            other => other.to_string(),
        })
        .no_grid()
        .legend(LegendPosition::Bottom)
        .animated()
        .id("bars-100")
        .grow(),
    ))
    .spacing(6.0)
    .grow()
}

/// Bars along the value axis, for categories whose names need the room: the y channel takes the
/// category and `x_range` spans the bar from zero to its value, which is `BarMark(xStart:xEnd:)`.
///
/// A bar's x is a band by default, because that is what a bar chart's x usually is, so a
/// horizontal one names its value axis with `x_scale_kind`. Sorted, the rows trade places.
fn bar_horizontal() -> impl Piece {
    const LANGUAGES: [(&str, f64); 5] = [
        ("Rust", 86.0),
        ("Swift", 62.0),
        ("Kotlin", 54.0),
        ("TypeScript", 47.0),
        ("Python", 38.0),
    ];
    let seed = Signal::new(0u64);
    let sorted = Signal::new(true);
    column((
        controls(vec![
            randomize(seed).any(),
            switch(res::str::ex_sort(), sorted, "charts-sort"),
        ]),
        chart(move || {
            let s = seed.get();
            let mut rng = Rng::new(s, 50);
            let mut rows: Vec<(&str, f64)> = LANGUAGES
                .iter()
                .map(|(l, v)| {
                    (
                        *l,
                        if s == 0 {
                            *v
                        } else {
                            rng.range(20.0, 95.0).round()
                        },
                    )
                })
                .collect();
            if sorted.get() {
                rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            rows.into_iter()
                .map(|(language, share)| {
                    bar(value("Share", share), value("Language", language))
                        .x_range(value("Share", 0.0), value("Share", share))
                        .corner_radius(4.0)
                })
                .collect()
        })
        .x_scale_kind(ScaleKind::Linear)
        .x_label("Share of respondents")
        .animated()
        .id("bars-horizontal")
        .grow(),
    ))
    .spacing(6.0)
    .grow()
}

// ---------------------------------------------------------------------------
// Parts of a whole
// ---------------------------------------------------------------------------

const SOURCES: [(&str, f64); 6] = [
    ("Search", 46.0),
    ("Direct", 28.0),
    ("Social", 17.0),
    ("Email", 9.0),
    ("Referral", 12.0),
    ("Ads", 7.0),
];

/// The slices a pie shows: the first `n` sources, with the README's shares at seed zero.
fn slices(seed: u64, n: usize) -> Vec<Mark> {
    let mut rng = Rng::new(seed, 60);
    SOURCES
        .iter()
        .take(n)
        .map(|(source, share)| {
            let share = if seed == 0 {
                *share
            } else {
                rng.range(5.0, 50.0).round()
            };
            sector(value("Share", share))
                .by_series(value("Source", *source))
                .angular_inset(2.0)
        })
        .collect()
}

fn slice_counts() -> [String; 4] {
    ["3", "4", "5", "6"].map(String::from)
}

/// A pie is a normalized stack in polar coordinates, which is how it is implemented: one `sector`
/// per slice, and `angular_inset` holds an even channel between neighbours. Animated, the stack is
/// summed again every frame, so a slice that arrives opens a gap its neighbours make room for.
fn pie_chart() -> impl Piece {
    let seed = Signal::new(0u64);
    let count = Signal::new(1usize);
    let hole = Signal::new(0.0f64);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(
                res::str::ex_slices(),
                &slice_counts(),
                count,
                "charts-slices",
            ),
            amount(res::str::ex_hole(), hole, 0.0..=90.0, "charts-hole"),
        ]),
        chart(move || slices(seed.get(), count.get() + 3))
            // `Coordinate::donut` takes the hole as a fraction of the radius, so one number walks
            // a pie through a donut to a ring gauge without touching a mark. The slider is a
            // percentage; the chart animates the hole like any other change.
            .configure(move |c| c.coordinate = Coordinate::donut(hole.get() / 100.0))
            .legend(LegendPosition::Trailing)
            .animated()
            .id("pie")
            .grow(),
    ))
    .spacing(6.0)
    .grow()
}

// ---------------------------------------------------------------------------
// Distributions and grids
// ---------------------------------------------------------------------------

/// A scatter of two measurements. `by_symbol` gives each series its own shape, so the chart still
/// reads where color cannot be relied on, and `symbol_size` is an area, the way magnitude should
/// be encoded. A scatter point is its place in its series, so a new sample travels rather than
/// blinking out and back.
///
/// The count and series controls are a stress test: up to a thousand points across eight series,
/// every change animated, and a slider drag retargets the transition on every step.
fn scatter_chart() -> impl Piece {
    const ALLOYS: [&str; 8] = [
        "Alloy A", "Alloy B", "Alloy C", "Alloy D", "Alloy E", "Alloy F", "Alloy G", "Alloy H",
    ];
    // The first two alloys' readings, drawn as they are until the data is randomized.
    const READINGS: [[(f64, f64); 6]; 2] = [
        [
            (12.0, 2.4),
            (18.0, 3.1),
            (24.0, 3.0),
            (31.0, 4.2),
            (37.0, 4.6),
            (44.0, 5.6),
        ],
        [
            (14.0, 1.6),
            (20.0, 2.0),
            (27.0, 2.6),
            (33.0, 2.5),
            (39.0, 3.4),
            (46.0, 3.9),
        ],
    ];
    let seed = Signal::new(0u64);
    let size = Signal::new(60.0f64);
    let total = Signal::new(12.0f64);
    let series = Signal::new(2usize);
    let dist = Signal::new(0usize);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            distribution_menu(dist),
            count(
                res::str::ex_count(),
                total,
                12.0..=1000.0,
                "charts-count",
                "charts-count-value",
            ),
            stepper(
                res::str::ex_series(),
                series,
                1..=ALLOYS.len(),
                ["charts-series-less", "charts-series", "charts-series-more"],
            ),
            amount(
                res::str::ex_symbol_size(),
                size,
                10.0..=240.0,
                "charts-symbol-size",
            ),
        ]),
        chart(move || {
            let (s, area) = (seed.get(), size.get());
            let (n, k) = (total.get().round() as usize, series.get().max(1));
            let distribution = DISTRIBUTIONS[dist.get().min(DISTRIBUTIONS.len() - 1)];
            let mut marks = Vec::with_capacity(n);
            for (a, alloy) in ALLOYS.iter().take(k).enumerate() {
                // The points split as evenly as they go, the first alloys taking the remainder.
                let share = n / k + usize::from(a < n % k);
                let mut rng = Rng::new(s, 70 + a as u64);
                // The series' traits come from a stream of their own, so choosing a
                // distribution never reshuffles another's draws.
                let mut traits_rng = Rng::new(s, 170 + a as u64);
                let traits = Traits {
                    slope: rng.range(0.05, 0.14),
                    center: (traits_rng.range(14.0, 44.0), traits_rng.range(1.2, 5.8)),
                    phase: traits_rng.range(0.0, std::f64::consts::TAU),
                };
                for i in 0..share {
                    let reading = READINGS.get(a).and_then(|r| r.get(i));
                    let (load, strain) = match reading {
                        Some(&reading) if s == 0 && distribution == Distribution::Trend => reading,
                        _ => distribution.sample(&mut rng, &traits),
                    };
                    marks.push(
                        point(value("Load (kN)", load), value("Strain (mm)", strain))
                            .by_series(value("Alloy", *alloy))
                            .by_symbol(value("Alloy", *alloy))
                            .symbol_size(area),
                    );
                }
            }
            marks
        })
        .x_label("Load (kN)")
        .y_label("Strain (mm)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestMark)
        .guides(Guides::CROSSHAIR)
        .animated()
        .id("scatter")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Both axes categorical and the value in the color: a `rect` mark per cell, `sequential` for the
/// ramp, and a per-cell annotation color, because no one color reads on both ends of a ramp.
/// Animated, each cell's color blends to its new value's color through a perceptual color space,
/// so a mid-transition cell is a mid-transition value.
fn heatmap_chart() -> impl Piece {
    const HOURS: [&str; 6] = ["09", "11", "13", "15", "17", "19"];
    const DAYS: [(&str, [f64; 6]); 4] = [
        ("Mon", [12.0, 34.0, 48.0, 41.0, 28.0, 15.0]),
        ("Tue", [15.0, 38.0, 52.0, 44.0, 31.0, 17.0]),
        ("Wed", [18.0, 42.0, 61.0, 49.0, 35.0, 21.0]),
        ("Thu", [22.0, 47.0, 68.0, 57.0, 39.0, 24.0]),
    ];
    let seed = Signal::new(0u64);
    let palette = Signal::new(0usize);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(
                res::str::ex_palette(),
                &[
                    res::str::ex_palette_sequential().format(),
                    res::str::ex_palette_diverging().format(),
                ],
                palette,
                "charts-palette",
            ),
        ]),
        chart(move || {
            let s = seed.get();
            let diverging_palette = palette.get() == 1;
            let mut rng = Rng::new(s, 80);
            let grid: Vec<(&str, Vec<f64>)> = DAYS
                .iter()
                .map(|(day, values)| {
                    let v = if s == 0 {
                        values.to_vec()
                    } else {
                        (0..6).map(|_| rng.range(5.0, 70.0).round()).collect()
                    };
                    (*day, v)
                })
                .collect();
            let peak = grid
                .iter()
                .flat_map(|(_, v)| v.iter().copied())
                .fold(1.0, f64::max);
            let mut marks = Vec::new();
            for (day, values) in &grid {
                for (hour, visits) in HOURS.iter().zip(values) {
                    let t = visits / peak;
                    // Colors blend in OKLab as the palette changes, so a cell passes through
                    // the colors between rather than through grey.
                    let (fill, light) = if diverging_palette {
                        (diverging(t), (0.2..0.8).contains(&t))
                    } else {
                        (sequential(t), t >= 0.6)
                    };
                    marks.push(
                        rect(value("Hour", *hour), value("Day", *day))
                            .foreground(fill)
                            .annotation(AnnotationPosition::Overlay, format!("{visits:.0}"))
                            .annotation_color(if !light {
                                Color::WHITE
                            } else {
                                Color::rgba(0.0, 0.0, 0.0, 0.75)
                            }),
                    );
                }
            }
            marks
        })
        .no_grid()
        .animated()
        .id("heatmap")
        .grow(),
    ))
    .spacing(6.0)
    .grow()
}

// ---------------------------------------------------------------------------
// Scales and small charts
// ---------------------------------------------------------------------------

/// A calendar axis: `time` takes an instant, and the axis labels itself by calendar rather than
/// by number, so the ticks land on dates a reader recognizes. The range picker changes the window:
/// animated, the time axis rescales while the samples already in view slide along it and the new
/// ones arrive from beyond its edge.
fn time_chart() -> impl Piece {
    const DAY: f64 = 86_400.0;
    // 2026-01-05, a Monday; one close a week for a year.
    const START: f64 = 1_767_571_200.0;
    const WEEKS: [usize; 4] = [5, 13, 26, 52];
    let ranges = ["1M", "3M", "6M", "1Y"].map(String::from);
    let seed = Signal::new(0u64);
    let range = Signal::new(1usize);
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        controls(vec![
            randomize(seed).any(),
            choice(res::str::ex_range(), &ranges, range, "charts-range"),
        ]),
        chart(move || {
            let closes = Rng::new(seed.get(), 90).walk(52, 150.0, 260.0, 9.0);
            let n = WEEKS[range.get().min(WEEKS.len() - 1)];
            closes
                .iter()
                .enumerate()
                .skip(52 - n)
                .map(|(week, close)| {
                    {
                        day_piece_charts::line(
                            time("Date", START + week as f64 * 7.0 * DAY),
                            value("Close", *close),
                        )
                    }
                    .line_width(2.0)
                    .rounded()
                })
                .collect()
        })
        .y_label("Close (USD)")
        .y_axis_trailing()
        .x_tick_count(4)
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .animated()
        .id("time")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// A chart small enough to sit beside a number: `bare` drops the axes, the grid and the legend and
/// draws the marks edge to edge, so the shape of the series is all that is left.
fn sparkline() -> impl Piece {
    const STATS: [(&str, &str, [f64; 12]); 3] = [
        (
            "Sessions",
            "58.2k",
            [
                18.0, 21.0, 20.0, 24.0, 23.0, 27.0, 31.0, 29.0, 34.0, 38.0, 41.0, 46.0,
            ],
        ),
        (
            "Signups",
            "1,204",
            [
                42.0, 39.0, 44.0, 41.0, 37.0, 40.0, 36.0, 38.0, 33.0, 35.0, 31.0, 29.0,
            ],
        ),
        (
            "Revenue",
            "$92.4k",
            [
                12.0, 14.0, 13.0, 17.0, 19.0, 18.0, 22.0, 26.0, 24.0, 28.0, 33.0, 37.0,
            ],
        ),
    ];
    let seed = Signal::new(0u64);
    let mut rows: Vec<AnyPiece> = vec![controls(vec![randomize(seed).any()]).any()];
    for (i, (name, total, samples)) in STATS.iter().enumerate() {
        let samples = *samples;
        let ink = categorical(i);
        rows.push(
            row((
                label(*name).font(Font::Caption),
                label(*total).font(Font::Title3),
            ))
            .spacing(8.0)
            .any(),
        );
        rows.push(
            chart(move || {
                let s = seed.get();
                let values = if s == 0 {
                    samples.to_vec()
                } else {
                    Rng::new(s, 100 + i as u64).walk(12, 8.0, 50.0, 6.0)
                };
                values
                    .iter()
                    .enumerate()
                    .flat_map(|(tick, v)| {
                        let x = value("Tick", tick as f64);
                        let y = value("Value", *v);
                        [
                            area(x.clone(), y.clone()).gradient(
                                Color::rgba(ink.r, ink.g, ink.b, 0.45),
                                Color::rgba(ink.r, ink.g, ink.b, 0.0),
                            ),
                            { day_piece_charts::line(x, y) }
                                .foreground(ink)
                                .line_width(2.0)
                                .rounded(),
                        ]
                    })
                    .collect()
            })
            .bare()
            .animated()
            .height(84.0)
            .grow_w()
            .any(),
        );
    }
    column(PieceVec(rows))
        .spacing(10.0)
        .align(HAlign::Leading)
        .id("sparkline")
        .grow_w()
}
