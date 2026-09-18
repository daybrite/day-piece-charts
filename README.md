<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# day-piece-charts

[![ci](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml/badge.svg)](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MPL--2.0-green.svg)](LICENSE)

## Overview and capabilities

`day-piece-charts` draws interactive charts in Day applications using Day's canvas.

**[Try the demo in a browser](https://daybrite.github.io/day-piece-charts/webapp/)**. That
is the web build of [`demo/`](demo/), published from `main` on every push alongside the
[project website](https://daybrite.github.io/day-piece-charts/) and its screenshot gallery; the
same source builds the same app for iOS, Android and the desktops. [Examples](#examples) lists every
chart in it with the code that draws it.

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

### Run the demo on your own machine

Install Day's command-line tool, then run this repository straight from its URL:

```sh
cargo install day-cli   # installs the `day` binary
day launch --git https://github.com/daybrite/day-piece-charts.git
```

`--git` clones the repository, finds the Day project inside it (`demo/`), builds it for your
desktop and runs it; the checkout and its build tree are cached per URL, so a second run starts
where the first left off. Add `-p ios-uikit`, `-p android-mdc` or `-p web-dom` to run it
somewhere else, and `--project demo` to name the project explicitly, which `day` asks for when
a repository holds more than one Day project.

Day's [getting started](https://daybrite.dev/docs/getting-started) covers installing the CLI in
full, and [system requirements](https://daybrite.dev/docs/system-requirements) lists what each
platform's tools are; `day doctor` reports which of them you already have.

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
re-records when its size changes, so an axis relabels itself for the width it has.

The marks closure is reactive on its own. Ordinary builder options are fixed when the
chart is built, so a control that changes the axes or the scales goes through
`.configure(...)`, `.x_domain_with(...)` or `.y_domain_with(...)`, which are re-read on
every draw. With a large data set, budget for a full re-record on each data or size
change and a scan of the drawn marks on each selection: this crate does that work itself,
in Rust, every time.

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

The Cargo dependency is the whole integration. Build with your app's configured
Day target, for example `day build -p android-mdc`, or run `day launch -p ios-uikit`
with the corresponding SDK installed. The [demo](demo/) is a complete Day project: this
app's own page of five compositions, and the gallery of examples below.

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

Most of the [examples](#examples) below are wired this way.

## Examples

Each chart below is a page of the [demo app](demo/), shown with the code that draws it and a
screenshot CI captured from that page. Every page has a route, which is the URL hash of the
published build, so the link under each example opens that chart in the browser. The pictures
come from the [project website](https://daybrite.github.io/day-piece-charts/), which CI rebuilds
from each run's captures; its gallery page carries the same screens as every other platform the
demo runs on. To take them locally: `day launch -p web-dom --script dayscript/gallery.yaml`.

Most of the charts are interactive. `.select(sig)` reports what the pointer is over into a signal
the app owns; `.guides(..)` reads that signal back to draw a rule through the position, a ring on
each selected mark, and a box naming their values; the line under each chart is the same selection
as text. Hover reports on a pointer, a tap or a drag on a touch screen. Each screenshot was taken
with a value selected, so the guides show in all of them.

A note on how the code below is written. `line` is spelled `day_piece_charts::line`, because
`day::prelude` exports a `line` shape of its own, and `.id(..)` sits on the chart itself, before
the sizing decorators, since that is the piece a tap has to reach. Each chart ends `.grow()` and
fills what its page has left, so resizing the window re-records it at the new size; `.frame(w, h)`
fixes both dimensions instead, and `.height(h).grow_w()` fixes one.

### Line chart

One series over six months, with every sample marked. `Interpolation::Monotone` is a
curve, not a styling option: a monotone spline (Fritsch–Carlson) cannot overshoot, so
the line never dips below a value the data did not take.

Three lines at the end of the chart add the interactivity. The chart writes the position
under the pointer into `selected`; `Snap::NearestX` resolves that to the nearest position
along x, with every series' value there; `Guides::RULE` draws the rule, the rings and the
label box.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#lines)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/lines.png" width="760" alt="Line chart"></kbd>
</p>

### Line series with a target

`by_series` splits the data into series, colors them and names the legend entries.
`rule_y` draws a horizontal line at a value; dashed and annotated, it carries the target
the series are read against.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#lines-target)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/lines-target.png" width="760" alt="Line series with a target"></kbd>
</p>

### Area with a gradient

`gradient` takes the color at the mark's top edge and the color at its baseline, so the
fill fades toward the axis and the grid stays visible through it. The line on top is a
second mark over the same values.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#area)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/area.png" width="760" alt="Area with a gradient"></kbd>
</p>

### Stacked areas

Areas stack by default, so three series are enough to make a stacked area chart.
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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#area-stacked)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/area-stacked.png" width="760" alt="Stacked areas"></kbd>
</p>

### Bar chart with labels

An annotation belongs to its mark, so each number follows its bar as the data changes.
`corner_radius` rounds the end the value grew toward.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#bars)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/bars.png" width="760" alt="Bar chart with labels"></kbd>
</p>

### Grouped bar chart

Dodging is a position adjustment, so the marks stay bars: `by_position` splits each band
between the series. It needs `Stacking::Unstacked` alongside it, because bars stack by
default and binding the dodge channel does not turn that off.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#bars-grouped)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/bars-grouped.png" width="760" alt="Grouped bar chart"></kbd>
</p>

### Stacked to 100%

`Stacking::Normalized` scales each column to fill the axis, turning magnitudes into
shares. `y_format` writes the axis as a percentage, and `no_grid` drops a grid the columns
have already replaced.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#bars-100)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/bars-100.png" width="760" alt="Stacked to 100%"></kbd>
</p>

### Horizontal bars

A category on y and the value along x, for names that need the room. `x_range` spans the
bar from zero to its value (Swift Charts' `BarMark(xStart:xEnd:)`). A bar's x is a band by
default, so a horizontal one names its value axis with `x_scale_kind`.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#bars-horizontal)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/bars-horizontal.png" width="760" alt="Horizontal bars"></kbd>
</p>

### Pie chart

A pie is a normalized stack in polar coordinates, implemented as exactly that: `sector`
marks and `Coordinate::polar`, running through the same code the stacked bar does.
`angular_inset` holds an even channel between neighbouring wedges.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#pie)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/pie.png" width="760" alt="Pie chart"></kbd>
</p>

### Donut chart

The same wedges with a hole. `Coordinate::donut` takes the hole as a fraction of the
radius, so that one number covers a pie, a donut and a ring gauge. The legend names the
slices, because an annotation is positioned from its mark's x and a sector has none.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#donut)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/donut.png" width="760" alt="Donut chart"></kbd>
</p>

### Scatter plot

`by_symbol` gives each series its own shape, so the chart reads without relying on color,
and `symbol_size` measures area, so a value twice as large shows twice the ink.

Selection is configured differently here: `Snap::NearestMark` picks the single nearest
point, and only within reach of one, so empty space selects nothing; `Guides::CROSSHAIR`
draws both rules, since on a scatter the y position carries as much as the x.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#scatter)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/scatter.png" width="760" alt="Scatter plot"></kbd>
</p>

### Heat map grid

Both axes categorical and the value in the color: one `rect` per cell, `sequential` for
the ramp, and a per-cell annotation color, because no one color reads on both ends of a
ramp. A cell too narrow for its label draws the color alone.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#heatmap)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/heatmap.png" width="760" alt="Heat map grid"></kbd>
</p>

### Time axis

`date` reads an ISO day into an instant, and a column of instants labels itself by
calendar rather than by number. `y_axis_trailing` puts the scale beside the latest value,
where a price chart keeps it.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#time)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/time.png" width="760" alt="Time axis"></kbd>
</p>

### Sparkline

`bare` drops the axes, the grid and the legend and draws the marks edge to edge, which
suits a chart that sits beside a number. Each of the three rows here is its own chart: a
line over its own gradient area.

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

**[View live](https://daybrite.github.io/day-piece-charts/webapp/#sparkline)**

<p align="center">
  <kbd><img src="https://daybrite.github.io/day-piece-charts/gallery/web-dom/default/sparkline.png" width="760" alt="Sparkline"></kbd>
</p>

## Architecture and dependencies

A chart is drawn in one pass. The marks closure produces `Mark` values, the pipeline resolves
them against scales and axes, and what comes out is a Day `Draw` display list that the backend
replays. Nothing survives the pass except the hit model a selection is resolved against.

| Stage | Source |
|---|---|
| Values, marks, and their styling | [data.rs](src/data.rs), [mark.rs](src/mark.rs) |
| Domains and categories, stacking and dodging, data to positions | [resolve.rs](src/resolve.rs), [scale.rs](src/scale.rs) |
| Tick choice, label measurement, and the margins labels need | [ticks.rs](src/ticks.rs), [axis.rs](src/axis.rs), [layout.rs](src/layout.rs) |
| Cartesian and polar projection, and the drawing commands | [coord.rs](src/coord.rs), [render.rs](src/render.rs) |
| Hit testing, and the guides drawn for a selection | [select.rs](src/select.rs) |
| Palettes, and the builders an app calls | [style.rs](src/style.rs), [lib.rs](src/lib.rs) |

Two stages explain most of what a reader notices. Ticks come from an extended-Wilkinson search
that scores candidate labellings on how round the numbers are, how well they cover the data, how
many there are, and how wide they measure in the font they will be drawn in. That search is why
an axis relabels itself as the plot resizes, rather than dividing the range by a constant. Polar
is a projection applied after positioning, so a pie is a normalized stacked bar with the
coordinate system bent; donuts and rose charts are the same bar again.

The default palettes are Okabe–Ito for categories and Viridis for sequential values, both of
which stay distinguishable for colorblind readers.

Every runtime dependency is a Day crate:
[day-core](https://github.com/daybrite/day/tree/main/crates/day-core) for the piece tree and text
measurement, [day-spec](https://github.com/daybrite/day/tree/main/crates/day-spec) for the shared
drawing types, [day-pieces](https://github.com/daybrite/day/tree/main/crates/day-pieces) for the
canvas, [day-reactive](https://github.com/daybrite/day/tree/main/crates/day-reactive) for
bindings, [day-geometry](https://github.com/daybrite/day/tree/main/crates/day-geometry) for
geometry, and [day-l10n](https://github.com/daybrite/day/tree/main/crates/day-l10n) for
locale-aware numbers, which brings Day's
[ICU4X](https://github.com/unicode-org/icu4x) formatting in transitively.
[day-mock](https://github.com/daybrite/day/tree/main/crates/day-mock) is a dev-dependency, for
the host tests. The charting itself (marks, scales, ticks, projection, drawing) is this crate's
own code, so adding it to an app is a Cargo dependency and nothing more.
[Cargo.toml](Cargo.toml) has the declarations; the versions there describe this checkout.

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
`day launch -p web-dom --script dayscript/charts.yaml` drives this app's own page and
`--script dayscript/gallery.yaml` walks the examples; `-p macos-appkit`, `-p ios-uikit`
and `-p android-mdc` run the same scripts on those targets. Both check the values a
chart derives and capture screenshots. Read the screenshots as well as the log: canvas
text is drawn content, so no assertion can reach the labels inside a plot.
