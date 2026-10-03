// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Transitions between two charts (README "Animation").
//!
//! A chart's pipeline is pure (marks → [`Resolved`]), so an animated change is a question about
//! two resolved charts: what does the picture look like `t` of the way from one to the other?
//! [`Transition`] answers it, and it answers in the grammar's terms rather than in pixels:
//!
//! * **Identity.** Marks are matched by what they encode ([`identity`]): a bar by its series and
//!   category, a point on a time series by its series and instant, a scatter point by its place in
//!   its series, a wedge by its series, or by [`Mark::key`] where the app names one. The matching
//!   itself, marks and axis ticks alike, is `day::tween`'s [`Pairs::by_key`].
//! * **Values move in data space.** A matched mark's value, its numeric positions and its color
//!   interpolate, and the scales' domains interpolate beside them, so a bar grows along the axis
//!   that is itself rescaling, and a time series slides as its window widens.
//! * **Stacks are re-stacked.** A stacked segment's *value* is interpolated and the stack is summed
//!   again every frame, by the same `stack` the pipeline uses. Interpolating the segments' edges
//!   instead would open gaps and overlaps mid-transition: a pie gaining a slice would show wedges
//!   crossing one another.
//! * **Entering and leaving mean something.** A new bar or wedge grows from its baseline and a
//!   removed one shrinks into it; a line's new samples start on the old line and slide out along
//!   it, and removed ones collapse onto the new line, so a series never spikes through zero. What
//!   each kind of mark does is one row of `motion`.
//! * **Bands move in device space.** A category's band moves when the set of categories changes,
//!   and a band is not a number, so a mark on a discrete axis is carried between its old and new
//!   slots ([`Placed::x_band`]).
//! * **Axes cross-fade.** The outgoing ticks fade as the incoming ones appear, each drawn through
//!   the interpolated scale, so gridlines slide with the data they measure.
//!
//! A frame of a transition is an ordinary [`Resolved`], which the renderer draws without knowing
//! it is one: a fading mark is a mark with less opacity, and a sliding one a mark in another slot.

use std::collections::HashMap;

use day_core::tween::{Change, Lerp, Pairs, lerp};
use day_spec::Color;

use crate::coord::Coordinate;
use crate::data::{Datum, Value};
use crate::mark::{Mark, MarkKind};
use crate::render;
use crate::resolve::{Placed, Resolved, StackKey, stack, stack_key, stacks};
use crate::scale::Scale;
use crate::ticks::Tick;

// ---------------------------------------------------------------------------
// What each kind of mark does in a transition
// ---------------------------------------------------------------------------

/// How one kind of mark behaves in a transition.
///
/// Every decision this module makes by mark kind is read from here, so a new kind of mark is one
/// row in `motion` (the match is exhaustive: a kind without a row does not compile), and the
/// rest of the machinery (matching, carrying between bands, re-stacking, cross-fading) serves it
/// as it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Motion {
    /// What a mark is matched by across two charts when the app names no [`Mark::key`].
    pub matched_by: MatchBy,
    /// What a mark looks like in the chart it is not in: where an entering one starts, and where a
    /// leaving one goes.
    pub absent: Absent,
    /// Whether the kind draws a connected series (a line, an area) that other marks can ride. A
    /// removed sample of one collapses onto the new series rather than fading: it *is* the line.
    pub traces: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MatchBy {
    /// Its series alone: a pie's wedges are the parts of one whole.
    Series,
    /// What it encodes: its x, a categorical y (a heat map's row) and its dodge slot. A numeric y
    /// is a value, and a value is what changes, so a moving target line or a candle's range keeps
    /// its identity as it moves.
    Encoding,
    /// Its encoding when it marks a traced series (a dot on a line), else its place in its series:
    /// a scatter point's x is a measurement rather than a name, so a re-sampled scatter moves its
    /// points instead of replacing them.
    EncodingOrPlace,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Absent {
    /// On the other chart's picture of its own series at its x, so a line's new samples slide out
    /// along the old line. With no such series, `otherwise`.
    OnSeries { otherwise: Fallback },
    /// On the traced series it marks (a dot rides its line), faded out.
    OnTracedSeries,
    /// Grown from nothing: an explicit span collapses onto its start, a value onto its baseline,
    /// and a cell with neither (a heat-map cell on two categorical axes) fades.
    Grow,
    /// Swept from nothing: a wedge's value, which is its angle, collapses.
    Sweep,
    /// Faded out in place.
    Fade,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Fallback {
    Fade,
    /// Collapsed onto its baseline, the way an area with nothing to ride rises.
    Baseline,
}

/// The table of per-kind behavior.
pub(crate) fn motion(kind: MarkKind) -> Motion {
    use Absent::*;
    use MatchBy::*;
    let row = |matched_by, absent, traces| Motion {
        matched_by,
        absent,
        traces,
    };
    match kind {
        MarkKind::Line => row(
            Encoding,
            OnSeries {
                otherwise: Fallback::Fade,
            },
            true,
        ),
        MarkKind::Area => row(
            Encoding,
            OnSeries {
                otherwise: Fallback::Baseline,
            },
            true,
        ),
        MarkKind::Point => row(EncodingOrPlace, OnTracedSeries, false),
        MarkKind::Bar | MarkKind::Rectangle => row(Encoding, Grow, false),
        MarkKind::Sector => row(Series, Sweep, false),
        MarkKind::Rule => row(Encoding, Fade, false),
    }
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// What a transition matches a mark by across two charts. Two marks with the same identity in
/// one chart are told apart by order: the *n*th matches the *n*th ([`Pairs::by_key`]).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Identity {
    kind: u8,
    /// Index into the transition's merged series list, which both charts' marks are mapped onto
    /// before matching.
    series: usize,
    what: String,
}

fn value_key(v: &Option<Value>) -> String {
    match v.as_ref().map(|v| &v.datum) {
        Some(Datum::Category(s)) => format!("c{s}"),
        Some(Datum::Number(n)) | Some(Datum::Time(n)) => format!("n{:x}", n.to_bits()),
        None => String::new(),
    }
}

fn continuous(v: &Option<Value>) -> bool {
    matches!(
        v.as_ref().map(|v| &v.datum),
        Some(Datum::Number(_)) | Some(Datum::Time(_))
    )
}

/// A mark's identity: its [`Mark::key`] when the app named one, else what its kind is matched by
/// (`MatchBy`). `traced` holds the series that draw a connected path in either chart: a point
/// on one of those is a sample on it, not a scatter point.
pub fn identity(p: &Placed, traced: &[usize]) -> Identity {
    let m = &p.mark;
    let what = match (&m.key, motion(m.kind).matched_by) {
        (Some(k), _) => format!("k{k}"),
        (None, MatchBy::Series) => String::new(),
        (None, MatchBy::EncodingOrPlace)
            if continuous(&m.x) && !traced.contains(&p.series_index) =>
        {
            "#".to_string()
        }
        (None, _) => format!(
            "{}|{}|{}",
            value_key(&m.x),
            if continuous(&m.y) {
                String::new()
            } else {
                value_key(&m.y)
            },
            value_key(&m.dodge)
        ),
    };
    Identity {
        kind: m.kind as u8,
        series: p.series_index,
        what,
    }
}

// ---------------------------------------------------------------------------
// Interpolating the parts of a chart
// ---------------------------------------------------------------------------

fn lerp_datum(a: &Datum, b: &Datum, t: f64) -> Datum {
    match (a, b) {
        (Datum::Number(x), Datum::Number(y)) => Datum::Number(lerp(*x, *y, t)),
        (Datum::Time(x), Datum::Time(y)) => Datum::Time(lerp(*x, *y, t)),
        _ => b.clone(),
    }
}

fn lerp_value(a: &Option<Value>, b: &Option<Value>, t: f64) -> Option<Value> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Value {
            label: b.label.clone(),
            datum: lerp_datum(&a.datum, &b.datum, t),
        }),
        _ => b.clone(),
    }
}

fn lerp_color(a: Option<Color>, b: Option<Color>, t: f64) -> Option<Color> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.lerp(&b, t)),
        _ => b,
    }
}

/// `t` of the way between two optional positions, or the arriving one when either is missing.
fn lerp_opt<T: Lerp + Copy>(a: Option<T>, b: Option<T>, t: f64) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.lerp(&b, t)),
        (_, b) => b,
    }
}

/// A scale `t` of the way from `a` to `b`. Continuous domains interpolate, so the axis rescales
/// smoothly; a discrete scale keeps the target's categories (their bands are carried in device
/// space by the marks instead) and interpolates only its range.
fn blend_scale(a: &Scale, b: &Scale, t: f64) -> Scale {
    let mut s = b.clone();
    s.range = (lerp(a.range.0, b.range.0, t), lerp(a.range.1, b.range.1, t));
    if !a.kind.is_discrete() && !b.kind.is_discrete() && a.kind == b.kind {
        s.domain = crate::data::Interval::new(
            lerp(a.domain.lo, b.domain.lo, t),
            lerp(a.domain.hi, b.domain.hi, t),
        );
    }
    s
}

/// A tick `t` of the way to another: its opacity and, on a discrete axis, its position move; the
/// label and value are the arriving tick's.
impl Lerp for Tick {
    fn lerp(&self, to: &Self, t: f64) -> Self {
        Tick {
            value: to.value,
            label: to.label.clone(),
            alpha: lerp(self.alpha, to.alpha, t).clamp(0.0, 1.0),
            at: lerp_opt(self.at, to.at, t),
        }
    }
}

/// An axis's ticks across a transition. A tick on both axes (the same label, and on a continuous
/// axis the same value) keeps its opacity and slides; the others cross-fade. On a discrete axis a
/// new category's label slides out of the neighbour it grows from, and a leaving one closes into
/// the neighbour that stays ([`anchor`]).
fn tick_pairs(a: &[Tick], ra: &Scale, b: &[Tick], rb: &Scale) -> Pairs<Tick> {
    let discrete = ra.kind.is_discrete() || rb.kind.is_discrete();
    let both_discrete = ra.kind.is_discrete() && rb.kind.is_discrete();
    Pairs::by_key(
        a,
        b,
        |tick| {
            (
                tick.label.clone(),
                (!discrete).then_some(tick.value.to_bits()),
            )
        },
        |tick, change| {
            let (own, other) = match change {
                Change::Entering => (rb, ra),
                _ => (ra, rb),
            };
            let mut ghost = tick.clone();
            ghost.alpha = 0.0;
            if both_discrete {
                ghost.at = anchor(&tick.label, own, other).or(tick.at);
            }
            ghost
        },
    )
}

/// Where category `cat` of `own`'s discrete scale sits on `other`'s: its own band there when it has
/// one, otherwise the band of its nearest neighbour that both charts share (the one before it in
/// `own`'s order, else the one after). A month appended after June grows out of June; one removed
/// from the middle closes into the month before it. `None` when the charts share no category.
fn anchor(cat: &str, own: &Scale, other: &Scale) -> Option<f64> {
    let project = |c: &str| other.project(&Datum::Category(c.to_string()));
    if let Some(at) = project(cat) {
        return Some(at);
    }
    let i = own.categories.iter().position(|c| c == cat)?;
    own.categories[..i]
        .iter()
        .rev()
        .chain(own.categories[i + 1..].iter())
        .find_map(|c| project(c))
}

fn category(v: &Option<Value>) -> Option<&str> {
    match v.as_ref().map(|v| &v.datum) {
        Some(Datum::Category(c)) => Some(c),
        _ => None,
    }
}

/// A mark's slot along x in device points, on either kind of axis: resolved on a discrete one,
/// projected on a continuous one. What a mark moving between the two kinds is carried along.
fn x_slot(p: &Placed, r: &Resolved) -> Option<(f64, f64)> {
    p.x_band
        .or_else(|| render::center_x(p, r).map(|c| (c, render::band_of(p, r))))
}

/// A polar coordinate system `t` of the way between two: the hole, the start and the sweep move,
/// so a slider on a donut's hole or a gauge's sweep animates like the data does.
fn blend_coordinate(a: Coordinate, b: Coordinate, t: f64) -> Coordinate {
    match (a, b) {
        (
            Coordinate::Polar {
                start_angle: s0,
                sweep: w0,
                hole: h0,
            },
            Coordinate::Polar {
                start_angle: s1,
                sweep: w1,
                hole: h1,
            },
        ) => Coordinate::Polar {
            start_angle: lerp(s0, s1, t),
            sweep: lerp(w0, w1, t),
            hole: lerp(h0, h1, t).clamp(0.0, 0.95),
        },
        (_, b) => b,
    }
}

// ---------------------------------------------------------------------------
// Entering and leaving
// ---------------------------------------------------------------------------

/// One series' samples along x: position, then its low and high values there.
type Samples = Vec<(f64, f64, f64)>;

/// Every traced series of one chart, sorted along x, so a transition can read "where was this
/// series at x" for thousands of entering samples without rescanning the chart for each.
struct SeriesIndex {
    series: HashMap<(u8, usize), Samples>,
    names: Vec<String>,
    categories: HashMap<String, usize>,
}

impl SeriesIndex {
    fn new(r: &Resolved) -> Self {
        let categories: HashMap<String, usize> =
            r.x.categories
                .iter()
                .enumerate()
                .map(|(i, c)| (c.clone(), i))
                .collect();
        let mut series: HashMap<(u8, usize), Samples> = HashMap::new();
        for p in r.marks.iter().filter(|p| motion(p.mark.kind).traces) {
            let pos = match p.mark.x.as_ref().map(|v| &v.datum) {
                Some(Datum::Category(c)) => match categories.get(c) {
                    Some(i) => *i as f64,
                    None => continue,
                },
                Some(other) => match other.as_continuous() {
                    Some(v) => v,
                    None => continue,
                },
                None => continue,
            };
            series
                .entry((p.mark.kind as u8, p.series_index))
                .or_default()
                .push((pos, p.v0, p.v1));
        }
        for pts in series.values_mut() {
            pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }
        SeriesIndex {
            series,
            names: r.series.clone(),
            categories,
        }
    }

    /// The values the `kind` series named `series` takes at `x`: linearly between the two samples
    /// around it, or the nearest end outside them. A category this chart does not have reads the
    /// series' last sample, which is where a series grows from when it gains a category. `None`
    /// when this chart has no such series.
    fn at(&self, kind: MarkKind, series: &str, x: &Datum) -> Option<(f64, f64)> {
        let si = self.names.iter().position(|s| s == series).unwrap_or(0);
        let pts = self.series.get(&(kind as u8, si))?;
        let (first, last) = (*pts.first()?, *pts.last()?);
        let x = match x {
            Datum::Category(c) => match self.categories.get(c) {
                Some(i) => *i as f64,
                None => last.0,
            },
            other => other.as_continuous()?,
        };
        if x <= first.0 {
            return Some((first.1, first.2));
        }
        if x >= last.0 {
            return Some((last.1, last.2));
        }
        let i = pts.partition_point(|p| p.0 < x);
        let (a, b) = (pts[i - 1], pts[i]);
        let u = if b.0 > a.0 {
            (x - a.0) / (b.0 - a.0)
        } else {
            0.0
        };
        Some((lerp(a.1, b.1, u), lerp(a.2, b.2, u)))
    }

    /// The values any traced series named `series` takes at `x`, for a mark riding it.
    fn any_at(&self, series: &str, x: &Datum) -> Option<(f64, f64)> {
        [MarkKind::Line, MarkKind::Area]
            .into_iter()
            .find_map(|k| self.at(k, series, x))
    }
}

/// Where a mark that is not in `other` comes from (entering) or goes to (leaving), expressed as
/// the placed mark it is imagined to be in `other`'s chart: its kind's [`Absent`] applied to its
/// values, then its slot moved to where it would sit on `other`'s discrete axes.
fn phantom(p: &Placed, own: &Resolved, other: &Resolved, index: &SeriesIndex) -> Placed {
    let mut q = p.clone();
    let series = own.series.get(p.series_index).cloned().unwrap_or_default();
    let x = p.mark.x.as_ref().map(|v| &v.datum);
    let fade = |q: &mut Placed| q.mark.style.opacity = 0.0;
    match motion(p.mark.kind).absent {
        Absent::OnSeries { otherwise } => match x.and_then(|x| index.at(p.mark.kind, &series, x)) {
            Some((v0, v1)) => (q.v0, q.v1) = (v0, v1),
            None => match otherwise {
                Fallback::Fade => fade(&mut q),
                Fallback::Baseline => q.v1 = q.v0,
            },
        },
        Absent::OnTracedSeries => {
            if let Some((_, v1)) = x.and_then(|x| index.any_at(&series, x)) {
                q.v1 = v1;
            }
            fade(&mut q);
        }
        Absent::Grow => {
            let spanned = p.mark.x_end.is_some() || p.mark.y_end.is_some();
            if p.mark.x_end.is_some() {
                q.mark.x_end = q.mark.x.clone();
            }
            if p.mark.y_end.is_some() {
                q.mark.y_end = q.mark.y.clone();
            }
            if continuous(&p.mark.y) {
                q.v1 = q.v0;
            } else if !spanned {
                fade(&mut q);
            }
        }
        Absent::Sweep => {
            if p.mark.y_end.is_some() {
                q.mark.y_end = q.mark.y.clone();
            }
            q.v1 = q.v0;
        }
        Absent::Fade => fade(&mut q),
    }
    // Along a discrete axis, where it would sit in the other chart: its own slot there if the
    // category exists in both, else beside the nearest category both share ([`anchor`]), keeping
    // its offset within the band (a dodged bar's slot) and its own width.
    let carry = |mine: (f64, f64), own_s: &Scale, other_s: &Scale, cat: Option<&str>, there| {
        let cat = cat.filter(|_| other_s.kind.is_discrete())?;
        let datum = Datum::Category(cat.to_string());
        if other_s.project(&datum).is_some() {
            return there;
        }
        let offset = mine.0 - own_s.project(&datum).unwrap_or(mine.0);
        anchor(cat, own_s, other_s).map(|a| (a + offset, mine.1))
    };
    if let Some(mine) = p.x_band {
        let there = resolve_slot(p, other, true);
        q.x_band = carry(mine, &own.x, &other.x, category(&p.mark.x), there).or(Some(mine));
    }
    if let Some(mine) = p.y_band {
        let there = resolve_slot(p, other, false);
        q.y_band = carry(mine, &own.y, &other.y, category(&p.mark.y), there).or(Some(mine));
    }
    q
}

/// The slot `p` would be given on `r`'s discrete x (or y) axis, as the pipeline would resolve it.
fn resolve_slot(p: &Placed, r: &Resolved, along_x: bool) -> Option<(f64, f64)> {
    if along_x {
        crate::resolve::x_slot(p, &r.x)
    } else {
        crate::resolve::y_slot(p, &r.y)
    }
}

/// One mark `t` of the way from `a` (in chart `ra`) to `b` (in chart `rb`). `b` supplies
/// everything that does not interpolate.
fn blend_mark(a: &Placed, ra: &Resolved, b: &Placed, rb: &Resolved, t: f64) -> Placed {
    let mut m: Mark = b.mark.clone();
    m.x = lerp_value(&a.mark.x, &b.mark.x, t);
    m.x_end = lerp_value(&a.mark.x_end, &b.mark.x_end, t);
    m.y = lerp_value(&a.mark.y, &b.mark.y, t);
    m.y_end = lerp_value(&a.mark.y_end, &b.mark.y_end, t);
    let (sa, sb) = (&a.mark.style, &b.mark.style);
    m.style.fill = lerp_color(sa.fill, sb.fill, t);
    m.style.gradient = match (sa.gradient, sb.gradient) {
        (Some((p, q)), Some((r, s))) => Some((p.lerp(&r, t), q.lerp(&s, t))),
        (_, g) => g,
    };
    // Clamped: a spring's overshoot must not push a fading mark past fully opaque or below clear.
    m.style.opacity = lerp(sa.opacity, sb.opacity, t).clamp(0.0, 1.0);
    m.style.symbol_size = lerp(sa.symbol_size, sb.symbol_size, t);
    m.style.line_width = lerp(sa.line_width, sb.line_width, t);
    m.style.corner_radius = lerp(sa.corner_radius, sb.corner_radius, t);
    // A slot is carried whenever either end has one; a mark moving between a discrete and a
    // continuous axis reads its continuous end's projected position.
    let x_band = (a.x_band.is_some() || b.x_band.is_some())
        .then(|| lerp_opt(x_slot(a, ra), x_slot(b, rb), t))
        .flatten();
    Placed {
        mark: m,
        v0: lerp(a.v0, b.v0, t),
        v1: lerp(a.v1, b.v1, t),
        series_index: b.series_index,
        dodge_index: b.dodge_index,
        dodge_count: b.dodge_count,
        stack_lo: b.stack_lo,
        stack_hi: b.stack_hi,
        x_band,
        y_band: lerp_opt(a.y_band, b.y_band, t),
    }
}

/// Sum every stacked mark again from its interpolated value, with the pipeline's own `stack`.
fn restack(marks: &mut [Placed], values: &[Option<f64>]) {
    let stacked: Vec<usize> = (0..marks.len()).filter(|&i| values[i].is_some()).collect();
    let entries: Vec<(StackKey, f64, _)> = stacked
        .iter()
        .map(|&i| {
            let m = &marks[i].mark;
            (stack_key(m), values[i].unwrap_or(0.0), m.stacking)
        })
        .collect();
    for (&i, (v0, v1)) in stacked.iter().zip(stack(&entries)) {
        marks[i].v0 = v0;
        marks[i].v1 = v1;
    }
}

// ---------------------------------------------------------------------------
// The transition
// ---------------------------------------------------------------------------

/// A transition between two resolved charts, matched once and sampled every frame.
///
/// Matching marks and ticks, finding where entering ones come from and leaving ones go, and
/// reading a series along x for each of them is the expensive part, and none of it depends on
/// `t`, so it is done here, once per change. [`Transition::at`] only interpolates.
pub struct Transition {
    from: Resolved,
    to: Resolved,
    marks: Pairs<Placed>,
    /// Each mark's stacked value at both ends, for the marks whose extent comes from stacking.
    stacked: Vec<Option<(f64, f64)>>,
    x_ticks: Pairs<Tick>,
    y_ticks: Pairs<Tick>,
    series: Vec<String>,
}

impl Transition {
    pub fn new(mut from: Resolved, to: Resolved) -> Self {
        // One series list for both charts: the arriving chart's, then any series only the
        // leaving one has, which keeps its name (and so its color) while its marks depart.
        // Mapping the leaving chart's marks onto it makes a series index mean the same thing on
        // both sides, which is what lets a mark's identity be read from the mark alone.
        let mut series = to.series.clone();
        for p in &mut from.marks {
            let name = from.series.get(p.series_index).cloned().unwrap_or_default();
            p.series_index = series.iter().position(|s| *s == name).unwrap_or_else(|| {
                series.push(name);
                series.len() - 1
            });
        }
        from.series = series.clone();
        let traced: Vec<usize> = from
            .marks
            .iter()
            .chain(&to.marks)
            .filter(|p| motion(p.mark.kind).traces)
            .map(|p| p.series_index)
            .collect();
        let (along_from, along_to) = (SeriesIndex::new(&from), SeriesIndex::new(&to));
        let marks = Pairs::by_key(
            &from.marks,
            &to.marks,
            |p| identity(p, &traced),
            |p, change| match change {
                Change::Entering => phantom(p, &to, &from, &along_from),
                _ => {
                    let mut gone = phantom(p, &from, &to, &along_to);
                    // Whatever else it does on the way out, a leaving mark is gone at the end
                    // (its slot may already belong to another category), unless it is a sample
                    // of a traced series, which has collapsed onto the new one.
                    if !motion(p.mark.kind).traces {
                        gone.mark.style.opacity = 0.0;
                    }
                    gone
                }
            },
        );
        let stacked = marks
            .iter()
            .map(|p| stacks(&p.to.mark).then_some((p.from.v1 - p.from.v0, p.to.v1 - p.to.v0)))
            .collect();
        let x_ticks = tick_pairs(&from.x_ticks, &from.x, &to.x_ticks, &to.x);
        let y_ticks = tick_pairs(&from.y_ticks, &from.y, &to.y_ticks, &to.y);
        Transition {
            from,
            to,
            marks,
            stacked,
            x_ticks,
            y_ticks,
            series,
        }
    }

    /// The chart `t` of the way along. `t = 0` is the starting picture and `t = 1` the arriving
    /// one; a spring's overshoot carries values past it and back.
    pub fn at(&self, t: f64) -> Resolved {
        let (from, to) = (&self.from, &self.to);
        let mut out = to.clone();
        out.clip_plot = !to.coordinate.is_polar();
        out.coordinate = blend_coordinate(from.coordinate, to.coordinate, t);
        out.plot = from.plot.lerp(&to.plot, t);
        out.x = blend_scale(&from.x, &to.x, t);
        out.y = blend_scale(&from.y, &to.y, t);
        out.x_ticks = self.x_ticks.at(t);
        out.y_ticks = self.y_ticks.at(t);
        let mut marks = self
            .marks
            .at_with(t, |p, t| blend_mark(&p.from, from, &p.to, to, t));
        let values: Vec<Option<f64>> = self
            .stacked
            .iter()
            .map(|s| s.map(|(va, vb)| lerp(va, vb, t)))
            .collect();
        restack(&mut marks, &values);
        out.series = self.series.clone();
        out.marks = marks;
        out
    }
}

/// The chart `t` of the way from `from` to `to`: [`Transition::new`] then [`Transition::at`], for
/// one sample. A driver sampling many frames keeps the `Transition` instead.
pub fn blend(from: &Resolved, to: &Resolved, t: f64) -> Resolved {
    Transition::new(from.clone(), to.clone()).at(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axis::AxisSpec;
    use crate::coord::Coordinate;
    use crate::data::value;
    use crate::mark::{Stacking, area, bar, sector};
    use crate::resolve::{Config, resolve};
    use crate::scale::ScaleSpec;
    use day_spec::Size;

    fn chart(marks: Vec<Mark>, coordinate: Coordinate) -> Resolved {
        let (xs, ys, xa, ya) = (
            ScaleSpec::default(),
            ScaleSpec::default(),
            AxisSpec::default(),
            AxisSpec::default(),
        );
        let colors = |i: usize, _: &str| crate::style::categorical(i);
        let cfg = Config {
            x_scale: &xs,
            y_scale: &ys,
            x_axis: &xa,
            y_axis: &ya,
            coordinate,
            series_colors: &colors,
            label_size: 11.0,
            font: Default::default(),
            legend_insets: Default::default(),
            plot_insets: Some(Default::default()),
        };
        resolve(marks, Size::new(400.0, 300.0), &cfg)
    }

    fn bars(values: &[(&str, f64)]) -> Resolved {
        chart(
            values
                .iter()
                .map(|(c, v)| bar(value("Month", *c), value("Revenue", *v)))
                .collect(),
            Coordinate::Cartesian,
        )
    }

    #[test]
    fn a_matched_bar_grows_and_a_new_one_rises_from_its_baseline() {
        let a = bars(&[("Jan", 10.0), ("Feb", 20.0)]);
        let b = bars(&[("Jan", 30.0), ("Feb", 20.0), ("Mar", 40.0)]);
        let mid = blend(&a, &b, 0.5);
        let jan = &mid.marks[0];
        assert_eq!((jan.v0, jan.v1), (0.0, 20.0));
        let mar = &mid.marks[2];
        assert_eq!((mar.v0, mar.v1), (0.0, 20.0), "Mar rises from zero");
        // The ends are the endpoints.
        let end = blend(&a, &b, 1.0);
        assert_eq!(end.marks.len(), 3);
        assert_eq!(end.marks[2].v1, 40.0);
        let start = blend(&a, &b, 0.0);
        assert_eq!(start.marks[2].v1, 0.0);
    }

    #[test]
    fn an_appended_category_grows_out_of_the_last_shared_one() {
        let a = bars(&[("Jan", 10.0), ("Feb", 20.0)]);
        let b = bars(&[("Jan", 10.0), ("Feb", 20.0), ("Mar", 30.0), ("Apr", 40.0)]);
        let start = blend(&a, &b, 0.0);
        let feb_then = a.marks[1].x_band.unwrap().0;
        for m in &start.marks[2..] {
            let (x, _) = m.x_band.expect("resolved on a discrete axis");
            assert!(
                (x - feb_then).abs() < 1e-9,
                "starts on Feb's old band: {x} vs {feb_then}"
            );
        }
        let end = blend(&a, &b, 1.0);
        assert_eq!(end.marks[3].x_band, b.marks[3].x_band);
    }

    #[test]
    fn a_removed_bar_shrinks_away_and_is_gone_at_the_end() {
        let a = bars(&[("Jan", 10.0), ("Feb", 20.0)]);
        let b = bars(&[("Jan", 10.0)]);
        let mid = blend(&a, &b, 0.5);
        assert_eq!(mid.marks.len(), 2, "the leaving bar is still drawn");
        assert_eq!(mid.marks[1].v1, 10.0);
        assert!(mid.marks[1].mark.style.opacity < 1.0);
        assert_eq!(blend(&a, &b, 1.0).marks[1].mark.style.opacity, 0.0);
    }

    #[test]
    fn a_pie_gaining_a_slice_never_overlaps_its_wedges() {
        let pie = |v: &[(&str, f64)]| {
            chart(
                v.iter()
                    .map(|(s, x)| sector(value("Share", *x)).by_series(value("Source", *s)))
                    .collect(),
                Coordinate::polar(),
            )
        };
        let a = pie(&[("Search", 50.0), ("Direct", 50.0)]);
        let b = pie(&[("Search", 40.0), ("Direct", 30.0), ("Email", 30.0)]);
        for step in 0..=10 {
            let r = blend(&a, &b, step as f64 / 10.0);
            let mut spans: Vec<(f64, f64)> = r.marks.iter().map(|p| (p.v0, p.v1)).collect();
            spans.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
            for w in spans.windows(2) {
                assert!(
                    (w[0].1 - w[1].0).abs() < 1e-9,
                    "wedges meet edge to edge at step {step}: {spans:?}"
                );
            }
        }
    }

    #[test]
    fn colors_interpolate_and_domains_rescale_between_the_endpoints() {
        let cell = |v: f64, c: Color| {
            chart(
                vec![
                    crate::mark::rect(value("Hour", "09"), value("Day", "Mon"))
                        .foreground(c)
                        .annotation(crate::mark::AnnotationPosition::Overlay, format!("{v}")),
                ],
                Coordinate::Cartesian,
            )
        };
        let a = cell(1.0, Color::rgb(0.0, 0.0, 1.0));
        let b = cell(2.0, Color::rgb(1.0, 1.0, 0.0));
        let mid = blend(&a, &b, 0.5).marks[0].mark.style.fill.unwrap();
        assert!(mid != Color::rgb(0.0, 0.0, 1.0) && mid != Color::rgb(1.0, 1.0, 0.0));
        let low = bars(&[("Jan", 10.0)]);
        let high = bars(&[("Jan", 100.0)]);
        let d = blend(&low, &high, 0.5).y.domain;
        assert!(d.hi > low.y.domain.hi && d.hi < high.y.domain.hi);
    }

    /// Every stacking mode, re-summed by the frames, lands exactly where the pipeline put it.
    #[test]
    fn a_restacked_frame_ends_where_the_pipeline_placed_it() {
        for stacking in [Stacking::Standard, Stacking::Normalized, Stacking::Center] {
            let areas = |values: &[(&str, &str, f64)]| {
                chart(
                    values
                        .iter()
                        .map(|(q, s, v)| {
                            area(value("Quarter", *q), value("Revenue", *v))
                                .by_series(value("Channel", *s))
                                .stacking(stacking)
                        })
                        .collect(),
                    Coordinate::Cartesian,
                )
            };
            let a = areas(&[("Q1", "A", 10.0), ("Q1", "B", 5.0), ("Q2", "A", 8.0)]);
            let b = areas(&[
                ("Q1", "A", 4.0),
                ("Q1", "B", 12.0),
                ("Q2", "A", 9.0),
                ("Q2", "B", 3.0),
            ]);
            let end = blend(&a, &b, 1.0);
            for (got, want) in end.marks.iter().zip(&b.marks) {
                assert!(
                    (got.v0 - want.v0).abs() < 1e-9 && (got.v1 - want.v1).abs() < 1e-9,
                    "{stacking:?}: {:?} vs {:?}",
                    (got.v0, got.v1),
                    (want.v0, want.v1)
                );
            }
        }
    }

    /// A chart re-recorded from the same data is equal to itself, which is what keeps a frame of
    /// its own transition from restarting it. A non-finite value is dropped rather than making the
    /// chart unequal to itself forever.
    #[test]
    fn the_same_data_resolves_to_an_equal_chart() {
        let make = || bars(&[("Jan", 10.0), ("Feb", 20.0)]);
        assert_eq!(make(), make());
        let with_nan = || {
            chart(
                vec![
                    bar(value("Month", "Jan"), value("Revenue", 10.0)),
                    bar(value("Month", "Feb"), value("Revenue", f64::NAN)),
                ],
                Coordinate::Cartesian,
            )
        };
        assert_eq!(with_nan().marks.len(), 1);
        assert_eq!(with_nan(), with_nan());
    }

    #[test]
    fn a_new_category_label_slides_out_of_its_neighbour_and_fades_in() {
        let a = bars(&[("Jan", 10.0), ("Feb", 20.0)]);
        let b = bars(&[("Jan", 10.0), ("Feb", 20.0), ("Mar", 30.0)]);
        let tick = |r: &Resolved, label: &str| {
            r.x_ticks
                .iter()
                .find(|t| t.label == label)
                .cloned()
                .expect("tick")
        };
        let feb_then = tick(&a, "Feb").at.expect("placed on a discrete axis");
        let start = blend(&a, &b, 0.0);
        let mar = tick(&start, "Mar");
        assert_eq!((mar.alpha, mar.at), (0.0, Some(feb_then)));
        // A shared label keeps its opacity throughout and slides to its new place.
        let mid = blend(&a, &b, 0.5);
        assert_eq!(tick(&mid, "Jan").alpha, 1.0);
        assert!(tick(&mid, "Mar").alpha > 0.0 && tick(&mid, "Mar").alpha < 1.0);
        let end = blend(&a, &b, 1.0);
        assert_eq!(tick(&end, "Mar"), tick(&b, "Mar"));
        // And removing it closes it back into February, fading.
        let back = blend(&b, &a, 1.0);
        let gone = tick(&back, "Mar");
        assert_eq!((gone.alpha, gone.at), (0.0, tick(&a, "Feb").at));
    }

    #[test]
    fn a_departing_series_keeps_its_name_while_its_marks_leave() {
        let grouped = |regions: &[&str]| {
            chart(
                regions
                    .iter()
                    .map(|r| {
                        bar(value("Month", "Jan"), value("Revenue", 10.0))
                            .by_series(value("Region", *r))
                            .stacking(Stacking::Unstacked)
                    })
                    .collect(),
                Coordinate::Cartesian,
            )
        };
        let a = grouped(&["North", "South"]);
        let b = grouped(&["North"]);
        let mid = blend(&a, &b, 0.5);
        assert_eq!(mid.series, vec!["North".to_string(), "South".to_string()]);
        let leaving = &mid.marks[1];
        assert_eq!(mid.series[leaving.series_index], "South");
        assert!(leaving.mark.style.opacity < 1.0);
        // The kept one is matched across the charts, not re-entered.
        assert_eq!(mid.marks[0].mark.style.opacity, 1.0);
    }
}
