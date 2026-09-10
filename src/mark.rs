// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Marks: the geometric objects a chart is made of, and the channels data is encoded into.
//!
//! This is the grammar's noun. A mark is a *kind* (bar, line, sector, …) plus a set of **encodings**
//! — data columns bound to visual channels: position (x, y), color (`by_series`), symbol, and the
//! dodging channel (`by_position`). Nothing here knows about pixels or scales; a mark is a
//! description, and [`crate::render`] is the only place it becomes geometry.
//!
//! The surface mirrors Swift Charts' 2D marks so that a chart written against one reads the same
//! against the other, with two adjustments for Rust and for Day's API style (docs/api-style.md):
//! constructors stay at two positional, conventionally-ordered arguments (`x` then `y`, the same
//! exemption `Size::new(w, h)` takes), and everything Swift expresses as a labelled initializer
//! variant — `BarMark(x:yStart:yEnd:)` — is a builder method here (`.y_range(start, end)`).

use day_spec::{Color, LineCap, LineJoin};

use crate::data::Value;

/// The geometric object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    /// A rectangle from the category's band across to the value. Stacks by default.
    Bar,
    /// A polyline through its points, in x order.
    Line,
    /// The region between a line and a baseline (or between two lines). Stacks by default.
    Area,
    /// One symbol per datum.
    Point,
    /// An explicit rectangle — the mark heat maps are built from.
    Rectangle,
    /// An infinite line at a value, or a segment between two: thresholds, medians, spans.
    Rule,
    /// An angular wedge. The mark that makes a chart polar (pie, donut, rose).
    Sector,
}

/// How successive points are joined.
///
/// The monotone and Catmull-Rom cases are genuinely different curves, not styling: a monotone
/// spline is constrained never to overshoot the data, which is what makes it the honest choice for
/// a series that must not appear to dip below a value it never took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Interpolation {
    #[default]
    Linear,
    /// Fritsch–Carlson monotone cubic: smooth, and provably free of overshoot.
    Monotone,
    /// Catmull-Rom spline through every point. Smooth, and *may* overshoot.
    CatmullRom,
    /// Cardinal spline, Catmull-Rom's slacker cousin (tension 0.5).
    Cardinal,
    /// Hold the previous value, then jump at the new point.
    StepStart,
    /// Jump halfway between the two points.
    StepCenter,
    /// Jump at the new point, then hold.
    StepEnd,
}

impl Interpolation {
    pub fn is_step(&self) -> bool {
        matches!(
            self,
            Interpolation::StepStart | Interpolation::StepCenter | Interpolation::StepEnd
        )
    }
}

/// The shape a point mark draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Circle,
    Square,
    Triangle,
    Diamond,
    Pentagon,
    Cross,
    Plus,
    Asterisk,
}

impl Symbol {
    /// The cycle a symbol scale assigns from, in an order that keeps neighbours distinguishable at
    /// small sizes — a filled round, a filled corner, a filled point, then the open marks.
    pub const CYCLE: [Symbol; 8] = [
        Symbol::Circle,
        Symbol::Square,
        Symbol::Triangle,
        Symbol::Diamond,
        Symbol::Pentagon,
        Symbol::Cross,
        Symbol::Plus,
        Symbol::Asterisk,
    ];

    /// Whether the symbol is drawn as a stroke rather than a fill, which decides whether
    /// `symbol_size` means area or extent.
    pub fn is_stroked(&self) -> bool {
        matches!(self, Symbol::Cross | Symbol::Plus | Symbol::Asterisk)
    }
}

/// How a mark's extent across its band is decided — Swift Charts' `MarkDimension`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dimension {
    /// The scale's own band width, less its padding.
    Automatic,
    /// Exactly this many points.
    Fixed(f64),
    /// This fraction of the band.
    Ratio(f64),
    /// The band less this many points, split evenly on both sides.
    Inset(f64),
}

impl Dimension {
    pub fn resolve(&self, band: f64) -> f64 {
        match self {
            Dimension::Automatic => band,
            Dimension::Fixed(v) => *v,
            Dimension::Ratio(r) => band * r,
            Dimension::Inset(i) => (band - 2.0 * i).max(0.0),
        }
        .max(0.0)
    }
}

/// How marks sharing an x position combine — the grammar's *position adjustment*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Stacking {
    /// Sum upward from the baseline.
    #[default]
    Standard,
    /// Sum, then scale each column to fill the axis: a part-to-whole chart.
    Normalized,
    /// Sum, then centre the stack on the baseline — a streamgraph.
    Center,
    /// Do not combine; marks overlap where they coincide.
    Unstacked,
}

/// Where an annotation sits relative to its mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnnotationPosition {
    #[default]
    Automatic,
    Top,
    Bottom,
    Leading,
    Trailing,
    Overlay,
}

/// A short piece of text attached to one mark.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub text: String,
    pub position: AnnotationPosition,
    /// `None` draws in the chart's label color. A heat map's cell text sets one per cell, because
    /// no single color reads on both ends of a ramp.
    pub color: Option<Color>,
}

/// Everything about a mark's appearance that is not its geometry. Named fields per the API style
/// rule: fill what you set and take the rest from `..Default::default()`.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkStyle {
    /// An explicit color. `None` means the series scale assigns one.
    pub fill: Option<Color>,
    pub opacity: f64,
    pub corner_radius: f64,
    /// A vertical gradient from the mark's top to its bottom, for area and bar fills. Set, it
    /// replaces the solid fill; the series color still names the mark in the legend.
    pub gradient: Option<(Color, Color)>,
    /// Stroke width for line and rule marks, and the outline of a stroked symbol.
    pub line_width: f64,
    /// Dash pattern in points, empty for solid.
    pub dash: Vec<f64>,
    /// How a line or rule ends, and how its segments meet. Butt and miter are the canvas
    /// defaults; a price line reads better rounded, which is what [`Mark::rounded`] sets.
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub interpolation: Interpolation,
    pub symbol: Option<Symbol>,
    /// Symbol AREA in square points, matching Swift Charts' `symbolSize` — area rather than
    /// diameter so that a value twice as large reads as twice as much ink, which is the whole
    /// reason area is the right channel for magnitude.
    pub symbol_size: f64,
}

impl Default for MarkStyle {
    fn default() -> Self {
        MarkStyle {
            fill: None,
            opacity: 1.0,
            corner_radius: 0.0,
            gradient: None,
            line_width: 2.0,
            dash: Vec::new(),
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            interpolation: Interpolation::Linear,
            symbol: None,
            symbol_size: 64.0,
        }
    }
}

/// One mark: a kind, its position encodings, and its style.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub kind: MarkKind,
    pub x: Option<Value>,
    pub x_end: Option<Value>,
    pub y: Option<Value>,
    pub y_end: Option<Value>,
    /// The color channel — Swift Charts' `.foregroundStyle(by:)`. Also what the legend lists.
    pub series: Option<Value>,
    /// The dodging channel — Swift Charts' `.position(by:)`. Splits a band between its values so
    /// grouped bars sit side by side instead of on top of one another.
    pub dodge: Option<Value>,
    /// The symbol channel — `.symbol(by:)`.
    pub symbol_by: Option<Value>,
    pub style: MarkStyle,
    pub width: Dimension,
    pub height: Dimension,
    pub stacking: Stacking,
    pub annotation: Option<Annotation>,
    /// Points added to the mark's position after projection, for nudging overlapping marks apart.
    pub offset: (f64, f64),
    /// Draw order. Marks sort by this before rendering; equal z keeps declaration order.
    pub z: f64,
    // --- Sector (polar) ---
    /// Inner radius as a fraction of the outer, which is what turns a pie into a donut.
    pub inner_radius: f64,
    /// Outer radius as a fraction of the available radius.
    pub outer_radius: f64,
    /// Points held between each angular edge and its own radial ray, separating adjacent wedges.
    pub angular_inset: f64,
}

impl Mark {
    fn new(kind: MarkKind) -> Self {
        Mark {
            kind,
            x: None,
            x_end: None,
            y: None,
            y_end: None,
            series: None,
            dodge: None,
            symbol_by: None,
            style: MarkStyle::default(),
            width: Dimension::Automatic,
            height: Dimension::Automatic,
            // Bars, areas and SECTORS stack by default; points and lines do not — the same
            // defaults Swift Charts picks, and for the same reason: stacking is meaningful only
            // where the marks partition a quantity. A sector belongs in that list because a pie is
            // a stack: unstacked, every wedge would start at twelve o'clock and overlap the last.
            stacking: match kind {
                MarkKind::Bar | MarkKind::Area | MarkKind::Sector => Stacking::Standard,
                _ => Stacking::Unstacked,
            },
            annotation: None,
            offset: (0.0, 0.0),
            z: 0.0,
            inner_radius: 0.0,
            outer_radius: 1.0,
            angular_inset: 0.0,
        }
    }

    // --- Encoding channels ---

    /// Bind the color channel, so marks are grouped and colored by this column and the legend
    /// lists its values.
    pub fn by_series(mut self, v: Value) -> Self {
        self.series = Some(v);
        self
    }
    /// Bind the dodging channel — grouped (side-by-side) bars.
    pub fn by_position(mut self, v: Value) -> Self {
        self.dodge = Some(v);
        self
    }
    /// Bind the symbol channel.
    pub fn by_symbol(mut self, v: Value) -> Self {
        self.symbol_by = Some(v);
        self
    }

    /// Span the x axis from `start` to `end` — `BarMark(xStart:xEnd:)`, `RuleMark(xStart:xEnd:)`.
    pub fn x_range(mut self, start: Value, end: Value) -> Self {
        self.x = Some(start);
        self.x_end = Some(end);
        self
    }
    /// Span the y axis from `start` to `end`.
    pub fn y_range(mut self, start: Value, end: Value) -> Self {
        self.y = Some(start);
        self.y_end = Some(end);
        self
    }

    // --- Style ---

    pub fn foreground(mut self, c: Color) -> Self {
        self.style.fill = Some(c);
        self
    }
    pub fn opacity(mut self, o: f64) -> Self {
        self.style.opacity = o;
        self
    }
    pub fn corner_radius(mut self, r: f64) -> Self {
        self.style.corner_radius = r;
        self
    }
    pub fn line_width(mut self, w: f64) -> Self {
        self.style.line_width = w;
        self
    }
    pub fn dash(mut self, pattern: impl Into<Vec<f64>>) -> Self {
        self.style.dash = pattern.into();
        self
    }
    /// Fill an area or bar with a vertical gradient, `top` at the mark's top edge and `bottom` at
    /// its baseline — the fade under a price line. The series color still names the mark in the
    /// legend; only the fill changes.
    pub fn gradient(mut self, top: Color, bottom: Color) -> Self {
        self.style.gradient = Some((top, bottom));
        self
    }
    pub fn line_cap(mut self, cap: LineCap) -> Self {
        self.style.line_cap = cap;
        self
    }
    pub fn line_join(mut self, join: LineJoin) -> Self {
        self.style.line_join = join;
        self
    }
    /// Round caps and joins, so a stroked series turns its corners without a spike and a thick
    /// rule ends in a half circle rather than a square edge.
    pub fn rounded(mut self) -> Self {
        self.style.line_cap = LineCap::Round;
        self.style.line_join = LineJoin::Round;
        self
    }
    pub fn interpolation(mut self, m: Interpolation) -> Self {
        self.style.interpolation = m;
        self
    }
    pub fn symbol(mut self, s: Symbol) -> Self {
        self.style.symbol = Some(s);
        self
    }
    /// Symbol AREA in square points (Swift Charts' `symbolSize`).
    pub fn symbol_size(mut self, area: f64) -> Self {
        self.style.symbol_size = area;
        self
    }
    pub fn style(mut self, s: MarkStyle) -> Self {
        self.style = s;
        self
    }

    // --- Geometry ---

    pub fn width(mut self, d: Dimension) -> Self {
        self.width = d;
        self
    }
    pub fn height(mut self, d: Dimension) -> Self {
        self.height = d;
        self
    }
    pub fn stacking(mut self, s: Stacking) -> Self {
        self.stacking = s;
        self
    }
    pub fn offset(mut self, dx: f64, dy: f64) -> Self {
        self.offset = (dx, dy);
        self
    }
    pub fn z_index(mut self, z: f64) -> Self {
        self.z = z;
        self
    }
    pub fn annotation(mut self, position: AnnotationPosition, text: impl Into<String>) -> Self {
        let color = self.annotation.as_ref().and_then(|a| a.color);
        self.annotation = Some(Annotation {
            text: text.into(),
            position,
            color,
        });
        self
    }
    /// Draw this mark's annotation in `color` instead of the chart's label color.
    pub fn annotation_color(mut self, color: Color) -> Self {
        match &mut self.annotation {
            Some(a) => a.color = Some(color),
            None => {
                self.annotation = Some(Annotation {
                    text: String::new(),
                    position: AnnotationPosition::Automatic,
                    color: Some(color),
                })
            }
        }
        self
    }

    // --- Sector ---

    /// Inner radius as a fraction of the outer edge: 0 is a pie, 0.6 a donut.
    pub fn inner_radius(mut self, fraction: f64) -> Self {
        self.inner_radius = fraction.clamp(0.0, 0.99);
        self
    }
    /// Outer radius as a fraction of the plot's available radius — a wedge that reaches less far
    /// than its neighbours, which is how a rose chart encodes magnitude radially.
    pub fn outer_radius(mut self, fraction: f64) -> Self {
        self.outer_radius = fraction.clamp(0.0, 1.0);
        self
    }
    /// Points held between each angular edge and its own radial ray, separating adjacent wedges.
    ///
    /// The distance is perpendicular to the edge, so the channel between two neighbors keeps an
    /// even width for its whole length rather than tapering shut toward the center. A wedge with
    /// no hole therefore ends in a blunt tip short of the center, where its two inset edges meet.
    ///
    /// Capped at 5% of the wedge's outer radius, which is where Swift Charts stops widening its
    /// own `angularInset`. The cap is what keeps a gap chosen against a full-page chart from
    /// swallowing the same chart in a phone-width pane.
    pub fn angular_inset(mut self, points: f64) -> Self {
        self.angular_inset = points.max(0.0);
        self
    }
}

/// `BarMark(x:y:)`.
pub fn bar(x: Value, y: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Bar);
    m.x = Some(x);
    m.y = Some(y);
    m
}

/// `LineMark(x:y:)`.
pub fn line(x: Value, y: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Line);
    m.x = Some(x);
    m.y = Some(y);
    m
}

/// `AreaMark(x:y:)`.
pub fn area(x: Value, y: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Area);
    m.x = Some(x);
    m.y = Some(y);
    m
}

/// `PointMark(x:y:)`.
pub fn point(x: Value, y: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Point);
    m.x = Some(x);
    m.y = Some(y);
    m
}

/// `RectangleMark(x:y:)` — one cell of a heat map. Give it ranges with [`Mark::x_range`] /
/// [`Mark::y_range`] for an explicit rectangle.
pub fn rect(x: Value, y: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Rectangle);
    m.x = Some(x);
    m.y = Some(y);
    m
}

/// `RuleMark(x:)` — a vertical rule spanning the plot.
pub fn rule_x(x: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Rule);
    m.x = Some(x);
    m
}

/// `RuleMark(y:)` — a horizontal rule spanning the plot.
pub fn rule_y(y: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Rule);
    m.y = Some(y);
    m
}

/// `SectorMark(angle:)` — a wedge whose angle is proportional to its value. The mark that makes a
/// chart polar; see [`crate::coord::Coordinate`].
pub fn sector(angle: Value) -> Mark {
    let mut m = Mark::new(MarkKind::Sector);
    // The angular quantity rides the y channel, so one stacking implementation serves both
    // coordinate spaces: a pie IS a normalized stacked bar in polar coordinates, which is
    // Wilkinson's observation and the reason this crate has no separate pie code path.
    m.y = Some(angle);
    m
}
