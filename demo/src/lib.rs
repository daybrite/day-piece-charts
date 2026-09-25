// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Charts Demo: the demo and on-device test app for `day-piece-charts`.
//!
//! A navigation app of one page per chart. The first page is this app's own: a picker of five
//! compositions of one data set, the chart, a readout of what the pipeline derives from it, and a
//! slider that changes how much data there is. The rest are the gallery from `src/examples.rs`,
//! the illustrative charts the crate's README documents, one route each.
//!
//! The route is the address: it is the URL hash on web-dom, so <https://…/#lines> opens the line
//! chart and the browser's back button walks the pages, and it is the name
//! `dayscript`'s `navigate:` step uses, which is how both walkthroughs move between charts. Every
//! element carries a stable id, so they can assert what is on the page as well as reach it.
//!
//! A chart is drawn on the canvas, so a walkthrough cannot read it the way it reads a label. What
//! it can read is the arithmetic the chart runs before it draws (the inferred domain and the tick
//! labelling), and that is what the readout below shows, from the same public functions the chart
//! itself calls. `tests/grammar.rs` in the crate asserts the same numbers on the host.

use day::prelude::*;
use day_piece_charts::select::{Guides, Selection, Snap};
use day_piece_charts::ticks::{self, LabelFit};
use day_piece_charts::{
    AnnotationPosition, Coordinate, Datum, Interpolation, Interval, LegendPosition, Mark, Stacking,
    bar, chart, rect, resolve, sector, sequential, value,
};

/// The charts the README documents, one page each.
mod examples;

// The entry point on iOS, Android, and the web; a plain cargo desktop build enters through
// src/main.rs.
day::day_start!(options: window(), root);

// Typed constants for everything under `resource/` (https://daybrite.dev/docs/resources).
day::resources!();

/// The x categories, in order.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Monthly revenue per region, in thousands. Fixed data, because the walkthrough asserts the
/// numbers the pipeline derives from it; a change here is a change to `dayscript/charts.yaml`.
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
/// a different chart type, which is the crate's claim, so the demo shows it as a choice.
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
    /// One cell per region per month, colored by revenue: both axes categorical.
    HeatMap,
}

const COMPOSITIONS: [Composition; 5] = [
    Composition::Grouped,
    Composition::Line,
    Composition::Stacked,
    Composition::Donut,
    Composition::HeatMap,
];

/// The compositions' picker entries, in the same order.
const COMPOSITION_LABELS: [fn() -> day::LocalizedText; 5] = [
    res::str::comp_grouped,
    res::str::comp_line,
    res::str::comp_stacked,
    res::str::comp_donut,
    res::str::comp_heatmap,
];

/// The largest value in the data, which is the top of the heat map's ramp.
const PEAK: f64 = 47.0;

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

day::routes! {
    /// One route per page (https://daybrite.dev/docs/navigation). The key is the app's route,
    /// which day-dom reflects into the URL hash, so `…/#bars-grouped` opens that chart on a fresh
    /// load and the browser's history walks the gallery. `dayscript`'s `navigate:` addresses the
    /// same names, and so does the README's "View live" link beside each example's source.
    pub(crate) enum Page {
        Pipeline => "pipeline",
        Lines => "lines",
        LinesTarget => "lines-target",
        Area => "area",
        AreaStacked => "area-stacked",
        Bars => "bars",
        BarsGrouped => "bars-grouped",
        Bars100 => "bars-100",
        BarsHorizontal => "bars-horizontal",
        Pie => "pie",
        Donut => "donut",
        Scatter => "scatter",
        Heatmap => "heatmap",
        Time => "time",
        Sparkline => "sparkline",
    }
}

/// The whole app: a sidebar of the demo's own page and the gallery, and whichever chart is open.
pub fn root() -> impl Piece {
    info!("Charts Demo starting");

    // The open page. The nav writes it, a launch deep link writes it before the first frame (the
    // URL hash on web-dom, `DAY_DEEPLINK` elsewhere), and the browser's back button writes it
    // again.
    let page = Signal::new(Page::Pipeline);
    let mut nav = nav(page)
        .style(NavStyle::Sidebar)
        .title(res::str::app_title())
        .section(res::str::nav_demo())
        .item(Page::Pipeline, res::str::nav_pipeline(), pipeline_page)
        .section(res::str::nav_examples());
    // One row per example, in the order the README documents them. Each row's page is the chart
    // itself under its name; the chart's own id is the route key, so a screenshot, a route and an
    // id are all one name.
    for ex in examples::gallery() {
        let (title, build) = (ex.title, ex.build);
        nav = nav.item(ex.page, title(), move || example_page(title, build));
    }
    nav.id("nav")
}

/// One gallery page: the example's name, and the chart the README prints under it.
fn example_page(title: fn() -> day::LocalizedText, build: fn() -> AnyPiece) -> impl Piece {
    column((
        label(title())
            .font(Font::Headline)
            .id("charts-example-title"),
        build(),
    ))
    .spacing(8.0)
    .padding(16.0)
    .grow()
}

/// This app's own page: the five compositions of its data, what the pipeline derives from it, and
/// the control that changes how much of it there is.
fn pipeline_page() -> impl Piece {
    // Which of the five compositions is drawn. The picker writes it; the marks closure reads it,
    // so the chart re-records when it changes.
    let picked = Signal::new(0usize);
    // How many months are plotted. Drives the marks closure and every number in the readout.
    let months = Signal::new(DEFAULT_MONTHS);
    // What the pointer is over on the Cartesian chart, owned here because two pieces read it: the
    // chart draws its guides from it, and the readout names it.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    // A window shorter than the Expanded height class (a phone at 720 points, a tablet in
    // landscape) cannot fit the picker, the readout, the slider AND a usable plot: grown into
    // what the controls leave, the chart came out 16 points tall on the HarmonyOS phone, with no
    // plot to read or tap. So a short window scrolls, and the chart gets a fixed height there.
    // `size_class()` is tracked (docs/size-classes.md), so a window crossing the breakpoint
    // re-lays out the page.
    let tall = day::size_class().is_none_or(|c| c.height == HeightClass::Expanded);

    let chart_area = column((when(
        move || composition(picked.get()) == Composition::Donut,
        move || donut(months),
    )
    .otherwise(move || {
        when(
            move || composition(picked.get()) == Composition::HeatMap,
            move || heat_map(months),
        )
        .otherwise(move || cartesian(picked, months, selected))
    }),))
    .align(HAlign::Center);
    // In a tall window the plot takes every point the page has left after the picker, the
    // readout and the slider, so a window resized in either direction re-records the chart at
    // the new size instead of padding around a fixed one.
    let chart_area = if tall {
        chart_area.grow().any()
    } else {
        chart_area.height(SHORT_WINDOW_CHART_HEIGHT).any()
    };

    let page = column((
        labeled(
            res::str::composition(),
            picker(
                COMPOSITION_LABELS
                    .iter()
                    .map(|name| name().format())
                    .collect::<Vec<_>>(),
                picked,
            )
            .id("charts-composition-picker"),
        ),
        // A donut needs polar coordinates, and the coordinate system is a property of the chart
        // rather than of its marks, so it lives in its own subtree and `when` swaps to it. The
        // heat map is Cartesian but titles its y axis by the row column rather than by revenue,
        // so it is a third subtree; the rest share one chart and differ only in the marks.
        chart_area,
        readout(picked, months, selected),
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
    .padding(16.0);
    if tall { page.any() } else { scroll(page).any() }
}

/// The chart's height when the page scrolls: room for the legend, the axes and a plot a finger
/// can pick a bar in, on a phone.
const SHORT_WINDOW_CHART_HEIGHT: f64 = 300.0;

/// The Cartesian chart: grouped bars, lines, or a stack, depending on the picker.
fn cartesian(
    picked: Signal<usize>,
    months: Signal<f64>,
    selected: Signal<Option<Selection>>,
) -> impl Piece {
    chart(move || marks(composition(picked.get()), month_count(months.get())))
        .y_label(res::str::revenue().format())
        .legend(LegendPosition::Bottom)
        // Hover, tap or drag writes what is under the pointer into `selected`; the chart reads the
        // same signal back to draw a rule, a ring on each mark and a box naming their values, and
        // the readout below puts the same selection into words.
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .id("charts-plot")
        .grow()
}

/// The same two regions as a donut: one sector per region, summed over the months in view.
///
/// Sectors stack by default, so this is a stacked bar the coordinate system bends into a ring;
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
    .grow()
}

/// The same numbers as a grid: months across, regions down, revenue as the cell's color on the
/// sequential ramp: the composition where both axes are bands, which is what a rect mark with
/// a categorical y asks for. Every cell also prints its value, in white on the dark end of the
/// ramp and in the label color on the light end, because no one color reads on both.
fn heat_map(months: Signal<f64>) -> impl Piece {
    chart(move || {
        let n = month_count(months.get());
        let mut out = Vec::with_capacity(SERIES.len() * n);
        for (region, values) in SERIES.iter() {
            for (month, v) in MONTHS.iter().zip(values.iter()).take(n) {
                let t = v / PEAK;
                out.push(
                    rect(value("Month", *month), value("Region", *region))
                        .foreground(sequential(t))
                        .annotation(AnnotationPosition::Overlay, number(*v))
                        .annotation_color(if t < 0.6 {
                            Color::WHITE
                        } else {
                            Color::rgba(0.0, 0.0, 0.0, 0.75)
                        }),
                );
            }
        }
        out
    })
    .no_grid()
    .id("charts-plot")
    .grow()
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
                // instead of stacked on each other. Both halves are needed: bars stack by
                // default, and binding the dodge channel does not by itself turn that off, so
                // without the `Unstacked` this draws the stacked composition below.
                Composition::Grouped => bar(x, y)
                    .by_series(series.clone())
                    .by_position(series)
                    .stacking(Stacking::Unstacked),
                Composition::Stacked => bar(x, y).by_series(series).stacking(Stacking::Standard),
                // `line` is qualified because `day::prelude` exports a `line` shape of its own.
                // Monotone interpolation is the right curve for revenue: it cannot dip below a
                // value the data never took.
                Composition::Line | Composition::Donut | Composition::HeatMap => {
                    { day_piece_charts::line(x, y) }
                        .by_series(series)
                        .interpolation(Interpolation::Monotone)
                }
            });
        }
    }
    out
}

/// What the chart works out before it draws anything: the composition it assembled, how many
/// marks that is, the series it found, the domain those values imply, and the tick labelling the
/// extended-Wilkinson search picks for that domain.
fn readout(
    picked: Signal<usize>,
    months: Signal<f64>,
    selected: Signal<Option<Selection>>,
) -> impl Piece {
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
        // The one row that comes from the pointer rather than from the data: what the chart
        // reported into `selected` on the last hover, tap or drag.
        labeled(
            res::str::fact_selection(),
            label(move || match selected.get() {
                Some(sel) => format!(
                    "{} \u{b7} {}",
                    sel.x_label,
                    sel.values
                        .iter()
                        .map(|v| format!("{}: {}", v.series, v.label))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None => res::str::fact_selection_none().format(),
            })
            .id("charts-selection"),
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
        Composition::HeatMap => "rect, colored by Revenue",
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

/// The interval the plotted values span: `resolve::domain_of`, the function the chart calls on
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
