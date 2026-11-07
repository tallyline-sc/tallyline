// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform startup outcomes.

pub use crate::ocr_startup::{OcrChildExit, OcrProcessStartup};

pub use crate::ocr_native::{CrtModuleVersion, NativeOcrModule, OcrNativeError};

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
pub use crate::local_model_process::model::{
    CleanupFailure, CleanupOutcome, CleanupReport, OwnedProcessCommand, OwnedProcessFailure,
    OwnedProcessGeneration, OwnedProcessLimits, OwnedProcessObservation, OwnedProcessState,
    ResumeRefusal,
};
