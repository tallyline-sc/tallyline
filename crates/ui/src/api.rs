// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Desktop shell operations and owned view state.

pub mod model;

pub use crate::catalogue::Catalogue;
pub use crate::dialog_presenter::DialogPresenter;
pub use crate::fan_kit::FanKitUnit;
pub use crate::fonts::configure_fonts;
pub use crate::shell::{show_shell, wake_up};
pub use crate::state::ShellState;

pub use crate::activation::AlreadyRunningNotice;
pub use crate::activation::apply_window_command;
pub use crate::desktop::DesktopShell;
pub use crate::legal_views::{show_about_view, show_onboarding_view};
pub use crate::licences::parse_licence_catalogue;
pub use crate::theme::resolve_theme;

pub use crate::manual_review_views::show_manual_review;
pub use crate::manual_views::show_manual_editor;
pub use crate::reference_views::show_reference_browser;

pub use crate::attachment_views::show_manual_attachment_editor;
pub use crate::submission_queue::{show_submission_mode, show_submission_queue};

pub use crate::submission_controls::show_submission_controls;

pub use crate::submission_settings::show_submission_settings;

pub use crate::submission_wake::wake_submission;

pub use crate::submission_accounts::show_submission_account_choice;
