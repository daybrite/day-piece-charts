// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! The day-piece-charts example pages, as a library any Day app can mount.
//!
//! The Charts Demo app is a sidebar of these pages; Day-Showcase shows the same pages under its
//! Charts section. Both call into this crate, so a fix or a new control lands in both at once.
//!
//! Two ways in:
//!
//! * [`pages`] lists every page with its route and title, for an app that builds its own
//!   navigation around them (the demo's sidebar is one `nav` item per entry), and
//!   [`ChartPage`] is the route type, so `…/#scatter` opens the same page in every host.
//! * [`gallery`] is the whole set as one page with its own picker, for an app that gives the
//!   charts a single slot (Showcase's Charts section).
//!
//! Every string comes from this crate's private catalog (`resource/locales`), so the pages read
//! the same in any host without the host installing anything, and follow the host's locale. A
//! dayscript names them by their qualified key (`day_piece_charts_gallery::ex_line`); [`register`]
//! makes that key resolvable, and every entry point here calls it.

use day::prelude::*;

// Typed accessors for this crate's own strings: `res::str::ex_line()` and friends.
day_fluent::locales!();

/// The charts the crate's README documents.
mod examples;
/// The pipeline page: one data set, five compositions, and what the pipeline derives.
mod pipeline;

day::routes! {
    /// One route per page. The key is what deep links, the URL hash on web-dom (`…/#scatter`)
    /// and dayscript's `navigate:` speak; it is also the id of the chart on each page, so a
    /// route, an id and a screenshot are all one name.
    pub enum ChartPage {
        Pipeline => "pipeline",
        Lines => "lines",
        LinesTarget => "lines-target",
        Area => "area",
        AreaStacked => "area-stacked",
        Bars => "bars",
        BarsGrouped => "bars-grouped",
        Bars100 => "bars-100",
        BarsHorizontal => "bars-horizontal",
        Pie => "pie",
        Scatter => "scatter",
        Heatmap => "heatmap",
        Time => "time",
        Sparkline => "sparkline",
    }
}

/// One page of the gallery: the route that opens it, its name, and how to build it.
#[derive(Clone, Copy)]
pub struct Entry {
    pub page: ChartPage,
    pub title: fn() -> day::LocalizedText,
    build: fn() -> AnyPiece,
}

impl Entry {
    /// The page as a host mounts it: an example under its own heading, or the pipeline page,
    /// which has its own layout.
    pub fn build(&self) -> AnyPiece {
        register();
        if self.page == ChartPage::Pipeline {
            return (self.build)();
        }
        column((
            label((self.title)())
                .font(Font::Headline)
                .id("charts-example-title"),
            (self.build)(),
        ))
        .spacing(8.0)
        .padding(16.0)
        .grow()
        .any()
    }
}

/// Every page, in order: the pipeline page, then the examples in the order the README documents
/// them.
pub fn pages() -> Vec<Entry> {
    let mut all = vec![Entry {
        page: ChartPage::Pipeline,
        title: res::str::nav_pipeline,
        build: || pipeline::pipeline_page().any(),
    }];
    all.extend(examples());
    all
}

/// The examples alone, without the pipeline page.
pub fn examples() -> Vec<Entry> {
    examples::gallery()
        .into_iter()
        .map(|e| Entry {
            page: e.page,
            title: e.title,
            build: e.build,
        })
        .collect()
}

/// The page for `page`, as [`Entry::build`] mounts it.
pub fn page(page: ChartPage) -> AnyPiece {
    pages()
        .into_iter()
        .find(|e| e.page == page)
        .map(|e| e.build())
        .unwrap_or_else(|| label("").any())
}

/// Every page as one: a picker of the pages above whichever is open, for a host that gives the
/// charts a single slot in its own navigation. `selected` is the open page, owned by the host
/// so it can remember or deep-link it.
pub fn gallery(selected: Signal<ChartPage>) -> impl Piece {
    register();
    let entries = pages();
    let names: Vec<String> = entries.iter().map(|e| (e.title)().format()).collect();
    let keys: Vec<ChartPage> = entries.iter().map(|e| e.page).collect();
    // The picker speaks indices; the host's signal speaks pages. The index is derived from the
    // page on every read, so a host that sets the page (a deep link) moves the picker too.
    let index = Signal::new(
        keys.iter()
            .position(|k| *k == selected.get_untracked())
            .unwrap_or(0),
    );
    {
        let keys = keys.clone();
        Effect::new(move || {
            let i = index.get();
            if let Some(k) = keys.get(i).copied()
                && untrack(|| selected.get()) != k
            {
                selected.set(k);
            }
        });
    }
    Effect::new(move || {
        let k = selected.get();
        if let Some(i) = keys.iter().position(|p| *p == k)
            && untrack(|| index.get()) != i
        {
            index.set(i);
        }
    });
    column((
        row((
            label(res::str::gallery_page()),
            picker(names, index).menu().id("charts-gallery-picker"),
        ))
        .spacing(8.0)
        .align(VAlign::Center)
        .padding(16.0),
        // One row keyed by the open page's route: a new page replaces the old one, so each page
        // starts from its own first state, and the old page's signals go with its scope.
        each(
            items(move || vec![selected.get()], |p: &ChartPage| p.key()),
            |slot| page(slot.get()),
        )
        .grow(),
    ))
    .grow()
}

/// Make this crate's strings resolvable by their qualified dayscript keys
/// (`day_piece_charts_gallery::ex_line`) without touching the app's locale. Idempotent; every
/// entry point calls it, so a host never has to.
pub fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(res::locales::register);
}
