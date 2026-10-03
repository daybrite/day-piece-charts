// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! day-piece-charts: charts for Day apps, built on the grammar of graphics and drawn on the
//! canvas.
//!
//! An external Day Piece with **no native half**. A chart is recorded as a `Draw` display list and
//! replayed by whichever backend the app runs on, so it looks the same on all nine targets because
//! it *is* the same; there is no per-toolkit chart code to drift.
//!
//! # The grammar
//!
//! The design follows Wilkinson's *Grammar of Graphics* (and Wickham's restatement of it), which is
//! also the lineage Swift Charts comes from. A chart is not a *type*; it is a composition:
//!
//! | stage | here |
//! |---|---|
//! | variables | [`value`] / [`time`] bind a column to a channel |
//! | marks | [`bar`], [`line()`], [`area`], [`point`], [`rect`], [`rule_x`], [`sector`] |
//! | position adjustment | [`Stacking`], [`Mark::by_position`] (dodging) |
//! | scales | [`ScaleKind::Linear`], `Log`, `Power`, `Time`, `Band`, `Point` |
//! | guides | [`AxisSpec`], [`LegendPosition`] |
//! | coordinates | [`Coordinate::Cartesian`], [`Coordinate::polar`] |
//!
//! Because the coordinate system is a transformation applied after positioning, a pie chart is a
//! normalized stacked bar in polar coordinates, and that is literally how it is implemented. There
//! is no pie-chart code path.
//!
//! # Reading it against Swift Charts
//!
//! The mark and modifier vocabulary tracks Swift Charts closely enough to port a chart by eye, with
//! two differences that come from Rust and from Day's API style (docs/api-style.md):
//! constructors take two positional, conventionally-ordered arguments (`x` then `y`), and Swift's
//! labelled initializer variants (`BarMark(x:yStart:yEnd:)`) are builder methods
//! ([`Mark::y_range`]).
//!
//! ```ignore
//! use day_piece_charts::*;
//!
//! chart(move || {
//!     sales.get().iter().map(|s| {
//!         bar(value("Month", s.month.clone()), value("Revenue", s.revenue))
//!             .by_series(value("Region", s.region.clone()))
//!     }).collect()
//! })
//! .y_label("Revenue (USD)")
//! .legend(LegendPosition::Bottom)
//! .frame(480.0, 280.0)
//! ```
//!
//! The marks closure is a **binding**: it re-runs when a signal it reads changes, and the chart
//! re-records. Resizing re-records too, which is what lets the axis relabel itself for the width it
//! actually has rather than the one it was authored at.

pub mod animate;
pub mod axis;
pub mod coord;
pub mod data;
pub mod interaction;
pub mod layout;
pub mod legend;
pub use interaction::{
    Axes, Bounds, EventSource, Extent, Inspect, Interaction, IntervalSelection, Links,
    PointSelection, Predicate, Projection, Record, Toggle, Visual,
};
pub mod mark;
pub mod render;
pub mod resolve;
pub mod scale;
pub mod select;
pub use legend::{Legend, LegendEntry, legend};
pub use select::{Guides, Selection, Snap};
pub mod style;
pub mod ticks;

pub use axis::{AxisPosition, AxisSpec, LegendPosition};
pub use coord::Coordinate;
pub use data::{Datum, Interval, IntoDatum, Value, date, parse_iso_date, time, value};
pub use day_spec::{LineCap, LineJoin};
pub use layout::Insets;
pub use mark::{
    Annotation, AnnotationPosition, Dimension, Interpolation, Mark, MarkKind, MarkStyle, Stacking,
    Symbol, area, bar, line, point, rect, rule_x, rule_y, sector,
};
pub use scale::{BandPadding, Scale, ScaleKind, ScaleSpec};
pub use style::{Chrome, categorical, diverging, sequential};
pub use ticks::Tick;

use std::rc::Rc;

use day_core::frame::{FrameClock, FrameHandle};
use day_core::tween::{Timing, animate};
use day_core::{BuildCx, Piece, RNode};
use day_pieces::{Draw, TextStyle};
use day_reactive::Signal;
use day_spec::props::TextAlign;
use day_spec::{AnimSpec, CanvasFont, Color, Point, Rect, Shape, Size, TextAnchor, TextVAlign};

/// Assigns a color to a series by index and name. Named because the chart stores one and every
/// stage of the pipeline is handed a reference to it.
pub type SeriesColors = Rc<dyn Fn(usize, &str) -> Color>;

/// Answers a scale's domain from inside the chart's binding; `None` leaves it inferred.
pub type DomainFn = Rc<dyn Fn() -> Option<(f64, f64)>>;
/// A [`Chart::configure`] closure, named for the same reason [`DomainFn`] is.
type ConfigureFn = Rc<dyn Fn(&mut ChartConfig)>;

/// A chart. Build it with [`chart`], configure it with the builder methods, and place it like any
/// other piece: it grows to fill, so give it a `.frame(w, h)` or let its container size it.
pub struct Chart {
    marks: Rc<dyn Fn() -> Vec<Mark>>,
    x_scale: ScaleSpec,
    y_scale: ScaleSpec,
    x_axis: AxisSpec,
    y_axis: AxisSpec,
    coordinate: Coordinate,
    legend: LegendPosition,
    chrome: Option<Chrome>,
    label_size: f64,
    font: CanvasFont,
    colors: SeriesColors,
    plot_insets: Option<Insets>,
    x_domain_fn: Option<DomainFn>,
    y_domain_fn: Option<DomainFn>,
    configure_fn: Option<ConfigureFn>,
    interactions: Vec<Interaction>,
    conditions: Vec<interaction::Condition>,
    filter: Option<Predicate>,
    domain: Option<IntervalSelection>,
    transition: Option<AnimSpec>,
    appear: bool,
}

/// The transition [`Chart::animated`] uses: a spring that settles in about half a second with a
/// trace of overshoot, quick enough to follow a slider and soft enough to read as the same data
/// moving rather than a new chart appearing.
pub const DEFAULT_TRANSITION: AnimSpec = AnimSpec {
    duration_ms: 600,
    delay_ms: 0,
    curve: day_spec::Curve::Spring {
        response: 0.6,
        damping: 0.86,
    },
    repeat: 0,
    autoreverse: false,
};

/// A chart of the marks the closure produces.
///
/// The closure is re-run as a reactive binding, so reading a `Signal` inside it makes the chart
/// follow that signal, which is how a chart animates, filters, or re-sorts without any imperative
/// update call.
pub fn chart(marks: impl Fn() -> Vec<Mark> + 'static) -> Chart {
    Chart {
        marks: Rc::new(marks),
        x_scale: ScaleSpec::default(),
        y_scale: ScaleSpec::default(),
        x_axis: AxisSpec::default(),
        y_axis: AxisSpec::default(),
        coordinate: Coordinate::Cartesian,
        legend: LegendPosition::Automatic,
        chrome: None,
        label_size: 11.0,
        font: CanvasFont::default(),
        colors: Rc::new(|i, _| style::categorical(i)),
        plot_insets: None,
        x_domain_fn: None,
        y_domain_fn: None,
        configure_fn: None,
        interactions: Vec::new(),
        conditions: Vec::new(),
        filter: None,
        domain: None,
        transition: None,
        appear: false,
    }
}

impl Chart {
    /// Bind an event-driven inspection, point query, brush, viewport or link interaction.
    /// Several bindings can coexist; shared parameters coordinate independent views.
    pub fn interact(mut self, interaction: impl Into<Interaction>) -> Self {
        let interaction = interaction.into();
        if let Interaction::Viewport(viewport) = &interaction {
            self.domain = Some(viewport.parameter);
        }
        self.interactions.push(interaction);
        self
    }
    /// Conditional encodings are evaluated after geometry/animation, so hovering never
    /// retargets a transition or changes the chart's scale domain.
    pub fn condition(mut self, predicate: Predicate, selected: Visual, other: Visual) -> Self {
        self.conditions.push(interaction::Condition {
            predicate,
            selected,
            other,
        });
        self
    }
    /// Filter input tuples declaratively. Pin a domain when a stable comparison is desired.
    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.filter = Some(predicate);
        self
    }
    /// Use a shared interval's continuous extents as scale domains (overview/detail).
    pub fn domain(mut self, parameter: IntervalSelection) -> Self {
        self.domain = Some(parameter);
        self
    }

    // --- Scales (Swift Charts' .chartXScale / .chartYScale) ---

    /// Pin the x domain.
    pub fn x_domain(mut self, lo: f64, hi: f64) -> Self {
        self.x_scale.domain = Some(Interval::new(lo, hi));
        self
    }
    /// Pin the y domain.
    pub fn y_domain(mut self, lo: f64, hi: f64) -> Self {
        self.y_scale.domain = Some(Interval::new(lo, hi));
        self
    }
    /// Pin the x domain to whatever the closure answers, re-asked inside the chart's binding,
    /// so a domain that follows a signal (a range picker) tracks it the way the marks do.
    /// `None` leaves the domain inferred.
    pub fn x_domain_with(mut self, f: impl Fn() -> Option<(f64, f64)> + 'static) -> Self {
        self.x_domain_fn = Some(Rc::new(f));
        self
    }
    /// Pin the y domain to whatever the closure answers, re-asked inside the chart's binding.
    pub fn y_domain_with(mut self, f: impl Fn() -> Option<(f64, f64)> + 'static) -> Self {
        self.y_domain_fn = Some(Rc::new(f));
        self
    }
    /// Adjust the scales and axes on every draw, inside the chart's own binding.
    ///
    /// The marks closure already re-runs whenever a signal it reads changes; this is the same for
    /// the chart's shape. Everything set through the plain builders is fixed when the piece is
    /// built, so a control bound to one of them (a log/linear picker, a tick-count slider, a grid
    /// switch) moves and the chart does not, unless something else happens to rebuild the piece.
    ///
    /// The closure is handed the configuration as the builders left it, with any
    /// [`Chart::y_domain_with`] already applied, and may change whatever it likes:
    ///
    /// ```ignore
    /// chart(marks).configure(move |c| {
    ///     c.y_axis.desired_count = ticks.get();
    ///     if log.get() {
    ///         c.y_scale.kind = Some(ScaleKind::Log { base: 10.0 });
    ///     }
    /// })
    /// ```
    pub fn configure(mut self, f: impl Fn(&mut ChartConfig) + 'static) -> Self {
        self.configure_fn = Some(Rc::new(f));
        self
    }
    /// Pin the x categories, and their order.
    pub fn x_categories(mut self, cats: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_scale.categories = Some(cats.into_iter().map(Into::into).collect());
        self
    }
    pub fn x_scale_kind(mut self, k: ScaleKind) -> Self {
        self.x_scale.kind = Some(k);
        self
    }
    pub fn y_scale_kind(mut self, k: ScaleKind) -> Self {
        self.y_scale.kind = Some(k);
        self
    }
    /// Reverse the y range, so values grow downward.
    pub fn y_reversed(mut self) -> Self {
        self.y_scale.reversed = true;
        self
    }
    /// Reverse the x range.
    pub fn x_reversed(mut self) -> Self {
        self.x_scale.reversed = true;
        self
    }
    /// Band padding for a discrete x scale.
    pub fn x_band_padding(mut self, p: BandPadding) -> Self {
        self.x_scale.padding = Some(p);
        self
    }

    // --- Axes (.chartXAxis / .chartYAxis) ---

    pub fn x_axis(mut self, spec: AxisSpec) -> Self {
        self.x_axis = spec;
        self
    }
    pub fn y_axis(mut self, spec: AxisSpec) -> Self {
        self.y_axis = spec;
        self
    }
    pub fn x_axis_hidden(mut self) -> Self {
        self.x_axis.hidden = true;
        self
    }
    pub fn y_axis_hidden(mut self) -> Self {
        self.y_axis.hidden = true;
        self
    }
    /// Draw the y axis on the trailing edge, the convention for a price chart, where the latest
    /// value sits beside its label rather than across the plot from it.
    pub fn y_axis_trailing(mut self) -> Self {
        self.y_axis.position = AxisPosition::Trailing;
        self
    }
    /// Draw the x axis along the top edge.
    pub fn x_axis_top(mut self) -> Self {
        self.x_axis.position = AxisPosition::Top;
        self
    }
    /// Hide both axes and the grid, and draw the marks edge to edge: a sparkline, a gauge, or a
    /// track whose labels the app places itself.
    pub fn bare(mut self) -> Self {
        self.x_axis.hidden = true;
        self.y_axis.hidden = true;
        self.x_axis.grid = false;
        self.y_axis.grid = false;
        self.legend = LegendPosition::Hidden;
        self.plot_insets = Some(Insets::default());
        self
    }
    /// Replace the measured margins with fixed ones. The default insets are computed from the
    /// axis labels the chart is about to draw; a chart with no axes wants none of that room, and
    /// a chart whose marks overhang the plot (a fat point at the last sample) wants a little.
    pub fn plot_insets(mut self, insets: Insets) -> Self {
        self.plot_insets = Some(insets);
        self
    }
    /// Drop the grid on both axes, the right call for a chart whose marks already partition the
    /// plot, like a stacked-to-100% bar.
    pub fn no_grid(mut self) -> Self {
        self.x_axis.grid = false;
        self.y_axis.grid = false;
        self
    }
    /// Draw a solid rule along both axes and a tick mark at every label.
    ///
    /// Off by default, which is what Swift Charts does: the grid already says where the plot is,
    /// and axis furniture drawn at label weight competes with the marks. This is here for a chart
    /// that has no grid to lean on.
    pub fn axis_rules(mut self) -> Self {
        for spec in [&mut self.x_axis, &mut self.y_axis] {
            spec.rule = true;
            spec.ticks = true;
        }
        self
    }
    /// Name the x axis (`.chartXAxisLabel`). An axis with no name drawn is the default; a data
    /// column's label is not used as one, because under labels already reading `C1 C2 C3` a
    /// title saying "Category" is noise.
    pub fn x_label(mut self, t: impl Into<String>) -> Self {
        self.x_axis.title = Some(t.into());
        self
    }
    pub fn y_label(mut self, t: impl Into<String>) -> Self {
        self.y_axis.title = Some(t.into());
        self
    }
    /// How many x labels to aim for. The tick search treats it as a target to score, not a promise.
    pub fn x_tick_count(mut self, n: usize) -> Self {
        self.x_axis.desired_count = n;
        self
    }
    pub fn y_tick_count(mut self, n: usize) -> Self {
        self.y_axis.desired_count = n;
        self
    }
    /// Render each x tick's value yourself: currency, percentages, a custom date format.
    pub fn x_format(mut self, f: impl Fn(&Datum) -> String + 'static) -> Self {
        self.x_axis.format = Some(Rc::new(f));
        self
    }
    pub fn y_format(mut self, f: impl Fn(&Datum) -> String + 'static) -> Self {
        self.y_axis.format = Some(Rc::new(f));
        self
    }

    // --- Animation ---

    /// Animate every change to what the chart draws with `spec` (README "Animation").
    ///
    /// The chart re-records whenever its marks closure or its configuration reads a changed
    /// signal; with a transition set, each change is drawn as the data moving from the picture on
    /// screen to the new one: bars grow along a rescaling axis, a time series slides as its window
    /// widens, a pie's wedges sweep, and mapped colors blend. A change made inside
    /// `with_animation(spec, ..)` animates with that spec whether or not this is set, the same
    /// contract native widgets keep. Resizing the chart never animates: the frame the window
    /// settles on is drawn directly.
    pub fn animation(mut self, spec: AnimSpec) -> Self {
        self.transition = Some(spec);
        self
    }

    /// [`Chart::animation`] with [`DEFAULT_TRANSITION`].
    pub fn animated(self) -> Self {
        self.animation(DEFAULT_TRANSITION)
    }

    /// Animate the chart's first appearance too, as its data arriving in an empty plot: bars and
    /// wedges grow from their baselines, areas rise, and lines and points fade in. The chart's
    /// transition spec is used ([`DEFAULT_TRANSITION`] if none was set). Meant for a chart that
    /// replaces another in place (an analysis picker swapping panels, a tab opening on a chart),
    /// where arriving in motion says "this is new" the way a transition says "this changed".
    pub fn animate_appearance(mut self) -> Self {
        self.appear = true;
        if self.transition.is_none() {
            self.transition = Some(DEFAULT_TRANSITION);
        }
        self
    }

    // --- Everything else ---

    /// Draw in polar coordinates. A stacked bar becomes a pie; a line becomes a radar trace.
    pub fn coordinate(mut self, c: Coordinate) -> Self {
        self.coordinate = c;
        self
    }
    pub fn legend(mut self, p: LegendPosition) -> Self {
        self.legend = p;
        self
    }
    /// Replace the series palette (`.chartForegroundStyleScale`). The closure gets the series
    /// index and its name, so a chart can key colors off meaning (red for "Loss") rather than
    /// off position.
    pub fn series_colors(mut self, f: impl Fn(usize, &str) -> Color + 'static) -> Self {
        self.colors = Rc::new(f);
        self
    }
    /// Override the chrome. Unset, it follows the app's light/dark appearance.
    pub fn chrome(mut self, c: Chrome) -> Self {
        self.chrome = Some(c);
        self
    }
    pub fn label_size(mut self, pt: f64) -> Self {
        self.label_size = pt;
        self
    }
    pub fn font(mut self, f: CanvasFont) -> Self {
        self.font = f;
        self
    }
}

/// The legend's own measurements, so the plot can be inset by exactly what it occupies.
struct LegendBox {
    insets: Insets,
    /// Where the swatches start, and how far apart they sit.
    origin: Point,
    horizontal: bool,
}

/// A key is a reference, not a second chart: Swift Charts draws a small circle and sets the
/// entries close together so the legend recedes behind the plot. These two are the measured
/// equivalents, and they are shared by the pass that reserves the space and the one that draws.
fn legend_swatch(line: f64) -> f64 {
    line * 0.64
}

fn legend_gap(label_size: f64) -> f64 {
    label_size * 0.95
}

fn legend_layout(
    entries: &[(String, Color)],
    position: LegendPosition,
    size: Size,
    label_size: f64,
    font: &CanvasFont,
) -> Option<LegendBox> {
    let visible = match position {
        LegendPosition::Hidden => false,
        // Nothing to explain about a single series: the axis title already names the quantity.
        LegendPosition::Automatic => entries.len() > 1,
        _ => !entries.is_empty(),
    };
    if !visible {
        return None;
    }
    let line = day_core::measure_text("0", label_size, font).height;
    let swatch = legend_swatch(line);
    let widths: Vec<f64> = entries
        .iter()
        .map(|(n, _)| swatch + 4.0 + day_core::measure_text(n, label_size, font).width)
        .collect();
    let pos = match position {
        LegendPosition::Automatic => LegendPosition::Bottom,
        p => p,
    };
    match pos {
        LegendPosition::Top | LegendPosition::Bottom => {
            let h = line + 8.0;
            let y = if pos == LegendPosition::Top {
                4.0
            } else {
                size.height - line - 4.0
            };
            Some(LegendBox {
                insets: if pos == LegendPosition::Top {
                    Insets {
                        top: h,
                        ..Default::default()
                    }
                } else {
                    Insets {
                        bottom: h,
                        ..Default::default()
                    }
                },
                // Only the cross-axis coordinate is settled here; `draw_legend` takes the other
                // one from the plot rectangle, which does not exist yet.
                origin: Point::new(0.0, y),
                horizontal: true,
            })
        }
        _ => {
            let w = widths.iter().cloned().fold(0.0f64, f64::max) + 12.0;
            let leading = pos == LegendPosition::Leading;
            Some(LegendBox {
                insets: if leading {
                    Insets {
                        leading: w,
                        ..Default::default()
                    }
                } else {
                    Insets {
                        trailing: w,
                        ..Default::default()
                    }
                },
                origin: Point::new(if leading { 4.0 } else { size.width - w + 4.0 }, 0.0),
                horizontal: false,
            })
        }
    }
}

fn draw_legend(
    d: &mut Draw,
    entries: &[(String, Color)],
    lb: &LegendBox,
    plot: Rect,
    paint: &render::Paint2<'_>,
    bindings: &[Interaction],
) -> Vec<select::HitMark> {
    let mut hits = Vec::new();
    let label_size = paint.label_size;
    let font = &paint.font;
    let color = paint.chrome.label;
    let line = day_core::measure_text("0", label_size, font).height;
    let (swatch, gap) = (legend_swatch(line), legend_gap(label_size));
    // Aligned to the plot rather than centered in the pane: a horizontal key starts where the
    // first bar does, which is where the eye already is after reading the axis, and a vertical
    // one hangs from the top of the plot instead of floating against its middle.
    let mut x = if lb.horizontal {
        plot.origin.x
    } else {
        lb.origin.x
    };
    let mut y = if lb.horizontal {
        lb.origin.y
    } else {
        plot.origin.y
    };
    for (name, c) in entries {
        let w = swatch + 4.0 + day_core::measure_text(name, label_size, font).width;
        let rect = Rect::new(x - 2.0, y - 2.0, w + 4.0, line + 4.0);
        let record = Record::series(name.clone());
        let selected=bindings.iter().any(|b| matches!(b,Interaction::Point(p) if !p.parameter.is_empty() && p.parameter.contains(&record)));
        if selected {
            d.fill(Shape::Rect(rect), c.with_alpha(0.14));
        }
        hits.push(select::HitMark {
            legend: true,
            record,
            link: None,
            region: select::HitRegion::Rect(rect),
            at: Point::new(x + w / 2.0, y + line / 2.0),
            series: name.clone(),
            color: *c,
            x_label: name.clone(),
            y_label: String::new(),
            value: 0.0,
        });
        d.fill(
            Shape::Ellipse(Rect::new(x, y + (line - swatch) / 2.0, swatch, swatch)),
            *c,
        );
        d.text(
            name,
            Point::new(x + swatch + 4.0, y + line / 2.0),
            TextStyle {
                size: label_size,
                color,
                // Centered on the same line the swatch is centered on. With a top anchor this sat
                // half a line low, because the position given is the row's middle, not the text's
                // top, the kind of off-by-a-half-line only the anchor can state away.
                anchor: TextAnchor {
                    h: TextAlign::Leading,
                    v: TextVAlign::Middle,
                },
                font: font.clone(),
            },
        );
        if lb.horizontal {
            x += w + gap;
        } else {
            y += line + 4.0;
        }
    }
    hits
}

/// A chart's transition state: what it drew last, and the blend in flight.
#[derive(Default)]
struct Motion {
    /// The last resolved chart the data produced, and the size it was resolved at. Compared whole
    /// (`Resolved: PartialEq`) with each new one, so a re-record of the same picture (a frame of
    /// its own transition, the pointer moving over it) is told apart from a change.
    target: Option<resolve::Resolved>,
    size: Option<Size>,
    /// What is on screen, which a new transition starts from, so an interrupted one bends.
    shown: Option<resolve::Resolved>,
    /// The transition in flight, matched once when it started, and how far along it is.
    flight: Option<animate::Transition>,
    progress: f64,
    handle: Option<FrameHandle>,
}

/// Decide what to draw this frame: start, retarget or finish a transition toward `target`.
fn motion_frame(
    motion: &Rc<std::cell::RefCell<Motion>>,
    target: resolve::Resolved,
    size: Size,
    spec: Option<AnimSpec>,
    appear: bool,
    clock: FrameClock,
    frame: day_reactive::Trigger,
) -> resolve::Resolved {
    let mut m = motion.borrow_mut();
    let first = m.target.is_none();
    let changed = m.target.as_ref().is_some_and(|last| *last != target);
    let resized = m.size.is_some_and(|s| s != size);
    if first || changed {
        m.target = Some(target.clone());
    }
    m.size = Some(size);
    // A first appearance starts from the same chart with nothing in it, so every mark enters.
    let start = if first && appear {
        let mut empty = target.clone();
        empty.marks.clear();
        Some(empty)
    } else {
        m.shown.clone()
    };
    if changed || resized || (first && appear) {
        if let Some(h) = m.handle.take() {
            h.cancel();
        }
        m.flight = None;
        if let (false, Some(spec), Some(shown)) = (resized, spec, start) {
            m.flight = Some(animate::Transition::new(shown, target.clone()));
            m.progress = 0.0;
            // Weak, so the subscription does not keep its own chart alive: a chart removed
            // mid-flight drops its `Motion`, and with it the handle, which cancels the frames.
            let state = Rc::downgrade(motion);
            m.handle = Some(animate(clock, Timing::new(spec), move |s| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                {
                    let mut st = state.borrow_mut();
                    st.progress = s.progress;
                    // The subscription ends itself on the last sample; its handle stays in place
                    // (cancelling from inside its own callback is not this closure's business)
                    // until the next change replaces it.
                    if s.done {
                        st.flight = None;
                    }
                }
                frame.notify();
            }));
        }
    }
    let shown = match &m.flight {
        Some(flight) => flight.at(m.progress),
        None => target,
    };
    m.shown = Some(shown.clone());
    shown
}

/// The parts of a chart's configuration that [`Chart::configure`] may change between draws.
///
/// One struct rather than a reactive setter per knob: a chart has a lot of shape, and an app that
/// puts any of it under live control needs the same escape hatch for all of it.
#[derive(Clone, Debug)]
pub struct ChartConfig {
    pub x_scale: ScaleSpec,
    pub y_scale: ScaleSpec,
    pub x_axis: AxisSpec,
    pub y_axis: AxisSpec,
    /// The coordinate system, so a control can bend a chart (a donut's hole, a gauge's sweep)
    /// without rebuilding it; an animated chart moves between the two.
    pub coordinate: Coordinate,
}

impl Piece for Chart {
    fn build(self, cx: &mut BuildCx) -> RNode {
        let Chart {
            marks,
            x_scale,
            y_scale,
            x_axis,
            y_axis,
            coordinate,
            legend,
            chrome,
            label_size,
            font,
            colors,
            plot_insets,
            x_domain_fn,
            y_domain_fn,
            configure_fn,
            interactions,
            conditions,
            filter,
            domain,
            transition,
            appear,
        } = self;

        // The transition in flight, if any (README "Animation"). The draw closure starts one when
        // the resolved chart changes, a display-frame subscription advances it, and each frame's
        // re-record draws the blend. Everything the frame callback writes is read back here.
        let motion: Rc<std::cell::RefCell<Motion>> = Rc::default();
        let frame = day_reactive::Trigger::new();
        let clock = FrameClock::current();

        // What the last draw positioned, for the pointer to hit (`select::HitModel`). Shared
        // between the draw closure that fills it and the gesture handlers that read it, so hit
        // testing runs against what was drawn rather than re-resolving the whole pipeline on
        // every pointer move.
        let hits: Rc<std::cell::RefCell<select::HitModel>> = Rc::default();

        // A chart fills what it is given: the axis is measured per draw so that the size is the
        // container's to decide, and the leaf grows rather than asking for an intrinsic size the
        // data cannot know.
        let hits_draw = hits.clone();
        let interactions = Rc::new(interactions);
        let interactions_draw = interactions.clone();
        let pointer = Signal::new(false);
        let sessions: Rc<std::cell::RefCell<Vec<interaction::DragSession>>> =
            Rc::new(std::cell::RefCell::new(
                (0..interactions.len())
                    .map(|_| interaction::DragSession::default())
                    .collect(),
            ));
        let pinch_scale = Rc::new(std::cell::Cell::new(1.0));
        let bindings_need_hover = interactions.iter().any(|b| {
            matches!(
                b,
                Interaction::Inspect(_) | Interaction::Point(_) | Interaction::Links(_)
            )
        });
        let bindings_need_tap = bindings_need_hover
            || interactions
                .iter()
                .any(|b| matches!(b, Interaction::Brush(brush) if brush.clear_on_outside));
        let bindings_need_drag = interactions.iter().any(|b| {
            matches!(
                b,
                Interaction::Inspect(_)
                    | Interaction::Brush(_)
                    | Interaction::Viewport(_)
                    | Interaction::Point(interaction::PointBinding {
                        trigger: EventSource::Hover,
                        ..
                    })
            )
        });
        let bindings_need_viewport = interactions
            .iter()
            .any(|b| matches!(b, Interaction::Viewport(_)));
        let mut piece =
            day_pieces::Decorate::grow(day_pieces::canvas(move |d: &mut Draw, size: Size| {
                if size.width <= 2.0 || size.height <= 2.0 {
                    return;
                }
                // Tracked inside the binding, so a light/dark switch re-records without the app
                // rebuilding the chart.
                let ch = chrome
                    .clone()
                    .unwrap_or_else(|| Chrome::for_dark(day_core::dark_mode()));
                let mut ms = (marks)();
                if let Some(predicate) = &filter {
                    ms.retain(|mark| predicate.matches(mark));
                }
                // A domain closure is read here, inside the binding, so the scale follows the same
                // signals the marks do.
                let mut x_scale = x_scale.clone();
                if let Some(f) = &x_domain_fn {
                    x_scale.domain = f().map(|(lo, hi)| Interval::new(lo, hi));
                }
                let mut y_scale = y_scale.clone();
                if let Some(f) = &y_domain_fn {
                    y_scale.domain = f().map(|(lo, hi)| Interval::new(lo, hi));
                }
                // Read inside the binding, so every signal the closure touches re-records the chart.
                let mut x_axis = x_axis.clone();
                let mut y_axis = y_axis.clone();
                let mut coordinate = coordinate;
                if let Some(f) = &configure_fn {
                    let mut cfg = ChartConfig {
                        x_scale,
                        y_scale,
                        x_axis,
                        y_axis,
                        coordinate,
                    };
                    f(&mut cfg);
                    x_scale = cfg.x_scale;
                    y_scale = cfg.y_scale;
                    x_axis = cfg.x_axis;
                    y_axis = cfg.y_axis;
                    coordinate = cfg.coordinate;
                }

                if let Some(parameter) = domain {
                    if parameter.axes.x()
                        && let Some((lo, hi)) = parameter.x_domain()
                    {
                        x_scale.domain = Some(Interval::new(lo, hi));
                    }
                    if parameter.axes.y()
                        && let Some((lo, hi)) = parameter.y_domain()
                    {
                        y_scale.domain = Some(Interval::new(lo, hi));
                    }
                }
                // The legend is measured first: it takes its space out of the pane before the plot
                // rectangle is computed, so the two can never overlap.
                let series: Vec<(String, Color)> = {
                    let mut seen: Vec<String> = Vec::new();
                    for m in &ms {
                        if let Some(s) = &m.series {
                            let k = s.datum.to_string();
                            if !seen.contains(&k) {
                                seen.push(k);
                            }
                        }
                    }
                    seen.iter()
                        .enumerate()
                        .map(|(i, s)| (s.clone(), (colors)(i, s)))
                        .collect()
                };
                let lb = legend_layout(&series, legend, size, label_size, &font);
                let legend_insets = lb.as_ref().map(|l| l.insets).unwrap_or_default();

                let cfg = resolve::Config {
                    x_scale: &x_scale,
                    y_scale: &y_scale,
                    x_axis: &x_axis,
                    y_axis: &y_axis,
                    coordinate,
                    series_colors: &*colors,
                    label_size,
                    font: font.clone(),
                    legend_insets,
                    plot_insets,
                };
                let target = resolve::resolve(ms, size, &cfg);
                // Subscribed so each frame of a transition re-records; idle, it never fires.
                frame.track();
                let resolved = motion_frame(
                    &motion,
                    target,
                    size,
                    // Ambient intent wins, as it does for native widgets.
                    day_core::current_anim().or(transition),
                    appear,
                    clock,
                    frame,
                );
                let paint = render::Paint2 {
                    chrome: &ch,
                    label_size,
                    font: font.clone(),
                    x_axis: &x_axis,
                    y_axis: &y_axis,
                    series_colors: &*colors,
                };
                let emphasized = if conditions.is_empty() {
                    None
                } else {
                    let mut painted = resolved.clone();
                    for mark in &mut painted.marks {
                        for condition in &conditions {
                            let visual = if condition.predicate.matches(&mark.mark) {
                                condition.selected
                            } else {
                                condition.other
                            };
                            visual.apply(&mut mark.mark);
                        }
                    }
                    Some(painted)
                };
                render::draw(d, emphasized.as_ref().unwrap_or(&resolved), &paint);
                let legend_hits = if let Some(lb) = lb {
                    draw_legend(d, &series, &lb, resolved.plot, &paint, &interactions_draw)
                } else {
                    Vec::new()
                };
                if !interactions_draw.is_empty() {
                    let mut model = render::hit_model(&resolved, &paint);
                    model.marks.extend(legend_hits);
                    *hits_draw.borrow_mut() = model;
                    let model = hits_draw.borrow();
                    for binding in interactions_draw.iter() {
                        match binding {
                            Interaction::Inspect(inspect) => {
                                if let Some(sel) = inspect.signal.get() {
                                    render::draw_guides(d, &resolved, &paint, &sel, inspect.guides);
                                }
                            }
                            Interaction::Brush(brush) => {
                                if let Some(bounds) = brush.parameter.state.get()
                                    && let Some(rect) = interaction::brush_rect(&bounds, &model)
                                {
                                    d.fill(Shape::Rect(rect), ch.label.with_alpha(0.10));
                                    d.stroke(Shape::Rect(rect), ch.label.with_alpha(0.65), 1.0);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }))
            .cursor({
                let bindings = interactions.clone();
                move || {
                    if pointer.get() {
                        day_spec::Cursor::Pointer
                    } else if bindings.iter().any(|b| matches!(b, Interaction::Brush(_))) {
                        day_spec::Cursor::Crosshair
                    } else if bindings
                        .iter()
                        .any(|b| matches!(b, Interaction::Viewport(_)))
                    {
                        day_spec::Cursor::Grab
                    } else {
                        day_spec::Cursor::Default
                    }
                }
            });
        if bindings_need_hover {
            piece = piece.on_hover({
                let hits = hits.clone();
                let bindings = interactions.clone();
                move |at| {
                    let model = hits.borrow();
                    pointer.set_if_changed(at.and_then(|p| model.hit(p)).is_some_and(|hit| {
                        hit.link.is_some()
                            && bindings.iter().any(|b| matches!(b, Interaction::Links(_)))
                            || hit.legend
                                && bindings.iter().any(|b| matches!(b, Interaction::Point(_)))
                    }));
                    for binding in bindings.iter() {
                        match binding {
                            Interaction::Inspect(inspect) => {
                                inspect.signal.set_if_changed(
                                    at.and_then(|p| model.resolve(p, inspect.snap)),
                                );
                            }
                            Interaction::Point(point) if point.trigger == EventSource::Hover => {
                                point.parameter.replace(
                                    at.and_then(|p| model.record_at(p, point.snap))
                                        .map(|h| h.record),
                                );
                            }
                            _ => {}
                        }
                    }
                }
            });
        }
        if bindings_need_tap {
            piece = piece.on_tap_at({
                let hits = hits.clone();
                let bindings = interactions.clone();
                move |p| {
                    let model = hits.borrow();
                    for binding in bindings.iter() {
                        match binding {
                            Interaction::Inspect(inspect) => {
                                inspect
                                    .signal
                                    .set_if_changed(model.resolve(p, inspect.snap));
                            }
                            Interaction::Point(point) => point.activate(
                                model.record_at(p, point.snap).map(|h| h.record),
                                day_core::modifiers().shift,
                            ),
                            Interaction::Brush(brush) => interaction::tap_brush(*brush, &model, p),
                            Interaction::Links(links) => {
                                if let Some(target) = model.hit(p).and_then(|h| h.link.as_ref()) {
                                    if let Some(handler) = &links.handler {
                                        handler(target);
                                    } else {
                                        day_pieces::open_link(target);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            });
        }
        if bindings_need_drag {
            piece = piece.on_drag({
                let hits = hits.clone();
                let bindings = interactions.clone();
                let sessions = sessions.clone();
                move |drag| {
                    let model = hits.borrow();
                    let mut sessions = sessions.borrow_mut();
                    for (i, binding) in bindings.iter().enumerate() {
                        match binding {
                            Interaction::Inspect(inspect) => {
                                inspect
                                    .signal
                                    .set_if_changed(model.resolve(drag.location, inspect.snap));
                            }
                            Interaction::Point(point) if point.trigger == EventSource::Hover => {
                                point.parameter.replace(
                                    model.record_at(drag.location, point.snap).map(|h| h.record),
                                )
                            }
                            Interaction::Brush(brush) => interaction::drag_brush(
                                *brush,
                                &mut sessions[i],
                                &model,
                                drag.phase,
                                drag.location,
                            ),
                            Interaction::Viewport(viewport)
                                if !bindings.iter().any(|b| matches!(b, Interaction::Brush(_))) =>
                            {
                                if drag.phase == day_spec::DragPhase::Began {
                                    sessions[i].previous = Some(drag.location);
                                } else if let Some(previous) = sessions[i].previous {
                                    interaction::translate(
                                        viewport.parameter,
                                        &model,
                                        Point::new(
                                            drag.location.x - previous.x,
                                            drag.location.y - previous.y,
                                        ),
                                    );
                                    sessions[i].previous = Some(drag.location);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            });
        }
        if bindings_need_viewport {
            piece = piece.on_pan({
                let hits = hits.clone();
                let bindings = interactions.clone();
                move |pan| {
                    for binding in bindings.iter() {
                        if let Interaction::Viewport(viewport) = binding {
                            if day_core::modifiers().primary {
                                interaction::zoom(
                                    viewport.parameter,
                                    &hits.borrow(),
                                    pan.location,
                                    (-pan.delta.y * 0.01).exp(),
                                );
                            } else {
                                interaction::translate(
                                    viewport.parameter,
                                    &hits.borrow(),
                                    pan.delta,
                                );
                            }
                        }
                    }
                }
            });
        }
        if bindings_need_viewport {
            piece = piece.on_pinch({
                let hits = hits.clone();
                let bindings = interactions.clone();
                move |pinch| {
                    let previous = if pinch.phase == day_spec::DragPhase::Began {
                        1.0
                    } else {
                        pinch_scale.get()
                    };
                    for binding in bindings.iter() {
                        if let Interaction::Viewport(viewport) = binding {
                            interaction::zoom(
                                viewport.parameter,
                                &hits.borrow(),
                                pinch.location,
                                pinch.scale / previous,
                            );
                        }
                    }
                    pinch_scale.set(pinch.scale);
                }
            });
        }
        if !interactions.is_empty() {
            piece = piece.on_key({
                let bindings = interactions.clone();
                move |key| {
                    if key.key == "Escape" {
                        for binding in bindings.iter() {
                            match binding {
                                Interaction::Inspect(i) => {
                                    i.signal.set_if_changed(None);
                                }
                                Interaction::Point(p) => p.parameter.clear(),
                                Interaction::Brush(b) => b.parameter.clear(),
                                Interaction::Viewport(v) => v.parameter.clear(),
                                _ => {}
                            }
                        }
                    }
                }
            });
        }
        piece.build(cx)
    }
}
