// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
use day_mock::MockToolkit;
use day_piece_charts::{LegendEntry, categorical, legend};
use day_pieces::prelude::*;
use day_reactive::{Signal, flush_sync};
use day_spec::{Cursor, DragPhase, Event, NodeId, Point, Size, WindowOptions};

#[test]
fn linked_legend_registers_activation_and_hover_emphasis() {
    day_core::uninstall_tree();
    let (mock, probe) = MockToolkit::new();
    let highlighted = Signal::new(None);
    let activated = Signal::new(String::new());
    let entries = Signal::new(Vec::new());
    day_core::launch_with(
        mock,
        WindowOptions {
            title: "Legend fixture".into(),
            size: Size::new(400.0, 300.0),
            ..Default::default()
        },
        move || {
            legend(move || entries.get())
                .highlight(highlighted)
                .on_link(move |target| activated.set(target.into()))
                .any()
        },
    );
    // Dashboard projections arrive after mounting; all newly inserted rows need layout.
    entries.set(vec![
        LegendEntry::new("one", "Same fixture title", categorical(0)).link("fixture:one"),
        LegendEntry::new("two", "Same fixture title", categorical(1)).link("fixture:two"),
    ]);
    flush_sync();
    let (_, first) = probe
        .find_by_kind("day.container")
        .into_iter()
        .find(|(_, widget)| widget.a11y.identifier.as_deref() == Some("chart-legend-one"))
        .unwrap();
    let (_, second) = probe
        .find_by_kind("day.container")
        .into_iter()
        .find(|(_, widget)| widget.a11y.identifier.as_deref() == Some("chart-legend-two"))
        .unwrap();
    assert!(first.frame.size.height > 0.0);
    assert!(
        second.frame.origin.y >= first.frame.origin.y + first.frame.size.height,
        "legend rows must be stacked, not overlaid: {:?}, {:?}",
        first.frame,
        second.frame
    );
    assert_eq!(first.cursor, Some(Cursor::Pointer));
    probe.emit(
        NodeId(first.node),
        Event::Hover {
            phase: DragPhase::Began,
            location: Point::new(5.0, 5.0),
        },
    );
    flush_sync();
    assert_eq!(highlighted.get_untracked().as_deref(), Some("one"));
    assert!(
        probe
            .find_by_kind("day.container")
            .iter()
            .any(|(_, widget)| widget.background == Some(categorical(0).with_alpha(0.14)))
    );
    probe.emit(NodeId(first.node), Event::Tap(Point::new(5.0, 5.0)));
    assert_eq!(activated.get_untracked(), "fixture:one");
    probe.emit(
        NodeId(first.node),
        Event::Hover {
            phase: DragPhase::Ended,
            location: Point::new(5.0, 5.0),
        },
    );
    flush_sync();
    assert_eq!(highlighted.get_untracked(), None);
    day_core::uninstall_tree();
}
