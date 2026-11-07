// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Raw readings, supplied-panel outcomes and local metadata session values.

pub use crate::metadata_ownership_session::{
    LoopbackConnection, MetadataAttempt, MetadataGeneration, MetadataOwnership, MetadataRefusal,
    MetadataSessionState,
};
pub use crate::numeric_reading::{DecimalPrices, NumericReading};
pub use crate::panel_geometry::{
    GeometryProvenance, InvalidPanelLimits, PanelGeometry, PanelLimits, PanelRefusal,
    PreparedPanel, PreparedPanels, SuppliedPanels,
};
pub use crate::raw_transcription::{RawCard, RawField, RawTranscription};
