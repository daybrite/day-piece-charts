// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
use day_mock::{MockProbe, MockToolkit};
use day_piece_charts::Insets;
use day_piece_charts::{self as ch, *};
use day_pieces::prelude::*;
use day_reactive::{Signal, flush_sync};
use day_spec::{DragPhase, Event, NodeId, Point, Size, WindowOptions};
fn boot(root: impl FnOnce() -> AnyPiece + 'static) -> MockProbe {
    day_core::uninstall_tree();
    let (mock, probe) = MockToolkit::new();
    day_core::launch_with(
        mock,
        WindowOptions {
            title: "Interaction fixture".into(),
            size: Size::new(400.0, 300.0),
            ..Default::default()
        },
        root,
    );
    probe
}
fn tap(probe: &MockProbe, node: u64, at: Point) {
    probe.emit(NodeId(node), Event::Tap(at));
    flush_sync();
}
#[test]
fn chart_points_populate_a_shared_data_query() {
    let selected = PointSelection::new();
    let probe = boot(move || {
        chart(|| {
            vec![
                ch::point(value("", 25), value("", 25)).key("fixture-a"),
                ch::point(value("", 75), value("", 75)).key("fixture-b"),
            ]
        })
        .x_domain(0.0, 100.0)
        .y_domain(0.0, 100.0)
        .x_axis_hidden()
        .y_axis_hidden()
        .plot_insets(Insets::uniform(0.0))
        .legend(LegendPosition::Hidden)
        .interact(selected.on(EventSource::Click))
        .id("chart-fixture")
        .any()
    });
    let (_, canvas) = probe.find_by_kind("day.canvas").remove(0);
    let node =
        day_core::with_tree(|t| day_core::rnode_to_id(t.find_by_id("chart-fixture").unwrap()).0);
    tap(
        &probe,
        node,
        Point::new(
            canvas.frame.size.width * 0.25,
            canvas.frame.size.height * 0.75,
        ),
    );
    assert_eq!(
        selected.state.get_untracked()[0].key.as_deref(),
        Some("fixture-a")
    );
    tap(
        &probe,
        node,
        Point::new(
            canvas.frame.size.width * 0.5,
            canvas.frame.size.height * 0.5,
        ),
    );
    assert!(
        selected.is_empty(),
        "empty plot space clears the discrete query"
    );
    day_core::uninstall_tree();
}
#[test]
fn donut_links_hit_wedges_but_not_the_hole() {
    let activated = Signal::new(String::new());
    let probe = boot(move || {
        chart(|| {
            vec![
                sector(value("", 50))
                    .by_series(value("", "fixture-a"))
                    .link("fixture:a"),
                sector(value("", 50))
                    .by_series(value("", "fixture-b"))
                    .link("fixture:b"),
            ]
        })
        .coordinate(Coordinate::donut(0.65))
        .x_axis_hidden()
        .y_axis_hidden()
        .plot_insets(Insets::uniform(0.0))
        .legend(LegendPosition::Hidden)
        .interact(Links::new().on_open(move |target| activated.set(target.into())))
        .id("chart-fixture")
        .any()
    });
    let (_, canvas) = probe.find_by_kind("day.canvas").remove(0);
    let node =
        day_core::with_tree(|t| day_core::rnode_to_id(t.find_by_id("chart-fixture").unwrap()).0);
    let center = Point::new(
        canvas.frame.size.width * 0.5,
        canvas.frame.size.height * 0.5,
    );
    tap(&probe, node, center);
    assert_eq!(activated.get_untracked(), "");
    tap(
        &probe,
        node,
        Point::new(center.x + canvas.frame.size.height * 0.44, center.y),
    );
    assert_eq!(activated.get_untracked(), "fixture:a");
    day_core::uninstall_tree();
}
#[test]
fn drag_events_query_a_horizontal_data_interval() {
    let brush = IntervalSelection::new().axes(Axes::X);
    let probe = boot(move || {
        chart(|| {
            vec![
                ch::point(value("", 0), value("", 0)),
                ch::point(value("", 100), value("", 100)),
            ]
        })
        .x_domain(0.0, 100.0)
        .y_domain(0.0, 100.0)
        .x_axis_hidden()
        .y_axis_hidden()
        .plot_insets(Insets::uniform(0.0))
        .legend(LegendPosition::Hidden)
        .interact(brush.brush())
        .id("chart-fixture")
        .any()
    });
    let (_, canvas) = probe.find_by_kind("day.canvas").remove(0);
    let node =
        day_core::with_tree(|t| day_core::rnode_to_id(t.find_by_id("chart-fixture").unwrap()).0);
    for (phase, x) in [
        (DragPhase::Began, 0.2),
        (DragPhase::Changed, 0.6),
        (DragPhase::Ended, 0.6),
    ] {
        probe.emit(
            NodeId(node),
            Event::Drag {
                phase,
                location: Point::new(canvas.frame.size.width * x, 50.0),
                translation: Point::new(canvas.frame.size.width * (x - 0.2), 0.0),
            },
        );
        flush_sync();
    }
    assert_eq!(brush.x_domain(), Some((20.0, 60.0)));
    assert_eq!(brush.y_domain(), None);
    tap(
        &probe,
        node,
        Point::new(canvas.frame.size.width * 0.4, 200.0),
    );
    assert_eq!(
        brush.x_domain(),
        Some((20.0, 60.0)),
        "inside taps preserve an X brush regardless of Y"
    );
    tap(&probe, node, Point::new(-10.0, 50.0));
    assert_eq!(
        brush.x_domain(),
        Some((20.0, 60.0)),
        "outside the plot is ignored"
    );
    tap(
        &probe,
        node,
        Point::new(canvas.frame.size.width * 0.8, 50.0),
    );
    assert_eq!(
        brush.x_domain(),
        None,
        "outside taps restore default domains"
    );
    assert!(brush.contains(&Record::from_mark(&ch::point(value("", 90), value("", 90)))));
    day_core::uninstall_tree();
}
