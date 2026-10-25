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
//! Each page also exports (README "Export"): every chart on it registers with a
//! [`ChartExports`], each chart's context menu offers [`export_menu_items`], and a host puts the
//! same items in its own app menu. "Export as PNG" saves the page's charts as one image and
//! "Export as PDF" as one page per chart; each item is present everywhere and enabled where
//! the toolkit can render it ([`day::canvas_exports`]).
//!
//! Every string comes from this crate's private catalog (`resource/locales`), so the pages read
//! the same in any host without the host installing anything, and follow the host's locale. A
//! dayscript names them by their qualified key (`day_piece_charts_gallery::ex_line`); [`register`]
//! makes that key resolvable, and every entry point here calls it.

use day::prelude::*;
use day_piece_charts::ChartExports;

// Typed accessors for this crate's own strings: `res::str::ex_line()` and friends.
day_localization_ui::locales!();

/// The charts the crate's README documents.
mod examples;
mod interactions;
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
        Linked => "interactive-linked",
        Brush => "interactive-brush",
        Overview => "interactive-overview",
        Legend => "interactive-legend",
        Cells => "interactive-cells",
        Viewport => "interactive-viewport",
        Compose => "interactive-compose",
        Links => "interactive-links",
        Filtered => "interactive-filtered",
        Inputs => "interactive-inputs",
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
        let page = self.page;
        let build = self.build;
        // Every chart on the page registers here, for the export commands to find, and wears
        // the same commands as its context menu.
        let exports = ChartExports::new().context_menu(move || export_menu_items(move || page));
        remember_exports(page, exports.clone());
        let content = exports.around(build);
        if page == ChartPage::Pipeline {
            return column((export_status(), content)).grow().any();
        }
        column((
            label((self.title)())
                .font(Font::Headline)
                .id(day::id!(charts_example_title)),
            export_status(),
            content,
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
    all.extend(interactions::pages());
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
    // The picker is bound to the host's page signal itself, so a host that sets the page (a
    // deep link) moves the picker too.
    let options: Vec<(ChartPage, day::LocalizedText)> =
        pages().iter().map(|e| (e.page, (e.title)())).collect();
    let exports = day::canvas_exports();
    column((
        row((
            label(res::str::gallery_page()),
            picker(selected, options)
                .menu()
                .id(day::id!(charts_gallery_picker)),
        ))
        .spacing(8.0)
        .align(VAlign::Center)
        .padding(16.0),
        // The export commands as buttons, for a host that gives the gallery no menu of its own;
        // the charts' context menus carry the same two. Present everywhere, enabled where the
        // toolkit renders the format.
        row((
            button(res::str::export_png())
                .enabled(exports.png())
                .action(move || export_png(selected.get_untracked()))
                .id(day::id!(charts_gallery_export_png)),
            button(res::str::export_pdf())
                .enabled(exports.pdf)
                .action(move || export_pdf(selected.get_untracked()))
                .id(day::id!(charts_gallery_export_pdf)),
        ))
        .spacing(8.0)
        .padding(Insets {
            leading: 16.0,
            trailing: 16.0,
            ..Default::default()
        }),
        // One subtree keyed by the open page: a new page replaces the old one, so each page
        // starts from its own first state, and the old page's signals go with its scope.
        keyed(move || Some(selected.get()), |p| page(*p)).grow(),
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

// ---------------------------------------------------------------------------
// Export (README "Export")
// ---------------------------------------------------------------------------

thread_local! {
    /// The exports of each page that is built now, by its route, with the generation that
    /// put it there: a page built again (a nav that rebuilds on return) replaces its entry, and
    /// the old page's cleanup then leaves the new one alone.
    static EXPORTS: std::cell::RefCell<Vec<(ChartPage, u64, ChartExports)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn remember_exports(page: ChartPage, exports: ChartExports) {
    let generation = GENERATION.with(|g| {
        g.set(g.get() + 1);
        g.get()
    });
    EXPORTS.with(|m| {
        let mut m = m.borrow_mut();
        m.retain(|(p, _, _)| *p != page);
        m.push((page, generation, exports));
    });
    Scope::current().on_cleanup(move || {
        EXPORTS.with(|m| m.borrow_mut().retain(|(_, g, _)| *g != generation));
    });
}

fn exports_of(page: ChartPage) -> Option<ChartExports> {
    EXPORTS.with(|m| {
        m.borrow()
            .iter()
            .find(|(p, _, _)| *p == page)
            .map(|(_, _, e)| e.clone())
    })
}

/// What the last export did, app-wide, so every page's status line and every host's menu
/// report the same thing.
#[derive(Clone, Copy)]
struct ExportStatus(Signal<String>);

impl Ambient for ExportStatus {
    fn create() -> Self {
        ExportStatus(Signal::new(String::new()))
    }
}

fn status() -> Signal<String> {
    ExportStatus::app().0
}

/// The line under a page's title that says what the last export did.
fn export_status() -> impl Piece {
    label(move || status().get())
        .font(Font::Footnote)
        .id(day::id!(charts_export_status))
}

/// "Export as PNG" and "Export as PDF" for the page `page` names, as menu entries: for a host's
/// app menu (pass the open page's signal) and for each chart's context menu. Both are always
/// present; each is enabled where the toolkit renders that format ([`day::canvas_exports`]).
pub fn export_menu_items(page: impl Fn() -> ChartPage + Clone + 'static) -> Vec<MenuEntry> {
    let exports = day::canvas_exports();
    let png_page = page.clone();
    vec![
        menu_item(res::str::export_png())
            .id(day::id!(charts_export_png))
            .enabled(exports.png())
            .action(move || export_png(png_page())),
        menu_item(res::str::export_pdf())
            .id(day::id!(charts_export_pdf))
            .enabled(exports.pdf)
            .action(move || export_pdf(page())),
    ]
}

/// The pixels per point a PNG export renders at: a crisp image on a high-density screen, and a
/// size a document can scale down.
const PNG_SCALE: f64 = 2.0;

/// Render `page`'s charts to one PNG and offer to save it.
pub fn export_png(page: ChartPage) {
    let Some(exports) = exports_of(page) else {
        return;
    };
    day::task(async move {
        let name = format!("{}.png", page.key());
        let result = exports.png(PNG_SCALE).await;
        finish(result, name, "PNG", "png").await;
    });
}

/// Render `page`'s charts to a PDF, one page per chart, and offer to save it.
pub fn export_pdf(page: ChartPage) {
    let Some(exports) = exports_of(page) else {
        return;
    };
    let title = pages()
        .into_iter()
        .find(|e| e.page == page)
        .map(|e| (e.title)().format())
        .unwrap_or_default();
    day::task(async move {
        let name = format!("{}.pdf", page.key());
        let result = exports.pdf(&title).await;
        finish(result, name, "PDF", "pdf").await;
    });
}

async fn finish(
    result: Result<Vec<u8>, ExportError>,
    name: String,
    kind: &'static str,
    extension: &'static str,
) {
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(e) => {
            status().set(res::str::export_failed(e.to_string()).format());
            return;
        }
    };
    let saved = save_file(bytes)
        .suggested_name(name.clone())
        .filter(kind, &[extension])
        .await;
    status().set(match saved {
        Some(url) => res::str::export_saved(saved_name(&url, name)).format(),
        None => res::str::export_canceled().format(),
    });
}

/// The name the user saved under: the chosen file's own name, which a save dialog lets them
/// change, else the one suggested.
fn saved_name(url: &FileUrl, suggested: String) -> String {
    url.as_str()
        .rsplit(['/', '\\'])
        .next()
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .unwrap_or(suggested)
}
