// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Where the tick marks go, and what they say.
//!
//! Choosing axis labels is a real optimization problem, not a loop that divides the range by five,
//! and this module solves it the way the literature does. For a linear axis it runs the **extended
//! Wilkinson algorithm** (Talbot, Lin & Hanrahan, *An Extension of Wilkinson's Algorithm for
//! Positioning Tick Labels on Axes*, InfoVis 2010): search candidate label sequences and score each
//! on four competing criteria, rather than picking the first that fits.
//!
//! - **simplicity** — humans read steps of 1, 5, 2, 2.5, 4, 3 (in that order of preference), and an
//!   axis that includes zero reads better than one that does not.
//! - **coverage** — labels should span the data closely; an axis running to 100 for data that stops
//!   at 61 wastes half the plot.
//! - **density** — the number of labels should land near the number asked for, penalized
//!   symmetrically so twice as many is as bad as half as many.
//! - **legibility** — the labels have to actually fit. The paper leaves this to the implementation;
//!   here it is computed from MEASURED text (`day::measure_text`) against the axis's pixel length,
//!   which is what makes the choice follow the window as it resizes rather than being fixed at
//!   authoring time.
//!
//! The search is the paper's: iterate skip amount `j`, step `q`, and label count `k`, pruning whole
//! branches with the per-criterion upper bounds so the loop terminates quickly instead of
//! enumerating a large space.
//!
//! Log and time axes are not this problem. A log axis wants decade boundaries and their 2/5
//! subdivisions; a time axis wants calendar steps (a month is not 30 days), so it walks civil dates
//! through the proleptic Gregorian conversion in [`civil`] rather than dividing seconds.

use crate::data::Interval;

/// One tick: where it sits in DATA space, and the text it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// The step multipliers the extended Wilkinson algorithm prefers, most-preferred first. Order is
/// the scoring input, not a formality: `simplicity` penalizes by index, so 1 and 5 win ties against
/// 3 even when both fit.
const Q: [f64; 6] = [1.0, 5.0, 2.0, 2.5, 4.0, 3.0];

/// The paper's criterion weights: simplicity, coverage, density, legibility.
const W: [f64; 4] = [0.25, 0.2, 0.5, 0.05];

/// How the labels are rendered, so legibility can be judged against real text extents.
#[derive(Clone, Copy)]
pub struct LabelFit<'a> {
    /// The axis's length in points along the direction labels are laid out.
    pub axis_length: f64,
    /// Points of clear space demanded between two neighbouring labels.
    pub gap: f64,
    /// Measures one label's extent along that same direction.
    pub measure: &'a dyn Fn(&str) -> f64,
}

impl LabelFit<'_> {
    /// The fraction of the paper's legibility criterion this label set earns: 1 when every label
    /// clears its neighbour by `gap`, falling to 0 as they collide.
    ///
    /// The paper scores format, orientation and overlap together and leaves the weighting open;
    /// overlap is the one that decides whether an axis is readable at a given size, and it is the
    /// one a resize changes, so it is the one computed here.
    fn legibility(&self, labels: &[String]) -> f64 {
        if labels.len() < 2 || self.axis_length <= 0.0 {
            return 1.0;
        }
        let needed: f64 = labels.iter().map(|l| (self.measure)(l)).sum::<f64>()
            + self.gap * (labels.len() - 1) as f64;
        if needed <= self.axis_length {
            return 1.0;
        }
        // Past the point where they fit, fall off with the overshoot rather than dropping to zero:
        // a set that is 10% too wide should still beat one that is twice too wide, so the search
        // degrades gracefully on an axis too short for any labelling.
        (self.axis_length / needed).clamp(0.0, 1.0)
    }
}

fn simplicity(q_index: usize, j: usize, lmin: f64, lmax: f64, lstep: f64) -> f64 {
    let eps = 1e-10;
    let n = Q.len() as f64;
    let i = q_index as f64 + 1.0;
    let rem = lmin.rem_euclid(lstep);
    let includes_zero = (rem < eps || (lstep - rem) < eps) && lmin <= 0.0 && lmax >= 0.0;
    let v = if includes_zero { 1.0 } else { 0.0 };
    1.0 - (i - 1.0) / (n - 1.0) - j as f64 + v
}

fn simplicity_max(q_index: usize, j: usize) -> f64 {
    let n = Q.len() as f64;
    let i = q_index as f64 + 1.0;
    1.0 - (i - 1.0) / (n - 1.0) - j as f64 + 1.0
}

fn coverage(dmin: f64, dmax: f64, l: f64, r: f64) -> f64 {
    let range = dmax - dmin;
    1.0 - 0.5 * ((dmax - r).powi(2) + (dmin - l).powi(2)) / (0.1 * range).powi(2)
}

fn coverage_max(dmin: f64, dmax: f64, span: f64) -> f64 {
    let range = dmax - dmin;
    if span > range {
        let half = (span - range) / 2.0;
        1.0 - 0.5 * (half * half + half * half) / (0.1 * range).powi(2)
    } else {
        1.0
    }
}

fn density(k: usize, m: usize, dmin: f64, dmax: f64, lmin: f64, lmax: f64) -> f64 {
    let r = (k as f64 - 1.0) / (lmax - lmin);
    let rt = (m as f64 - 1.0) / (lmax.max(dmax) - lmin.min(dmin));
    2.0 - (r / rt).max(rt / r)
}

fn density_max(k: usize, m: usize) -> f64 {
    if k >= m {
        2.0 - (k as f64 - 1.0) / (m as f64 - 1.0)
    } else {
        1.0
    }
}

/// The chosen labelling: the sequence, and the step it advances by (which is what decides how many
/// decimal places the labels need).
#[derive(Clone, Debug, PartialEq)]
pub struct Labelling {
    pub values: Vec<f64>,
    pub step: f64,
}

/// Extended Wilkinson: the best labelling of `domain` at roughly `target` labels.
///
/// `fit` makes the answer depend on the axis's real length and the real width of the text, so the
/// same domain labels itself differently in a narrow pane than in a wide one — which is the whole
/// point of computing it per draw instead of once.
pub fn extended(domain: Interval, target: usize, fit: &LabelFit<'_>) -> Labelling {
    let dmin = domain.lo;
    let dmax = domain.hi;
    // Cap the target by what the axis can physically hold. The paper's `density` term (weight 0.5)
    // pulls hard toward the requested count while `legibility` (weight 0.05) can only nudge, so
    // asking for eight labels on an axis with room for three would score eight overlapping ones
    // best. Feasibility belongs upstream of the search: ask for a count the axis can show, then let
    // the four criteria choose among the labellings of that size.
    let sample = format_value(
        dmax.abs().max(dmin.abs()),
        (dmax - dmin) / target.max(2) as f64,
    );
    let sample_w = (fit.measure)(&sample).max(1.0);
    let cap = ((fit.axis_length + fit.gap) / (sample_w + fit.gap)).floor();
    let m = target.min(cap.max(2.0) as usize).max(2);

    // A degenerate or non-finite domain has no labelling to search for; answer with the single
    // value rather than looping over an empty interval.
    if !dmin.is_finite() || !dmax.is_finite() || dmax <= dmin {
        return Labelling {
            values: vec![dmin],
            step: 0.0,
        };
    }

    let mut best: Option<(f64, Labelling)> = None;
    let mut best_score = -2.0;

    // `j` is the paper's skip amount: j = 1 uses every multiple of q, j = 2 every second one, and
    // so on, which is how sequences like 0, 20, 40 are reachable from q = 1.
    let mut j = 1usize;
    while j < 10 {
        let mut any_q_viable = false;
        for (qi, &q) in Q.iter().enumerate() {
            let sm = simplicity_max(qi, j);
            if W[0] * sm + W[1] + W[2] + W[3] < best_score {
                continue;
            }
            any_q_viable = true;
            // `k` is how many labels the sequence has.
            let mut k = 2usize;
            while k < 2 + m * 3 {
                let dm = density_max(k, m);
                if W[0] * sm + W[1] + W[2] * dm + W[3] < best_score {
                    break;
                }
                let delta = (dmax - dmin) / (k as f64 + 1.0) / j as f64 / q;
                let mut z = if delta > 0.0 {
                    delta.log10().ceil()
                } else {
                    0.0
                };
                // Walk the decade upward until the step is wider than the data, which is where
                // coverage can only get worse.
                loop {
                    let step = j as f64 * q * 10f64.powf(z);
                    if !step.is_finite() || step <= 0.0 {
                        break;
                    }
                    let cm = coverage_max(dmin, dmax, step * (k as f64 - 1.0));
                    if W[0] * sm + W[1] * cm + W[2] * dm + W[3] < best_score {
                        break;
                    }
                    // `start` slides the sequence so it can straddle the data rather than being
                    // pinned to a multiple of the step.
                    let min_start = (dmax / step).floor() * j as f64 - (k as f64 - 1.0) * j as f64;
                    let max_start = (dmin / step).ceil() * j as f64;
                    if min_start <= max_start {
                        let mut start = min_start;
                        while start <= max_start {
                            let lmin = start * step / j as f64;
                            let lmax = lmin + step * (k as f64 - 1.0);
                            let s = simplicity(qi, j, lmin, lmax, step);
                            let c = coverage(dmin, dmax, lmin, lmax);
                            let g = density(k, m, dmin, dmax, lmin, lmax);
                            // Score the cheap three first; only measure text when the result could
                            // still win, since measuring goes through the toolkit.
                            let optimistic = W[0] * s + W[1] * c + W[2] * g + W[3];
                            if optimistic > best_score {
                                let values: Vec<f64> =
                                    (0..k).map(|i| lmin + i as f64 * step).collect();
                                let labels = format_all(&values, step);
                                let l = fit.legibility(&labels);
                                let score = W[0] * s + W[1] * c + W[2] * g + W[3] * l;
                                if score > best_score {
                                    best_score = score;
                                    best = Some((score, Labelling { values, step }));
                                }
                            }
                            start += 1.0;
                        }
                    }
                    z += 1.0;
                    if z > 30.0 {
                        break;
                    }
                }
                k += 1;
            }
        }
        if !any_q_viable {
            break;
        }
        j += 1;
    }

    best.map(|(_, l)| l).unwrap_or_else(|| {
        // Nothing scored: fall back to the endpoints, which is always a legal labelling.
        Labelling {
            values: vec![dmin, dmax],
            step: dmax - dmin,
        }
    })
}

/// Decimal places a step of this size needs, so 0.1 does not print as `0.1000000000000000055`.
///
/// The decade alone is not the answer: a step of 0.25 sits in the same decade as 0.5 and needs one
/// more place than it. The count is the smallest `d` at which the step is a whole number of
/// `10^-d` units, which is exactly the question "how many places does this step actually use".
pub fn decimals_for_step(step: f64) -> usize {
    if !step.is_finite() || step <= 0.0 {
        return 0;
    }
    for d in 0..=12i32 {
        let scaled = step * 10f64.powi(d);
        if (scaled - scaled.round()).abs() <= 1e-9 * scaled.abs().max(1.0) {
            return d as usize;
        }
    }
    12
}

/// Format one value at a step's precision.
pub fn format_value(v: f64, step: f64) -> String {
    let d = decimals_for_step(step);
    // `-0` is a real f64 and prints with its sign, which reads as an error on an axis.
    let v = if v == 0.0 { 0.0 } else { v };
    format!("{v:.d$}")
}

fn format_all(values: &[f64], step: f64) -> Vec<String> {
    values.iter().map(|v| format_value(*v, step)).collect()
}

/// Decade ticks for a log axis, subdivided by 2 and 5 while there is room.
///
/// A log axis is not a candidate search: its readable labellings are the powers of the base, and
/// below a handful of decades the 2 and 5 multiples between them. Anything else (a "nice" step in
/// log space) produces labels like 3.16, which no one reads as a decade.
pub fn log_ticks(domain: Interval, base: f64, target: usize) -> Labelling {
    let lo = domain.lo.max(f64::MIN_POSITIVE);
    let hi = domain.hi.max(lo * base);
    let e0 = (lo.log(base)).floor() as i32;
    let e1 = (hi.log(base)).ceil() as i32;
    let decades = (e1 - e0).max(1);

    let mut values = Vec::new();
    // Only subdivide when the decades alone would under-fill the axis; a log plot spanning eight
    // decades with 2s and 5s in each is unreadable.
    let subdivide = base == 10.0 && decades <= 3 && target > decades as usize;
    for e in e0..=e1 {
        let d = base.powi(e);
        values.push(d);
        if subdivide {
            for m in [2.0, 5.0] {
                let v = d * m;
                if v < base.powi(e + 1) {
                    values.push(v);
                }
            }
        }
    }
    values.retain(|v| *v >= domain.lo * 0.999 && *v <= domain.hi * 1.001);
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // Thin uniformly when even the decades are too many to label.
    if values.len() > target.max(2) {
        let stride = values.len().div_ceil(target.max(2));
        values = values
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, _)| i % stride == 0)
            .map(|(_, v)| v)
            .collect();
    }
    Labelling { step: 0.0, values }
}

// ---------------------------------------------------------------------------
// Civil dates: enough calendar to tick a time axis, and no date dependency.
// ---------------------------------------------------------------------------

/// Proleptic Gregorian conversion, Howard Hinnant's `days_from_civil` / `civil_from_days`
/// (*chrono-Compatible Low-Level Date Algorithms*), which is the standard branch-free formulation
/// and is exact for the whole range an `f64` of seconds can express.
///
/// A time axis needs this rather than arithmetic on seconds because months and years are not fixed
/// numbers of seconds — a "monthly" tick that advanced by 2 592 000 s would drift off the first of
/// the month within a year and label February 29th as March 1st in leap years.
pub mod civil {
    /// `(year, month, day)` from a day count since 1970-01-01.
    pub fn from_days(z: i64) -> (i64, u32, u32) {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    /// Day count since 1970-01-01 from `(year, month, day)`.
    pub fn to_days(y: i64, m: u32, d: u32) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

const MINUTE: f64 = 60.0;
const HOUR: f64 = 3600.0;
const DAY: f64 = 86_400.0;

/// A calendar step a time axis may advance by.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TimeStep {
    /// A fixed number of seconds — everything up to a week, where the length never varies.
    Fixed(f64),
    Month(i32),
    Year(i32),
}

/// The ladder a time axis climbs, coarsest decision last. Each rung is a step a reader recognizes
/// as round: 15 seconds, quarter hours, six hours, a week, a quarter, a decade.
const TIME_STEPS: &[TimeStep] = &[
    TimeStep::Fixed(1.0),
    TimeStep::Fixed(5.0),
    TimeStep::Fixed(15.0),
    TimeStep::Fixed(30.0),
    TimeStep::Fixed(MINUTE),
    TimeStep::Fixed(5.0 * MINUTE),
    TimeStep::Fixed(15.0 * MINUTE),
    TimeStep::Fixed(30.0 * MINUTE),
    TimeStep::Fixed(HOUR),
    TimeStep::Fixed(3.0 * HOUR),
    TimeStep::Fixed(6.0 * HOUR),
    TimeStep::Fixed(12.0 * HOUR),
    TimeStep::Fixed(DAY),
    TimeStep::Fixed(7.0 * DAY),
    TimeStep::Month(1),
    TimeStep::Month(3),
    TimeStep::Month(6),
    TimeStep::Year(1),
    TimeStep::Year(2),
    TimeStep::Year(5),
    TimeStep::Year(10),
    TimeStep::Year(25),
    TimeStep::Year(50),
    TimeStep::Year(100),
];

/// Ticks for a time axis: the coarsest calendar step that still yields at least `target` ticks,
/// aligned to real calendar boundaries and labelled at the precision the step implies.
pub fn time_ticks(domain: Interval, target: usize) -> Labelling {
    let target = target.max(2);
    let span = domain.span();
    if !span.is_finite() || span <= 0.0 {
        return Labelling {
            values: vec![domain.lo],
            step: 0.0,
        };
    }
    // Walk from fine to coarse and stop at the first step that does not overfill the axis.
    let mut chosen = TIME_STEPS[TIME_STEPS.len() - 1];
    for st in TIME_STEPS {
        let approx = match st {
            TimeStep::Fixed(s) => *s,
            TimeStep::Month(n) => *n as f64 * 30.44 * DAY,
            TimeStep::Year(n) => *n as f64 * 365.2425 * DAY,
        };
        if span / approx <= target as f64 {
            chosen = *st;
            break;
        }
    }
    let values = walk(domain, chosen);
    Labelling {
        values,
        // A time labelling's precision comes from the step's KIND, not a decimal count; `step`
        // stays 0 and `time_label` reads the kind instead.
        step: 0.0,
    }
}

fn walk(domain: Interval, step: TimeStep) -> Vec<f64> {
    let mut out = Vec::new();
    match step {
        TimeStep::Fixed(s) => {
            let first = (domain.lo / s).ceil() * s;
            let mut t = first;
            while t <= domain.hi && out.len() < 1000 {
                out.push(t);
                t += s;
            }
        }
        TimeStep::Month(n) => {
            let (y0, m0, _) = civil::from_days((domain.lo / DAY).floor() as i64);
            // Snap to the step's own grid so quarterly ticks land on Jan/Apr/Jul/Oct rather than
            // wherever the data happens to start.
            let mut month_index = y0 * 12 + (m0 as i64 - 1);
            month_index -= month_index.rem_euclid(n as i64);
            loop {
                let y = month_index.div_euclid(12);
                let m = month_index.rem_euclid(12) as u32 + 1;
                let t = civil::to_days(y, m, 1) as f64 * DAY;
                if t > domain.hi || out.len() >= 1000 {
                    break;
                }
                if t >= domain.lo {
                    out.push(t);
                }
                month_index += n as i64;
            }
        }
        TimeStep::Year(n) => {
            let (y0, _, _) = civil::from_days((domain.lo / DAY).floor() as i64);
            let mut y = y0 - y0.rem_euclid(n as i64);
            loop {
                let t = civil::to_days(y, 1, 1) as f64 * DAY;
                if t > domain.hi || out.len() >= 1000 {
                    break;
                }
                if t >= domain.lo {
                    out.push(t);
                }
                y += n as i64;
            }
        }
    }
    out
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Label an instant at a precision the axis's own span justifies: a clock time when the axis spans
/// hours, a date when it spans months, a bare year when it spans decades. Showing the full
/// timestamp at every tick is what makes a time axis unreadable.
pub fn time_label(t: f64, span: f64) -> String {
    let days = (t / DAY).floor() as i64;
    let (y, m, d) = civil::from_days(days);
    let secs_of_day = t - days as f64 * DAY;
    let hh = (secs_of_day / HOUR).floor() as i64;
    let mm = ((secs_of_day - hh as f64 * HOUR) / MINUTE).floor() as i64;
    let ss = (secs_of_day - hh as f64 * HOUR - mm as f64 * MINUTE).round() as i64;
    let mon = MONTH_NAMES[(m as usize - 1).min(11)];
    if span < 2.0 * MINUTE {
        format!("{hh:02}:{mm:02}:{ss:02}")
    } else if span < 2.0 * DAY {
        format!("{hh:02}:{mm:02}")
    } else if span < 200.0 * DAY {
        format!("{mon} {d}")
    } else if span < 4.0 * 365.0 * DAY {
        format!("{mon} {y}")
    } else {
        format!("{y}")
    }
}
