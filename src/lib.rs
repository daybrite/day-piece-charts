// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! day-piece-charts — charts for Day apps, built on the grammar of graphics and drawn on the
//! canvas.
//!
//! An external Day Piece with **no native half**. A chart is recorded as a `Draw` display list and
//! replayed by whichever backend the app runs on, so it looks the same on all nine targets because
//! it *is* the same — there is no per-toolkit chart code to drift.
//!
//! # The grammar
//!
//! The design follows Wilkinson's *Grammar of Graphics* (and Wickham's restatement of it), which is
//! also the lineage Swift Charts comes from. A chart is not a *type*; it is a composition:
//!
//! | stage | here |
//! |---|---|
//! | variables | [`value`] / [`time`] bind a column to a channel |
//! | marks | [`bar`], [`line`], [`area`], [`point`], [`rect`], [`rule_x`], [`sector`] |
//! | position adjustment | [`Stacking`], [`Mark::by_position`] (dodging) |
//! | scales | [`ScaleKind::Linear`], `Log`, `Power`, `Time`, `Band`, `Point` |
//! | guides | [`AxisSpec`], [`LegendPosition`] |
//! | coordinates | [`Coordinate::Cartesian`], [`Coordinate::polar`] |
//!
//! Because the coordinate system is a transformation applied after positioning, a pie chart is a
//! normalized stacked bar in polar coordinates — and that is literally how it is implemented. There
//! is no pie-chart code path.
//!
//! # Reading it against Swift Charts
//!
//! The mark and modifier vocabulary tracks Swift Charts closely enough to port a chart by eye, with
//! two deliberate differences that come from Rust and from Day's API style (docs/api-style.md):
//! constructors take two positional, conventionally-ordered arguments (`x` then `y`), and Swift's
//! labelled initializer variants — `BarMark(x:yStart:yEnd:)` — are builder methods
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

pub mod axis;
pub mod coord;
pub mod data;
pub mod layout;
pub mod mark;
pub mod render;
pub mod resolve;
pub mod scale;
pub mod select;
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

use day_core::{BuildCx, Piece, RNode};
use day_pieces::{Draw, TextStyle};
use day_reactive::Signal;
use day_spec::props::TextAlign;
use day_spec::{CanvasFont, Color, Point, Rect, Shape, Size, TextAnchor, TextVAlign};

/// Assigns a color to a series by index and name. Named because the chart stores one and every
/// stage of the pipeline is handed a reference to it.
pub type SeriesColors = Rc<dyn Fn(usize, &str) -> Color>;

/// Answers a scale's domain from inside the chart's binding; `None` leaves it inferred.
pub type DomainFn = Rc<dyn Fn() -> Option<(f64, f64)>>;

/// A chart. Build it with [`chart`], configure it with the builder methods, and place it like any
/// other piece — it grows to fill, so give it a `.frame(w, h)` or let its container size it.
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
    select: Option<Signal<Option<select::Selection>>>,
    snap: select::Snap,
    guides: select::Guides,
}

/// A chart of the marks the closure produces.
///
/// The closure is re-run as a reactive binding, so reading a `Signal` inside it makes the chart
/// follow that signal — which is how a chart animates, filters, or re-sorts without any imperative
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
        select: None,
        snap: select::Snap::default(),
        guides: select::Guides::NONE,
    }
}

impl Chart {
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
    /// Pin the x domain to whatever the closure answers, re-asked inside the chart's binding —
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
    /// Draw the y axis on the trailing edge — the convention for a price chart, where the latest
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
    /// Report what the pointer is over into `sig`, and read the same signal to draw
    /// [`guides`](Chart::guides) for it (README.md "Selection").
    ///
    /// Two-way, like a slider's value: the chart writes, the chart reads, and the app owns the
    /// signal — so the same selection can drive a readout, a detail pane or anything else without
    /// the chart knowing. `None` means nothing is selected, which is what a pointer leaving the
    /// plot writes.
    ///
    /// Fed by a tap, a drag and a hover together, because no one of them covers every device: a
    /// hover is pointer-only, a tap is what a phone has, and a press that wiggles a pixel becomes
    /// a drag on some backends. All three write the same value, so a backend reporting two of them
    /// for one press changes nothing.
    pub fn select(mut self, sig: Signal<Option<select::Selection>>) -> Self {
        self.select = Some(sig);
        self
    }

    /// How a point becomes a selection: the nearest x with every series' value there
    /// ([`Snap::NearestX`](select::Snap::NearestX), the default — a line chart), or the single
    /// nearest mark ([`Snap::NearestMark`](select::Snap::NearestMark) — a scatter).
    pub fn snap(mut self, snap: select::Snap) -> Self {
        self.snap = snap;
        self
    }

    /// What the chart draws for the current selection —
    /// [`Guides::RULE`](select::Guides::RULE), [`Guides::CROSSHAIR`](select::Guides::CROSSHAIR),
    /// or a struct of your own. The default draws nothing, so `.select(sig)` alone reports without
    /// changing the picture.
    pub fn guides(mut self, guides: select::Guides) -> Self {
        self.guides = guides;
        self
    }

    /// Replace the measured margins with fixed ones. The default insets are computed from the
    /// axis labels the chart is about to draw; a chart with no axes wants none of that room, and
    /// a chart whose marks overhang the plot (a fat point at the last sample) wants a little.
    pub fn plot_insets(mut self, insets: Insets) -> Self {
        self.plot_insets = Some(insets);
        self
    }
    /// Drop the grid on both axes — the right call for a chart whose marks already partition the
    /// plot, like a stacked-to-100% bar.
    pub fn no_grid(mut self) -> Self {
        self.x_axis.grid = false;
        self.y_axis.grid = false;
        self
    }
    /// Name the x axis (`.chartXAxisLabel`). Unset, it takes the label the data column carries.
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
    /// Render each x tick's value yourself — currency, percentages, a custom date format.
    pub fn x_format(mut self, f: impl Fn(&Datum) -> String + 'static) -> Self {
        self.x_axis.format = Some(Rc::new(f));
        self
    }
    pub fn y_format(mut self, f: impl Fn(&Datum) -> String + 'static) -> Self {
        self.y_axis.format = Some(Rc::new(f));
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
    /// index and its name, so a chart can key colors off meaning — red for "Loss" — rather than
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
    let swatch = line * 0.8;
    let gap = label_size * 1.4;
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
            let total: f64 = widths.iter().sum::<f64>() + gap * (entries.len() as f64 - 1.0);
            let h = line + 8.0;
            let x = ((size.width - total) / 2.0).max(0.0);
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
                origin: Point::new(x, y),
                horizontal: true,
            })
        }
        _ => {
            let w = widths.iter().cloned().fold(0.0f64, f64::max) + 12.0;
            let h = entries.len() as f64 * (line + 4.0);
            let y = ((size.height - h) / 2.0).max(0.0);
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
                origin: Point::new(if leading { 4.0 } else { size.width - w + 4.0 }, y),
                horizontal: false,
            })
        }
    }
}

fn draw_legend(
    d: &mut Draw,
    entries: &[(String, Color)],
    lb: &LegendBox,
    label_size: f64,
    font: &CanvasFont,
    color: Color,
) {
    let line = day_core::measure_text("0", label_size, font).height;
    let swatch = line * 0.8;
    let gap = label_size * 1.4;
    let mut x = lb.origin.x;
    let mut y = lb.origin.y;
    for (name, c) in entries {
        d.fill(
            Shape::RoundedRect(
                Rect::new(x, y + (line - swatch) / 2.0, swatch, swatch),
                swatch * 0.25,
            ),
            *c,
        );
        d.text(
            name,
            Point::new(x + swatch + 4.0, y + line / 2.0),
            TextStyle {
                size: label_size,
                color,
                // Centered on the same line the swatch is centered on. With a top anchor this sat
                // half a line low, because the point given is the ROW's middle, not the text's
                // top — the kind of off-by-a-half-line only the anchor can state away.
                anchor: TextAnchor {
                    h: TextAlign::Leading,
                    v: TextVAlign::Middle,
                },
                font: font.clone(),
            },
        );
        let w = swatch + 4.0 + day_core::measure_text(name, label_size, font).width;
        if lb.horizontal {
            x += w + gap;
        } else {
            y += line + 4.0;
        }
    }
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
            select,
            snap,
            guides,
        } = self;

        // What the last draw positioned, for the pointer to hit (`select::HitModel`). Shared
        // between the draw closure that fills it and the gesture handlers that read it — hit
        // testing runs against what was DRAWN rather than re-resolving the whole pipeline on
        // every pointer move.
        let hits: Rc<std::cell::RefCell<select::HitModel>> = Rc::default();

        // A chart fills what it is given: the whole point of measuring the axis per draw is that
        // the size is the container's to decide, so the leaf grows rather than asking for an
        // intrinsic size the data cannot know.
        let hits_draw = hits.clone();
        day_pieces::Decorate::grow(day_pieces::canvas(move |d: &mut Draw, size: Size| {
            if size.width <= 2.0 || size.height <= 2.0 {
                return;
            }
            // Tracked inside the binding, so a light/dark switch re-records without the app
            // rebuilding the chart.
            let ch = chrome
                .clone()
                .unwrap_or_else(|| Chrome::for_dark(day_core::dark_mode()));
            let ms = (marks)();
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
            let resolved = resolve::resolve(ms, size, &cfg);
            let paint = render::Paint2 {
                chrome: &ch,
                label_size,
                font: font.clone(),
                x_axis: &x_axis,
                y_axis: &y_axis,
                series_colors: &*colors,
            };
            render::draw(d, &resolved, &paint);
            if let Some(lb) = lb {
                draw_legend(d, &series, &lb, label_size, &font, ch.label);
            }
            // Record the hit model AFTER drawing, from the same `resolved` the marks came from,
            // so a pointer can only ever select something that is actually on screen.
            if select.is_some() {
                *hits_draw.borrow_mut() = render::hit_model(&resolved, &paint);
            }
            // The guides are drawn from the signal, so they follow the pointer without the app
            // rebuilding anything: this read subscribes the whole recording to the selection.
            if let Some(sig) = &select
                && guides != select::Guides::NONE
                && let Some(sel) = sig.get()
            {
                render::draw_guides(d, &resolved, &paint, &sel, guides);
            }
        }))
        .on_hover({
            let hits = hits.clone();
            let sig = select;
            move |at| {
                if let Some(sig) = &sig {
                    // Leaving clears it — which is why the handler takes an `Option` rather than
                    // a phase to match on.
                    let next = at.and_then(|p| hits.borrow().resolve(p, snap));
                    if sig.get_untracked() != next {
                        sig.set(next);
                    }
                }
            }
        })
        .on_tap_at({
            let hits = hits.clone();
            let sig = select;
            move |p| {
                if let Some(sig) = &sig {
                    let next = hits.borrow().resolve(p, snap);
                    if sig.get_untracked() != next {
                        sig.set(next);
                    }
                }
            }
        })
        .on_drag({
            let hits = hits.clone();
            let sig = select;
            // A finger scrubbing along the plot: every phase re-selects, so the label tracks the
            // drag. Nothing is cleared at the end — a touch device has no pointer to leave with,
            // so the last selection stands until the next press, which is what a reader wants.
            move |drag| {
                if let Some(sig) = &sig {
                    let next = hits.borrow().resolve(drag.location, snap);
                    if sig.get_untracked() != next {
                        sig.set(next);
                    }
                }
            }
        })
        .build(cx)
    }
}
