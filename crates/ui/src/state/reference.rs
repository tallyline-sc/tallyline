// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference browsing transitions through the shell application boundary.

use super::{ShellState, View};

impl ShellState {
    /// Returns user-owned reference browsing state.
    #[must_use]
    pub const fn references(&self) -> &crate::api::model::ReferenceBrowser {
        &self.references
    }

    /// Returns editable reference browsing state.
    pub fn references_mut(&mut self) -> &mut crate::api::model::ReferenceBrowser {
        &mut self.references
    }

    /// Returns the snapshot selected through the durable game-channel mapping.
    #[must_use]
    pub fn reference_snapshot(
        &self,
    ) -> Option<&tallyline_reference_data::api::model::ReferenceSnapshot> {
        self.references.snapshot(
            self.game_witness
                .settings
                .environment_mapping
                .environment(self.references.channel()),
        )
    }

    /// Opens the reference browser in direct response to a user action.
    pub fn show_reference(&mut self) {
        self.view = View::Reference;
    }

    /// Selects only a terminal present in the mapped vocabulary and requests its prices.
    pub fn open_reference_terminal(
        &mut self,
        id: tallyline_shared_kernel::api::model::TerminalId,
    ) -> bool {
        if self
            .reference_snapshot()
            .is_some_and(|snapshot| snapshot.terminals("").iter().any(|item| item.id() == id))
        {
            self.references.open_terminal(id);
            true
        } else {
            false
        }
    }

    /// Selects only a commodity present in the mapped vocabulary.
    pub fn select_reference_commodity(
        &mut self,
        id: tallyline_shared_kernel::api::model::CommodityId,
    ) -> bool {
        if self
            .reference_snapshot()
            .is_some_and(|snapshot| snapshot.commodities("").iter().any(|item| item.id() == id))
        {
            self.references.select_commodity(id);
            true
        } else {
            false
        }
    }

    /// Retains cached browsing data and displays a failed command delivery.
    pub fn reference_command_failed(&mut self, notice: String) {
        self.references.unavailable();
        self.reference_notice = Some(notice);
    }
}
