// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public parsing, supplied-panel and local metadata ownership interfaces.

pub mod model;

pub use crate::metadata_ownership_session::MetadataOwnershipSession;
pub use crate::numeric_reading::{parse_price_text, parse_quantity_text};
pub use crate::panel_preparation::prepare_panels;
