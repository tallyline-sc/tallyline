// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! OS instance election, bounded local activation and optional release metadata checks.

#![deny(unsafe_code)]

pub mod api;
#[cfg(test)]
mod contract_tests;
mod instance;
mod protocol;
mod socket;
#[cfg(windows)]
mod windows_socket;

mod release_token;

mod ocr_startup;
mod releases;

mod ocr_native;

#[cfg(windows)]
mod windows_ocr;
mod worker_priority;

#[cfg(any(windows, target_os = "linux"))]
mod credential_bundle;
#[cfg(windows)]
mod windows_credentials;

#[cfg(target_os = "linux")]
mod linux_credential_bus;

#[cfg(target_os = "linux")]
mod linux_credentials;

#[cfg(target_os = "linux")]
mod linux_fallback;

mod token_registry;

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
mod local_model_process;
