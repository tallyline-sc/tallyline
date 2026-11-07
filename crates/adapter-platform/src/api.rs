// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-user writer election and fixed-token activation.
pub use crate::instance::{
    ActivationListener, ActivationOutcome, Election, InstancePaths, PrimaryInstance, WriterLease,
    elect,
};

pub use crate::release_token::ReleaseTokenFile;
pub use crate::releases::ReleaseClient;

/// Platform startup outcomes.
pub mod model;

pub use crate::ocr_startup::prepare_ocr_process;

pub use crate::ocr_native::{restrict_ocr_dll_search, verify_ocr_native_modules};
pub use crate::worker_priority::{WorkerPriority, lower_worker_priority, worker_priority};

#[cfg(windows)]
pub use crate::windows_credentials::WindowsCredentialStore;

#[cfg(target_os = "linux")]
pub use crate::linux_credential_bus::LinuxCredentialBus;

#[cfg(target_os = "linux")]
pub use crate::linux_credentials::LinuxCredentialStore;

#[cfg(target_os = "linux")]
pub use crate::linux_fallback::LinuxFallbackStore;

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
pub use crate::local_model_process::OwnedProcess;
