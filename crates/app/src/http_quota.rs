// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nonsecret shared HTTP quota admission and bounded persistence handoff.

use tallyline_submission::api::model::{
    JobId, SubmissionJob, SubmissionServiceFence, TokenAdmission, TokenQuotaObservation,
    TokenQuotaObservations,
};

const MAX_PENDING_POST_HOLDS: usize = 64;

#[derive(Clone, Copy)]
pub(crate) struct PostQuotaSource {
    job: JobId,
    sending_revision: u64,
}
impl PostQuotaSource {
    pub(crate) fn new(sending: &SubmissionJob) -> Self {
        Self {
            job: sending.id(),
            sending_revision: sending.revision(),
        }
    }
    fn matches(self, fence: &SubmissionServiceFence) -> bool {
        self.job == fence.source()
            && self.sending_revision.checked_add(1) == Some(fence.source_revision())
    }
}
struct PostQuotaHold {
    observation: TokenQuotaObservation,
    source: Option<PostQuotaSource>,
}
#[derive(Default)]
pub(crate) struct HttpQuota {
    ready: bool,
    pending: TokenQuotaObservations,
    saving: Vec<TokenQuotaObservation>,
    post: Vec<PostQuotaHold>,
    post_overflowed: bool,
    durable: Vec<TokenQuotaObservation>,
}
impl HttpQuota {
    pub(crate) fn record(&mut self, observation: TokenQuotaObservation) {
        self.pending.record(observation);
    }
    pub(crate) fn hold_post(
        &mut self,
        observation: TokenQuotaObservation,
        source: Option<PostQuotaSource>,
    ) {
        if self.post.len() == MAX_PENDING_POST_HOLDS {
            self.post_overflowed = true;
        } else {
            self.post.push(PostQuotaHold {
                observation,
                source,
            });
        }
    }
    pub(crate) fn take(&mut self) -> Vec<TokenQuotaObservation> {
        let observations = self.pending.take();
        self.saving.extend(observations.iter().copied());
        observations
    }
    pub(crate) fn replace_holds(
        &mut self,
        holds: Vec<TokenQuotaObservation>,
        post_fences: &[SubmissionServiceFence],
    ) {
        self.saving.clear();
        self.post.retain(|post| {
            post.source
                .is_none_or(|source| !post_fences.iter().any(|fence| source.matches(fence)))
        });
        self.durable = holds;
        self.ready = true;
    }
    pub(crate) fn ready(&self) -> bool {
        self.ready
    }
    pub(crate) fn blocks(&self, proof: &TokenAdmission) -> bool {
        !self.ready
            || self.post_overflowed
            || self
                .durable
                .iter()
                .chain(self.pending.as_ref())
                .chain(&self.saving)
                .any(|hold| hold.blocks_admission(proof))
            || self
                .post
                .iter()
                .any(|post| post.observation.blocks_admission(proof))
    }
}
