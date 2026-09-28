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

use day::prelude::*;
use day_piece_charts_gallery::{self as gallery, ChartPage};

// The entry point on iOS, Android, and the web; a plain cargo desktop build enters through
// src/main.rs.
day::day_start!(options: window(), root);

// Typed constants for everything under `resource/` (https://daybrite.dev/docs/resources).
day::resources!();

/// The window every entry point opens.
pub fn window() -> day::WindowOptions {
    day::WindowOptions {
        locales: Some((res::locales::DEFAULT, res::locales::CATALOG)),
        title_fn: Some(|| res::str::app_title().format()),
        size: day::prelude::Size::new(480.0, 800.0),
        ..Default::default()
    }
}

/// The whole app: a sidebar of the gallery's pages, and whichever is open.
pub fn root() -> impl Piece {
    info!("Charts Demo starting");

    // The open page. The nav writes it, a launch deep link writes it before the first frame (the
    // URL hash on web-dom, `DAY_DEEPLINK` elsewhere), and the browser's back button writes it
    // again.
    let page = Signal::new(ChartPage::Pipeline);
    let mut nav = nav(page)
        .style(NavStyle::Sidebar)
        .title(res::str::app_title());
    // The pipeline page under "This demo", then the examples in the order the README documents
    // them. Each row's page is the gallery's, under its own route key, so a screenshot, a route
    // and the chart's id are all one name.
    for entry in gallery::pages() {
        match entry.page {
            ChartPage::Pipeline => nav = nav.section(res::str::nav_demo()),
            ChartPage::Lines => nav = nav.section(res::str::nav_examples()),
            _ => {}
        }
        nav = nav.item(entry.page, (entry.title)(), move || entry.build());
    }
    nav.id("nav")
}
