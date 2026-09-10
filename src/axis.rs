// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Axes and the legend — the grammar's *guides*: the marks that describe the scales rather than
//! the data.
//!
//! What an axis shows is configuration; WHERE its labels go is not. Their positions come from the
//! tick search in [`crate::ticks`], which reads the axis's measured pixel length, so an axis
//! relabels itself as the pane resizes instead of carrying a spacing decided when the chart was
//! written.

use std::rc::Rc;

use crate::data::Datum;

/// Renders one tick's value. Named because it appears in two places and the bare `Rc<dyn Fn…>`
/// reads as noise at both.
pub type TickFormatter = Rc<dyn Fn(&Datum) -> String>;

/// Which edge an axis is drawn on. `Automatic` is the conventional edge for that channel — the
/// bottom for x, the leading edge for y — resolved at render time so a right-to-left layout gets
/// the correct side without the app restating it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AxisPosition {
    #[default]
    Automatic,
    Leading,
    Trailing,
    Top,
    Bottom,
}

/// How one axis presents itself. Named fields per the API style rule; the builder methods on
/// [`crate::Chart`] set them without a struct literal for the common cases.
#[derive(Clone)]
pub struct AxisSpec {
    pub hidden: bool,
    pub grid: bool,
    /// A solid line along the axis's own edge. Off by default: Swift Charts draws none, and a
    /// grid already says where the plot ends. `Chart::axis_rule` puts it back.
    pub rule: bool,
    /// A short mark from the axis edge to each label. Off by default, for the same reason.
    pub ticks: bool,
    pub labels: bool,
    pub position: AxisPosition,
    /// The axis's own name, drawn only when the app sets one. A data column's name is NOT used as
    /// a fallback: under six labels already reading `C1 C2 C3`, a title saying "Category" is noise,
    /// and it costs a line of plot height. Swift Charts asks for the same thing explicitly.
    pub title: Option<String>,
    /// How many labels to aim for. The tick search treats this as a target to be scored against,
    /// not a promise — an axis too short for five readable labels gets three.
    pub desired_count: usize,
    /// Pin the tick positions instead of searching for them.
    pub values: Option<Vec<f64>>,
    /// Render one tick's value. `None` formats by the scale's kind.
    pub format: Option<TickFormatter>,
}

impl Default for AxisSpec {
    fn default() -> Self {
        AxisSpec {
            hidden: false,
            grid: true,
            rule: false,
            ticks: false,
            labels: true,
            position: AxisPosition::Automatic,
            title: None,
            // Five is the count the extended-Wilkinson paper uses as its default target, and it is
            // the number readers reconstruct a scale from most reliably.
            desired_count: 5,
            values: None,
            format: None,
        }
    }
}

impl std::fmt::Debug for AxisSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AxisSpec")
            .field("hidden", &self.hidden)
            .field("grid", &self.grid)
            .field("rule", &self.rule)
            .field("ticks", &self.ticks)
            .field("labels", &self.labels)
            .field("position", &self.position)
            .field("title", &self.title)
            .field("desired_count", &self.desired_count)
            .field("values", &self.values)
            .field("format", &self.format.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

/// Where the legend sits, or that there is none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LegendPosition {
    /// Shown when the color channel is bound to more than one series, hidden otherwise — there is
    /// nothing to explain about a chart with one series.
    #[default]
    Automatic,
    Top,
    Bottom,
    Leading,
    Trailing,
    Hidden,
}
