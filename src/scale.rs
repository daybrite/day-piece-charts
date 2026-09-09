// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Scales: the map from data space to plot space, and back.
//!
//! In the grammar of graphics a scale is one of the few genuinely separate concerns — it knows
//! nothing about marks and marks know nothing about pixels. Everything a chart draws goes through
//! one, which is what lets the same mark render on a linear axis, a log axis, or a band of
//! categories without knowing which.
//!
//! Two families, because they answer different questions:
//!
//! - **Continuous** ([`ScaleKind::Linear`], [`Log`](ScaleKind::Log), [`Power`](ScaleKind::Power),
//!   [`Time`](ScaleKind::Time)) maps an interval onto an interval. It is invertible, which is what
//!   a hit test needs.
//! - **Discrete** ([`ScaleKind::Band`], [`Point`](ScaleKind::Point)) maps a finite ordered set onto
//!   evenly spaced positions. A band has WIDTH — it is what a bar is drawn across — and a point
//!   does not; that is the whole difference, and it is why a bar chart and a line chart of the same
//!   categorical column want different scales.
//!
//! Band geometry follows d3's `scaleBand` (`paddingInner`, `paddingOuter`, `align`), which is the
//! de-facto standard and the same arithmetic Swift Charts' automatic bar width lands on.

use crate::data::{Datum, Interval};

/// How a scale interpolates.
#[derive(Clone, Debug, PartialEq)]
pub enum ScaleKind {
    Linear,
    /// Log base `base`. The domain is clamped to positive values: a log scale has no answer for
    /// zero or a negative, and silently plotting one at the axis floor invents data.
    Log {
        base: f64,
    },
    /// `v^exponent`, the family square-root scales come from (`exponent: 0.5`). Signed-symmetric,
    /// so a negative domain behaves.
    Power {
        exponent: f64,
    },
    /// Continuous like linear, but labelled by calendar (see [`crate::ticks::time_ticks`]).
    Time,
    /// Discrete with width.
    Band,
    /// Discrete without width.
    Point,
}

impl ScaleKind {
    pub fn is_discrete(&self) -> bool {
        matches!(self, ScaleKind::Band | ScaleKind::Point)
    }
}

/// Padding for a band scale, as fractions of one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandPadding {
    /// Space between adjacent bands, as a fraction of the step. d3's `paddingInner`.
    pub inner: f64,
    /// Space before the first and after the last band, as a fraction of the step.
    pub outer: f64,
    /// Where leftover space goes: 0 all before, 1 all after, 0.5 split evenly.
    pub align: f64,
}

impl Default for BandPadding {
    fn default() -> Self {
        // A fifth of the step between bars is the ratio bar charts converge on: enough that two
        // bars read as two, little enough that a category still reads as one block.
        BandPadding {
            inner: 0.2,
            outer: 0.1,
            align: 0.5,
        }
    }
}

/// A resolved scale: domain, output range, and how to interpolate between them.
#[derive(Clone, Debug)]
pub struct Scale {
    pub kind: ScaleKind,
    /// The continuous domain, for a continuous kind.
    pub domain: Interval,
    /// The ordered categories, for a discrete kind.
    pub categories: Vec<String>,
    /// The output extent in plot points. `lo > hi` is legal and is how a y axis points up: the
    /// range is simply reversed.
    pub range: (f64, f64),
    pub padding: BandPadding,
    /// Whether the domain was given by the app rather than inferred, which is what decides if
    /// values outside it are clipped or allowed to extend the plot.
    pub explicit_domain: bool,
}

impl Scale {
    pub fn continuous(kind: ScaleKind, domain: Interval, range: (f64, f64)) -> Self {
        Scale {
            kind,
            domain: domain.nondegenerate(),
            categories: Vec::new(),
            range,
            padding: BandPadding::default(),
            explicit_domain: false,
        }
    }

    pub fn discrete(kind: ScaleKind, categories: Vec<String>, range: (f64, f64)) -> Self {
        Scale {
            kind,
            domain: Interval::new(0.0, 1.0),
            categories,
            range,
            padding: BandPadding::default(),
            explicit_domain: false,
        }
    }

    /// The normalized position of a continuous value in `0..=1` before the range is applied.
    ///
    /// Kept separate from [`Scale::project`] because the normalization is where the scale's KIND
    /// lives; everything after it is the same affine step for every continuous kind.
    fn normalize(&self, v: f64) -> f64 {
        let Interval { lo, hi } = self.domain;
        match &self.kind {
            ScaleKind::Linear | ScaleKind::Time => (v - lo) / (hi - lo),
            ScaleKind::Log { base } => {
                let (l, h, v) = (lo.max(f64::MIN_POSITIVE), hi, v.max(f64::MIN_POSITIVE));
                (v.log(*base) - l.log(*base)) / (h.log(*base) - l.log(*base))
            }
            ScaleKind::Power { exponent } => {
                // Signed power keeps the transform monotone through zero, so a domain that
                // straddles it does not fold onto itself.
                let f = |x: f64| x.abs().powf(*exponent) * x.signum();
                (f(v) - f(lo)) / (f(hi) - f(lo))
            }
            ScaleKind::Band | ScaleKind::Point => (v - lo) / (hi - lo),
        }
    }

    /// Data space → plot points.
    pub fn project(&self, d: &Datum) -> Option<f64> {
        if self.kind.is_discrete() {
            let key = match d {
                Datum::Category(s) => s.clone(),
                other => other.to_string(),
            };
            let i = self.categories.iter().position(|c| *c == key)?;
            return Some(match self.kind {
                ScaleKind::Band => self.band_start(i) + self.band_width() / 2.0,
                _ => self.point_at(i),
            });
        }
        let v = d.as_continuous()?;
        if !v.is_finite() {
            return None;
        }
        let t = self.normalize(v);
        Some(self.range.0 + t * (self.range.1 - self.range.0))
    }

    /// Plot points → data space, for a continuous scale. `None` for a discrete one, which has no
    /// continuous inverse (use [`Scale::category_at`]).
    pub fn invert(&self, p: f64) -> Option<f64> {
        if self.kind.is_discrete() {
            return None;
        }
        let t = (p - self.range.0) / (self.range.1 - self.range.0);
        let Interval { lo, hi } = self.domain;
        Some(match &self.kind {
            ScaleKind::Linear | ScaleKind::Time => lo + t * (hi - lo),
            ScaleKind::Log { base } => {
                let l = lo.max(f64::MIN_POSITIVE).log(*base);
                let h = hi.log(*base);
                base.powf(l + t * (h - l))
            }
            ScaleKind::Power { exponent } => {
                let f = |x: f64| x.abs().powf(*exponent) * x.signum();
                let g = |x: f64| x.abs().powf(1.0 / *exponent) * x.signum();
                g(f(lo) + t * (f(hi) - f(lo)))
            }
            ScaleKind::Band | ScaleKind::Point => lo + t * (hi - lo),
        })
    }

    /// Which category covers plot position `p`, for a discrete scale.
    pub fn category_at(&self, p: f64) -> Option<&str> {
        if !self.kind.is_discrete() || self.categories.is_empty() {
            return None;
        }
        let step = self.step();
        if step == 0.0 {
            return None;
        }
        let start = self.range.0.min(self.range.1);
        let i = ((p - start - self.padding.outer * step) / step).floor();
        let i = i.clamp(0.0, self.categories.len() as f64 - 1.0) as usize;
        self.categories.get(i).map(|s| s.as_str())
    }

    /// The extent one category occupies, padding included.
    pub fn step(&self) -> f64 {
        let n = self.categories.len() as f64;
        if n == 0.0 {
            return 0.0;
        }
        let extent = (self.range.1 - self.range.0).abs();
        match self.kind {
            // n bands, (n-1) inner gaps, 2 outer gaps — all as fractions of one step.
            ScaleKind::Band => {
                extent / (n - self.padding.inner + 2.0 * self.padding.outer).max(1e-9)
            }
            // Points sit ON the dividers, so there are (n-1) gaps between them.
            _ => {
                if n > 1.0 {
                    extent / (n - 1.0 + 2.0 * self.padding.outer).max(1e-9)
                } else {
                    extent
                }
            }
        }
    }

    /// The drawable width of one band — the step less its inner padding. Zero for a point scale,
    /// which is the honest answer: a point has no width.
    pub fn band_width(&self) -> f64 {
        match self.kind {
            ScaleKind::Band => self.step() * (1.0 - self.padding.inner),
            _ => 0.0,
        }
    }

    fn band_start(&self, i: usize) -> f64 {
        let step = self.step();
        let start = self.range.0.min(self.range.1);
        start + self.padding.outer * step + i as f64 * step
    }

    fn point_at(&self, i: usize) -> f64 {
        let step = self.step();
        let start = self.range.0.min(self.range.1);
        if self.categories.len() <= 1 {
            return (self.range.0 + self.range.1) / 2.0;
        }
        start + self.padding.outer * step + i as f64 * step
    }

    /// Clamp a plot position into the range, for a mark whose value sits outside an explicit
    /// domain. Drawing it where the arithmetic lands would put it outside the plot rectangle.
    pub fn clamp_to_range(&self, p: f64) -> f64 {
        let (a, b) = (
            self.range.0.min(self.range.1),
            self.range.0.max(self.range.1),
        );
        p.clamp(a, b)
    }

    /// Whether a value falls inside the domain — what decides if a mark is drawn at all when the
    /// app pinned the domain itself.
    pub fn contains(&self, d: &Datum) -> bool {
        if self.kind.is_discrete() {
            let key = match d {
                Datum::Category(s) => s.clone(),
                other => other.to_string(),
            };
            return self.categories.contains(&key);
        }
        match d.as_continuous() {
            Some(v) => v >= self.domain.lo && v <= self.domain.hi,
            None => false,
        }
    }
}

/// What an app may pin about a scale, before the data is known. Everything unset is inferred.
#[derive(Clone, Debug, Default)]
pub struct ScaleSpec {
    pub kind: Option<ScaleKind>,
    pub domain: Option<Interval>,
    pub categories: Option<Vec<String>>,
    pub padding: Option<BandPadding>,
    /// Grow an inferred numeric domain to include zero. A bar chart that does not is lying about
    /// its proportions, which is why Swift Charts does this by default for bars.
    pub include_zero: bool,
    /// Reverse the output range — a y axis that grows downward, or a descending category order.
    pub reversed: bool,
}

/// Infer a domain from a column of data, honouring whatever the app pinned.
///
/// The inferred continuous domain is the data's own extent, NOT a rounded one: rounding belongs to
/// the tick search, which is free to place labels outside the data (that is its `coverage`
/// criterion). Rounding here as well would round twice and leave the plot padded oddly.
pub fn infer(
    spec: &ScaleSpec,
    data: &[Datum],
    range: (f64, f64),
    default_kind: ScaleKind,
) -> Scale {
    let discrete_data = data.iter().any(|d| matches!(d, Datum::Category(_)));
    let timey = data.iter().all(|d| matches!(d, Datum::Time(_))) && !data.is_empty();
    let kind = spec.kind.clone().unwrap_or(if discrete_data {
        default_kind_discrete(&default_kind)
    } else if timey {
        ScaleKind::Time
    } else {
        default_kind
    });

    let range = if spec.reversed {
        (range.1, range.0)
    } else {
        range
    };

    let mut scale = if kind.is_discrete() {
        let cats = spec.categories.clone().unwrap_or_else(|| {
            // First-seen order, not sorted: the app's row order is information, and sorting it
            // silently would reorder a chart whose categories are months.
            let mut seen: Vec<String> = Vec::new();
            for d in data {
                let key = match d {
                    Datum::Category(s) => s.clone(),
                    other => other.to_string(),
                };
                if !seen.contains(&key) {
                    seen.push(key);
                }
            }
            seen
        });
        Scale::discrete(kind, cats, range)
    } else {
        let mut iv = match spec.domain {
            Some(d) => d,
            None => {
                let mut it = data
                    .iter()
                    .filter_map(|d| d.as_continuous())
                    .filter(|v| v.is_finite());
                match it.next() {
                    Some(first) => {
                        let mut iv = Interval::new(first, first);
                        for v in it {
                            iv.extend(v);
                        }
                        iv
                    }
                    None => Interval::new(0.0, 1.0),
                }
            }
        };
        if spec.include_zero && spec.domain.is_none() {
            iv.extend(0.0);
        }
        if let ScaleKind::Log { .. } = kind {
            // A log domain that reaches zero has no bottom. Lift it to the decade below the
            // smallest positive value rather than to an arbitrary epsilon.
            if iv.lo <= 0.0 {
                let smallest = data
                    .iter()
                    .filter_map(|d| d.as_continuous())
                    .filter(|v| *v > 0.0)
                    .fold(f64::INFINITY, f64::min);
                iv.lo = if smallest.is_finite() {
                    10f64.powf(smallest.log10().floor())
                } else {
                    1.0
                };
                if iv.hi <= iv.lo {
                    iv.hi = iv.lo * 10.0;
                }
            }
        }
        let mut s = Scale::continuous(kind, iv, range);
        s.explicit_domain = spec.domain.is_some();
        s
    };
    if let Some(p) = spec.padding {
        scale.padding = p;
    }
    scale
}

/// A discrete column asked for a continuous default becomes a band: the default exists to pick
/// between Linear and Time, and neither can hold a category.
fn default_kind_discrete(k: &ScaleKind) -> ScaleKind {
    match k {
        ScaleKind::Point => ScaleKind::Point,
        _ => ScaleKind::Band,
    }
}
