// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Honest token-quota reset checks without promised dispatch times.

use crate::api::Catalogue;
use tallyline_shared_kernel::api::model::Timestamp;
pub(crate) fn show(ui: &mut egui::Ui, reset: Option<Timestamp>, catalogue: &Catalogue) {
    match reset {
        Some(reset) => {
            ui.label(format!(
                "{} {}",
                catalogue.text("queue.quota_reset_check"),
                crate::attachment_views::format_time(reset)
            ));
        }
        None => {
            ui.label(catalogue.text("queue.quota_reset_unavailable"));
        }
    }
    ui.label(catalogue.text("queue.quota_full_gate"));
}
