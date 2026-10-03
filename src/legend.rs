// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
//! A native, interactive legend that can share its highlighted series with any chart.
use day_core::{BuildCx, Piece, RNode};
use day_pieces::prelude::*;
use day_reactive::Signal;
use day_spec::{Color, Cursor, Font, Role};
use std::rc::Rc;

/// Stable series identity is separate from its localized or user-supplied display label.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendEntry {
    pub key: String,
    pub label: String,
    pub color: Color,
    pub detail: Option<String>,
    pub link: Option<String>,
}
impl LegendEntry {
    pub fn new(key: impl Into<String>, label: impl Into<String>, color: Color) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            color,
            detail: None,
            link: None,
        }
    }
    pub fn detail(mut self, text: impl Into<String>) -> Self {
        self.detail = Some(text.into());
        self
    }
    pub fn link(mut self, target: impl Into<String>) -> Self {
        self.link = Some(target.into());
        self
    }
}
type Entries = Rc<dyn Fn() -> Vec<LegendEntry>>;
type LinkHandler = Rc<dyn Fn(&str)>;
pub struct Legend {
    entries: Entries,
    highlight: Option<Signal<Option<String>>>,
    on_link: Option<LinkHandler>,
    id_prefix: String,
}
/// Supply labels and detail text already localized by the application. Entries without a link
/// remain plain text. Link activation defaults to Day's registered route / URL handling.
pub fn legend(entries: impl Fn() -> Vec<LegendEntry> + 'static) -> Legend {
    Legend {
        entries: Rc::new(entries),
        highlight: None,
        on_link: None,
        id_prefix: "chart-legend".into(),
    }
}
impl Legend {
    /// Share with `Chart::highlight_series` to emphasize the matching marks on hover.
    pub fn highlight(mut self, signal: Signal<Option<String>>) -> Self {
        self.highlight = Some(signal);
        self
    }
    /// Override link handling, e.g. select an app object before navigating to its registered route.
    pub fn on_link(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }
    /// Each entry is addressable as `<prefix>-<stable key>` in accessibility and DayScript.
    pub fn id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.id_prefix = prefix.into();
        self
    }
}
impl Piece for Legend {
    fn build(self, cx: &mut BuildCx) -> RNode {
        let entries = self.entries;
        let highlighted = self.highlight.unwrap_or_else(|| Signal::new(None));
        let handler = self.on_link;
        let prefix = self.id_prefix;
        column((each(
            items(move || entries(), |entry| entry.key.clone()),
            move |slot| {
                let handler = handler.clone();
                let prefix = prefix.clone();
                let content = move || {
                    row((
                        circle()
                            .fill(move || slot.field(|e| e.color))
                            .frame(6.0, 6.0),
                        label(move || slot.field(|e| e.label.clone()))
                            .font(Font::Caption2)
                            .single_line()
                            .grow_w()
                            .color(move || {
                                if highlighted.with(|h| {
                                    h.as_deref() == Some(slot.field(|e| e.key.clone()).as_str())
                                }) {
                                    slot.field(|e| e.color)
                                } else {
                                    crate::Chrome::for_dark(day_core::dark_mode()).label
                                }
                            }),
                        label(move || slot.field(|e| e.detail.clone().unwrap_or_default()))
                            .font(Font::Caption2)
                            .single_line()
                            .color(move || crate::Chrome::for_dark(day_core::dark_mode()).label),
                    ))
                    .spacing(5.0)
                    .align(VAlign::Center)
                    .grow_w()
                    .padding(2.0)
                    .background(move || {
                        if highlighted
                            .with(|h| h.as_deref() == Some(slot.field(|e| e.key.clone()).as_str()))
                        {
                            slot.field(|e| e.color).with_alpha(0.14)
                        } else {
                            Color::rgba(0.0, 0.0, 0.0, 0.0)
                        }
                    })
                    .corner_radius(5.0)
                    .on_hover(move |point| {
                        let key = slot.field(|e| e.key.clone());
                        if point.is_some() {
                            highlighted.set_if_changed(Some(key));
                        } else if highlighted.with_untracked(|h| h.as_deref() == Some(key.as_str()))
                        {
                            highlighted.set_if_changed(None);
                        }
                    })
                };
                when(
                    move || slot.field(|e| e.link.is_some()),
                    move || {
                        let handler = handler.clone();
                        content()
                            .cursor(Cursor::Pointer)
                            .on_tap(move || {
                                if let Some(target) = slot.field(|e| e.link.clone()) {
                                    if let Some(handler) = &handler {
                                        handler(&target);
                                    } else {
                                        open_link(&target);
                                    }
                                }
                            })
                            .a11y(|b| b.role(Role::Button))
                            .id_of({
                                let prefix = prefix.clone();
                                move || format!("{prefix}-{}", slot.field(|e| e.key.clone()))
                            })
                    },
                )
                .otherwise(content)
            },
        ),))
        .spacing(0.0)
        .grow_w()
        .build(cx)
    }
}
