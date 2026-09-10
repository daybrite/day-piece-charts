//! Selection: turning a point on the chart back into data, and drawing guides for it.
//!
//! The chart WRITES what is under the pointer into an app-owned signal and READS the same signal
//! to draw its guides. Two-way, like a slider's value — which is what makes the whole behaviour
//! `.select(sig).snap(..).guides(..)` and leaves the app free to show the same selection its own
//! way (a readout beside the chart, a detail pane) by reading the signal it already owns.
//!
//! Three gestures feed it, and all three are needed. `on_hover` is the desktop idiom but is
//! pointer-only; `on_tap_at` is what a phone has; `on_drag` is what a press that wiggles a pixel
//! becomes on some backends, and is also how a finger scrubs along a series. Writing the same
//! selection from all three is idempotent, so a backend that reports two of them for one press
//! costs nothing (docs/canvas.md "Interaction").

use day_spec::{Color, Point, Rect};

/// One series' value at the selected position.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectedValue {
    /// The series name, empty for a chart with no color channel.
    pub series: String,
    pub color: Color,
    /// The value, formatted by the chart's own `y_format` — so it is already localized if the
    /// app's formatter localizes, and identical to what the axis shows.
    pub label: String,
    /// The mark's own position in canvas points, for a highlight ring.
    pub at: Point,
    /// The underlying number, for an app that wants to compute rather than display.
    pub value: f64,
}

/// What the pointer is over. `None` in the signal means nothing is.
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    /// Where the guides cross, in canvas points.
    pub at: Point,
    /// The position's label, formatted by the chart's own `x_format`.
    pub x_label: String,
    /// The selected marks. One entry under [`Snap::NearestMark`]; one per series that has a value
    /// at the selected position under [`Snap::NearestX`].
    pub values: Vec<SelectedValue>,
}

/// How a point becomes a selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Snap {
    /// The nearest position along x, with EVERY series' value there — what a line chart wants: a
    /// scrub along the axis reading all the series at once.
    #[default]
    NearestX,
    /// The single nearest mark in both axes — what a scatter wants, where two points sharing an x
    /// are unrelated.
    NearestMark,
}

/// Which guides the chart draws for the current selection.
///
/// A plain struct of flags rather than an enum: they compose, and a chart that wants the rule
/// without the label is as reasonable as one that wants both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Guides {
    /// A rule through the selection, across the plot's height.
    pub vertical: bool,
    /// A rule through it across the plot's width.
    pub horizontal: bool,
    /// A ring around each selected mark.
    pub points: bool,
    /// A box naming the position and every selected value.
    pub label: bool,
}

impl Guides {
    /// Nothing — the default, so `.select(sig)` alone reports without drawing.
    pub const NONE: Guides = Guides {
        vertical: false,
        horizontal: false,
        points: false,
        label: false,
    };
    /// A vertical rule, rings on the marks, and the label box: the line-chart idiom.
    pub const RULE: Guides = Guides {
        vertical: true,
        horizontal: false,
        points: true,
        label: true,
    };
    /// Both rules, a ring, and the label: the scatter idiom, where the y position matters as much
    /// as the x.
    pub const CROSSHAIR: Guides = Guides {
        vertical: true,
        horizontal: true,
        points: true,
        label: true,
    };
}

/// One candidate mark, recorded during the draw that positioned it.
///
/// Hit testing runs against what was DRAWN, not against a re-resolve: the draw already projected
/// every mark, and re-deriving it on each pointer move would repeat the whole pipeline — the tick
/// search included — for every mouse motion.
#[derive(Clone, Debug)]
pub struct HitMark {
    pub at: Point,
    pub series: String,
    pub color: Color,
    pub x_label: String,
    pub y_label: String,
    pub value: f64,
}

/// What the last draw left behind for the pointer to hit.
#[derive(Clone, Debug, Default)]
pub struct HitModel {
    pub plot: Rect,
    pub marks: Vec<HitMark>,
}

impl HitModel {
    /// The selection at `p`, or `None` if the point is outside the plot or nothing is near enough.
    pub fn resolve(&self, p: Point, snap: Snap) -> Option<Selection> {
        if self.marks.is_empty() || !contains(self.plot, p) {
            return None;
        }
        match snap {
            Snap::NearestMark => {
                // A radius, so hovering empty space selects nothing rather than the far-off mark
                // that happens to be closest. Generous enough to forgive a few pixels of aim.
                const REACH: f64 = 24.0;
                let hit = self.marks.iter().min_by(|a, b| {
                    dist2(a.at, p)
                        .partial_cmp(&dist2(b.at, p))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })?;
                (dist2(hit.at, p) <= REACH * REACH).then(|| Selection {
                    at: hit.at,
                    x_label: hit.x_label.clone(),
                    values: vec![value_of(hit)],
                })
            }
            Snap::NearestX => {
                // The nearest x with anything on it, then every mark sharing that x — so scrubbing
                // a multi-series line chart reads all the series at one position, which is what
                // makes the label box worth having.
                let target = self
                    .marks
                    .iter()
                    .min_by(|a, b| {
                        (a.at.x - p.x)
                            .abs()
                            .partial_cmp(&(b.at.x - p.x).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })?
                    .at
                    .x;
                // One value per SERIES, the one nearest the selected position. Two samples of a
                // series can land within a pixel of each other on a dense time axis — a year of
                // daily closes in a few hundred points — and listing both says the same series
                // has two values at one date, which is not a thing.
                let mut values: Vec<SelectedValue> = Vec::new();
                for m in self.marks.iter().filter(|m| (m.at.x - target).abs() < 1.0) {
                    match values.iter_mut().find(|v| v.series == m.series) {
                        Some(existing) => {
                            if (m.at.x - target).abs() < (existing.at.x - target).abs() {
                                *existing = value_of(m);
                            }
                        }
                        None => values.push(value_of(m)),
                    }
                }
                let x_label = self
                    .marks
                    .iter()
                    .find(|m| (m.at.x - target).abs() < 1.0)
                    .map(|m| m.x_label.clone())
                    .unwrap_or_default();
                (!values.is_empty()).then_some(Selection {
                    at: Point::new(target, p.y),
                    x_label,
                    values,
                })
            }
        }
    }
}

fn value_of(m: &HitMark) -> SelectedValue {
    SelectedValue {
        series: m.series.clone(),
        color: m.color,
        label: m.y_label.clone(),
        at: m.at,
        value: m.value,
    }
}

fn dist2(a: Point, b: Point) -> f64 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy
}

/// Whether `p` is inside `r`, with a little slack: a pointer on the axis line itself is over the
/// plot as far as a reader is concerned.
fn contains(r: Rect, p: Point) -> bool {
    const SLACK: f64 = 2.0;
    p.x >= r.origin.x - SLACK
        && p.x <= r.origin.x + r.size.width + SLACK
        && p.y >= r.origin.y - SLACK
        && p.y <= r.origin.y + r.size.height + SLACK
}
