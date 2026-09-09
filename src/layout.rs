// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Where the plot rectangle ends up.
//!
//! The margins around a plot are not a constant. They are exactly as wide as the axis labels that
//! have to fit in them, and those labels are not known until the scale's domain and the tick search
//! have run — which themselves need the plot's size. That circularity is real, and this module
//! resolves it the way layout engines do: **measure, then settle**.
//!
//! 1. Assume a generous margin and compute the ticks that would fit.
//! 2. Measure those labels with `day::measure_text` and derive the margin they actually need.
//! 3. Recompute the ticks against the plot rectangle that margin leaves.
//!
//! One refinement pass is enough in practice — the second pass changes the axis length by the
//! difference between an estimated and a measured margin, which moves the label count by at most
//! one — and stopping at one keeps the draw closure cheap enough to run on every frame of a resize.

use day_spec::{Rect, Size};

/// The space each edge of the plot gives up to its guides.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f64,
    pub leading: f64,
    pub bottom: f64,
    pub trailing: f64,
}

impl Insets {
    pub fn uniform(v: f64) -> Self {
        Insets {
            top: v,
            leading: v,
            bottom: v,
            trailing: v,
        }
    }

    /// The rectangle left for the plot after the guides take theirs. Never inverted: a pane too
    /// small for its own axes yields an empty plot rather than a negative one, which would draw
    /// marks outside the canvas.
    pub fn apply(&self, size: Size) -> Rect {
        let w = (size.width - self.leading - self.trailing).max(0.0);
        let h = (size.height - self.top - self.bottom).max(0.0);
        Rect::new(self.leading, self.top, w, h)
    }
}
