// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local OCR inference and model-specific tensor decoding.

pub mod api;

mod assets;
mod ctc;
mod preprocessing;

mod detection;
mod research;
mod runtime;
