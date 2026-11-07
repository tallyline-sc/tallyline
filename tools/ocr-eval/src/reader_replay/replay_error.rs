// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed refusals for frozen retained-reader evidence.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;

/// A refusal to measure unavailable, changed or invalid retained evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum ReplayError {
    /// Retained packet hash mismatch.
    PacketHashMismatch,
    /// Retained packet is malformed.
    MalformedPacket,
    /// Unsupported retained packet schema.
    UnsupportedSchema,
    /// Retained reader identities are invalid, duplicate or outside bounds.
    InvalidReaders,
    /// Retained field count is outside bounds.
    RowCountOutOfBounds,
    /// Duplicate retained field identity.
    DuplicateField,
    /// Retained reference path is unsafe or escapes public corpus.
    UnsafeReferencePath,
    /// Too many retained reference entries.
    TooManyReferences,
    /// Retained reference is malformed.
    MalformedReference,
    /// Retained reference is not verified.
    UnverifiedReference,
    /// Retained reference field identity is unavailable.
    ReferenceFieldUnavailable,
    /// Retained reference hash mismatch.
    ReferenceHashMismatch,
    /// Retained card is partial or ambiguous.
    AmbiguousCard,
    /// Retained field does not contain every reader slot.
    IncompleteObservations,
    /// Unsupported retained field.
    UnsupportedField,
    /// Numeric value is not an exact bounded decimal.
    InvalidDecimal,
    /// Status is not a bounded literal.
    InvalidStatus,
    /// Retained observation has missing values or inconsistent literals.
    InvalidObservation,
    /// Unscorable requires original alignment failure or unknown reference.
    InvalidUnscorable,
    /// Unsupported observation outcome.
    UnsupportedOutcome,
    /// Retained literal is absent or exceeds bounds.
    InvalidLiteral,
    /// Retained input size overflows.
    InputSizeOverflow,
    /// Retained input exceeds size bound.
    InputSizeExceeded,
    /// Reader selection is unavailable or empty.
    InvalidReaderSelection,
    /// A required schema member is not text.
    TextRequired(&'static str),
    /// A required schema member is not an array.
    ArrayRequired(&'static str),
    /// The retained input could not be opened.
    InputOpen(io::Error),
    /// The retained input could not be read.
    InputRead(io::Error),
    /// The public corpus directory could not be resolved.
    PublicCorpusUnavailable(io::Error),
    /// The retained reference file could not be resolved.
    ReferenceUnavailable(io::Error),
}

impl Display for ReplayError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketHashMismatch => formatter.write_str("retained packet hash mismatch"),
            Self::MalformedPacket => formatter.write_str("retained packet is malformed"),
            Self::UnsupportedSchema => formatter.write_str("unsupported retained packet schema"),
            Self::InvalidReaders => formatter
                .write_str("retained reader identities are invalid, duplicate or outside bounds"),
            Self::RowCountOutOfBounds => {
                formatter.write_str("retained field count is outside bounds")
            }
            Self::DuplicateField => formatter.write_str("duplicate retained field identity"),
            Self::UnsafeReferencePath => {
                formatter.write_str("retained reference path is unsafe or escapes public corpus")
            }
            Self::TooManyReferences => formatter.write_str("too many retained reference entries"),
            Self::MalformedReference => formatter.write_str("retained reference is malformed"),
            Self::UnverifiedReference => formatter.write_str("retained reference is not verified"),
            Self::ReferenceFieldUnavailable => {
                formatter.write_str("retained reference field identity is unavailable")
            }
            Self::ReferenceHashMismatch => formatter.write_str("retained reference hash mismatch"),
            Self::AmbiguousCard => formatter.write_str("retained card is partial or ambiguous"),
            Self::IncompleteObservations => {
                formatter.write_str("retained field does not contain every reader slot")
            }
            Self::UnsupportedField => formatter.write_str("unsupported retained field"),
            Self::InvalidDecimal => {
                formatter.write_str("numeric value is not an exact bounded decimal")
            }
            Self::InvalidStatus => formatter.write_str("status is not a bounded literal"),
            Self::InvalidObservation => formatter
                .write_str("retained observation has missing values or inconsistent literals"),
            Self::InvalidUnscorable => formatter
                .write_str("unscorable requires original alignment failure or unknown reference"),
            Self::UnsupportedOutcome => formatter.write_str("unsupported observation outcome"),
            Self::InvalidLiteral => {
                formatter.write_str("retained literal is absent or exceeds bounds")
            }
            Self::InputSizeOverflow => formatter.write_str("retained input size overflows"),
            Self::InputSizeExceeded => formatter.write_str("retained input exceeds size bound"),
            Self::InvalidReaderSelection => {
                formatter.write_str("reader selection is unavailable or empty")
            }
            Self::TextRequired(name) => write!(formatter, "retained {name} is not text"),
            Self::ArrayRequired(name) => write!(formatter, "retained {name} is not an array"),
            Self::InputOpen(_) => formatter.write_str("retained input cannot be opened"),
            Self::InputRead(_) => formatter.write_str("retained input cannot be read"),
            Self::PublicCorpusUnavailable(_) => formatter.write_str("public corpus is unavailable"),
            Self::ReferenceUnavailable(_) => {
                formatter.write_str("retained reference is unavailable")
            }
        }
    }
}

impl Error for ReplayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InputOpen(source)
            | Self::InputRead(source)
            | Self::PublicCorpusUnavailable(source)
            | Self::ReferenceUnavailable(source) => Some(source),
            Self::PacketHashMismatch
            | Self::MalformedPacket
            | Self::UnsupportedSchema
            | Self::InvalidReaders
            | Self::RowCountOutOfBounds
            | Self::DuplicateField
            | Self::UnsafeReferencePath
            | Self::TooManyReferences
            | Self::MalformedReference
            | Self::UnverifiedReference
            | Self::ReferenceFieldUnavailable
            | Self::ReferenceHashMismatch
            | Self::AmbiguousCard
            | Self::IncompleteObservations
            | Self::UnsupportedField
            | Self::InvalidDecimal
            | Self::InvalidStatus
            | Self::InvalidObservation
            | Self::InvalidUnscorable
            | Self::UnsupportedOutcome
            | Self::InvalidLiteral
            | Self::InputSizeOverflow
            | Self::InputSizeExceeded
            | Self::InvalidReaderSelection
            | Self::TextRequired(_)
            | Self::ArrayRequired(_) => None,
        }
    }
}
