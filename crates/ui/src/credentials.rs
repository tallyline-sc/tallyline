// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Masked credential input and safe account feedback.

mod credential_setup;
mod feedback;
mod secret_field;

pub(crate) use credential_setup::show;
pub use credential_setup::{CredentialCommand, CredentialSetup};
