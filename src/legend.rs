// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0
//! A native, interactive legend that can share its highlighted series with any chart.
use crate::interaction::{EventSource, Links, PointBinding, Record};
use day_core::{BuildCx, Piece, RNode};
use day_pieces::prelude::*;
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
    pub record: Option<Record>,
}
impl LegendEntry {
    pub fn new(key: impl Into<String>, label: impl Into<String>, color: Color) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            color,
            detail: None,
            link: None,
            record: None,
        }
    }
    /// Supply a semantic tuple for field-projected queries. Defaults to the series key.
    pub fn record(mut self, record: Record) -> Self {
        self.record = Some(record);
        self
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
pub struct Legend {
    entries: Entries,
    interaction: Option<PointBinding>,
    links: Links,
    id_prefix: String,
}
/// Supply labels and detail text already localized by the application. Entries without a link
/// remain plain text. Link activation defaults to Day's registered route / URL handling.
pub fn legend(entries: impl Fn() -> Vec<LegendEntry> + 'static) -> Legend {
    Legend {
        entries: Rc::new(entries),
        interaction: None,
        links: Links::new(),
        id_prefix: "chart-legend".into(),
    }
}
impl Legend {
    /// Bind this legend to the same projected point parameter used by charts.
    pub fn interact(mut self, interaction: PointBinding) -> Self {
        self.interaction = Some(interaction);
        self
    }
    /// Use the same registered link handler as a chart's `Links` binding.
    pub fn links(mut self, links: Links) -> Self {
        self.links = links;
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
        let interaction = self.interaction;
        let handler = self.links.handler;
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
                                if interaction.is_some_and(|binding| {
                                    !binding.parameter.is_empty()
                                        && binding.parameter.contains(&slot.field(|e| {
                                            e.record
                                                .clone()
                                                .unwrap_or_else(|| Record::series(e.key.clone()))
                                        }))
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
                        if interaction.is_some_and(|binding| {
                            !binding.parameter.is_empty()
                                && binding.parameter.contains(&slot.field(|e| {
                                    e.record
                                        .clone()
                                        .unwrap_or_else(|| Record::series(e.key.clone()))
                                }))
                        }) {
                            slot.field(|e| e.color).with_alpha(0.14)
                        } else {
                            Color::rgba(0.0, 0.0, 0.0, 0.0)
                        }
                    })
                    .corner_radius(5.0)
                    .on_hover(move |point| {
                        if let Some(binding) = interaction
                            && binding.trigger == EventSource::Hover
                        {
                            let record = slot.field(|e| {
                                e.record
                                    .clone()
                                    .unwrap_or_else(|| Record::series(e.key.clone()))
                            });
                            if point.is_some() {
                                binding.parameter.replace(Some(record));
                            } else if binding.parameter.contains(&record) {
                                binding.parameter.clear();
                            }
                        }
                    })
                };
                let plain_prefix = prefix.clone();
                when(
                    move || slot.field(|e| e.link.is_some()),
                    move || {
                        let handler = handler.clone();
                        content()
                            .cursor(Cursor::Pointer)
                            .on_tap(move || {
                                if let Some(binding) = interaction {
                                    binding.activate(
                                        Some(slot.field(|e| {
                                            e.record
                                                .clone()
                                                .unwrap_or_else(|| Record::series(e.key.clone()))
                                        })),
                                        day_core::modifiers().shift,
                                    );
                                }
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
                .otherwise(move || {
                    content()
                        .on_tap(move || {
                            if let Some(binding) = interaction {
                                binding.activate(
                                    Some(slot.field(|e| {
                                        e.record
                                            .clone()
                                            .unwrap_or_else(|| Record::series(e.key.clone()))
                                    })),
                                    day_core::modifiers().shift,
                                );
                            }
                        })
                        .a11y(move |b| {
                            b.role(if interaction.is_some() {
                                Role::Button
                            } else {
                                Role::None
                            })
                        })
                        .cursor(move || {
                            if interaction.is_some() {
                                Cursor::Pointer
                            } else {
                                Cursor::Default
                            }
                        })
                        .id_of({
                            let prefix = plain_prefix.clone();
                            move || format!("{prefix}-{}", slot.field(|e| e.key.clone()))
                        })
                })
            },
        ),))
        .spacing(0.0)
        .grow_w()
        .build(cx)
    }
}
