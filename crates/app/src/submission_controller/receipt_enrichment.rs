// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! One optional receipt lookup with durable save retries independent of known submission success.

use super::SubmissionControlError;
use crate::api::HttpClient;
use crate::api::PendingReceiptVerification;
use crate::api::model::ReceiptLookupOutcome;
use tallyline_adapter_storage::api::SqliteReferenceCache;
use tallyline_submission::api::CredentialApplication;
use tallyline_submission::api::model::{
    ReleaseToken, SubmissionJob, SubmissionResult, VerifiedReceiptMapping,
};
use tallyline_workflows::api::ReceiptEnrichmentStore;

struct ReceiptWork {
    pending: Option<PendingReceiptVerification>,
    mapping: Option<VerifiedReceiptMapping>,
}
pub(super) struct ReceiptEnrichment {
    work: Option<ReceiptWork>,
    mappings: Vec<VerifiedReceiptMapping>,
    failure: Option<SubmissionControlError>,
}
impl ReceiptEnrichment {
    pub(super) fn new(mappings: Vec<VerifiedReceiptMapping>) -> Self {
        Self {
            work: None,
            mappings,
            failure: None,
        }
    }
    pub(super) fn mappings(&self) -> &[VerifiedReceiptMapping] {
        &self.mappings
    }
    pub(super) const fn failure(&self) -> Option<&SubmissionControlError> {
        self.failure.as_ref()
    }
    pub(super) fn failed(&mut self, error: SubmissionControlError) {
        self.failure = Some(error);
    }

    pub(super) fn begin_enrichment(
        &mut self,
        store: &mut SqliteReferenceCache,
        client: &HttpClient,
        job: &SubmissionJob,
        credentials: &CredentialApplication,
        token: Option<&ReleaseToken>,
    ) -> Result<(), SubmissionControlError> {
        if !matches!(job.result(), Some(SubmissionResult::ProductionAccepted(_))) {
            return Ok(());
        }
        if !store
            .claim_receipt_enrichment(job)
            .map_err(SubmissionControlError::Repository)?
        {
            return Ok(());
        }
        if self.work.is_some() {
            return Ok(());
        }
        let Some(values) = credentials.credentials_for(&job.request().header().account) else {
            return Ok(());
        };
        let pending = client
            .begin_receipt_verification(job, values, token.map(ReleaseToken::for_validation))
            .map_err(|_| SubmissionControlError::Unavailable)?;
        self.work = Some(ReceiptWork {
            pending: Some(pending),
            mapping: None,
        });
        Ok(())
    }

    fn advance_enrichment(
        &mut self,
        store: &mut SqliteReferenceCache,
    ) -> Result<bool, SubmissionControlError> {
        let Some(mut work) = self.work.take() else {
            return Ok(false);
        };
        if work.mapping.is_none() {
            match work
                .pending
                .as_mut()
                .and_then(PendingReceiptVerification::poll_result)
            {
                Some(ReceiptLookupOutcome::Verified(mapping)) => {
                    work.mapping = Some(*mapping);
                    work.pending = None;
                }
                Some(ReceiptLookupOutcome::Unavailable) => {
                    return Err(SubmissionControlError::Unavailable);
                }
                None => {
                    self.work = Some(work);
                    return Ok(false);
                }
            }
        }
        let mapping = work
            .mapping
            .as_ref()
            .ok_or(SubmissionControlError::Invalid)?;
        if let Err(error) = store.save_receipt_mapping(mapping) {
            self.work = Some(work);
            return Err(SubmissionControlError::Repository(error));
        }
        if !self.mappings.iter().any(|previous| previous == mapping) {
            self.mappings.push(mapping.clone());
        }
        self.failure = None;
        Ok(true)
    }

    pub(super) fn poll_enrichment(&mut self, store: &mut SqliteReferenceCache) -> bool {
        match self.advance_enrichment(store) {
            Ok(changed) => changed,
            Err(error) => {
                self.failure = Some(error);
                false
            }
        }
    }
}
