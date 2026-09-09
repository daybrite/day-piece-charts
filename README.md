<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# day-piece-charts

[![ci](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml/badge.svg)](https://github.com/daybrite/day-piece-charts/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MPL--2.0-green.svg)](LICENSE)

Charts for [Day](https://daybrite.dev) apps: a grammar-of-graphics API drawn on the canvas.

```rust
use day_piece_charts::*;

chart(move || {
    sales.get().iter().map(|s| {
        bar(value("Month", s.month.clone()), value("Revenue", s.revenue))
            .by_series(value("Region", s.region.clone()))
    }).collect()
})
.y_label("Revenue (USD)")
.legend(LegendPosition::Bottom)
.frame(480.0, 280.0)
```

## Not a chart type — a composition

The design follows Wilkinson's *Grammar of Graphics*, the lineage Swift Charts and ggplot2 both
come from. A chart is not picked from a list of types; it is assembled:

| stage | here |
|---|---|
| variables | `value("Revenue", 42.0)`, `time("Date", secs)`, `date("Date", "2026-07-01")` |
| marks | `bar` `line` `area` `point` `rect` `rule_x` `rule_y` `sector` |
| position adjustment | `Stacking::{Standard, Normalized, Center, Unstacked}`, `.by_position(…)` to dodge |
| scales | `Linear` `Log` `Power` `Time` `Band` `Point` |
| guides | `AxisSpec`, `AxisPosition`, `LegendPosition` |
| coordinates | `Coordinate::Cartesian`, `Coordinate::polar()`, `Coordinate::donut(0.6)` |

A heat map is a `rect` whose x *and* y are categories: both scales become bands and each cell fills
its own. `.gradient(top, bottom)` fades an area or bar, `.rounded()` gives a line round caps and
joins, and `.annotation_color(…)` lets a cell's text pick a color that reads on its fill.

Because the coordinate system is a transformation applied *after* the marks are positioned, a pie
chart is a normalized stacked bar in polar coordinates — and that is literally the implementation.
There is no pie-chart code path, and a bar mark in polar space comes out as a wedge because that is
what a bar is there.

## What it does carefully

- **Tick selection is the extended Wilkinson algorithm** (Talbot, Lin & Hanrahan, InfoVis 2010):
  candidate labellings scored on simplicity, coverage, density and legibility, rather than dividing
  the range by five. The legibility term is computed from *measured* text against the axis's real
  pixel length, so an axis relabels itself as the pane resizes.
- **Time axes walk the calendar**, not seconds: months and years are found through the proleptic
  Gregorian conversion, so a monthly tick lands on the first of the month in leap years too.
- **Monotone interpolation is Fritsch–Carlson**, whose tangent clamp is what guarantees a spline
  through non-negative data never dips below zero on the way between two points.
- **Bars and areas include their baseline** in the inferred domain. A bar chart that crops it
  exaggerates every difference, which is the most common way a chart misleads.
- **Bars on a time axis are sized by their closest pair**: a year of daily volume draws as 250
  bars that touch nothing, instead of a fixed share of the axis that would pile them up.
- **A pinned domain clips**. Marks outside a `.y_domain(…)` the app set are cut at the plot edge
  rather than drawn over the axis labels; an inferred axis is never clipped, so a fat point at the
  last sample keeps its far half.
- **The default palette is Okabe–Ito** and the sequential ramp is Viridis — colorblind-safe and
  perceptually uniform respectively, because a chart whose series are told apart by hue has to work
  for readers who cannot distinguish the pretty defaults.
- **Symbol size is an area**, matching Swift Charts, so twice the value is twice the ink.

## Reading it against Swift Charts

The vocabulary tracks Swift Charts' 2D marks closely enough to port a chart by eye. Two differences
come from Rust and from Day's API style: constructors take two positional, conventionally-ordered
arguments (`x` then `y`), and Swift's labelled initializer variants — `BarMark(x:yStart:yEnd:)` —
are builder methods (`.y_range(start, end)`).

| Swift Charts | day-piece-charts |
|---|---|
| `BarMark(x:y:)` | `bar(x, y)` |
| `.foregroundStyle(by: .value("Region", r))` | `.by_series(value("Region", r))` |
| `.position(by:)` | `.by_position(…)` |
| `.interpolationMethod(.monotone)` | `.interpolation(Interpolation::Monotone)` |
| `.chartYScale(domain: 0...100)` | `.y_domain(0.0, 100.0)` |
| `.chartYAxis { AxisMarks(position: .trailing) }` | `.y_axis_trailing()` |
| `.chartLegend(position: .bottom)` | `.legend(LegendPosition::Bottom)` |
| `.chartPlotStyle { $0.padding(0) }` | `.plot_insets(Insets::default())`, or `.bare()` for a sparkline |
| `SectorMark(angle:innerRadius:)` | `sector(angle).inner_radius(0.6)` |
| `RectangleMark(x:y:)` with two categories | `rect(x, y)` — a heat map cell |

## No native half

Every chart is recorded as a `Draw` display list and replayed by whichever backend the app runs on,
so this crate has no per-toolkit code and no backend features. A chart looks the same on all nine
targets because it *is* the same. The cost is that it draws rather than delegating: there is no
native chart control on any platform to delegate to.

## Compatibility

| This crate | Tested against day | Toolkits |
|---|---|---|
| 0.4 | 0.4 (`main`, at its newest revision on every CI run) | every target Day draws a canvas on |

Every day dependency names the bare canonical URL with no branch or tag, and your app's
`Cargo.lock` picks one day revision for the whole graph. Cargo unifies a git dependency only when
URL and ref match, so a crate that pinned a tag would double every day crate in an app on `main`.
`[package.metadata.day] compat = "0.4"` records the minor this release was tested against, and
`day build` notes a mismatch before compiling.

To build against a fork of day, patch the canonical URL once in your app and this crate follows:

```sh
day patch --git https://github.com/acme/day.git@acme
```

## Develop it

```sh
cargo test                                            # the grammar arithmetic, on the host
cd demo && day launch -p ios-uikit --script dayscript/charts.yaml
cd demo && day launch -p android-mdc --script dayscript/charts.yaml
```

The [demo app](demo/) depends on this crate by path and draws the same two series five ways —
grouped bars, monotone lines, a stack, a donut, and a heat map. Its walkthrough asserts what the chart derives
before it draws, since a canvas has no text for a script to read, and captures the drawing as
screenshots. CI runs it on the iOS Simulator and the Android emulator on every push, and daily
against day's newest `main`. To work against a local day checkout, `day patch --local ../day` in
either directory writes a gitignored patch table.

CI also compiles the crate for `aarch64-apple-ios-sim`, `aarch64-linux-android` and
`wasm32-unknown-unknown` on every push, which is what keeps the "same on every target" claim
above honest.

Extending Day is documented at
[daybrite.dev/docs/extending](https://daybrite.dev/docs/extending).

## Part of Day

This crate is one piece of [Day](https://daybrite.dev), a Rust framework for building apps out of
each platform's own widgets — AppKit, UIKit, Android's Material widgets, GTK 4, Qt 6, XAML, and
ArkUI — from one codebase.

New to Day? Start at [daybrite.dev](https://daybrite.dev), or browse the
[source repository](https://github.com/daybrite/day).
