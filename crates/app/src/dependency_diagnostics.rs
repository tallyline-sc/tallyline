// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Immutable process diagnostics filtering for spawned credential and HTTP tasks.

use crate::credential_logs::sensitive_dependency_target;
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Metadata, Subscriber};

/// Filters sensitive dependency targets before fields reach an ordinary diagnostics subscriber.
pub struct DependencyDiagnostics<S> {
    inner: S,
}
impl<S> DependencyDiagnostics<S> {
    /// Wraps the selected diagnostics subscriber without mutable process state.
    #[must_use]
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}
impl<S: Subscriber> Subscriber for DependencyDiagnostics<S> {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        !sensitive_dependency_target(metadata.target()) && self.inner.enabled(metadata)
    }
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if sensitive_dependency_target(metadata.target()) {
            Interest::never()
        } else {
            self.inner.register_callsite(metadata)
        }
    }
    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        self.inner.max_level_hint()
    }
    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        self.inner.new_span(attributes)
    }
    fn record(&self, id: &Id, values: &Record<'_>) {
        self.inner.record(id, values);
    }
    fn record_follows_from(&self, id: &Id, follows: &Id) {
        self.inner.record_follows_from(id, follows);
    }
    fn event(&self, event: &Event<'_>) {
        if !sensitive_dependency_target(event.metadata().target()) {
            self.inner.event(event);
        }
    }
    fn enter(&self, id: &Id) {
        self.inner.enter(id);
    }
    fn exit(&self, id: &Id) {
        self.inner.exit(id);
    }
    fn clone_span(&self, id: &Id) -> Id {
        self.inner.clone_span(id)
    }
    fn try_close(&self, id: Id) -> bool {
        self.inner.try_close(id)
    }
}
