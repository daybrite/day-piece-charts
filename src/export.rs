// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Export (README "Export"): the charts an app shows, rendered to PNG or PDF through Day's canvas
//! export, which replays the drawing with the toolkit's own painter.
//!
//! A [`ChartExports`] collects every [`Chart`](crate::Chart) built inside it. The app places one
//! around a page, then asks it for [`ChartExports::png`] or [`ChartExports::pdf`] from a menu
//! command, and can hand it the context menu each chart's canvas shows.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use day_core::Piece;
use day_geometry::Affine;
use day_pieces::{Draw, Drawing, MenuEntry, PdfDocument};
use day_spec::{Color, ExportError, Rect, Shape, Size};

/// What a registered chart draws for export: the drawing at a size, in light or dark chrome.
pub(crate) type StillFn = Rc<dyn Fn(&mut Draw, Size, bool)>;

/// The context menu a chart under these exports shows.
type MenuFn = Rc<dyn Fn() -> Vec<MenuEntry>>;

/// The size a chart that never drew records at: a common chart proportion, large enough for the
/// axis labels to keep their room.
const FALLBACK_SIZE: Size = Size::new(640.0, 400.0);

/// The space between charts stacked into one PNG.
const STACK_GAP: f64 = 24.0;

/// The charts built inside it, kept so an app can export what is on screen.
///
/// ```ignore
/// let exports = ChartExports::new().context_menu(move || export_menu_items());
/// exports.around(|| column((chart(…), chart(…))))
/// // later, from a menu command:
/// day::task(async move {
///     if let Some(png) = exports.png(2.0).await { /* save it */ }
/// });
/// ```
///
/// An export is a still: each chart is drawn at the size it last had on screen, at its final
/// state with no transition in flight and no pointer guides, in light chrome on white, the way a
/// chart reads on a printed page or in a document. A chart with its own [`Chrome`](crate::Chrome)
/// keeps it.
#[derive(Clone, Default)]
pub struct ChartExports(Rc<Inner>);

#[derive(Default)]
struct Inner {
    charts: RefCell<Vec<Registered>>,
    next: Cell<u64>,
    menu: RefCell<Option<MenuFn>>,
}

struct Registered {
    key: u64,
    shown: Rc<Cell<Option<Size>>>,
    draw: StillFn,
}

impl ChartExports {
    /// Exports with no charts yet; they register as they build inside [`ChartExports::around`].
    pub fn new() -> Self {
        Self::default()
    }

    /// The context menu every chart under these exports shows on secondary-click or long-press:
    /// typically the app's own "Export as PNG" and "Export as PDF" commands. Built when the menu
    /// is summoned, so `enabled` states are current.
    pub fn context_menu(self, items: impl Fn() -> Vec<MenuEntry> + 'static) -> Self {
        *self.0.menu.borrow_mut() = Some(Rc::new(items));
        self
    }

    /// `content`, with every chart built inside it registered here.
    pub fn around<P: Piece>(&self, content: impl FnOnce() -> P + 'static) -> impl Piece {
        day_core::with_environment(self.clone(), content)
    }

    /// How many charts are registered now.
    pub fn len(&self) -> usize {
        self.0.charts.borrow().len()
    }

    /// Whether no chart is registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One drawing per chart, each at the size it last drew on screen, on white.
    pub fn drawings(&self) -> Vec<Drawing> {
        self.0
            .charts
            .borrow()
            .iter()
            .map(|c| {
                let size = c.shown.get().unwrap_or(FALLBACK_SIZE);
                let draw = c.draw.clone();
                Drawing::record(size, move |d, size| {
                    d.fill(Shape::Rect(Rect::from_size(size)), Color::WHITE);
                    draw(d, size, false);
                })
            })
            .collect()
    }

    /// Every chart in one drawing, stacked top to bottom in build order on one white sheet, the
    /// shape a PNG of a page with several charts takes. `None` when no chart is registered.
    pub fn drawing(&self) -> Option<Drawing> {
        let charts = self.0.charts.borrow();
        if charts.is_empty() {
            return None;
        }
        let sizes: Vec<Size> = charts
            .iter()
            .map(|c| c.shown.get().unwrap_or(FALLBACK_SIZE))
            .collect();
        let width = sizes.iter().map(|s| s.width).fold(0.0, f64::max);
        let height =
            sizes.iter().map(|s| s.height).sum::<f64>() + STACK_GAP * (sizes.len() - 1) as f64;
        let draws: Vec<StillFn> = charts.iter().map(|c| c.draw.clone()).collect();
        Some(Drawing::record(
            Size::new(width, height),
            move |d, total| {
                d.fill(Shape::Rect(Rect::from_size(total)), Color::WHITE);
                let mut y = 0.0;
                for (draw, size) in draws.iter().zip(&sizes) {
                    d.transformed(Affine::translate(0.0, y), |d| {
                        d.clipped(Shape::Rect(Rect::from_size(*size)), |d| {
                            draw(d, *size, false)
                        });
                    });
                    y += size.height + STACK_GAP;
                }
            },
        ))
    }

    /// The charts as one PNG at `scale` pixels per point (see [`ChartExports::drawing`]).
    /// `Err(ExportError::Invalid)` when no chart is registered.
    pub async fn png(&self, scale: f64) -> Result<Vec<u8>, ExportError> {
        let drawing = self.drawing().ok_or(ExportError::Invalid)?;
        drawing.to_png(scale).await
    }

    /// The charts as one PDF, one page per chart at the chart's size, with `title` as the
    /// document title where the platform's writer records one.
    pub async fn pdf(&self, title: &str) -> Result<Vec<u8>, ExportError> {
        let mut doc = PdfDocument::new().title(title);
        for drawing in self.drawings() {
            doc = doc.page(drawing);
        }
        doc.render().await
    }

    /// Register a chart for as long as the scope building it lives.
    pub(crate) fn register(&self, shown: Rc<Cell<Option<Size>>>, draw: StillFn) {
        let key = self.0.next.get();
        self.0.next.set(key + 1);
        self.0
            .charts
            .borrow_mut()
            .push(Registered { key, shown, draw });
        let inner = self.0.clone();
        day_reactive::Scope::current().on_cleanup(move || {
            inner.charts.borrow_mut().retain(|c| c.key != key);
        });
    }

    pub(crate) fn context_menu_items(&self) -> Option<MenuFn> {
        self.0.menu.borrow().clone()
    }
}
