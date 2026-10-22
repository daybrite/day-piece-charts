// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
use day::prelude::*;
use day_mock::MockToolkit;
use day_piece_charts_gallery::ChartPage;
use day_spec::{MenuItem, WindowOptions};

/// Every chart on a gallery page offers both export commands in its context menu, each enabled
/// only where the toolkit renders that format. The mock renders bitmaps and encodes PNG but has
/// no PDF writer, like ArkUI and the web, so PDF must be present and disabled.
#[test]
fn a_charts_context_menu_offers_both_exports_and_disables_the_missing_one() {
    day_core::uninstall_tree();
    let (mock, probe) = MockToolkit::new();
    day_core::launch_with(
        mock,
        WindowOptions::default().size(600.0, 800.0),
        || day_piece_charts_gallery::page(ChartPage::Bars),
    );
    day_reactive::flush_sync();
    let (_, canvas) = probe.find_by_kind("day.canvas").remove(0);
    let node = day_core::id_to_rnode(day_spec::NodeId(canvas.node));
    let menu = day_core::with_tree(|t| t.context_menu_of(node)).expect("the chart has a menu");
    let items = menu(Point::ZERO);
    let state = |id: &str| {
        items.iter().find_map(|i| match i {
            MenuItem::Action {
                id: Some(item), enabled, ..
            } if item == id => Some(*enabled),
            _ => None,
        })
    };
    assert_eq!(state("charts_export_png"), Some(true), "{items:?}");
    assert_eq!(state("charts_export_pdf"), Some(false), "{items:?}");
}
