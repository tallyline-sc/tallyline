// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded selection and borrowed documents for the offline licence catalogue.

use crate::api::Catalogue;
use crate::api::model::{CopyrightInformation, LicenceCatalogue, LicenceComponent};
use tallyline_egui::TypeStyle;

const COMPONENT_ROW_HEIGHT: f32 = 40.0;
const COMPONENT_LIST_HEIGHT: f32 = COMPONENT_ROW_HEIGHT * 3.0;
const SELECTION_MARK_WIDTH: f32 = 4.0;

#[derive(Default)]
pub(crate) struct LicenceBrowser {
    selected: usize,
    notice: usize,
    viewer: crate::licence_text_viewer::LicenceTextViewer,
}

impl LicenceBrowser {
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        messages: &Catalogue,
        catalogue: &LicenceCatalogue,
        palette: &tallyline_egui::tokens::ThemeColors,
    ) {
        let components = catalogue.components();
        self.selected = self.selected.min(components.len().saturating_sub(1));
        let focus_document = self.show_list(ui, messages, components, palette);
        ui.separator();
        let Some(component) = components.get(self.selected) else {
            return;
        };
        egui::ScrollArea::vertical()
            .id_salt("component_details")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                show_metadata(ui, messages, component);
                self.notice = self.notice.min(component.notices.len().saturating_sub(1));
                if let Some(notice) = component.notices.get(self.notice) {
                    egui::ComboBox::new("licence_document", messages.text("about.licence_file"))
                        .selected_text(&notice.path)
                        .show_ui(ui, |ui| {
                            for (index, notice) in component.notices.iter().enumerate() {
                                ui.selectable_value(&mut self.notice, index, &notice.path);
                            }
                        });
                }
                if let Some(notice) = component.notices.get(self.notice) {
                    ui.push_id((self.selected, self.notice), |ui| {
                        let response = self.viewer.show(ui, messages, &notice.text);
                        if focus_document {
                            response.request_focus();
                            ui.scroll_to_rect(
                                egui::Rect::from_min_size(
                                    response.rect.min,
                                    egui::vec2(response.rect.width(), COMPONENT_ROW_HEIGHT),
                                ),
                                Some(egui::Align::Min),
                            );
                        }
                    });
                }
            });
    }

    fn show_list(
        &mut self,
        ui: &mut egui::Ui,
        messages: &Catalogue,
        components: &[LicenceComponent],
        palette: &tallyline_egui::tokens::ThemeColors,
    ) -> bool {
        let label = ui.label(TypeStyle::BODY_STRONG.text(messages.text("about.components")));
        let list_id = ui.make_persistent_id("shipped_components");
        let old_selection = self.selected;
        let mut focus_document = false;
        let mut previous_focus = false;
        if ui.memory(|memory| memory.has_focus(list_id)) {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    list_id,
                    egui::EventFilter {
                        tab: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            ui.input_mut(|input| {
                previous_focus = input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
                if input.modifiers != egui::Modifiers::NONE {
                    return;
                }
                focus_document = input.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    self.selected = (self.selected + 1).min(components.len().saturating_sub(1));
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    self.selected = self.selected.saturating_sub(1);
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                    self.selected = 0;
                }
                if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                    self.selected = components.len().saturating_sub(1);
                }
            });
        }
        if previous_focus {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::Previous));
        }
        let list_height =
            (ui.available_height() / 3.0).clamp(COMPONENT_ROW_HEIGHT, COMPONENT_LIST_HEIGHT);
        let row_height = COMPONENT_ROW_HEIGHT - ui.spacing().item_spacing.y;
        let mut area = egui::ScrollArea::vertical()
            .id_salt("components_scroll")
            .max_height(list_height)
            .min_scrolled_height(list_height)
            .auto_shrink([false, false]);
        if old_selection != self.selected {
            area = area.vertical_scroll_offset(self.selected as f32 * COMPONENT_ROW_HEIGHT);
        }
        let selection = palette
            .color("selection-row-bg")
            .unwrap_or(ui.visuals().selection.bg_fill);
        let mark = palette
            .color("selection-row-mark")
            .unwrap_or(ui.visuals().hyperlink_color);
        let output = ui
            .scope(|ui| {
                ui.visuals_mut().selection.bg_fill = selection;
                area.show_rows(ui, row_height, components.len(), |ui, rows| {
                    for (index, component) in components
                        .iter()
                        .enumerate()
                        .skip(rows.start)
                        .take(rows.len())
                    {
                        let response = ui.add_sized(
                            [ui.available_width(), row_height],
                            egui::Button::selectable(
                                self.selected == index,
                                TypeStyle::BODY
                                    .text(format!("{} {}", component.name, component.version)),
                            )
                            .truncate()
                            .sense(egui::Sense::CLICK),
                        );
                        if self.selected == index {
                            let bar = egui::Rect::from_min_size(
                                response.rect.min,
                                egui::vec2(SELECTION_MARK_WIDTH, response.rect.height()),
                            );
                            ui.painter().rect_filled(bar, 0.0, mark);
                        }
                        if response.clicked() {
                            self.selected = index;
                            ui.memory_mut(|memory| memory.request_focus(list_id));
                        }
                    }
                })
            })
            .inner;
        let response = ui
            .interact(
                output.inner_rect,
                list_id,
                egui::Sense::focusable_noninteractive(),
            )
            .labelled_by(label.id);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::ListBox, true, messages.text("about.components"))
        });
        ui.ctx().accesskit_node_builder(list_id, |node| {
            node.set_role(egui::accesskit::Role::ListBox);
        });
        if response.has_focus() {
            ui.painter().rect_stroke(
                output.inner_rect.shrink(1.0),
                ui.visuals().widgets.active.corner_radius,
                ui.visuals().widgets.active.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        if old_selection != self.selected {
            self.notice = 0;
        }
        focus_document
    }
}

fn show_metadata(ui: &mut egui::Ui, messages: &Catalogue, component: &LicenceComponent) {
    ui.label(TypeStyle::HEADING_3.text(format!("{} {}", component.name, component.version)));
    ui.label(&component.licence);
    if let Some(declared) = &component.declared_licence {
        ui.label(TypeStyle::BODY_STRONG.text(messages.text("about.declared_licence")));
        ui.label(declared);
    }
    match &component.copyright {
        CopyrightInformation::Provided { notices } => {
            for line in notices {
                ui.label(line);
            }
        }
        CopyrightInformation::NotStatedByUpstream => {
            ui.label(messages.text("about.no_copyright_stated"));
        }
    }
    ui.hyperlink_to(messages.text("about.source"), &component.source);
}
