// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Full-size private region review and explicit local attachment actions.

use crate::api::model::{ManualAttachmentAction, ManualAttachmentCommand, ManualCommand};
use crate::api::{Catalogue, ShellState};
use tallyline_reporting::api::model::{AttachmentRegion, AttachmentTime};
use tallyline_reporting::api::{AttachmentEdit, RepositoryError};
use tallyline_shared_kernel::api::model::Timestamp;
use tallyline_submission::api::model::{AccountState, ScreenshotPolicy};
use tallyline_workflows::api::{
    ManualAttachmentBlock, ManualAttachmentFailure, ManualAttachmentInputFailure,
    WorkingCopyFailure,
};

/// Draws explicit source selection, source-scale private preview, time and dated account policy.
/// Image decoding, persistence and release-related evidence checks run only on the bounded app worker.
pub fn show_manual_attachment_editor(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    account: &AccountState,
) {
    let Some(report) = state.manual().selected().cloned() else {
        return;
    };
    ui.separator();
    ui.heading(catalogue.text("attachment.title"));
    ui.label(catalogue.text("attachment.instructions"));
    let stale = state
        .manual()
        .attachment
        .preview
        .as_ref()
        .is_some_and(|preview| {
            state.manual().attachment.preview_report != Some(report.id())
                || report
                    .attachment()
                    .is_none_or(|attachment| attachment.binding() != preview.binding())
        });
    if stale {
        state.manual_mut().attachment.clear_preview();
    }
    if state
        .manual()
        .attachment
        .policy
        .as_ref()
        .is_none_or(|(identity, _)| identity != account)
    {
        state.manual_mut().attachment.readiness = None;
    }
    policy(ui, state, catalogue, account);
    if let Some(failure) = state.manual().attachment.failure {
        ui.label(catalogue.text(failure_key(failure)));
    }
    if let Some(readiness) = state.manual().attachment.readiness {
        ui.label(catalogue.text(match readiness {
            Ok(()) => "attachment.ready",
            Err(ManualAttachmentBlock::PolicyUnavailable) => "attachment.policy_unavailable",
            Err(ManualAttachmentBlock::ImageRequired) => "attachment.required_missing",
            Err(ManualAttachmentBlock::PreviewUnconfirmed) => "attachment.preview_unconfirmed",
            Err(ManualAttachmentBlock::TimeUncertain) => "attachment.time_uncertain",
            Err(ManualAttachmentBlock::WorkingCopyMissingOrChanged) => {
                "attachment.missing_or_changed"
            }
        }));
    }
    ui.add_enabled_ui(!state.manual().busy, |ui| {
        select(ui, state, catalogue);
        if let Some(attachment) = report.attachment() {
            ui.label(format!(
                "{} {}",
                catalogue.text("attachment.observed"),
                format_time(report.observed_at())
            ));
            ui.label(catalogue.text(match attachment.time() {
                AttachmentTime::ImportedFile(_) => "attachment.time_uncertain",
                AttachmentTime::User(_) => "attachment.time_user",
            }));
            let mut edit = None;
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(tallyline_egui::button_text(
                        catalogue.text("attachment.view"),
                    ))
                    .clicked()
                {
                    edit = Some(ManualAttachmentAction::View);
                }
                if ui
                    .button(tallyline_egui::button_text(
                        catalogue.text("attachment.remove"),
                    ))
                    .clicked()
                {
                    edit = Some(ManualAttachmentAction::Edit(AttachmentEdit::Remove));
                }
            });
            if let Some(edit) = edit {
                queue(state, edit);
            }
            ui.label(catalogue.text("attachment.time_input"));
            let label = catalogue.text("attachment.time_input");
            ui.add(
                egui::TextEdit::singleline(&mut state.manual_mut().attachment.time)
                    .id_salt("attachment-time"),
            )
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
            if ui
                .button(tallyline_egui::button_text(
                    catalogue.text("attachment.confirm_time"),
                ))
                .clicked()
            {
                match state.manual().attachment.time.trim().parse::<u64>() {
                    Ok(seconds) if Timestamp::from_seconds(seconds) <= report.observed_at() => {
                        queue(
                            state,
                            ManualAttachmentAction::Edit(AttachmentEdit::ConfirmTime(
                                Timestamp::from_seconds(seconds),
                            )),
                        )
                    }
                    Ok(_) | Err(_) => {
                        state.manual_mut().notice =
                            Some(catalogue.text("attachment.invalid_time").to_owned())
                    }
                }
            }
            preview(ui, state, catalogue);
        }
    });
}
fn policy(
    ui: &mut egui::Ui,
    state: &mut ShellState,
    catalogue: &Catalogue,
    account: &AccountState,
) {
    let policy = state
        .manual()
        .attachment
        .policy
        .as_ref()
        .filter(|(identity, _)| identity == account)
        .map(|(_, policy)| *policy);
    match policy {
        Some(Ok(ScreenshotPolicy::Required { observed_at })) => {
            ui.label(format!(
                "{} {}",
                catalogue.text("attachment.required_since"),
                format_time(observed_at)
            ));
            if ui
                .add_enabled(
                    !state.manual().busy,
                    egui::Button::new(tallyline_egui::button_text(
                        catalogue.text("attachment.try_without"),
                    )),
                )
                .clicked()
            {
                queue(
                    state,
                    ManualAttachmentAction::TryWithoutScreenshot(account.clone()),
                );
            }
        }
        Some(Err(_)) => {
            ui.label(catalogue.text("attachment.policy_unavailable"));
        }
        Some(Ok(ScreenshotPolicy::Unknown)) | None => {
            ui.label(catalogue.text("attachment.policy_unknown"));
        }
    }
    if ui
        .add_enabled(
            !state.manual().busy,
            egui::Button::new(tallyline_egui::button_text(
                catalogue.text("attachment.check"),
            )),
        )
        .clicked()
    {
        queue(
            state,
            ManualAttachmentAction::RefreshPolicy(account.clone()),
        );
    }
    if policy.is_none() && matches!(account, AccountState::Validated { .. }) && !state.manual().busy
    {
        queue(
            state,
            ManualAttachmentAction::RefreshPolicy(account.clone()),
        );
    }
}
fn select(ui: &mut egui::Ui, state: &mut ShellState, catalogue: &Catalogue) {
    for dropped in ui.input(|input| input.raw.dropped_files.clone()) {
        state.manual_mut().attachment.source = dropped.path().to_string_lossy().into_owned();
    }
    let label = catalogue.text("attachment.source");
    ui.label(label);
    ui.add(
        egui::TextEdit::singleline(&mut state.manual_mut().attachment.source)
            .id_salt("attachment-source"),
    )
    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, label));
    ui.horizontal_wrapped(|ui| {
        for (index, key) in [
            "attachment.x",
            "attachment.y",
            "attachment.width",
            "attachment.height",
        ]
        .into_iter()
        .enumerate()
        {
            ui.label(catalogue.text(key));
            if let Some(value) = state.manual_mut().attachment.rectangle.get_mut(index) {
                ui.add(egui::DragValue::new(value)).widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::SpinButton, true, catalogue.text(key))
                });
            }
        }
    });
    let [x, y, width, height] = state.manual().attachment.rectangle;
    let region = AttachmentRegion::new(x, y, width, height).ok();
    if ui
        .add_enabled(
            region.is_some() && !state.manual().attachment.source.trim().is_empty(),
            egui::Button::new(tallyline_egui::button_text(
                catalogue.text("attachment.prepare"),
            )),
        )
        .clicked()
        && let Some(region) = region
    {
        queue(
            state,
            ManualAttachmentAction::Prepare {
                source: state.manual().attachment.source.clone(),
                region,
            },
        );
    }
}
fn preview(ui: &mut egui::Ui, state: &mut ShellState, catalogue: &Catalogue) {
    let Some(preview) = state.manual().attachment.preview.as_ref() else {
        return;
    };
    if state.manual().attachment.texture.is_none() {
        let raster = preview.raster();
        let pixels = raster
            .argb()
            .iter()
            .map(|pixel| {
                let [a, r, g, b] = pixel.to_be_bytes();
                egui::Color32::from_rgba_unmultiplied(r, g, b, a)
            })
            .collect();
        let image =
            egui::ColorImage::new([raster.width() as usize, raster.height() as usize], pixels);
        state.manual_mut().attachment.texture = Some(ui.ctx().load_texture(
            "manual-private-preview",
            image,
            egui::TextureOptions::NEAREST,
        ));
    }
    ui.label(catalogue.text("attachment.full_size"));
    if let Some(texture) = &state.manual().attachment.texture {
        egui::ScrollArea::both()
            .id_salt("private-preview-scroll")
            .max_height(260.0)
            .show(ui, |ui| {
                ui.add(
                    egui::Image::new((texture.id(), texture.size_vec2()))
                        .alt_text(catalogue.text("attachment.preview_alt")),
                );
            });
    }
    if ui
        .button(tallyline_egui::button_text(
            catalogue.text("attachment.confirm_preview"),
        ))
        .clicked()
        && let Some(preview) = state.manual().attachment.preview.clone()
    {
        queue(state, ManualAttachmentAction::ConfirmPreview(preview));
    }
}
fn queue(state: &mut ShellState, action: ManualAttachmentAction) {
    if let Some(report) = state.manual().selected() {
        let command = ManualAttachmentCommand {
            id: report.id(),
            expected: report.version(),
            action,
        };
        state.manual_mut().queue(ManualCommand::Attachment(command));
    }
}
fn failure_key(failure: ManualAttachmentFailure) -> &'static str {
    match failure {
        ManualAttachmentFailure::Report(RepositoryError::Conflict) => "attachment.conflict",
        ManualAttachmentFailure::Report(RepositoryError::Invalid) => "attachment.invalid_region",
        ManualAttachmentFailure::Report(RepositoryError::AssessmentUnavailable) => {
            "review.precision_unavailable"
        }
        ManualAttachmentFailure::Report(RepositoryError::Unavailable)
        | ManualAttachmentFailure::WorkingCopy(WorkingCopyFailure::Unavailable) => {
            "attachment.storage_unavailable"
        }
        ManualAttachmentFailure::WorkingCopy(WorkingCopyFailure::MissingOrChanged) => {
            "attachment.missing_or_changed"
        }
        ManualAttachmentFailure::WorkingCopy(WorkingCopyFailure::Unconfirmed) => {
            "attachment.preview_unconfirmed"
        }
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::Hdr) => "attachment.hdr",
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::Unsupported) => {
            "attachment.unsupported"
        }
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::InputLimit) => {
            "attachment.input_limit"
        }
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::SourceReduced) => {
            "attachment.reduced"
        }
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::InvalidRegion) => {
            "attachment.invalid_region"
        }
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::Unavailable) => {
            "attachment.source_unavailable"
        }
        ManualAttachmentFailure::Input(ManualAttachmentInputFailure::EncodingLimit) => {
            "attachment.encoding_limit"
        }
    }
}
pub(crate) fn format_time(at: Timestamp) -> String {
    let seconds = u128::from(at.seconds());
    let days = seconds / 86_400 + 135_140;
    let mut year = 1600 + days / 146_097 * 400;
    let mut day = days % 146_097;
    loop {
        let length = if leap(year) { 366 } else { 365 };
        if day < length {
            break;
        }
        day -= length;
        year += 1;
    }
    let mut month = 1;
    for length in [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if day < length {
            break;
        }
        day -= length;
        month += 1;
    }
    format!(
        "{year:04}-{month:02}-{:02} {:02}:{:02}:{:02} UTC",
        day + 1,
        seconds % 86_400 / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}
fn leap(year: u128) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}
