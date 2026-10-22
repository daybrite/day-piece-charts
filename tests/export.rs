// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
use day_mock::{MockProbe, MockToolkit};
use day_piece_charts::{self as ch, *};
use day_pieces::prelude::*;
use day_reactive::flush_sync;
use day_spec::{DrawOp, WindowOptions};

fn boot(root: impl FnOnce() -> AnyPiece + 'static) -> MockProbe {
    day_core::uninstall_tree();
    let (mock, probe) = MockToolkit::new();
    day_core::launch_with(
        mock,
        WindowOptions::default()
            .title("Export fixture")
            .size(400.0, 600.0),
        root,
    );
    probe
}

fn bars() -> Chart {
    chart(|| {
        vec![
            ch::bar(value("Month", "Jan"), value("Sales", 3)),
            ch::bar(value("Month", "Feb"), value("Sales", 5)),
        ]
    })
}

#[test]
fn charts_built_inside_exports_register_and_record_a_still() {
    let exports = ChartExports::new();
    let around = exports.clone();
    let shown = day_reactive::Signal::global(true);
    let _probe = boot(move || {
        // Keyed, so switching it off disposes the charts' scope the way leaving a page does.
        keyed(
            move || Some(shown.get()),
            move |on| {
                if *on {
                    around
                        .around(|| column((bars().frame(300.0, 200.0), bars().frame(200.0, 100.0))))
                        .any()
                } else {
                    bars().frame(100.0, 100.0).any()
                }
            },
        )
        .any()
    });
    flush_sync();
    assert_eq!(exports.len(), 2);

    // One drawing per chart, at the size each drew on screen, on a white sheet.
    let drawings = exports.drawings();
    assert_eq!(drawings.len(), 2);
    assert_eq!(drawings[0].size(), Size::new(300.0, 200.0));
    assert_eq!(drawings[1].size(), Size::new(200.0, 100.0));
    assert!(matches!(drawings[0].ops().first(), Some(DrawOp::Fill(..))));
    assert!(drawings[0].ops().len() > 3, "the still drew the chart");

    // Stacked: the widest chart's width, both heights plus the gap between them.
    let stacked = exports.drawing().expect("two charts");
    assert_eq!(stacked.size(), Size::new(300.0, 200.0 + 24.0 + 100.0));

    // Leaving the page unregisters its charts; a chart built outside the exports registers
    // nothing.
    shown.set(false);
    flush_sync();
    assert_eq!(exports.len(), 0, "the removed charts unregistered");
    assert!(exports.drawing().is_none());
}
