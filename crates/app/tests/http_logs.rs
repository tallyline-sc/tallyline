// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! HTTP connection-task diagnostics are filtered before sensitive formatting.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::PreparedUexSubmission;
use tallyline_app::api::{DependencyDiagnostics, HttpBackend, HttpDriver};
use tallyline_reference_data::api::model::{Endpoint, EndpointData, TerminalPriors};
use tallyline_shared_kernel::api::model::TerminalId;
use tallyline_submission::api::CredentialValidator;
use tallyline_submission::api::model::{
    CredentialFailure, CredentialValues, SubmissionResult, ValidatedAccount,
};

struct Sentinel(Arc<AtomicUsize>, &'static str);
impl std::fmt::Debug for Sentinel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        formatter.write_str(self.1)
    }
}
impl std::fmt::Display for Sentinel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, formatter)
    }
}
struct Backend {
    hidden: Arc<AtomicUsize>,
    visible: Arc<AtomicUsize>,
}
impl HttpBackend for Backend {
    async fn fetch(&self, _: Endpoint) -> Result<EndpointData, ReadError> {
        Err(ReadError::Transport("unused fake endpoint".to_owned()))
    }
    async fn fetch_terminal_priors(
        &self,
        _: Vec<TerminalId>,
    ) -> Result<Vec<TerminalPriors>, ReadError> {
        Ok(Vec::new())
    }
    async fn verify_receipts(
        &self,
        _: tallyline_submission::api::model::SubmissionJob,
        _: CredentialValues,
        _: Option<String>,
    ) -> Option<Vec<tallyline_submission::api::model::VerifiedUexReport>> {
        None
    }
    async fn submit(
        &self,
        _: PreparedUexSubmission,
        _: CredentialValues,
        _: Option<String>,
    ) -> SubmissionResult {
        SubmissionResult::Unknown
    }
    async fn validate(&self, _: CredentialValues) -> Result<ValidatedAccount, CredentialFailure> {
        let hidden = Arc::clone(&self.hidden);
        let visible = Arc::clone(&self.visible);
        tokio::task::spawn(async move {
            tracing::error!(target:"h2::codec",query=?Sentinel(Arc::clone(&hidden),"dummy-source-query"),body=?Sentinel(Arc::clone(&hidden),"dummy-image-body"),header=?Sentinel(Arc::clone(&hidden),"dummy-secret-header"));
            log::error!(target:"reqwest::connect","{} {:?} {}",Sentinel(Arc::clone(&hidden),"dummy-source-query"),Sentinel(Arc::clone(&hidden),"dummy-image-body"),Sentinel(hidden,"dummy-secret-header"));
            tracing::info!(target:"tallyline_http_probe",message=?Sentinel(visible,"allowed-probe"));
        }).await.map_err(|_|CredentialFailure::Unavailable)?;
        Ok(ValidatedAccount::new("dummy-a".to_owned()))
    }
}
struct Capture(Arc<Mutex<Vec<String>>>);
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
        let mut visitor = Fields(String::new());
        event.record(&mut visitor);
        if let Ok(mut lines) = self.0.lock() {
            lines.push(visitor.0);
        }
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
struct Fields(String);
impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{value:?}"));
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_4_r_sub_2__spawned_http_tasks_cannot_format_secret_query_or_body_fields() {
    let lines = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(DependencyDiagnostics::new(Capture(Arc::clone(
        &lines,
    ))))
    .unwrap();
    tallyline_app::api::install_credential_log_bridge().unwrap();
    let hidden = Arc::new(AtomicUsize::new(0));
    let visible = Arc::new(AtomicUsize::new(0));
    let backend = Backend {
        hidden: Arc::clone(&hidden),
        visible: Arc::clone(&visible),
    };
    let (driver, client) = HttpDriver::start(move || Ok::<_, ()>(backend)).unwrap();
    let caller = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let values =
        CredentialValues::new("dummy-key".to_owned(), Some("dummy-token".to_owned())).unwrap();
    caller.block_on(client.validate(&values)).unwrap();
    assert_eq!(hidden.load(Ordering::SeqCst), 0);
    assert_eq!(visible.load(Ordering::SeqCst), 1);
    assert!(
        lines
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.contains("allowed-probe"))
    );
    drop(client);
    drop(caller);
    driver.finish().unwrap();
}
