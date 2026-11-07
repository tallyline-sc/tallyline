// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filesystem adapter operations.

pub mod model;

pub use crate::config::ConfigFile;
pub use crate::diagnostic_logs::DiagnosticLogFiles;
pub use crate::diagnostics::DiagnosticFileExport;
pub use crate::game_log::GameLogFiles;
pub use crate::layout::StorageLayout;
pub use crate::logo::decode_logo;

pub use crate::backup::BackupSetStore;
pub use crate::image_intake::read_image;
pub use crate::image_intake::read_image_file;
pub use crate::manual_attachment::{encode_manual_attachment, prepare_manual_attachment_file};

pub use crate::private_artifact::write_private_artifact;

pub use crate::png_output::encode_raster_png;
pub use crate::working_copies::{ManualWorkingCopies, restore_manual_attachment};
