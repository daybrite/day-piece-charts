// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
//! Native interaction recipes shared by Charts Demo and Day Showcase.
use crate::{ChartPage, Entry, res};
use day::prelude::*;
use day_piece_charts::{self as ch, *};

pub fn pages() -> Vec<Entry> {
    vec![
        Entry {
            page: ChartPage::Linked,
            title: res::str::inter_linked,
            build: linked,
        },
        Entry {
            page: ChartPage::Brush,
            title: res::str::inter_brush,
            build: brushing,
        },
        Entry {
            page: ChartPage::Overview,
            title: res::str::inter_overview,
            build: overview,
        },
        Entry {
            page: ChartPage::Legend,
            title: res::str::inter_legend,
            build: legend_page,
        },
        Entry {
            page: ChartPage::Cells,
            title: res::str::inter_cells,
            build: cells,
        },
        Entry {
            page: ChartPage::Viewport,
            title: res::str::inter_viewport,
            build: viewport,
        },
        Entry {
            page: ChartPage::Compose,
            title: res::str::inter_compose,
            build: compose,
        },
        Entry {
            page: ChartPage::Links,
            title: res::str::inter_links,
            build: links,
        },
        Entry {
            page: ChartPage::Filtered,
            title: res::str::inter_filtered,
            build: filtered,
        },
        Entry {
            page: ChartPage::Inputs,
            title: res::str::inter_inputs,
            build: inputs,
        },
    ]
}
fn groups() -> [String; 3] {
    [
        res::str::inter_group_a().format(),
        res::str::inter_group_b().format(),
        res::str::inter_group_c().format(),
    ]
}
fn scatter() -> Vec<Mark> {
    let names = groups();
    (0..60)
        .map(|i| {
            ch::point(
                value(res::str::inter_x().format(), (i * 37 % 97 + 2) as f64),
                value(res::str::inter_y().format(), (i * 23 % 91 + 4) as f64),
            )
            .key(format!("sample-{i}"))
            .field("group", (i % 3) as f64)
            .by_series(value("", names[i % 3].clone()))
            .symbol_size(65.0)
        })
        .collect()
}
fn plot(marks: impl Fn() -> Vec<Mark> + 'static) -> Chart {
    chart(marks)
        .x_domain(0.0, 100.0)
        .y_domain(0.0, 100.0)
        .legend(LegendPosition::Hidden)
        .x_tick_count(4)
        .y_tick_count(4)
}
fn count(predicate: &Predicate) -> usize {
    scatter().iter().filter(|m| predicate.matches(m)).count()
}
fn entries() -> Vec<LegendEntry> {
    groups()
        .into_iter()
        .enumerate()
        .map(|(i, n)| LegendEntry::new(n.clone(), n, categorical(i)))
        .collect()
}
fn linked() -> AnyPiece {
    let selected = PointSelection::new();
    let predicate = selected.predicate();
    let other = predicate.clone();
    column((
        label(res::str::inter_linked_note()).font(Font::Caption),
        row((
            label(move || {
                res::str::inter_count(if selected.is_empty() {
                    0.0
                } else {
                    selected.state.with(|v| v.len()) as f64
                })
                .format()
            })
            .id("linked-count")
            .grow_w(),
            button(res::str::inter_select_sample())
                .action(move || selected.replace(Some(Record::from_mark(&scatter()[0]))))
                .id("linked-select-sample"),
            button(res::str::inter_reset())
                .action(move || selected.clear())
                .id("linked-reset"),
        )),
        plot(scatter)
            .interact(selected.on(EventSource::Click).toggle(Toggle::Shift))
            .condition(predicate, Visual::default(), Visual::opacity(0.16))
            .id("linked-primary")
            .grow(),
        plot(|| {
            scatter()
                .into_iter()
                .map(|mut m| {
                    std::mem::swap(&mut m.x, &mut m.y);
                    m
                })
                .collect()
        })
        .interact(selected.on(EventSource::Click).toggle(Toggle::Shift))
        .condition(other, Visual::color(categorical(1)), Visual::opacity(0.16))
        .id("linked-secondary")
        .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn brushing() -> AnyPiece {
    let brush = IntervalSelection::new();
    let predicate = brush.predicate();
    let counted = predicate.clone();
    column((
        label(res::str::inter_brush_note()).font(Font::Caption),
        label(move || res::str::inter_count(count(&counted) as f64).format())
            .id("brush-count")
            .grow_w(),
        plot(scatter)
            .interact(brush.brush())
            .condition(predicate, Visual::default(), Visual::opacity(0.12))
            .id("brush-source")
            .grow(),
        chart(move || {
            groups()
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                    let n = scatter()
                        .iter()
                        .filter(|m| {
                            brush.contains(&Record::from_mark(m))
                                && m.fields.get("group") == Some(&Datum::Number(i as f64))
                        })
                        .count();
                    bar(
                        value("", name.clone()),
                        value(res::str::inter_count_axis().format(), n),
                    )
                    .by_series(value("", name))
                })
                .collect()
        })
        .y_domain(0.0, 20.0)
        .legend(LegendPosition::Hidden)
        .animated()
        .id("brush-histogram")
        .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn waveform() -> Vec<Mark> {
    (0..80)
        .map(|i| {
            ch::line(
                value(res::str::inter_x().format(), i),
                value(
                    res::str::inter_y().format(),
                    50.0 + (i as f64 * 0.19).sin() * 25.0 + (i as f64 * 0.73).cos() * 10.0,
                ),
            )
            .key(format!("wave-{i}"))
        })
        .collect()
}
fn overview() -> AnyPiece {
    let range = IntervalSelection::new().axes(Axes::X);
    let inspection = Signal::new(None);
    column((
        label(res::str::inter_overview_note()).font(Font::Caption),
        chart(waveform)
            .x_domain(0.0, 79.0)
            .y_domain(0.0, 100.0)
            .legend(LegendPosition::Hidden)
            .interact(range.brush())
            .id("overview-source")
            .height(160.0)
            .grow_w(),
        chart(waveform)
            .x_domain(0.0, 79.0)
            .y_domain(0.0, 100.0)
            .legend(LegendPosition::Hidden)
            .domain(range)
            .interact(Inspect::new(inspection))
            .id("overview-detail")
            .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn legend_page() -> AnyPiece {
    let selection = PointSelection::new().project(Projection::Series);
    let predicate = selection.predicate();
    let other = predicate.clone();
    column((
        label(res::str::inter_legend_note()).font(Font::Caption),
        button(res::str::inter_reset())
            .action(move || selection.clear())
            .id("legend-reset"),
        chart(|| {
            groups()
                .into_iter()
                .enumerate()
                .map(|(i, n)| sector(value("", [32, 24, 44][i])).by_series(value("", n)))
                .collect()
        })
        .coordinate(Coordinate::donut(0.65))
        .legend(LegendPosition::Hidden)
        .interact(selection.on(EventSource::Click).toggle(Toggle::Always))
        .condition(predicate, Visual::default(), Visual::opacity(0.15))
        .id("legend-donut")
        .grow(),
        ch::legend(entries)
            .interact(selection.on(EventSource::Click).toggle(Toggle::Always))
            .id_prefix("interactive-legend"),
        plot(scatter)
            .legend(LegendPosition::Bottom)
            .interact(selection.on(EventSource::Click).toggle(Toggle::Always))
            .condition(other, Visual::default(), Visual::opacity(0.15))
            .id("legend-scatter")
            .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn cells() -> AnyPiece {
    let selection = PointSelection::new().project(Projection::XY);
    let inspection = Signal::new(None);
    column((
        label(res::str::inter_cells_note()).font(Font::Caption),
        chart(|| {
            groups()
                .into_iter()
                .enumerate()
                .flat_map(|(x, name)| {
                    (0..6).map(move |y| {
                        let n = (x * 13 + y * 7) % 24;
                        ch::rect(
                            value("", name.clone()),
                            value("", res::str::inter_band(y as f64 + 1.0).format()),
                        )
                        .foreground(sequential(n as f64 / 24.0))
                        .corner_radius(4.0)
                        .selection_value(value(
                            res::str::inter_cell(y as f64 + 1.0, n as f64).format(),
                            n,
                        ))
                    })
                })
                .collect()
        })
        .legend(LegendPosition::Hidden)
        .y_reversed()
        .interact(selection.on(EventSource::Hover))
        .interact(
            Inspect::new(inspection)
                .snap(Snap::Hit)
                .guides(Guides::CROSSHAIR),
        )
        .condition(
            selection.predicate(),
            Visual::default(),
            Visual::opacity(0.30),
        )
        .id("interactive-cells")
        .grow(),
        label(move || {
            inspection.with(|s| {
                s.as_ref()
                    .map(|s| s.values[0].label.clone())
                    .unwrap_or_else(|| res::str::inter_cells_note().format())
            })
        })
        .id("cells-readout"),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn zoom_by(parameter: IntervalSelection, factor: f64) {
    let x = parameter.x_domain().unwrap_or((0.0, 100.0));
    let y = parameter.y_domain().unwrap_or((0.0, 100.0));
    let zoom = |(a, b): (f64, f64)| {
        let center = (a + b) * 0.5;
        Extent::Continuous(
            center + (a - center) / factor,
            center + (b - center) / factor,
        )
    };
    parameter.state.set(Some(Bounds {
        x: Some(zoom(x)),
        y: Some(zoom(y)),
    }));
}
fn viewport() -> AnyPiece {
    let viewport = IntervalSelection::new();
    let inspection = Signal::new(None);
    column((
        label(res::str::inter_viewport_note()).font(Font::Caption),
        row((
            button(res::str::inter_zoom_in())
                .action(move || zoom_by(viewport, 1.5))
                .id("viewport-zoom-in"),
            button(res::str::inter_zoom_out())
                .action(move || zoom_by(viewport, 1.0 / 1.5))
                .id("viewport-zoom-out"),
            button(res::str::inter_reset())
                .action(move || viewport.clear())
                .id("viewport-reset"),
        ))
        .spacing(8.0),
        plot(scatter)
            .interact(viewport.pan_zoom())
            .interact(
                Inspect::new(inspection)
                    .snap(Snap::NearestMark)
                    .guides(Guides::CROSSHAIR),
            )
            .id("interactive-viewport")
            .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn compose() -> AnyPiece {
    let brush = IntervalSelection::new();
    let series = PointSelection::new().project(Projection::Series);
    let intersection = brush.predicate().and(series.predicate());
    let union = brush.predicate().or(series.predicate());
    let readout = intersection.clone();
    column((
        label(res::str::inter_compose_note()).font(Font::Caption),
        label(move || res::str::inter_count(count(&readout) as f64).format())
            .id("compose-count")
            .grow_w(),
        ch::legend(entries)
            .interact(series.on(EventSource::Click).toggle(Toggle::Always))
            .id_prefix("compose-series"),
        plot(scatter)
            .interact(brush.brush())
            .condition(intersection, Visual::default(), Visual::opacity(0.12))
            .id("compose-intersection")
            .grow(),
        plot(scatter)
            .condition(union, Visual::color(categorical(1)), Visual::opacity(0.12))
            .id("compose-union")
            .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn links() -> AnyPiece {
    let target = Signal::new(String::new());
    let hover = PointSelection::new().project(Projection::Series);
    let links = Links::new().on_open(move |link| target.set(link.into()));
    column((
        label(res::str::inter_links_note()).font(Font::Caption),
        chart(|| {
            groups()
                .into_iter()
                .enumerate()
                .map(|(i, n)| {
                    sector(value("", [32, 24, 44][i]))
                        .by_series(value("", n))
                        .link(format!("sample:{i}"))
                })
                .collect()
        })
        .coordinate(Coordinate::donut(0.60))
        .legend(LegendPosition::Hidden)
        .interact(links.clone())
        .interact(hover.on(EventSource::Hover))
        .condition(hover.predicate(), Visual::default(), Visual::opacity(0.20))
        .id("interactive-links")
        .grow(),
        ch::legend(|| {
            entries()
                .into_iter()
                .enumerate()
                .map(|(i, e)| e.link(format!("sample:{i}")))
                .collect()
        })
        .interact(hover.on(EventSource::Hover))
        .links(links)
        .id_prefix("link-series"),
        label(move || res::str::inter_link_target(target.get()).format()).id("link-target"),
    ))
    .spacing(8.0)
    .grow()
    .any()
}

fn filtered() -> AnyPiece {
    let brush = IntervalSelection::new();
    let count_query = brush.predicate();
    column((
        label(res::str::inter_filtered_note()).font(Font::Caption),
        label(move || res::str::inter_count(count(&count_query) as f64).format())
            .id("filtered-count")
            .grow_w(),
        plot(scatter)
            .interact(brush.brush())
            .id("filtered-source")
            .grow(),
        plot(scatter)
            .filter(brush.predicate())
            .id("filtered-result")
            .grow(),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
fn inputs() -> AnyPiece {
    let group = PointSelection::new().project(Projection::Fields(&["group"]));
    let picked = Signal::new(0usize);
    Effect::new(move || {
        let index = picked.get();
        group.replace(if index == 0 {
            None
        } else {
            Some(Record {
                fields: std::collections::BTreeMap::from([(
                    "group".into(),
                    Datum::Number((index - 1) as f64),
                )]),
                ..Record::default()
            })
        });
    });
    Effect::new(move || {
        let index = group.state.with(|points| {
            points
                .first()
                .and_then(|r| r.fields.get("group"))
                .and_then(Datum::as_continuous)
                .map(|v| v as usize + 1)
                .unwrap_or(0)
        });
        picked.set_if_changed(index);
    });
    let query = group.predicate();
    let counted = query.clone();
    column((
        label(res::str::inter_inputs_note()).font(Font::Caption),
        picker(
            std::iter::once(res::str::inter_all_groups().format())
                .chain(groups())
                .collect::<Vec<_>>(),
            picked,
        )
        .id("inputs-picker"),
        label(move || res::str::inter_count(count(&counted) as f64).format()).id("inputs-count"),
        plot(scatter)
            .interact(group.on(EventSource::Click))
            .condition(query, Visual::default(), Visual::opacity(0.15))
            .id("inputs-plot")
            .grow(),
        ch::legend(|| {
            entries()
                .into_iter()
                .enumerate()
                .map(|(i, entry)| {
                    entry.record(Record {
                        fields: std::collections::BTreeMap::from([(
                            "group".into(),
                            Datum::Number(i as f64),
                        )]),
                        ..Record::default()
                    })
                })
                .collect()
        })
        .interact(group.on(EventSource::Click))
        .id_prefix("input-group"),
    ))
    .spacing(8.0)
    .grow()
    .any()
}
