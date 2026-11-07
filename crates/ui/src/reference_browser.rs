// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! User-owned browsing choices and passive reference snapshots.

use tallyline_game::api::model::GameEnvironment;
use tallyline_reference_data::api::model::{ReferenceSnapshot, UexEnvironment};
use tallyline_shared_kernel::api::model::{CommodityId, TerminalId, Timestamp};

/// An explicit reference-data request handled outside the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceCommand {
    /// Publish committed local projection with cached records, without requesting network data.
    PublishCached,
    /// Request refreshed public reference data.
    Refresh,
    /// Request the verified terminal price endpoint for this UEX identity.
    OpenTerminal(TerminalId),
}

/// Last non-JSON reference response retained as passive, memory-only diagnostic metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceDiagnostic {
    /// The response's HTTP status.
    pub http_status: u16,
    /// Validated bounded Cloudflare Ray metadata, if available.
    pub cf_ray: Option<String>,
    /// Injected worker observation time, distinct from successful cache freshness.
    pub observed_at: Timestamp,
}

/// Browsing choices retained independently of reference refreshes.
pub struct ReferenceBrowser {
    channel: GameEnvironment,
    terminal_query: String,
    commodity_query: String,
    selected_terminal: Option<TerminalId>,
    selected_commodity: Option<CommodityId>,
    live: Option<ReferenceSnapshot>,
    ptu: Option<ReferenceSnapshot>,
    observed_at: Option<Timestamp>,
    refreshing: bool,
    diagnostic: Option<ReferenceDiagnostic>,
    command: Option<ReferenceCommand>,
}

impl Default for ReferenceBrowser {
    fn default() -> Self {
        Self {
            channel: GameEnvironment::Live,
            terminal_query: String::new(),
            commodity_query: String::new(),
            selected_terminal: None,
            selected_commodity: None,
            live: None,
            ptu: None,
            observed_at: None,
            refreshing: false,
            diagnostic: None,
            command: None,
        }
    }
}

impl ReferenceBrowser {
    /// Returns the explicitly selected game channel.
    #[must_use]
    pub const fn channel(&self) -> GameEnvironment {
        self.channel
    }

    /// Selects a channel without changing witness permission or durable mapping.
    pub fn choose_channel(&mut self, channel: GameEnvironment) {
        if self.channel != channel {
            self.channel = channel;
            self.selected_terminal = None;
            self.selected_commodity = None;
        }
    }

    /// Returns the editable terminal search.
    pub fn terminal_query(&mut self) -> &mut String {
        &mut self.terminal_query
    }

    /// Returns the editable commodity search.
    pub fn commodity_query(&mut self) -> &mut String {
        &mut self.commodity_query
    }

    /// Returns the terminal search text.
    #[must_use]
    pub fn terminal_search(&self) -> &str {
        &self.terminal_query
    }

    /// Returns the commodity search text.
    #[must_use]
    pub fn commodity_search(&self) -> &str {
        &self.commodity_query
    }

    /// Returns the user's selected terminal identity.
    #[must_use]
    pub const fn selected_terminal(&self) -> Option<TerminalId> {
        self.selected_terminal
    }

    /// Returns the user's selected commodity identity.
    #[must_use]
    pub const fn selected_commodity(&self) -> Option<CommodityId> {
        self.selected_commodity
    }

    /// Returns the last non-JSON response, including after a later successful refresh.
    #[must_use]
    pub const fn diagnostic(&self) -> Option<&ReferenceDiagnostic> {
        self.diagnostic.as_ref()
    }

    pub(crate) fn report_diagnostic(&mut self, diagnostic: ReferenceDiagnostic) {
        self.diagnostic = Some(diagnostic);
    }

    /// Returns whether a refresh is waiting for its result.
    #[must_use]
    pub const fn refreshing(&self) -> bool {
        self.refreshing
    }

    /// Returns the snapshot for the explicitly mapped environment.
    #[must_use]
    pub const fn snapshot(&self, environment: UexEnvironment) -> Option<&ReferenceSnapshot> {
        match environment {
            UexEnvironment::Live => self.live.as_ref(),
            UexEnvironment::Ptu => self.ptu.as_ref(),
        }
    }

    /// Returns elapsed seconds since an endpoint fetch, absent for unknown or reversed time.
    #[must_use]
    pub fn age_seconds(&self, fetched_at: Timestamp) -> Option<u128> {
        self.observed_at?
            .millis()
            .checked_sub(fetched_at.millis())
            .map(|millis| millis / 1_000)
    }

    /// Queues a refresh once while retaining cached records and browsing choices.
    pub fn refresh(&mut self) {
        if !self.refreshing && self.command.is_none() {
            self.refreshing = true;
            self.command = Some(ReferenceCommand::Refresh);
        }
    }

    /// Takes the next explicit worker request.
    pub fn take_command(&mut self) -> Option<ReferenceCommand> {
        self.command.take()
    }

    pub(crate) fn started(&mut self) {
        self.refreshing = true;
    }
    pub(crate) fn unavailable(&mut self) {
        self.refreshing = false;
    }
    pub(crate) fn updated(
        &mut self,
        live: ReferenceSnapshot,
        ptu: ReferenceSnapshot,
        observed_at: Timestamp,
    ) {
        self.live = Some(live);
        self.ptu = Some(ptu);
        self.observed_at = Some(observed_at);
        self.refreshing = false;
    }
    pub(crate) fn open_terminal(&mut self, id: TerminalId) {
        self.selected_terminal = Some(id);
        self.command = Some(ReferenceCommand::OpenTerminal(id));
    }
    pub(crate) fn select_commodity(&mut self, id: CommodityId) {
        self.selected_commodity = Some(id);
    }
}
