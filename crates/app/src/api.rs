// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Application bootstrap operations.

pub mod model;

pub use crate::storage_bootstrap::initialise_storage;

pub use crate::attachment_worker::{AttachmentWorker, AttachmentWorkerError};
pub use crate::cpu_worker::CpuWorker;
pub use crate::manual_worker::ManualWorker;
pub use crate::reference_worker::ReferenceWorker;

pub use crate::manual_attachments::ManualAttachmentStorage;

pub use crate::credential_worker::{CredentialStores, CredentialWorker};

pub use crate::credential_logs::{CredentialLogBridge, install_credential_log_bridge};

pub use crate::build_info::compiled_build_information;

pub use crate::build_entry::enter_application;

pub use crate::dependency_diagnostics::DependencyDiagnostics;
pub use crate::http_client::HttpClient;
pub use crate::http_driver::HttpDriver;
pub use crate::http_protocol::HttpBackend;

pub use crate::receipt_lookup::PendingReceiptVerification;

pub use crate::response_worker::ResponseWorker;

pub use crate::submission_slot::{PendingSubmission, PendingSubmissionReservation, SubmissionSlot};

pub use crate::credential_queue::{CredentialQueue, DeferredCredentialQueue};

pub use crate::submission_controller::{
    SubmissionControlError, SubmissionQueueController, SubmissionQueuePreferences,
};

pub use crate::submission_bridge::SubmissionQueueBridge;

pub use crate::release_reference::{
    ReleaseReferenceClient, ReleaseReferenceOwner, ReleaseReferenceRefresh,
};

pub use crate::native_icon::native_window_icon;
