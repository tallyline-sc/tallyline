// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Raw independent observations without resolution or confidence.

use tallyline_shared_kernel::api::model::TradeSide;

/// An observed field or an explicit unreadable mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RawField<T> {
    /// The literal observation, without correction.
    Read(T),
    /// The reader could not transcribe the field.
    Unreadable,
}

/// Field-assigned text from one card, without a completeness claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawCard {
    /// Commodity name text.
    pub name: RawField<String>,
    /// Stock status text.
    pub status: RawField<String>,
    /// Quantity text without the SCU unit.
    pub quantity_text: RawField<String>,
    /// Price text without the currency glyph or unit.
    pub price_text: RawField<String>,
    /// Cargo-size list text.
    pub cargo_sizes: RawField<String>,
}

/// Independent header and card observations from one transcription.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTranscription {
    /// Observed active terminal tab.
    pub side: RawField<TradeSide>,
    /// Observed location text.
    pub location: RawField<String>,
    /// Observed rows, requiring separate layout matching before fusion.
    pub cards: Vec<RawCard>,
}
