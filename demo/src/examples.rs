// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! The illustrative charts the crate's README documents, one function each.
//!
//! Every entry carries its own data and reads as a complete chart, because the README prints the
//! same source beside the screenshot `dayscript/gallery.yaml` captures here. The two live in one
//! file so that the documented source and the pictured chart stay the same chart.
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
//! Each chart ends `.grow()`, so it takes the whole of the space its page has left over in both
//! directions: a window resized in either one re-records the chart at the new size, and the axis
//! relabels itself for the width it actually has rather than the one it was authored at.
//! `.frame(w, h)` fixes both dimensions instead, and `.height(h).grow_w()` fixes one.
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
        e(Page::Donut, res::str::ex_donut, || donut_chart().any()),
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

// ---------------------------------------------------------------------------
// Lines
// ---------------------------------------------------------------------------

/// A line through one series, with the samples marked.
///
/// The monotone spline is a curve, not a styling choice: it is constrained never to overshoot, so
/// the line cannot dip below a value the data never took.
fn line_chart() -> impl Piece {
    const REVENUE: [(&str, f64); 6] = [
        ("Jan", 12.0),
        ("Feb", 18.0),
        ("Mar", 15.0),
        ("Apr", 22.0),
        ("May", 28.0),
        ("Jun", 24.0),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            REVENUE
                .iter()
                .flat_map(|(month, revenue)| {
                    let x = value("Month", *month);
                    let y = value("Revenue", *revenue);
                    [
                        { day_piece_charts::line(x.clone(), y.clone()) }
                            .interpolation(Interpolation::Monotone)
                            .line_width(2.0)
                            .rounded(),
                        point(x, y).symbol_size(40.0),
                    ]
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
        .id("lines")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Two series and a target line: `by_series` splits the data and names the legend entries, and a
/// `rule_y` is a horizontal line at a value, dashed and annotated here.
fn line_series() -> impl Piece {
    const MONTHS: [&str; 6] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
    const REGIONS: [(&str, [f64; 6]); 2] = [
        ("North", [12.0, 18.0, 15.0, 22.0, 28.0, 24.0]),
        ("South", [8.0, 11.0, 14.0, 12.0, 19.0, 23.0]),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            let mut marks: Vec<Mark> = REGIONS
                .iter()
                .flat_map(|(region, values)| {
                    MONTHS.iter().zip(values).map(move |(month, revenue)| {
                        {
                            day_piece_charts::line(
                                value("Month", *month),
                                value("Revenue", *revenue),
                            )
                        }
                        .by_series(value("Region", *region))
                        .line_width(2.0)
                        .rounded()
                    })
                })
                .collect();
            marks.push(
                rule_y(value("Target", 20.0))
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
    const SESSIONS: [(&str, f64); 7] = [
        ("Mon", 320.0),
        ("Tue", 410.0),
        ("Wed", 385.0),
        ("Thu", 520.0),
        ("Fri", 610.0),
        ("Sat", 455.0),
        ("Sun", 390.0),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            let ink = categorical(0);
            SESSIONS
                .iter()
                .flat_map(|(day, sessions)| {
                    let x = value("Day", *day);
                    let y = value("Sessions", *sessions);
                    [
                        area(x.clone(), y.clone())
                            .interpolation(Interpolation::Monotone)
                            .gradient(
                                Color::rgba(ink.r, ink.g, ink.b, 0.55),
                                Color::rgba(ink.r, ink.g, ink.b, 0.02),
                            ),
                        { day_piece_charts::line(x, y) }
                            .interpolation(Interpolation::Monotone)
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
        .id("area")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Three series stacked into a total. Areas stack by default, so the composition is the marks
/// themselves; `opacity` keeps the bands distinct where they meet.
fn area_stacked() -> impl Piece {
    const QUARTERS: [&str; 5] = ["Q1", "Q2", "Q3", "Q4", "Q5"];
    const CHANNELS: [(&str, [f64; 5]); 3] = [
        ("Direct", [18.0, 22.0, 25.0, 31.0, 36.0]),
        ("Partner", [12.0, 14.0, 19.0, 21.0, 24.0]),
        ("Online", [6.0, 11.0, 14.0, 20.0, 29.0]),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            CHANNELS
                .iter()
                .flat_map(|(channel, values)| {
                    QUARTERS.iter().zip(values).map(move |(quarter, revenue)| {
                        area(value("Quarter", *quarter), value("Revenue", *revenue))
                            .by_series(value("Channel", *channel))
                            .interpolation(Interpolation::Monotone)
                            .opacity(0.85)
                    })
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
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
/// placed on the plot, so it follows the bar as the data changes.
fn bar_chart() -> impl Piece {
    const REVENUE: [(&str, f64); 6] = [
        ("Jan", 12.0),
        ("Feb", 18.0),
        ("Mar", 15.0),
        ("Apr", 22.0),
        ("May", 28.0),
        ("Jun", 24.0),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            REVENUE
                .iter()
                .map(|(month, revenue)| {
                    bar(value("Month", *month), value("Revenue", *revenue))
                        .corner_radius(4.0)
                        .annotation(AnnotationPosition::Top, format!("{revenue:.0}"))
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
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
    const MONTHS: [&str; 6] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
    const REGIONS: [(&str, [f64; 6]); 2] = [
        ("North", [12.0, 18.0, 15.0, 22.0, 28.0, 24.0]),
        ("South", [8.0, 11.0, 14.0, 12.0, 19.0, 23.0]),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            REGIONS
                .iter()
                .flat_map(|(region, values)| {
                    MONTHS.iter().zip(values).map(move |(month, revenue)| {
                        bar(value("Month", *month), value("Revenue", *revenue))
                            .by_series(value("Region", *region))
                            .by_position(value("Region", *region))
                            .stacking(Stacking::Unstacked)
                            .corner_radius(3.0)
                    })
                })
                .collect()
        })
        .y_label("Revenue (thousands)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestX)
        .guides(Guides::RULE)
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
    chart(|| {
        CHANNELS
            .iter()
            .flat_map(|(channel, values)| {
                QUARTERS.iter().zip(values).map(move |(quarter, revenue)| {
                    bar(value("Quarter", *quarter), value("Revenue", *revenue))
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
    .id("bars-100")
    .grow()
}

/// Bars along the value axis, for categories whose names need the room: the y channel takes the
/// category and `x_range` spans the bar from zero to its value, which is `BarMark(xStart:xEnd:)`.
///
/// A bar's x is a band by default, because that is what a bar chart's x usually is, so a
/// horizontal one names its value axis with `x_scale_kind`.
fn bar_horizontal() -> impl Piece {
    const LANGUAGES: [(&str, f64); 5] = [
        ("Rust", 86.0),
        ("Swift", 62.0),
        ("Kotlin", 54.0),
        ("TypeScript", 47.0),
        ("Python", 38.0),
    ];
    chart(|| {
        LANGUAGES
            .iter()
            .map(|(language, share)| {
                bar(value("Share", *share), value("Language", *language))
                    .x_range(value("Share", 0.0), value("Share", *share))
                    .corner_radius(4.0)
            })
            .collect()
    })
    .x_scale_kind(ScaleKind::Linear)
    .x_label("Share of respondents")
    .id("bars-horizontal")
    .grow()
}

// ---------------------------------------------------------------------------
// Polar
// ---------------------------------------------------------------------------

/// A pie is a normalized stack in polar coordinates, which is how it is implemented: one `sector`
/// per slice, and `angular_inset` holds an even channel between neighbours.
fn pie_chart() -> impl Piece {
    const TRAFFIC: [(&str, f64); 4] = [
        ("Search", 46.0),
        ("Direct", 28.0),
        ("Social", 17.0),
        ("Email", 9.0),
    ];
    chart(|| {
        TRAFFIC
            .iter()
            .map(|(source, share)| {
                sector(value("Share", *share))
                    .by_series(value("Source", *source))
                    .angular_inset(2.0)
            })
            .collect()
    })
    .coordinate(Coordinate::polar())
    .legend(LegendPosition::Trailing)
    .id("pie")
    .grow()
}

/// The same slices with a hole: `Coordinate::donut` takes the hole as a fraction of the radius,
/// so one number walks a pie through a donut to a ring gauge without touching a mark.
///
/// The wedges carry no labels of their own: an annotation is placed from a mark's x position, and
/// a sector has none, so a pie names its slices through the legend.
fn donut_chart() -> impl Piece {
    const TRAFFIC: [(&str, f64); 4] = [
        ("Search", 46.0),
        ("Direct", 28.0),
        ("Social", 17.0),
        ("Email", 9.0),
    ];
    chart(|| {
        TRAFFIC
            .iter()
            .map(|(source, share)| {
                sector(value("Share", *share))
                    .by_series(value("Source", *source))
                    .angular_inset(2.0)
            })
            .collect()
    })
    .coordinate(Coordinate::donut(0.58))
    .legend(LegendPosition::Trailing)
    .id("donut")
    .grow()
}

// ---------------------------------------------------------------------------
// Points and cells
// ---------------------------------------------------------------------------

/// A scatter of two measurements. `by_symbol` gives each series its own shape, so the chart still
/// reads where color cannot be relied on, and `symbol_size` is an area, the way magnitude should
/// be encoded.
fn scatter_chart() -> impl Piece {
    const READINGS: [(&str, [(f64, f64); 6]); 2] = [
        (
            "Alloy A",
            [
                (12.0, 2.4),
                (18.0, 3.1),
                (24.0, 3.0),
                (31.0, 4.2),
                (37.0, 4.6),
                (44.0, 5.6),
            ],
        ),
        (
            "Alloy B",
            [
                (14.0, 1.6),
                (20.0, 2.0),
                (27.0, 2.6),
                (33.0, 2.5),
                (39.0, 3.4),
                (46.0, 3.9),
            ],
        ),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            READINGS
                .iter()
                .flat_map(|(alloy, samples)| {
                    samples.iter().map(move |(load, strain)| {
                        point(value("Load (kN)", *load), value("Strain (mm)", *strain))
                            .by_series(value("Alloy", *alloy))
                            .by_symbol(value("Alloy", *alloy))
                            .symbol_size(60.0)
                    })
                })
                .collect()
        })
        .x_label("Load (kN)")
        .y_label("Strain (mm)")
        .legend(LegendPosition::Bottom)
        .select(selected)
        .snap(Snap::NearestMark)
        .guides(Guides::CROSSHAIR)
        .id("scatter")
        .grow(),
        selection_readout(selected),
    ))
    .spacing(6.0)
    .grow()
}

/// Both axes categorical and the value in the color: a `rect` mark per cell, `sequential` for the
/// ramp, and a per-cell annotation color, because no one color reads on both ends of a ramp.
fn heatmap_chart() -> impl Piece {
    const HOURS: [&str; 6] = ["09", "11", "13", "15", "17", "19"];
    const DAYS: [(&str, [f64; 6]); 4] = [
        ("Mon", [12.0, 34.0, 48.0, 41.0, 28.0, 15.0]),
        ("Tue", [15.0, 38.0, 52.0, 44.0, 31.0, 17.0]),
        ("Wed", [18.0, 42.0, 61.0, 49.0, 35.0, 21.0]),
        ("Thu", [22.0, 47.0, 68.0, 57.0, 39.0, 24.0]),
    ];
    const PEAK: f64 = 68.0;
    chart(|| {
        DAYS.iter()
            .flat_map(|(day, values)| {
                HOURS.iter().zip(values).map(move |(hour, visits)| {
                    let t = visits / PEAK;
                    rect(value("Hour", *hour), value("Day", *day))
                        .foreground(sequential(t))
                        .annotation(AnnotationPosition::Overlay, format!("{visits:.0}"))
                        .annotation_color(if t < 0.6 {
                            Color::WHITE
                        } else {
                            Color::rgba(0.0, 0.0, 0.0, 0.75)
                        })
                })
            })
            .collect()
    })
    .no_grid()
    .id("heatmap")
    .grow()
}

// ---------------------------------------------------------------------------
// Scales and small charts
// ---------------------------------------------------------------------------

/// A calendar axis: `date` reads an ISO day into an instant, and the axis labels itself by
/// calendar rather than by number, so the ticks land on dates a reader recognizes.
fn time_chart() -> impl Piece {
    const CLOSES: [(&str, f64); 9] = [
        ("2026-01-05", 184.2),
        ("2026-01-12", 188.9),
        ("2026-01-19", 186.4),
        ("2026-01-26", 193.1),
        ("2026-02-02", 201.7),
        ("2026-02-09", 198.3),
        ("2026-02-16", 207.5),
        ("2026-02-23", 214.8),
        ("2026-03-02", 211.2),
    ];
    // What the pointer is over. The chart writes it, reads it back to draw the guides,
    // and the readout below reads the same signal.
    let selected: Signal<Option<Selection>> = Signal::new(None);
    column((
        chart(|| {
            CLOSES
                .iter()
                .map(|(day, close)| {
                    { day_piece_charts::line(date("Date", day), value("Close", *close)) }
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
    let mut rows: Vec<AnyPiece> = Vec::new();
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
                samples
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
