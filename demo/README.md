<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Charts Demo

The demo and test app for [`day-piece-charts`](..): a navigation app of one page per chart. The
first page is this app's own — a picker of five compositions of one data set, the chart, a readout
of what the pipeline derives from it, and a slider for how much data there is. It depends on the
piece by path (`day-piece-charts = { path = ".." }`), so a change to the piece and a change here
land in one pull request and one CI run.

The other pages are the gallery ([src/examples.rs](src/examples.rs)): the illustrative charts the
crate's README documents, each one printed there beside the screenshot this app's walkthrough
captures of it.

Every page has a route (`day::routes!` in [src/lib.rs](src/lib.rs)), and one name does three jobs:
it addresses the page (`navigate: { route: lines }`, and the URL hash of the web build, so
<https://daybrite.github.io/day-piece-charts/#lines> opens the line chart), it is the id of the
chart on that page, and it is the name its screenshot is filed under. A new example is a function
in `examples.rs`, a variant in `Page`, an entry in `gallery()`, a name in
`resource/locales/en/app.ftl`, six lines in `dayscript/gallery.yaml`, and a section in the
README.

The five compositions are the same two regions of monthly revenue assembled five ways — grouped
bars, monotone lines, a stack, a donut, and a heat map. The donut is the point the crate makes:
it is the stack in polar coordinates, so it lives in the other arm of a `when` and shares every
line of mark-building code with the bars.

## Run it

```sh
day doctor                                             # the toolchains for the targets below
day launch -p ios-uikit  --script dayscript/charts.yaml
day launch -p android-mdc --script dayscript/charts.yaml
day launch -p web-dom    --script dayscript/charts.yaml
day launch -p web-dom    --script dayscript/gallery.yaml   # the README's examples
```

The scripts are the test. A chart is drawn on the canvas, so `charts.yaml` asserts the arithmetic
the chart runs before it draws — the domain `resolve::domain_of` infers and the labelling the
extended-Wilkinson search picks — and captures seven screenshots under
`build/day/screenshots/<target>/` for the drawing itself. `gallery.yaml` opens each of the fourteen
examples by its route and captures one screenshot each, named for that route.
CI runs both on the iOS Simulator, the Android emulator, and a headless browser
([../.github/workflows/ci.yml](../.github/workflows/ci.yml)). Each backend decodes the display
list itself, so the three sets of screenshots are three renderings of the same drawings.

The web build is published to GitHub Pages from `main`, at
<https://daybrite.github.io/day-piece-charts/>.

## Build against a local day

No `Cargo.lock` is committed: the first build resolves day at the tip of `main`, and `cargo
update` moves it there again. To build against a checkout of day (or of the piece) instead:

```sh
day patch --local ../../day             # writes .cargo/config.toml, gitignored
day patch --check                       # every day crate now resolves from the checkout
```

Delete `.cargo/config.toml` to go back to the git dependency. The lock is gitignored, so a
patched build cannot leave the checkout's paths behind for anyone else.
