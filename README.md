<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: MPL-2.0
-->

# day-piece-charts

Build reactive charts from marks, scales, and data.

[Day](https://daybrite.dev/docs/) is a Rust framework for building apps with native desktop and
mobile controls, plus HTML controls on the web.

Compose line, area, bar, rectangle, rule, sector, and point marks on Day’s canvas. Add
selection, legends, animation, and image or PDF export. The gallery crate supplies reusable example pages.

## Bands, ridges, and labelled rules

A mark with no y spans the plot without sizing its axis. `band_x` shades a window, `hatched`
fills it with diagonal lines so the data under it stays readable, and an annotation set as a
pill sits in a capsule with a shadow:

```rust
chart(move || {
    let mut marks: Vec<Mark> = samples
        .iter()
        .map(|s| area(value("Minute", s.minute), value("Flux", s.flux)))
        .collect();
    marks.push(
        band_x(value("Minute", -6.5), value("Minute", -3.5))
            .hatched(5.0)
            .annotation(AnnotationPosition::Top, "cloud")
            .annotation_pill(),
    );
    marks.push(
        rule_y(value("Flux", average))
            .dash([3.0, 4.0])
            .annotation(AnnotationPosition::Trailing, "avg 56")
            .annotation_pill(),
    );
    marks
})
```

`ridge` stands an area or a line on its row of a categorical y axis. Each row is its own
series, the tallest value spans `ridge_overlap` rows, and a ridge hides the rows behind it:

```rust
chart(move || {
    spectra
        .iter()
        .map(|s| {
            area(value("Wavelength", s.nm), value("Class", s.class)).ridge(value("Flux", s.flux))
        })
        .collect()
})
.ridge_overlap(1.7)
```

A point takes a categorical y, so a dumbbell chart is a `rule_y(row).x_range(before, after)`
with a `point` at each end.

## Entrances and replay

`entrance(Entrance::Grow)` brings the data into an empty plot: bars and ridges rise, a range
is drawn out from its start, and the point at its far end rides it. `Entrance::Sweep` uncovers
the finished chart from the leading edge. A `Replay` handle plays the entrance again:

```rust
let replay = Replay::new();
column((
    button("Replay").action(move || replay.play()),
    chart(marks).entrance(Entrance::Sweep).replay(replay),
))
```

## Appearance

The default `Chrome` draws axis text in the platform's secondary-text color and the grid, ticks,
selection box and pill shadow in light/dark pairs. A live chart re-records when the appearance
changes, so the app never rebuilds it. Each `Chrome` field is an `Ink`: pass `Chrome { .. }`
with fixed colors to draw the same in both appearances, or `Chrome::for_dark(true)` for the
fixed dark chrome. An export draws the light chrome on white.

## Drags inside a scroll view

A chart takes a drag only along the axis its bindings scrub. An inspection that snaps along x
(`Snap::NearestX`, the default) and an x-only brush or viewport scrub sideways, so on a touch
screen a vertical swipe that starts on the chart scrolls the page. `Snap::NearestY` and y-only
bindings scrub up and down. A chart with a binding that moves in both axes
(`Snap::NearestMark`, `Snap::Hit`, a two-axis brush or viewport), or with bindings that
disagree, takes every drag that starts on it. A tap and the mouse pointer select as before.

## Changes since 0.5.1

- `Chrome` fields are `Ink` (were `Color`), and `Chrome::default()` follows the appearance.
  `Chrome::resolve(&palette)` gives the fixed chrome for one appearance. Code that read a
  `Chrome` color as numbers resolves it first, with `day::resolve(ink)`.
- `render::Paint2::ground` is an `Option<Ink>`.
- Transitions run on `day::motion`. `DEFAULT_TRANSITION` is `AnimSpec::spring(0.6, 0.86)`,
  whose `duration_ms` is now the spring's settle time, 842 ms.
- Drag bindings follow the axis rule above.

## Use

Add `day-piece-charts = "0.*"` to your app’s `[dependencies]`.
Build with the Day CLI to select the matching toolkit integration.

## Documentation

- [Setup and usage guide](https://daybrite.dev/docs/reference/charts/)
- [Rust API](https://docs.rs/day-piece-charts/0.4.3/day_piece_charts/)
- [Crates.io](https://crates.io/crates/day-piece-charts)
- [Reusable chart gallery](https://daybrite.dev/docs/reference/charts/#examples)
- [Run the demo](demo/README.md)

[Source](https://github.com/daybrite/day-piece-charts) · [MPL-2.0 license](https://github.com/daybrite/day-piece-charts/blob/main/LICENSE)
