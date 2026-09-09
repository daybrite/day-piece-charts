// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Charts Demo — the demo and on-device test app for `day-piece-charts`.
//!
//! One page: a picker of chart compositions, the chart itself, a readout of what the pipeline
//! derives from the data, and a slider that changes how much data there is. Every element carries
//! a stable id, so `dayscript/charts.yaml` can assert all of it on the iOS Simulator and the
//! Android emulator.
//!
//! A chart is drawn on the canvas, so a walkthrough cannot read it the way it reads a label. What
//! it can read is the arithmetic the chart runs before it draws — the inferred domain and the tick
//! labelling — and that is what the readout below shows, from the same public functions the chart
//! itself calls. `tests/grammar.rs` in the crate asserts the same numbers on the host.

use day::prelude::*;
use day_piece_charts::ticks::{self, LabelFit};
use day_piece_charts::{
    Coordinate, Datum, Interpolation, Interval, LegendPosition, Mark, Stacking, bar, chart,
    resolve, sector, value,
};

// The mobile entry point; a plain cargo desktop build enters through src/main.rs.
day::day_start!(options: window(), root);

// Typed constants for everything under `resource/` (https://daybrite.dev/docs/resources).
day::resources!();

/// The x categories, in order.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Monthly revenue per region, in thousands. Fixed data, because the walkthrough asserts the
/// numbers the pipeline derives from it — a change here is a change to `dayscript/charts.yaml`.
const SERIES: [(&str, [f64; 12]); 2] = [
    (
        "North",
        [
            12.0, 18.0, 15.0, 22.0, 28.0, 24.0, 31.0, 35.0, 29.0, 38.0, 42.0, 47.0,
        ],
    ),
    (
        "South",
        [
            8.0, 11.0, 14.0, 12.0, 19.0, 23.0, 21.0, 26.0, 30.0, 27.0, 33.0, 36.0,
        ],
    ),
];

/// How many months the slider starts on.
const DEFAULT_MONTHS: f64 = 6.0;

/// The four compositions the picker offers. Each is a different assembly of the same grammar, not
/// a different chart type — which is the point the crate makes, so the demo shows it as a choice.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Composition {
    /// Bars dodged side by side within each month's band.
    Grouped,
    /// One monotone line per region.
    Line,
    /// Bars stacked into a monthly total.
    Stacked,
    /// The same stack in polar coordinates, which is a donut.
    Donut,
}

const COMPOSITIONS: [Composition; 4] = [
    Composition::Grouped,
    Composition::Line,
    Composition::Stacked,
    Composition::Donut,
];

/// The picker's selection, clamped so a stale index never panics.
fn composition(index: usize) -> Composition {
    COMPOSITIONS[index.min(COMPOSITIONS.len() - 1)]
}

/// The slider's value as a month count, clamped to the data that exists.
fn month_count(slider: f64) -> usize {
    (slider.round().max(1.0) as usize).min(MONTHS.len())
}

/// The window every entry point opens.
pub fn window() -> day::WindowOptions {
    day::WindowOptions {
        locales: Some((res::locales::DEFAULT, res::locales::CATALOG)),
        title_fn: Some(|| res::str::app_title().format()),
        size: day::prelude::Size::new(480.0, 800.0),
        ..Default::default()
    }
}

/// The whole app: title, composition picker, the chart, the pipeline's readout, the data control.
pub fn root() -> impl Piece {
    info!("Charts Demo starting");

    // Which composition is drawn. The picker writes it; the marks closure reads it, so the chart
    // re-records when it changes.
    let picked = Signal::new(0usize);
    // How many months are plotted. Drives the marks closure and every number in the readout.
    let months = Signal::new(DEFAULT_MONTHS);

    column((
        label(res::str::app_title())
            .font(Font::Title)
            .id("charts-title"),
        labeled(
            res::str::composition(),
            picker(
                [
                    res::str::comp_grouped().format(),
                    res::str::comp_line().format(),
                    res::str::comp_stacked().format(),
                    res::str::comp_donut().format(),
                ],
                picked,
            )
            .id("charts-composition-picker"),
        ),
        // A donut needs polar coordinates, and the coordinate system is a property of the chart
        // rather than of its marks — so the two live in two subtrees and `when` swaps between
        // them. Everything Cartesian shares one chart and differs only in the marks it records.
        column((when(
            move || composition(picked.get()) == Composition::Donut,
            move || donut(months),
        )
        .otherwise(move || cartesian(picked, months)),))
        .align(HAlign::Center)
        .grow_w(),
        readout(picked, months),
        section((labeled(
            res::str::months(),
            row((
                slider(months)
                    .range(3.0..=12.0)
                    .step(1.0)
                    .id("charts-months"),
                label(move || month_count(months.get()).to_string()).id("charts-months-value"),
            ))
            .spacing(8.0),
        ),))
        .title(res::str::data_section()),
    ))
    .spacing(12.0)
    .padding(16.0)
}

/// The Cartesian chart: grouped bars, lines, or a stack, depending on the picker.
fn cartesian(picked: Signal<usize>, months: Signal<f64>) -> impl Piece {
    chart(move || marks(composition(picked.get()), month_count(months.get())))
        .y_label(res::str::revenue().format())
        .legend(LegendPosition::Bottom)
        .id("charts-plot")
        .frame(320.0, 220.0)
}

/// The same two regions as a donut: one sector per region, summed over the months in view.
///
/// Sectors stack by default, so this is a stacked bar the coordinate system bends into a ring —
/// there is no pie-specific code here or in the crate.
fn donut(months: Signal<f64>) -> impl Piece {
    chart(move || {
        let n = month_count(months.get());
        SERIES
            .iter()
            .map(|(region, values)| {
                sector(value("Revenue", total(values, n))).by_series(value("Region", *region))
            })
            .collect()
    })
    .coordinate(Coordinate::donut(0.55))
    .legend(LegendPosition::Bottom)
    .no_grid()
    .x_axis_hidden()
    .y_axis_hidden()
    .id("charts-plot")
    .frame(320.0, 220.0)
}

/// One mark per region per month, assembled the way the picked composition asks for.
fn marks(comp: Composition, n: usize) -> Vec<Mark> {
    let mut out = Vec::with_capacity(SERIES.len() * n);
    for (region, values) in SERIES.iter() {
        for (month, v) in MONTHS.iter().zip(values.iter()).take(n) {
            let x = value("Month", *month);
            let y = value("Revenue", *v);
            let series = value("Region", *region);
            out.push(match comp {
                // Dodging is a position adjustment: the same bars, offset within the band
                // instead of stacked on each other. Both halves are needed — bars stack by
                // default, and binding the dodge channel does not by itself turn that off, so
                // without the `Unstacked` this draws the stacked composition below.
                Composition::Grouped => bar(x, y)
                    .by_series(series.clone())
                    .by_position(series)
                    .stacking(Stacking::Unstacked),
                Composition::Stacked => bar(x, y).by_series(series).stacking(Stacking::Standard),
                // `line` is qualified because `day::prelude` exports a `line` shape of its own.
                // Monotone interpolation is the honest curve for revenue: it cannot dip below a
                // value the data never took.
                Composition::Line | Composition::Donut => day_piece_charts::line(x, y)
                    .by_series(series)
                    .interpolation(Interpolation::Monotone),
            });
        }
    }
    out
}

/// What the chart works out before it draws anything: the composition it assembled, how many
/// marks that is, the series it found, the domain those values imply, and the tick labelling the
/// extended-Wilkinson search picks for that domain.
fn readout(picked: Signal<usize>, months: Signal<f64>) -> impl Piece {
    section((
        labeled(
            res::str::fact_grammar(),
            label(move || grammar(composition(picked.get()))).id("charts-grammar"),
        ),
        labeled(
            res::str::fact_marks(),
            label(move || {
                mark_count(composition(picked.get()), month_count(months.get())).to_string()
            })
            .id("charts-marks"),
        ),
        labeled(
            res::str::fact_series(),
            label(|| {
                SERIES
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .id("charts-series"),
        ),
        labeled(
            res::str::fact_domain(),
            label(move || {
                let iv = domain(month_count(months.get()));
                format!("{} \u{2013} {}", number(iv.lo), number(iv.hi))
            })
            .id("charts-domain"),
        ),
        labeled(
            res::str::fact_ticks(),
            label(move || tick_summary(month_count(months.get()))).id("charts-ticks"),
        ),
    ))
    .title(res::str::facts_section())
}

/// The grammar the picker composed, in the crate's own vocabulary.
fn grammar(comp: Composition) -> String {
    match comp {
        Composition::Grouped => "bar, dodged by Region",
        Composition::Line => "line, monotone",
        Composition::Stacked => "bar, stacked",
        Composition::Donut => "sector, polar",
    }
    .to_string()
}

/// How many marks that composition records. A donut sums each region to one wedge; everything
/// else keeps one mark per region per month.
fn mark_count(comp: Composition, n: usize) -> usize {
    match comp {
        Composition::Donut => SERIES.len(),
        _ => SERIES.len() * n,
    }
}

/// The revenue a region made over the first `n` months.
fn total(values: &[f64; 12], n: usize) -> f64 {
    values.iter().take(n).sum()
}

/// The interval the plotted values span — `resolve::domain_of`, the function the chart calls on
/// its own way to a scale.
///
/// The drawn axis is usually wider than this: a bar or an area includes its baseline, and a stack
/// sums its column, both of which happen after this point. So this is the range of the data, not
/// the range of the axis, and the readout says so.
fn domain(n: usize) -> Interval {
    let data: Vec<Datum> = SERIES
        .iter()
        .flat_map(|(_, values)| values.iter().take(n).map(|v| Datum::Number(*v)))
        .collect();
    resolve::domain_of(&data)
}

/// The labelling the extended-Wilkinson search picks for that domain.
///
/// The fit measures text at a fixed six points per character rather than through the toolkit, so
/// this readout is the search's answer and reads the same on every platform the walkthrough runs
/// on. The chart itself measures real text, which is what lets a live axis relabel on resize.
fn tick_summary(n: usize) -> String {
    let fit = LabelFit {
        axis_length: 220.0,
        gap: 4.0,
        measure: &|s: &str| s.len() as f64 * 6.0,
    };
    let labelling = ticks::extended(domain(n), 5, &fit);
    format!(
        "{} labels, step {}",
        labelling.values.len(),
        number(labelling.step)
    )
}

/// Whole numbers without a trailing `.0`; anything else to one place.
fn number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}
