// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Commands and results exchanged with the desktop shell.

pub use crate::fan_kit::{FAN_KIT_LOGO_ALT_TEXT, FAN_KIT_LOGO_TEXTURE_NAME, LegalPlacement};
pub use crate::state::{BackgroundEvent, SettingsCommand, UserAction, View};

pub use crate::activation::WindowCommand;
pub use crate::fonts::FontFiles;
pub use crate::game_witness::GameWitnessView;
pub use crate::legal_views::AboutAction;
pub use crate::licences::CopyrightInformation;
pub use crate::licences::{
    LicenceCatalogue, LicenceCatalogueError, LicenceComponent, LicenceNotice,
};
pub use crate::material_symbols_font::MaterialSymbolsFont;

pub use crate::manual_editor::{ManualCommand, ManualEditRequestId, ManualEditor};
pub use crate::reference_browser::{ReferenceBrowser, ReferenceCommand, ReferenceDiagnostic};

pub use crate::attachment_editor::{ManualAttachmentAction, ManualAttachmentCommand};

pub use crate::credentials::{CredentialCommand, CredentialSetup};
pub use crate::manual_review::{
    ManualReviewAction, ManualReviewCommand, ManualReviewState, ReviewDirection,
};

pub use crate::build_presentation::BuildPresentation;

pub use crate::submission_queue::{QueueCommand, SubmissionQueue};

pub use crate::submission_control::{
    SubmissionControl, SubmissionFeedback, SubmissionIntent, SubmissionUpdate,
};

pub use crate::resolution_queue_action::ResolutionQueueAction;
