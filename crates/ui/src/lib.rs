// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tallyline desktop shell, legal unit and user-driven view commands.

#![forbid(unsafe_code)]

mod dialog_presenter;
mod state;

mod catalogue;
mod fan_kit;
mod fonts;
mod material_symbols_font;

mod control_focus;
mod shell;
mod shell_content;
mod shell_metrics;
mod shell_navigation;
mod shell_status;

mod activation;
pub mod api;
mod desktop;
mod game_witness;
mod legal_views;
mod licence_browser;
mod licence_text_viewer;
mod licences;
mod theme;

mod updates;

mod reference_browser;

mod manual_editor;
mod manual_frame;
mod manual_rows;
mod manual_views;
mod reference_views;

mod attachment_editor;

mod attachment_views;

mod credentials;
mod manual_confirmation;
mod manual_review;
mod manual_review_details;
mod manual_review_views;

mod build_presentation;
mod stock_settings;

mod submission_queue;

mod submission_control;

mod submission_controls;
mod submission_feedback;

mod submission_settings;

mod submission_waiting;

mod submission_wake;

mod submission_accounts;

mod number_text;
mod settings_page;

mod submission_rejected;
mod submission_unknown;

mod unknown_attempt;

mod submission_partial;

mod resolution_queue_action;

mod submission_quota;
