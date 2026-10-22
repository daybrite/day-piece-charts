// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Charts Demo: the demo and on-device test app for `day-piece-charts`.
//!
//! A navigation app of one page per chart. Every page comes from `day-piece-charts-gallery`, the
//! crate that holds them for any app that wants to show them (Day-Showcase mounts the same
//! pages), so this app is only the shell: a window, a sidebar, and the routes.
//!
//! The first page is the pipeline page: a picker of five compositions of one data set, the chart,
//! a readout of what the pipeline derives from it, and a slider that changes how much data there
//! is. The rest are the illustrative charts the crate's README documents, one route each.
//!
//! The route is the address: it is the URL hash on web-dom, so <https://…/#lines> opens the line
//! chart and the browser's back button walks the pages, and it is the name `dayscript`'s
//! `navigate:` step uses, which is how both walkthroughs move between charts. Every element carries
//! a stable id, so they can assert what is on the page as well as reach it.
//!
//! File ▸ Export as PNG and File ▸ Export as PDF render the open page's charts through the
//! toolkit's own painter (README "Export"); each chart's context menu offers the same two. Both
//! items are present on every target and enabled where the toolkit renders that format.

use day::prelude::*;
use day_piece_charts_gallery::{self as gallery, ChartPage};

// The entry point on iOS, Android, and the web; a plain cargo desktop build enters through
// src/main.rs.
day::day_start!(options: window(), root);

// Typed constants for everything under `resource/` (https://daybrite.dev/docs/how-to/resources).
day::resources!();

/// The window every entry point opens.
pub fn window() -> day::WindowOptions {
    res::window_options().size(480.0, 800.0)
}

/// The whole app: a sidebar of the gallery's pages, and whichever is open.
pub fn root() -> impl Piece {
    info!("Charts Demo starting");

    // The open page. The nav writes it, a launch deep link writes it before the first frame (the
    // URL hash on web-dom, `DAY_DEEPLINK` elsewhere), and the browser's back button writes it
    // again.
    let page = Signal::new(ChartPage::Pipeline);
    // The menu bar's File menu carries the gallery's export commands for whichever page is open.
    // On a target with no menu bar the charts' context menus carry them alone.
    app_menu(move || {
        vec![
            sub_menu(
                res::str::day_file(),
                gallery::export_menu_items(move || page.get_untracked()),
            )
            .bar_role(MenuBarRole::File),
        ]
    });
    // The pipeline page under "This demo", then the examples in the order the README documents
    // them, then the interactions. Each row's page is the gallery's, under its own route key, so
    // a screenshot, a route and the chart's id are all one name.
    let entries = gallery::pages();
    let starts = |p: ChartPage| {
        entries
            .iter()
            .position(|e| e.page == p)
            .unwrap_or(entries.len())
    };
    let (examples, interactions) = (starts(ChartPage::Lines), starts(ChartPage::Linked));
    let rows = |range: std::ops::Range<usize>| -> Vec<NavItem<ChartPage>> {
        entries[range]
            .iter()
            .map(|entry| {
                let entry = *entry;
                item(entry.page, (entry.title)()).page(move || entry.build())
            })
            .collect()
    };
    nav(page)
        .style(NavStyle::Sidebar)
        .title(res::str::app_title())
        .section(res::str::nav_demo(), rows(0..examples))
        .section(res::str::nav_examples(), rows(examples..interactions))
        .section(
            res::str::nav_interactions(),
            rows(interactions..entries.len()),
        )
        .id(day::id!(nav))
}
