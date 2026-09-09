// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Coordinate spaces.
//!
//! Wilkinson's *Grammar of Graphics* treats the coordinate system as a transformation applied
//! AFTER the marks are positioned, not as a different kind of chart. That is the design here, and
//! it is why this crate has no separate pie-chart code: a pie is a normalized stacked bar drawn in
//! polar coordinates, a rose is a stacked bar with a radial magnitude, and a radar plot is a line
//! mark whose x axis has been wrapped around a circle. One renderer, two projections.
//!
//! Every mark is positioned in **unit plot space** — `u` and `v` in `0..=1`, with `v` measured
//! upward from the baseline the way a reader thinks about a y axis — and a [`Coordinate`] turns
//! that into device points inside the plot rectangle.

use day_spec::{Point, Rect};

/// How unit plot space is laid onto the plot rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Coordinate {
    /// `u` runs left to right, `v` bottom to top. The default.
    #[default]
    Cartesian,
    /// `u` runs around the circle from `start_angle`, `v` from the centre outward.
    ///
    /// Angles are radians measured clockwise from twelve o'clock, which is where every pie chart
    /// in the world starts; the canvas's own zero is at three o'clock, and [`Coordinate::project`]
    /// is the only place that difference is expressed.
    Polar {
        start_angle: f64,
        /// One full turn is `start_angle + 2π`. A half turn draws a gauge.
        sweep: f64,
        /// The fraction of the radius left empty at the centre — a donut hole that applies to
        /// every mark, as distinct from one sector's own `inner_radius`.
        hole: f64,
    },
}

impl Coordinate {
    /// A full-turn polar space starting at twelve o'clock.
    pub fn polar() -> Self {
        Coordinate::Polar {
            start_angle: 0.0,
            sweep: std::f64::consts::TAU,
            hole: 0.0,
        }
    }

    /// A polar space with a hole — the donut every part-to-whole chart wants.
    pub fn donut(hole: f64) -> Self {
        Coordinate::Polar {
            start_angle: 0.0,
            sweep: std::f64::consts::TAU,
            hole: hole.clamp(0.0, 0.95),
        }
    }

    pub fn is_polar(&self) -> bool {
        matches!(self, Coordinate::Polar { .. })
    }

    /// Unit plot space → device points inside `rect`.
    pub fn project(&self, rect: Rect, u: f64, v: f64) -> Point {
        match self {
            Coordinate::Cartesian => Point::new(
                rect.origin.x + u * rect.size.width,
                // v = 0 is the BOTTOM: unit space is read the way an axis is, and the flip to
                // the canvas's downward y lives here and nowhere else.
                rect.origin.y + (1.0 - v) * rect.size.height,
            ),
            Coordinate::Polar {
                start_angle,
                sweep,
                hole,
            } => {
                let c = self.center(rect);
                let r_max = self.radius(rect);
                let r = r_max * (hole + v * (1.0 - hole));
                // Clockwise from twelve o'clock: subtract a quarter turn to move the canvas's
                // zero from three o'clock, and let u run positive clockwise.
                let a = start_angle + u * sweep - std::f64::consts::FRAC_PI_2;
                Point::new(c.x + r * a.cos(), c.y + r * a.sin())
            }
        }
    }

    /// The centre a polar space turns about — the rectangle's own centre.
    pub fn center(&self, rect: Rect) -> Point {
        Point::new(
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        )
    }

    /// The largest radius that fits, which is what makes a polar chart circular in a rectangular
    /// pane rather than elliptical.
    pub fn radius(&self, rect: Rect) -> f64 {
        rect.size.width.min(rect.size.height) / 2.0
    }

    /// The device angle for a unit position, in canvas radians. Sector rendering needs the angle
    /// itself, not just the projected point.
    pub fn angle(&self, u: f64) -> f64 {
        match self {
            Coordinate::Cartesian => 0.0,
            Coordinate::Polar {
                start_angle, sweep, ..
            } => start_angle + u * sweep - std::f64::consts::FRAC_PI_2,
        }
    }

    /// The device radius for a unit `v`.
    pub fn radius_at(&self, rect: Rect, v: f64) -> f64 {
        match self {
            Coordinate::Cartesian => 0.0,
            Coordinate::Polar { hole, .. } => self.radius(rect) * (hole + v * (1.0 - hole)),
        }
    }
}
