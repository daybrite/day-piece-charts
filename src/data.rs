// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! What a mark can be positioned by: the plottable value, and the domain a scale infers from a
//! column of them.
//!
//! Swift Charts spells this `.value("Label", x)` and lets the type of `x` pick the scale. The same
//! idea here is one enum with three inhabitants, because those are the three that behave
//! differently under a scale: a number interpolates, a date interpolates but labels itself by
//! calendar, and a category does neither — it is a set with an order, which is what a BAND scale
//! needs (docs/charts.md "Scales").

use std::fmt;

/// One plottable quantity. `Number` and `Time` are continuous; `Category` is discrete.
///
/// `Time` is seconds since the Unix epoch as `f64`, not a calendar type: this crate takes no date
/// dependency, and every calendar decision it makes (which tick spacing, which label) is made in
/// [`crate::ticks`] from that one number. An app that has a real date type converts on the way in.
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Number(f64),
    Time(f64),
    Category(String),
}

impl Datum {
    /// The continuous position, for a scale that interpolates. `None` for a category, which has
    /// no position of its own — its scale assigns one from its index in the domain.
    pub fn as_continuous(&self) -> Option<f64> {
        match self {
            Datum::Number(v) | Datum::Time(v) => Some(*v),
            Datum::Category(_) => None,
        }
    }

    /// The discrete identity, for a band or point scale and for grouping.
    pub fn as_category(&self) -> Option<&str> {
        match self {
            Datum::Category(s) => Some(s),
            _ => None,
        }
    }

    /// Whether this datum is drawable. A non-finite number is dropped rather than drawn: it has no
    /// position, and letting a NaN reach the canvas is how a whole plot silently disappears.
    pub fn is_finite(&self) -> bool {
        match self {
            Datum::Number(v) | Datum::Time(v) => v.is_finite(),
            Datum::Category(_) => true,
        }
    }
}

impl fmt::Display for Datum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Datum::Number(v) => write!(f, "{v}"),
            Datum::Time(v) => write!(f, "{v}"),
            Datum::Category(s) => f.write_str(s),
        }
    }
}

/// Anything that can be a plottable value. Numbers become [`Datum::Number`], strings become
/// [`Datum::Category`]; a date arrives through [`time`].
pub trait IntoDatum {
    fn into_datum(self) -> Datum;
}

macro_rules! datum_from_number {
    ($($t:ty),*) => {$(
        impl IntoDatum for $t {
            fn into_datum(self) -> Datum { Datum::Number(self as f64) }
        }
    )*};
}
datum_from_number!(f64, f32, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl IntoDatum for &str {
    fn into_datum(self) -> Datum {
        Datum::Category(self.to_owned())
    }
}
impl IntoDatum for String {
    fn into_datum(self) -> Datum {
        Datum::Category(self)
    }
}
impl IntoDatum for Datum {
    fn into_datum(self) -> Datum {
        self
    }
}

/// A plottable value and the name of the field it came from — Swift Charts' `.value(_:_:)`.
///
/// The label is not decoration: it is what an axis titles itself with and what a legend groups by
/// when the app names no title of its own, which is why it travels with the value rather than
/// being configured separately.
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub label: String,
    pub datum: Datum,
}

/// `value("Revenue", 42.0)` — a labelled plottable value.
pub fn value(label: impl Into<String>, datum: impl IntoDatum) -> Value {
    Value {
        label: label.into(),
        datum: datum.into_datum(),
    }
}

/// `time("Date", secs)` — a labelled instant, seconds since the Unix epoch.
///
/// Separate from [`value`] because a bare `f64` cannot say whether it means a quantity or an
/// instant, and the two label themselves completely differently.
pub fn time(label: impl Into<String>, epoch_seconds: f64) -> Value {
    Value {
        label: label.into(),
        datum: Datum::Time(epoch_seconds),
    }
}

/// `date("Date", "2026-07-01")` — a labelled calendar day from an ISO `YYYY-MM-DD` string, as the
/// instant that day begins.
///
/// The everyday source of a time axis is a column of date strings — an exchange's daily bars, a
/// CSV export — and every app holding one would otherwise write the same calendar arithmetic on
/// the way in. A string that does not parse becomes a non-finite instant, which the pipeline drops
/// the way it drops a NaN (see [`Datum::is_finite`]) rather than plotting it at the epoch.
pub fn date(label: impl Into<String>, iso: &str) -> Value {
    Value {
        label: label.into(),
        datum: Datum::Time(parse_iso_date(iso).unwrap_or(f64::NAN)),
    }
}

/// Seconds since the epoch at the start of an ISO `YYYY-MM-DD` day. A time of day after a `T` is
/// ignored; anything that is not a date is `None`.
pub fn parse_iso_date(s: &str) -> Option<f64> {
    let day = s.trim().split('T').next()?;
    let mut parts = day.splitn(3, '-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(crate::ticks::civil::to_days(y, m, d) as f64 * 86_400.0)
}

/// The extent a continuous scale covers: a closed interval, always ordered `lo <= hi`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Self {
        if hi < lo {
            Interval { lo: hi, hi: lo }
        } else {
            Interval { lo, hi }
        }
    }

    pub fn span(&self) -> f64 {
        self.hi - self.lo
    }

    /// Grow to contain `v`.
    pub fn extend(&mut self, v: f64) {
        if v < self.lo {
            self.lo = v;
        }
        if v > self.hi {
            self.hi = v;
        }
    }

    /// Grow to contain `other`.
    pub fn union(self, other: Interval) -> Interval {
        Interval {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    /// A domain with no width cannot be projected — every value would land on the same pixel and
    /// the scale would divide by zero. Widen it symmetrically instead, by a unit related to the
    /// value itself so the padding is sensible at any magnitude.
    pub fn nondegenerate(self) -> Interval {
        if self.span() > 0.0 {
            return self;
        }
        let pad = if self.lo == 0.0 {
            0.5
        } else {
            self.lo.abs() * 0.05
        };
        Interval::new(self.lo - pad, self.hi + pad)
    }
}
