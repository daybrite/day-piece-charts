<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# day-piece-charts

[![ci](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml/badge.svg)](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MPL--2.0-green.svg)](LICENSE)

## Overview and capabilities

`day-piece-charts` draws interactive charts in Day applications using Day's canvas.

**[Try the demo in a browser](https://daybrite.github.io/day-piece-charts/)** — the
web build of [`demo/`](demo/), published from `main` on every push. The same app runs
on iOS and Android from the same source. [Examples](#examples) shows each chart below
with the code that draws it.

[Day](https://github.com/daybrite/day) is a Rust framework for building applications
from a shared codebase using each platform's native UI toolkit. A **piece** is a UI
component you place in a layout; a **part** provides a capability without drawing UI.
Day's `day` command builds and packages the Rust code, resources, and native platform
code together. `Cargo.toml` declares Rust dependencies; `Day.toml` configures the app
and its target platforms.

Build a chart by combining **marks** (bars, lines, points, areas, rectangles, rules,
or sectors) with named data values. Scales map data to positions, axes explain the
scales, and a coordinate system determines how the marks appear. This approach is
called a grammar of graphics: the same components make grouped bars, time series,
scatter plots, heat maps, pies, and donuts.

Capabilities include stacking and grouping, linear/log/power/time/category scales,
custom labels and colors, legends, annotations, gradients, and pointer/touch
selection. Data read from Day signals updates automatically. Numeric axis labels
use Day's locale-aware number formatting, and chart styling follows light/dark
appearance unless overridden.

## Platform support and limitations

The crate has no platform-specific renderer or backend features. It records Day
canvas drawing commands, so it can render on the desktop, mobile, and web backends
where Day provides a canvas. The host mock backend supports logic tests, not a
visible chart. The included demo declares every platform-toolkit pair Day supports,
and CI builds and drives it on the eight primary ones.

Chart layout and drawing logic are shared across platforms; text measurement and
input still depend on the backend. Canvas labels are drawn content, rather than
individual native text controls. Provide an accessible textual summary or data
view when users need to read chart values without seeing the canvas.

Charts need a nonzero layout size: `.grow()` takes what the container has left,
`.frame(w, h)` fixes both dimensions, and `.height(h).grow_w()` fixes one. A chart
re-records when its size changes, so an axis relabels itself for the width it has. Ordinary builder options are set at construction. For changing axis/scale
configuration, use `.configure(...)`, `.x_domain_with(...)`, or `.y_domain_with(...)`.
The marks closure is reactive by itself. For large datasets, account for full chart
re-recording on data/size changes and scans of the drawn marks during selection;
there is no separate native chart engine handling those operations.

## Add it to a Day project

Add the crate to your app's `Cargo.toml`:

```toml
[dependencies]
day-piece-charts = { git = "https://github.com/daybrite/day-piece-charts.git" }
```

Return a chart from your UI function, then place it in the app's layout:

```rust
use day::prelude::*;
use day_piece_charts::{bar, chart, value};

fn sales_chart() -> impl Piece {
    chart(|| vec![
        bar(value("Month", "Jan"), value("Revenue", 120.0)),
        bar(value("Month", "Feb"), value("Revenue", 180.0)),
        bar(value("Month", "Mar"), value("Revenue", 150.0)),
    ])
    .y_label("Revenue (USD)")
    .frame(480.0, 280.0)
}
```

Read a `Signal` inside the closure to make the data live. A signal holds observable
state; Day reruns bindings that read it when its value changes. Use `.by_series(...)`
on marks to assign a series and `.by_position(...)` to place grouped bars side by side.

No permissions, native packages, assets, or chart-specific feature forwarding are
required. Build with your app's configured Day target, for example
`day build -p android-mdc`, or run `day launch -p ios-uikit` with the corresponding
SDK installed. The [demo](demo/) is a complete Day project: five compositions of one
data set, and the gallery of examples below.

### Selection and reactive configuration

Import selection types from `day_piece_charts::select`. This example reports the
selected month and draws a guide; the app can also read `selected` to show details:

```rust
use day::prelude::*;
use day_piece_charts::{bar, chart, value};
use day_piece_charts::select::{Guides, Snap};

fn selectable_chart() -> impl Piece {
    let selected = Signal::new(None);
    let tick_count = Signal::new(5usize);
    chart(|| vec![
        bar(value("Month", "Jan"), value("Revenue", 120.0)),
        bar(value("Month", "Feb"), value("Revenue", 180.0)),
    ])
    .configure(move |config| config.y_axis.desired_count = tick_count.get())
    .select(selected)
    .snap(Snap::NearestX)
    .guides(Guides::RULE)
    .frame(480.0, 280.0)
}
```

`NearestX` selects values across series at the nearest x position; `NearestMark`
selects one nearby mark, useful for scatter plots. `.select(...)` alone adds no
visual guides. Hover, tap, and drag feed selection; leaving with a pointer clears
it, while ending a touch drag retains it.

Most of the [examples](#examples) below are wired this way, and their screenshots
were taken with a value selected, so each one shows the rule, the rings and the
label box a selection draws.

## Examples

Each chart below is a page of the [demo app](demo/): the code is what that page runs, and the
picture is the screenshot CI captures from it. Every page has a route, which is the URL hash of
the published build, so each example links to the chart itself — open one, and the browser's back
button walks the gallery.

**[Try the demo in a browser](https://daybrite.github.io/day-piece-charts/)** — the web-dom build
of `demo/`, published from `main` on every push. Run the same gallery locally with `day launch -p
web-dom --script dayscript/gallery.yaml`.

Most of them are interactive. `.select(sig)` reports what the pointer is over into a signal the
app owns, `.guides(..)` reads the same signal back to draw a rule through the position, a ring on
each selected mark and a box naming their values, and the line under each chart is that selection
in words — what an app does with a signal it holds. Hover reports on a pointer, and a tap or a
drag reports on a touch screen. The screenshots were captured with a value selected, so each one
shows its guides.

Three conventions run through every example. `line` is written `day_piece_charts::line`, because
`day::prelude` exports a `line` shape of its own. `.id(..)` sits on the chart itself and before
the sizing decorators, because that is the piece a tap has to reach; the id is also the page's
route and the name its screenshot is filed under. And each chart ends
`.grow()`, so it fills what its page has left in both directions: resize the window and the chart
re-records at the new size, with the axis relabelling itself for the width it now has. Give it
`.frame(w, h)` for a fixed size instead, or `.height(h).grow_w()` to fix one dimension.

### Line chart

One series over six months, with every sample marked. `Interpolation::Monotone` is a
curve rather than a styling choice: a monotone spline cannot overshoot, so the line
never dips below a value the data did not take.

The three selection lines are the whole of the interactivity: the chart writes the
position under the pointer into `selected`, `Snap::NearestX` makes that the nearest
position along x with every series' value there, and `Guides::RULE` draws the rule, the
rings and the label box for it.

```rust
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
                        day_piece_charts::line(x.clone(), y.clone())
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#lines)** — the same chart in the demo,
at its own route.

![Line chart](SCREENSHOT-URL/lines.png)

### Line series with a target

`by_series` splits the data into series, colors them, and names the legend entries.
`rule_y` draws a horizontal line at a value; dashed and annotated, it is the target a
line chart is usually read against.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#lines-target)** — the same chart in the demo,
at its own route.

![Line series with a target](SCREENSHOT-URL/lines-target.png)

### Area with a gradient

`gradient` takes the color at the mark's top edge and the color at its baseline, so the
fill fades toward the axis instead of covering the grid. The line on top is a second
mark over the same values.

```rust
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
                        day_piece_charts::line(x, y)
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#area)** — the same chart in the demo,
at its own route.

![Area with a gradient](SCREENSHOT-URL/area.png)

### Stacked areas

Areas stack by default, so three series and nothing else make a stacked area chart.
`opacity` keeps the bands distinct where they meet.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#area-stacked)** — the same chart in the demo,
at its own route.

![Stacked areas](SCREENSHOT-URL/area-stacked.png)

### Bar chart with labels

An annotation is attached to a mark rather than placed on the plot, so each number
follows its bar as the data changes. `corner_radius` rounds the end the value grew
toward.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#bars)** — the same chart in the demo,
at its own route.

![Bar chart with labels](SCREENSHOT-URL/bars.png)

### Grouped bar chart

Dodging is a position adjustment rather than a different mark: `by_position` splits each
band between the series, and `Stacking::Unstacked` turns off the stacking bars do by
default. Both are needed — binding the dodge channel alone still stacks.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#bars-grouped)** — the same chart in the demo,
at its own route.

![Grouped bar chart](SCREENSHOT-URL/bars-grouped.png)

### Stacked to 100%

`Stacking::Normalized` scales each column to fill the axis, which turns magnitudes into
shares. The axis then reads as a percentage, which `y_format` writes, and the columns
already partition the plot, so `no_grid` drops the grid.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#bars-100)** — the same chart in the demo,
at its own route.

![Stacked to 100%](SCREENSHOT-URL/bars-100.png)

### Horizontal bars

A category on y and the value along x, for names that need the room. `x_range` spans the
bar from zero to its value (Swift Charts' `BarMark(xStart:xEnd:)`), and because a bar's x
is a band by default, a horizontal one names its value axis with `x_scale_kind`.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#bars-horizontal)** — the same chart in the demo,
at its own route.

![Horizontal bars](SCREENSHOT-URL/bars-horizontal.png)

### Pie chart

A pie is a normalized stack in polar coordinates, and that is how it is implemented:
`sector` marks and `Coordinate::polar`, with no pie-specific code path. `angular_inset`
holds an even channel between neighbouring wedges.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#pie)** — the same chart in the demo,
at its own route.

![Pie chart](SCREENSHOT-URL/pie.png)

### Donut chart

The same wedges with a hole. `Coordinate::donut` takes the hole as a fraction of the
radius, so one number walks a pie through a donut to a ring gauge without touching a
mark. Slices are named by the legend: an annotation is placed from a mark's x position,
and a sector has none.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#donut)** — the same chart in the demo,
at its own route.

![Donut chart](SCREENSHOT-URL/donut.png)

### Scatter plot

`by_symbol` gives each series its own shape, so the chart still reads where color cannot
be relied on, and `symbol_size` is an area rather than a diameter, which is the right
channel for magnitude. Selection differs here too: `Snap::NearestMark` picks the single
nearest point and only within reach of one, so empty space selects nothing, and
`Guides::CROSSHAIR` draws both rules, because on a scatter the y position carries as much
as the x.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#scatter)** — the same chart in the demo,
at its own route.

![Scatter plot](SCREENSHOT-URL/scatter.png)

### Heat map grid

Both axes categorical and the value in the color: one `rect` per cell, `sequential` for
the ramp, and a per-cell annotation color, because no one color reads on both ends of a
ramp. A cell whose label would not fit draws the color alone.

```rust
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#heatmap)** — the same chart in the demo,
at its own route.

![Heat map grid](SCREENSHOT-URL/heatmap.png)

### Time axis

`date` reads an ISO day into an instant, and a column of instants labels itself by
calendar rather than by number. `y_axis_trailing` puts the scale beside the latest
value, the convention for a price chart.

```rust
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
                    day_piece_charts::line(date("Date", day), value("Close", *close))
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#time)** — the same chart in the demo,
at its own route.

![Time axis](SCREENSHOT-URL/time.png)

### Sparkline

`bare` drops the axes, the grid and the legend and draws the marks edge to edge, which is
what a chart beside a number wants. Three rows of a line over its own gradient area, each
one a chart.

```rust
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
                            day_piece_charts::line(x, y)
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
```

**[View live](https://daybrite.github.io/day-piece-charts/#sparkline)** — the same chart in the demo,
at its own route.

![Sparkline](SCREENSHOT-URL/sparkline.png)

## Architecture and dependencies

Dependency links below lead to upstream source repositories or official API
documentation. Version requirements describe this checkout's [Cargo.toml](Cargo.toml),
not necessarily the newest upstream releases.

The implementation is a Rust pipeline that ends in a Day `Draw` display list:

| Source | Responsibility |
|---|---|
| [data.rs](src/data.rs), [mark.rs](src/mark.rs) | Typed values, intervals, dates, mark constructors, and styling options. |
| [resolve.rs](src/resolve.rs), [scale.rs](src/scale.rs) | Infer domains/categories, resolve stacking/grouping, and map data through scales. |
| [ticks.rs](src/ticks.rs), [axis.rs](src/axis.rs), [layout.rs](src/layout.rs) | Choose ticks, measure labels, and reserve plot margins. |
| [coord.rs](src/coord.rs), [render.rs](src/render.rs) | Project Cartesian/polar coordinates and emit drawing commands. |
| [select.rs](src/select.rs) | Resolve input against a hit model derived from the drawn geometry. |
| [style.rs](src/style.rs), [lib.rs](src/lib.rs) | Palettes, public builders, reactive configuration, canvas integration, and guides. |

Tick selection scores candidate labels for simplicity, coverage, density, and
measured legibility. Calendar ticks align to month/year boundaries. Monotone curves
use Fritsch–Carlson interpolation. Bar/area domain inference includes the baseline,
and explicitly pinned domains clip marks at the plot boundary. Polar projection
turns stacked bars into wedges, allowing pie/donut charts to share the pipeline.
Default palettes include Okabe–Ito for categories and Viridis for sequential values.

Direct runtime dependencies are all Day crates: [day-core](https://github.com/daybrite/day/tree/main/crates/day-core) for piece/tree and text
measurement, [day-spec](https://github.com/daybrite/day/tree/main/crates/day-spec) for shared drawing types, [day-pieces](https://github.com/daybrite/day/tree/main/crates/day-pieces) for canvas/builders,
[day-reactive](https://github.com/daybrite/day/tree/main/crates/day-reactive) for bindings, [day-geometry](https://github.com/daybrite/day/tree/main/crates/day-geometry) for geometry, and [day-l10n](https://github.com/daybrite/day/tree/main/crates/day-l10n) for localized
numbers. The latter uses Day's [ICU4X](https://github.com/unicode-org/icu4x) formatting infrastructure transitively.
There is no third-party plotting library, JavaScript chart package, SwiftPM package,
or Gradle dependency added by this crate. [day-mock](https://github.com/daybrite/day/tree/main/crates/day-mock) is a development dependency.
See [Cargo.toml](Cargo.toml) for the direct dependency declarations.

## Compatibility and development

This checkout requires Rust 1.89 or newer and declares compatibility with Day 0.4 in
[Cargo.toml](Cargo.toml). The crate is consumed from Git, not crates.io. Its Day
dependencies use `https://github.com/daybrite/day.git` without a branch, tag, or
revision. Use the same source in your app and keep its `Cargo.lock` to record the
resolved revisions. Mixing Day source URLs or refs can introduce duplicate framework
crates and incompatible types.

For a local framework checkout, run `day patch --local ../day` from this repository
(adjust the path when running from `demo/`). The [demo](demo/) depends on this crate
by path and is a complete integration example.

Run `cargo test` for the grammar, geometry, and rendering-model checks. From `demo/`,
run `day launch -p ios-uikit --script dayscript/charts.yaml` or use `-p android-mdc`.
The walkthrough checks derived values and captures screenshots; inspect those for
visual correctness because canvas text is not available as native label nodes.
