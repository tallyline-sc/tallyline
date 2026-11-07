// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Credential dependency exclusion before log argument formatting.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
struct Sentinel(Arc<AtomicUsize>);
impl std::fmt::Debug for Sentinel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}
impl std::fmt::Display for Sentinel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        formatter.write_str("dummy-secret-probe")
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__credential_log_targets_are_rejected_before_display_or_debug_formatting() {
    let bridge = tallyline_app::api::CredentialLogBridge;
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    let _dispatch = tracing::subscriber::set_default(Capture(Arc::clone(&captured)));
    let formatted = Arc::new(AtomicUsize::new(0));
    for target in [
        "keyring_core",
        "keyring_core::api",
        "windows_native_keyring_store",
        "windows_native_keyring_store::utils",
        "secret_service",
        "secret_service::session",
        "zbus",
        "zbus::connection",
        "zvariant::serialized",
    ] {
        assert!(!log::Log::enabled(
            &bridge,
            &log::Metadata::builder()
                .level(log::Level::Error)
                .target(target)
                .build()
        ));
        log::Log::log(
            &bridge,
            &log::Record::builder()
                .level(log::Level::Error)
                .target(target)
                .args(format_args!("{}", Sentinel(Arc::clone(&formatted))))
                .build(),
        );
    }
    for target in ["keyring_core::api", "secret_service::session"] {
        log::Log::log(
            &bridge,
            &log::Record::builder()
                .level(log::Level::Error)
                .target(target)
                .args(format_args!("{:?}", Sentinel(Arc::clone(&formatted))))
                .build(),
        );
    }
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    log::Log::log(
        &bridge,
        &log::Record::builder()
            .level(log::Level::Warn)
            .target("tallyline_app")
            .args(format_args!("{}", Sentinel(Arc::clone(&formatted))))
            .build(),
    );
    assert_eq!(formatted.load(Ordering::SeqCst), 1);
    assert!(
        captured
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains("dummy-secret-probe"))
    );
    assert!(log::Log::enabled(
        &bridge,
        &log::Metadata::builder()
            .level(log::Level::Warn)
            .target("tallyline_app")
            .build()
    ));
}

struct Capture(Arc<std::sync::Mutex<Vec<String>>>);
impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct Visitor<'a>(&'a mut Vec<String>);
        impl tracing::field::Visit for Visitor<'_> {
            fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.push(format!("{value:?}"));
            }
        }
        event.record(&mut Visitor(
            &mut self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        ));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

struct NoStore;
impl tallyline_submission::api::SecretStore for NoStore {
    async fn load(
        &mut self,
    ) -> Result<
        Option<tallyline_submission::api::model::SavedCredentials>,
        tallyline_submission::api::model::StorageFailure,
    > {
        Ok(None)
    }
    async fn save(
        &mut self,
        _: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Err(tallyline_submission::api::model::StorageFailure::Locked)
    }
    async fn save_passive(
        &mut self,
        saved: &tallyline_submission::api::model::SavedCredentials,
    ) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        self.save(saved).await
    }
    async fn remove(&mut self) -> Result<(), tallyline_submission::api::model::StorageFailure> {
        Ok(())
    }
}
struct Fixed;
impl tallyline_shared_kernel::api::Clock for Fixed {
    fn now(&self) -> tallyline_shared_kernel::api::model::Timestamp {
        tallyline_shared_kernel::api::model::Timestamp::from_seconds(123)
    }
}
struct TracingValidator(Arc<AtomicUsize>);
impl tallyline_submission::api::CredentialValidator for TracingValidator {
    async fn validate(
        &self,
        values: &tallyline_submission::api::model::CredentialValues,
    ) -> Result<
        tallyline_submission::api::model::ValidatedAccount,
        tallyline_submission::api::model::CredentialFailure,
    > {
        tracing::error!(target: "secret_service::session", probe = %Sentinel(Arc::clone(&self.0)), key = values.secret_key(), token = values.own_token());
        Ok(tallyline_submission::api::model::ValidatedAccount::new(
            "dummy-runner".to_owned(),
        ))
    }
}
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4__worker_suppresses_dependency_tracing_before_secret_field_formatting() {
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(Capture(Arc::clone(&captured))).unwrap();
    let formatted = Arc::new(AtomicUsize::new(0));
    let (commands, incoming) = std::sync::mpsc::sync_channel(1);
    let (outgoing, events) = std::sync::mpsc::sync_channel(4);
    let worker = tallyline_app::api::CredentialWorker::start(
        || async { Ok(NoStore) },
        TracingValidator(Arc::clone(&formatted)),
        Fixed,
        None,
        incoming,
        outgoing,
        || (),
    )
    .unwrap();
    events
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    commands
        .send(tallyline_ui::api::model::CredentialCommand::Validate(
            tallyline_submission::api::model::CredentialCandidate::new(
                tallyline_submission::api::model::CredentialValues::new(
                    "dummy-key-tracing-probe".to_owned(),
                    Some("dummy-token-tracing-probe".to_owned()),
                )
                .unwrap(),
                tallyline_submission::api::model::Persistence::Session,
            ),
        ))
        .ok()
        .unwrap();
    let event = events
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    assert!(matches!(
        event,
        tallyline_ui::api::model::BackgroundEvent::CredentialsUpdated { failure: None, .. }
    ));
    assert_eq!(formatted.load(Ordering::SeqCst), 0);
    let debug = format!("{event:?} {:?}", captured.lock().unwrap());
    assert!(!debug.contains("dummy-key-tracing-probe"));
    assert!(!debug.contains("dummy-token-tracing-probe"));
    drop(commands);
    worker.finish().unwrap();
}
