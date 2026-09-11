<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# day-piece-charts

[![ci](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml/badge.svg)](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MPL--2.0-green.svg)](LICENSE)

## Overview and capabilities

`day-piece-charts` draws interactive charts in Day applications using Day's canvas.

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
visible chart. The included demo targets iOS/UIKit and Android/Material.

Chart layout and drawing logic are shared across platforms; text measurement and
input still depend on the backend. Canvas labels are drawn content, rather than
individual native text controls. Provide an accessible textual summary or data
view when users need to read chart values without seeing the canvas.

Charts need a nonzero layout size: give them a frame or a container that allocates
space. Ordinary builder options are set at construction. For changing axis/scale
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
SDK installed. The [demo](demo/) is a complete Day project showing five chart styles.

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
