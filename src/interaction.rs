// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
//! Declarative interactions: reusable data-space parameters, event bindings and predicates.
//! A parameter is owned by the application and can coordinate any number of charts or legends.
use crate::{
    Datum, Mark, Scale,
    select::{Guides, HitModel, Selection, Snap},
};
use day_reactive::Signal;
use day_spec::{Color, DragPhase, Point, Rect};
use std::{collections::BTreeMap, rc::Rc};

/// A data tuple independent of a chart's pixels, title or formatted axis labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub key: Option<String>,
    pub x: Option<Datum>,
    pub y: Option<Datum>,
    pub series: Option<Datum>,
    pub fields: BTreeMap<String, Datum>,
}
impl Record {
    pub fn from_mark(mark: &Mark) -> Self {
        Self {
            key: mark.key.clone(),
            x: mark.x.as_ref().map(|v| v.datum.clone()),
            y: mark.y.as_ref().map(|v| v.datum.clone()),
            series: mark.series.as_ref().map(|v| v.datum.clone()),
            fields: mark.fields.clone(),
        }
    }
    pub fn series(key: impl Into<String>) -> Self {
        Self {
            series: Some(Datum::Category(key.into())),
            ..Self::default()
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Projection {
    #[default]
    Key,
    X,
    Y,
    XY,
    Series,
    /// All listed fields must match. Missing fields never match.
    Fields(&'static [&'static str]),
}
impl Projection {
    fn matches(self, a: &Record, b: &Record) -> bool {
        let eq = |a: &Option<Datum>, b: &Option<Datum>| a.is_some() && a == b;
        match self {
            Self::Key => {
                if a.key.is_some() {
                    a.key == b.key
                } else {
                    eq(&a.x, &b.x) && a.y == b.y && a.series == b.series
                }
            }
            Self::X => eq(&a.x, &b.x),
            Self::Y => eq(&a.y, &b.y),
            Self::XY => eq(&a.x, &b.x) && eq(&a.y, &b.y),
            Self::Series => eq(&a.series, &b.series),
            Self::Fields(fields) => {
                !fields.is_empty()
                    && fields.iter().all(|field| {
                        a.fields.contains_key(*field)
                            && a.fields.get(*field) == b.fields.get(*field)
                    })
            }
        }
    }
}
/// A discrete query. Clone/copy this handle to bind several views to the same selection.
#[derive(Clone, Copy)]
pub struct PointSelection {
    pub state: Signal<Vec<Record>>,
    pub projection: Projection,
    pub empty_matches: bool,
}
impl Default for PointSelection {
    fn default() -> Self {
        Self::new()
    }
}
impl PointSelection {
    pub fn new() -> Self {
        Self {
            state: Signal::new(Vec::new()),
            projection: Projection::Key,
            empty_matches: true,
        }
    }
    pub fn project(mut self, projection: Projection) -> Self {
        self.projection = projection;
        self
    }
    pub fn empty_matches(mut self, matches: bool) -> Self {
        self.empty_matches = matches;
        self
    }
    pub fn predicate(self) -> Predicate {
        Predicate::Point(self)
    }
    pub fn clear(self) {
        self.state.set_if_changed(Vec::new());
    }
    pub fn is_empty(self) -> bool {
        self.state.with(|v| v.is_empty())
    }
    pub fn contains(self, record: &Record) -> bool {
        self.state.with(|points| {
            if points.is_empty() {
                self.empty_matches
            } else {
                points.iter().any(|p| self.projection.matches(p, record))
            }
        })
    }
    pub fn replace(self, record: Option<Record>) {
        self.state.set_if_changed(record.into_iter().collect());
    }
    pub fn toggle(self, record: Record) {
        self.state.update(|points| {
            if let Some(i) = points
                .iter()
                .position(|p| self.projection.matches(p, &record))
            {
                points.remove(i);
            } else {
                points.push(record);
            }
        });
    }
    pub fn on(self, trigger: EventSource) -> PointBinding {
        PointBinding {
            parameter: self,
            trigger,
            toggle: Toggle::Replace,
            snap: Snap::Hit,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventSource {
    Hover,
    Click,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Replace,
    Always,
    Shift,
}
#[derive(Clone, Copy)]
pub struct PointBinding {
    pub parameter: PointSelection,
    pub trigger: EventSource,
    pub toggle: Toggle,
    pub snap: Snap,
}
impl PointBinding {
    pub fn toggle(mut self, mode: Toggle) -> Self {
        self.toggle = mode;
        self
    }
    pub fn nearest(mut self, snap: Snap) -> Self {
        self.snap = snap;
        self
    }
    pub(crate) fn activate(self, record: Option<Record>, shift: bool) {
        if self.toggle == Toggle::Always || self.toggle == Toggle::Shift && shift {
            if let Some(record) = record {
                self.parameter.toggle(record);
            }
        } else {
            self.parameter.replace(record);
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axes {
    X,
    Y,
    #[default]
    XY,
}
impl Axes {
    pub fn x(self) -> bool {
        self != Self::Y
    }
    pub fn y(self) -> bool {
        self != Self::X
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Extent {
    Continuous(f64, f64),
    Categories(Vec<String>),
}
impl Extent {
    pub fn contains(&self, datum: &Datum) -> bool {
        match self {
            Self::Continuous(lo, hi) => datum.as_continuous().is_some_and(|v| v >= *lo && v <= *hi),
            Self::Categories(keys) => datum
                .as_category()
                .is_some_and(|v| keys.iter().any(|k| k == v)),
        }
    }
    pub fn domain(&self) -> Option<(f64, f64)> {
        match self {
            Self::Continuous(a, b) if b > a => Some((*a, *b)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bounds {
    pub x: Option<Extent>,
    pub y: Option<Extent>,
}
/// A range query stored in data space, so resizing or linking views does not move its meaning.
#[derive(Clone, Copy)]
pub struct IntervalSelection {
    pub state: Signal<Option<Bounds>>,
    pub axes: Axes,
    pub empty_matches: bool,
}
impl Default for IntervalSelection {
    fn default() -> Self {
        Self::new()
    }
}
impl IntervalSelection {
    pub fn new() -> Self {
        Self {
            state: Signal::new(None),
            axes: Axes::XY,
            empty_matches: true,
        }
    }
    pub fn axes(mut self, axes: Axes) -> Self {
        self.axes = axes;
        self
    }
    pub fn empty_matches(mut self, matches: bool) -> Self {
        self.empty_matches = matches;
        self
    }
    pub fn predicate(self) -> Predicate {
        Predicate::Interval(self)
    }
    pub fn clear(self) {
        self.state.set_if_changed(None);
    }
    pub fn x_domain(self) -> Option<(f64, f64)> {
        self.state.with(|b| b.as_ref()?.x.as_ref()?.domain())
    }
    pub fn y_domain(self) -> Option<(f64, f64)> {
        self.state.with(|b| b.as_ref()?.y.as_ref()?.domain())
    }
    pub fn contains(self, record: &Record) -> bool {
        self.state.with(|bounds| match bounds {
            None => self.empty_matches,
            Some(b) => {
                b.x.as_ref()
                    .is_none_or(|e| record.x.as_ref().is_some_and(|v| e.contains(v)))
                    && b.y
                        .as_ref()
                        .is_none_or(|e| record.y.as_ref().is_some_and(|v| e.contains(v)))
            }
        })
    }
    pub fn brush(self) -> Brush {
        Brush {
            parameter: self,
            translate: true,
            clear_on_outside: true,
        }
    }
    pub fn pan_zoom(self) -> Viewport {
        Viewport { parameter: self }
    }
}
#[derive(Clone, Copy)]
pub struct Brush {
    pub parameter: IntervalSelection,
    pub translate: bool,
    pub clear_on_outside: bool,
}
impl Brush {
    /// Clear the interval when clicking or tapping inside the plot but outside the
    /// current brush. Enabled by default; clicks inside the brush preserve it.
    pub fn clear_on_outside(mut self, enabled: bool) -> Self {
        self.clear_on_outside = enabled;
        self
    }
    pub fn translate(mut self, enabled: bool) -> Self {
        self.translate = enabled;
        self
    }
}
#[derive(Clone, Copy)]
pub struct Viewport {
    pub parameter: IntervalSelection,
}
/// Predicate algebra coordinates independent views: conjunction, union and complement.
#[derive(Clone)]
pub enum Predicate {
    Point(PointSelection),
    Interval(IntervalSelection),
    And(Rc<Predicate>, Rc<Predicate>),
    Or(Rc<Predicate>, Rc<Predicate>),
    Not(Rc<Predicate>),
}
impl Predicate {
    pub fn and(self, other: Self) -> Self {
        Self::And(Rc::new(self), Rc::new(other))
    }
    pub fn or(self, other: Self) -> Self {
        Self::Or(Rc::new(self), Rc::new(other))
    }
    pub fn negate(self) -> Self {
        Self::Not(Rc::new(self))
    }
    pub fn contains(&self, record: &Record) -> bool {
        match self {
            Self::Point(p) => p.contains(record),
            Self::Interval(p) => p.contains(record),
            Self::And(a, b) => a.contains(record) && b.contains(record),
            Self::Or(a, b) => a.contains(record) || b.contains(record),
            Self::Not(p) => !p.contains(record),
        }
    }
    pub fn matches(&self, mark: &Mark) -> bool {
        self.contains(&Record::from_mark(mark))
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Visual {
    pub opacity: Option<f64>,
    pub color: Option<Color>,
    pub symbol_size: Option<f64>,
    pub line_width: Option<f64>,
}
impl Visual {
    pub fn opacity(opacity: f64) -> Self {
        Self {
            opacity: Some(opacity.clamp(0.0, 1.0)),
            ..Self::default()
        }
    }
    pub fn color(color: Color) -> Self {
        Self {
            color: Some(color),
            ..Self::default()
        }
    }
    pub(crate) fn apply(self, mark: &mut Mark) {
        if let Some(v) = self.opacity {
            mark.style.opacity *= v;
        }
        if let Some(v) = self.color {
            mark.style.fill = Some(v);
            mark.style.gradient = None;
        }
        if let Some(v) = self.symbol_size {
            mark.style.symbol_size = v;
        }
        if let Some(v) = self.line_width {
            mark.style.line_width = v;
        }
    }
}
#[derive(Clone)]
pub struct Condition {
    pub predicate: Predicate,
    pub selected: Visual,
    pub other: Visual,
}
/// Inspection is one interaction, not a separate chart-wide mode; it can coexist with selection.
#[derive(Clone, Copy)]
pub struct Inspect {
    pub signal: Signal<Option<Selection>>,
    pub snap: Snap,
    pub guides: Guides,
}
impl Inspect {
    pub fn new(signal: Signal<Option<Selection>>) -> Self {
        Self {
            signal,
            snap: Snap::NearestX,
            guides: Guides::RULE,
        }
    }
    pub fn snap(mut self, snap: Snap) -> Self {
        self.snap = snap;
        self
    }
    pub fn guides(mut self, guides: Guides) -> Self {
        self.guides = guides;
        self
    }
}
pub type LinkHandler = Rc<dyn Fn(&str)>;
#[derive(Clone, Default)]
pub struct Links {
    pub handler: Option<LinkHandler>,
}
impl Links {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn on_open(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.handler = Some(Rc::new(handler));
        self
    }
}
#[derive(Clone)]
pub enum Interaction {
    Inspect(Inspect),
    Point(PointBinding),
    Brush(Brush),
    Viewport(Viewport),
    Links(Links),
}
macro_rules! interaction_from { ($($ty:ident),*) => { $(impl From<$ty> for Interaction { fn from(value: $ty) -> Self { Self::$ty(value) } })* }; }
interaction_from!(Inspect, Brush, Viewport, Links);
impl From<PointBinding> for Interaction {
    fn from(value: PointBinding) -> Self {
        Self::Point(value)
    }
}

pub(crate) fn extent(scale: &Scale, a: f64, b: f64) -> Option<Extent> {
    if scale.kind.is_discrete() {
        Some(Extent::Categories(
            scale
                .categories
                .iter()
                .filter(|k| {
                    scale
                        .project(&Datum::Category((*k).clone()))
                        .is_some_and(|v| v >= a.min(b) && v <= a.max(b))
                })
                .cloned()
                .collect(),
        ))
    } else {
        let (a, b) = (scale.invert(a)?, scale.invert(b)?);
        (a.is_finite() && b.is_finite()).then_some(Extent::Continuous(a.min(b), a.max(b)))
    }
}
pub(crate) fn bounds(hits: &HitModel, a: Point, b: Point, axes: Axes) -> Option<Bounds> {
    let clamp = |p: Point| {
        Point::new(
            p.x.clamp(
                hits.plot.origin.x,
                hits.plot.origin.x + hits.plot.size.width,
            ),
            p.y.clamp(
                hits.plot.origin.y,
                hits.plot.origin.y + hits.plot.size.height,
            ),
        )
    };
    let (a, b) = (clamp(a), clamp(b));
    Some(Bounds {
        x: if axes.x() {
            extent(hits.x_scale.as_ref()?, a.x, b.x)
        } else {
            None
        },
        y: if axes.y() {
            extent(hits.y_scale.as_ref()?, a.y, b.y)
        } else {
            None
        },
    })
}
pub(crate) fn brush_rect(bounds: &Bounds, hits: &HitModel) -> Option<Rect> {
    let projected =
        |extent: &Option<Extent>, scale: &Scale, fallback: (f64, f64)| -> Option<(f64, f64)> {
            match extent {
                None => Some(fallback),
                Some(Extent::Continuous(a, b)) => Some((
                    scale.project(&Datum::Number(*a))?,
                    scale.project(&Datum::Number(*b))?,
                )),
                Some(Extent::Categories(keys)) => {
                    let positions: Vec<_> = keys
                        .iter()
                        .filter_map(|k| scale.project(&Datum::Category(k.clone())))
                        .collect();
                    let lo = positions.iter().copied().reduce(f64::min)?;
                    let hi = positions.iter().copied().reduce(f64::max)?;
                    Some((lo - scale.band_width() / 2.0, hi + scale.band_width() / 2.0))
                }
            }
        };
    let (x0, x1) = projected(
        &bounds.x,
        hits.x_scale.as_ref()?,
        (
            hits.plot.origin.x,
            hits.plot.origin.x + hits.plot.size.width,
        ),
    )?;
    let (y0, y1) = projected(
        &bounds.y,
        hits.y_scale.as_ref()?,
        (
            hits.plot.origin.y,
            hits.plot.origin.y + hits.plot.size.height,
        ),
    )?;
    Some(Rect::new(
        x0.min(x1),
        y0.min(y1),
        (x1 - x0).abs(),
        (y1 - y0).abs(),
    ))
}
/// Gesture session state stays outside reactive state: starting a drag alone does not repaint.
#[derive(Default)]
pub(crate) struct DragSession {
    pub origin: Option<Point>,
    pub previous: Option<Point>,
    pub translating: bool,
}
pub(crate) fn tap_brush(binding: Brush, hits: &HitModel, point: Point) {
    if !binding.clear_on_outside || !contains_rect(hits.plot, point) {
        return;
    }
    let outside = binding.parameter.state.with_untracked(|bounds| {
        bounds
            .as_ref()
            .and_then(|b| brush_rect(b, hits))
            .is_some_and(|r| !contains_rect(r, point))
    });
    if outside {
        binding.parameter.clear();
    }
}
pub(crate) fn drag_brush(
    binding: Brush,
    session: &mut DragSession,
    hits: &HitModel,
    phase: DragPhase,
    point: Point,
) {
    if phase == DragPhase::Began {
        if !contains_rect(hits.plot, point) {
            session.origin = None;
            return;
        }
        session.origin = Some(point);
        session.previous = Some(point);
        session.translating = binding.translate
            && binding.parameter.state.with_untracked(|b| {
                b.as_ref()
                    .and_then(|b| brush_rect(b, hits))
                    .is_some_and(|r| contains_rect(r, point))
            });
    }
    if let Some(origin) = session.origin {
        if session.translating {
            if let Some(previous) = session.previous {
                translate(
                    binding.parameter,
                    hits,
                    Point::new(previous.x - point.x, previous.y - point.y),
                );
            }
        } else {
            binding.parameter.state.set_if_changed(bounds(
                hits,
                origin,
                point,
                binding.parameter.axes,
            ));
        }
        session.previous = Some(point);
    }
    if phase == DragPhase::Ended {
        session.origin = None;
    }
}
pub(crate) fn contains_rect(r: Rect, p: Point) -> bool {
    p.x >= r.origin.x
        && p.x <= r.origin.x + r.size.width
        && p.y >= r.origin.y
        && p.y <= r.origin.y + r.size.height
}
pub(crate) fn translate(parameter: IntervalSelection, hits: &HitModel, delta: Point) {
    let current = parameter.state.get_untracked().unwrap_or_else(|| Bounds {
        x: if parameter.axes.x() {
            hits.x_scale
                .as_ref()
                .map(|s| Extent::Continuous(s.domain.lo, s.domain.hi))
        } else {
            None
        },
        y: if parameter.axes.y() {
            hits.y_scale
                .as_ref()
                .map(|s| Extent::Continuous(s.domain.lo, s.domain.hi))
        } else {
            None
        },
    });
    let shift = |extent: Option<Extent>, scale: &Option<Scale>, pixels: f64| -> Option<Extent> {
        match (extent, scale) {
            (Some(Extent::Continuous(a, b)), Some(s)) => {
                let a = s.invert(s.project(&Datum::Number(a))? - pixels)?;
                let b = s.invert(s.project(&Datum::Number(b))? - pixels)?;
                Some(Extent::Continuous(a.min(b), a.max(b)))
            }
            (e, _) => e,
        }
    };
    let next = Bounds {
        x: shift(current.x, &hits.x_scale, delta.x),
        y: shift(current.y, &hits.y_scale, delta.y),
    };
    parameter.state.set_if_changed(Some(next));
}
pub(crate) fn zoom(parameter: IntervalSelection, hits: &HitModel, anchor: Point, factor: f64) {
    if !factor.is_finite() || factor <= 0.0 {
        return;
    }
    let scale_extent = |scale: &Option<Scale>, enabled: bool, p: f64| -> Option<Extent> {
        if !enabled {
            return None;
        }
        let s = scale.as_ref()?;
        if s.kind.is_discrete() {
            return None;
        }
        let a = s.invert(p + (s.range.0 - p) / factor)?;
        let b = s.invert(p + (s.range.1 - p) / factor)?;
        (a.is_finite() && b.is_finite() && (b - a).abs() > f64::EPSILON)
            .then_some(Extent::Continuous(a.min(b), a.max(b)))
    };
    parameter.state.set_if_changed(Some(Bounds {
        x: scale_extent(&hits.x_scale, parameter.axes.x(), anchor.x),
        y: scale_extent(&hits.y_scale, parameter.axes.y(), anchor.y),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Interval, ScaleKind, point, value};
    fn record(key: &str, x: f64, y: f64, series: &str) -> Record {
        Record {
            key: Some(key.into()),
            x: Some(Datum::Number(x)),
            y: Some(Datum::Number(y)),
            series: Some(Datum::Category(series.into())),
            ..Record::default()
        }
    }
    fn model() -> HitModel {
        HitModel {
            plot: Rect::new(0.0, 0.0, 200.0, 100.0),
            x_scale: Some(Scale::continuous(
                ScaleKind::Linear,
                Interval::new(0.0, 100.0),
                (0.0, 200.0),
            )),
            y_scale: Some(Scale::continuous(
                ScaleKind::Linear,
                Interval::new(0.0, 100.0),
                (100.0, 0.0),
            )),
            ..HitModel::default()
        }
    }
    #[test]
    fn outside_clear_is_optional_and_respects_both_brush_axes() {
        let selected = IntervalSelection::new();
        let hits = model();
        selected.state.set(bounds(
            &hits,
            Point::new(40.0, 20.0),
            Point::new(120.0, 60.0),
            Axes::XY,
        ));
        let original = selected.state.get_untracked();
        tap_brush(
            selected.brush().clear_on_outside(false),
            &hits,
            Point::new(180.0, 80.0),
        );
        assert_eq!(selected.state.get_untracked(), original);
        tap_brush(selected.brush(), &hits, Point::new(40.0, 20.0));
        assert_eq!(
            selected.state.get_untracked(),
            original,
            "brush borders belong to the selection"
        );
        tap_brush(selected.brush(), &hits, Point::new(80.0, 80.0));
        assert_eq!(
            selected.state.get_untracked(),
            None,
            "outside either selected axis clears"
        );
        tap_brush(selected.brush(), &hits, Point::new(180.0, 80.0));
        assert_eq!(selected.state.get_untracked(), None);
    }
    #[test]
    fn key_queries_link_different_encodings_and_toggle_without_duplicates() {
        let selected = PointSelection::new();
        let a = record("a", 10.0, 20.0, "one");
        let changed = record("a", 70.0, 60.0, "other");
        selected.replace(Some(a.clone()));
        assert!(selected.contains(&changed));
        selected.toggle(changed);
        assert!(selected.is_empty());
        let bind = selected.on(EventSource::Click).toggle(Toggle::Shift);
        bind.activate(Some(a.clone()), false);
        bind.activate(Some(record("b", 10.0, 20.0, "one")), true);
        assert_eq!(selected.state.get_untracked().len(), 2);
        bind.activate(Some(a), false);
        assert_eq!(selected.state.get_untracked().len(), 1);
    }
    #[test]
    fn field_projection_ignores_positions_but_rejects_missing_fields() {
        let selected = PointSelection::new().project(Projection::Fields(&["country", "year"]));
        let a = point(value("", 10), value("", 20))
            .field("country", "fixture")
            .field("year", 2026);
        selected.replace(Some(Record::from_mark(&a)));
        let b = point(value("", 50), value("", 80))
            .field("country", "fixture")
            .field("year", 2026);
        assert!(selected.predicate().matches(&b));
        assert!(
            !selected
                .predicate()
                .matches(&point(value("", 10), value("", 20)).field("country", "fixture"))
        );
    }
    #[test]
    fn predicates_compose_with_explicit_empty_semantics() {
        let a = PointSelection::new()
            .project(Projection::Series)
            .empty_matches(false);
        let b = IntervalSelection::new().axes(Axes::X).empty_matches(false);
        a.replace(Some(Record::series("one")));
        b.state.set(Some(Bounds {
            x: Some(Extent::Continuous(10.0, 30.0)),
            y: None,
        }));
        let inside = record("inside", 20.0, 99.0, "one");
        let group_only = record("outside", 40.0, 99.0, "one");
        assert!(a.predicate().and(b.predicate()).contains(&inside));
        assert!(!a.predicate().and(b.predicate()).contains(&group_only));
        assert!(a.predicate().or(b.predicate()).contains(&group_only));
        assert!(b.predicate().negate().contains(&group_only));
        b.clear();
        assert!(!b.contains(&inside));
        assert!(IntervalSelection::new().contains(&inside));
    }
    #[test]
    fn brush_projects_reversed_axes_and_survives_view_resizing() {
        let first = model();
        let data = bounds(
            &first,
            Point::new(40.0, 20.0),
            Point::new(100.0, 80.0),
            Axes::XY,
        )
        .unwrap();
        assert_eq!(
            data,
            Bounds {
                x: Some(Extent::Continuous(20.0, 50.0)),
                y: Some(Extent::Continuous(20.0, 80.0))
            }
        );
        let mut resized = model();
        resized.x_scale.as_mut().unwrap().range = (0.0, 400.0);
        let rect = brush_rect(&data, &resized).unwrap();
        assert_eq!(rect.origin.x, 80.0);
        assert_eq!(rect.size.width, 120.0);
    }
    #[test]
    fn categorical_brush_queries_data_categories() {
        let scale = Scale::discrete(
            ScaleKind::Band,
            vec!["a".into(), "b".into(), "c".into()],
            (0.0, 300.0),
        );
        let extent = extent(&scale, 0.0, 190.0).unwrap();
        assert!(extent.contains(&Datum::Category("a".into())));
        assert!(extent.contains(&Datum::Category("b".into())));
        assert!(!extent.contains(&Datum::Category("c".into())));
        assert_eq!(extent.domain(), None);
    }
    #[test]
    fn moving_a_brush_moves_its_query_with_the_pointer() {
        let selected = IntervalSelection::new().axes(Axes::X);
        selected.state.set(Some(Bounds {
            x: Some(Extent::Continuous(20.0, 40.0)),
            y: None,
        }));
        let mut session = DragSession::default();
        let hits = model();
        drag_brush(
            selected.brush(),
            &mut session,
            &hits,
            DragPhase::Began,
            Point::new(60.0, 50.0),
        );
        drag_brush(
            selected.brush(),
            &mut session,
            &hits,
            DragPhase::Ended,
            Point::new(80.0, 50.0),
        );
        assert_eq!(selected.x_domain(), Some((30.0, 50.0)));
    }
    #[test]
    fn viewport_pan_and_zoom_use_scale_inversion_and_keep_the_anchor() {
        let selected = IntervalSelection::new();
        let hits = model();
        translate(selected, &hits, Point::new(20.0, 0.0));
        assert_eq!(selected.x_domain(), Some((-10.0, 90.0)));
        zoom(selected, &hits, Point::new(50.0, 50.0), 2.0);
        assert_eq!(selected.x_domain(), Some((12.5, 62.5)));
        assert_eq!(selected.y_domain(), Some((25.0, 75.0)));
        let before = selected.state.get_untracked();
        zoom(selected, &hits, Point::new(0.0, 0.0), f64::NAN);
        assert_eq!(selected.state.get_untracked(), before);
    }
    #[test]
    fn logarithmic_brush_has_data_space_extents() {
        let mut hits = model();
        hits.x_scale = Some(Scale::continuous(
            ScaleKind::Log { base: 10.0 },
            Interval::new(1.0, 10000.0),
            (0.0, 200.0),
        ));
        let selected = bounds(
            &hits,
            Point::new(50.0, 0.0),
            Point::new(100.0, 50.0),
            Axes::X,
        )
        .unwrap();
        assert_eq!(selected.x, Some(Extent::Continuous(10.0, 100.0)));
    }
}
