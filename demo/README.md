<!--
Copyright © The Daybrite Project
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Charts Demo

The demo and on-device test app for [`day-piece-charts`](..): one page with a picker of chart
compositions, the chart itself, and a readout of what the pipeline derives from the data. It
depends on the piece by path (`day-piece-charts = { path = ".." }`), so a change to the piece and
a change here land in one pull request and one CI run.

The four compositions are the same two regions of monthly revenue assembled four ways — grouped
bars, monotone lines, a stack, and a donut. The donut is the point the crate makes: it is the
stack in polar coordinates, so it lives in the other arm of a `when` and shares every line of
mark-building code with the bars.

## Run it

```sh
day doctor                                            # the iOS and Android toolchains
day launch -p ios-uikit --script dayscript/charts.yaml
day launch -p android-mdc --script dayscript/charts.yaml
```

The script is the test. A chart is drawn on the canvas, so it asserts the arithmetic the chart
runs before it draws — the domain `resolve::domain_of` infers and the labelling the
extended-Wilkinson search picks — and captures six screenshots under
`build/day/screenshots/<target>/` for the drawing itself. CI runs exactly this on the iOS
Simulator and the Android emulator ([../.github/workflows/ci.yml](../.github/workflows/ci.yml)).

## Build against a local day

No `Cargo.lock` is committed: the first build resolves day at the tip of `main`, and `cargo
update` moves it there again. To build against a checkout of day (or of the piece) instead:

```sh
day patch --local ../../day             # writes .cargo/config.toml, gitignored
day patch --check                       # every day crate now resolves from the checkout
```

Delete `.cargo/config.toml` to go back to the git dependency. The lock is gitignored, so a
patched build cannot leave the checkout's paths behind for anyone else.
