// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generation-bound ownership observations for credential-free local metadata.

use std::future::Future;
use std::num::{NonZeroU16, NonZeroU64};

/// Identifies one immutable owned runtime generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataGeneration(NonZeroU64);

impl MetadataGeneration {
    /// Constructs an identity from a nonzero generation.
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// Identifies the ports of an actual connection whose addresses are both 127.0.0.1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopbackConnection {
    client_port: NonZeroU16,
    server_port: NonZeroU16,
}

impl LoopbackConnection {
    /// Records the client and server ports after their addresses have been checked.
    #[must_use]
    pub const fn new(client_port: NonZeroU16, server_port: NonZeroU16) -> Self {
        Self {
            client_port,
            server_port,
        }
    }

    /// Returns the actual client port.
    #[must_use]
    pub const fn client_port(self) -> NonZeroU16 {
        self.client_port
    }

    /// Returns the actual server port.
    #[must_use]
    pub const fn server_port(self) -> NonZeroU16 {
        self.server_port
    }
}

/// Requests an observation for one generation and actual connected tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetadataAttempt {
    generation: MetadataGeneration,
    connection: LoopbackConnection,
}

impl MetadataAttempt {
    /// Binds an observation request to its generation and connection.
    #[must_use]
    pub const fn new(generation: MetadataGeneration, connection: LoopbackConnection) -> Self {
        Self {
            generation,
            connection,
        }
    }

    /// Returns the requested generation.
    #[must_use]
    pub const fn generation(self) -> MetadataGeneration {
        self.generation
    }

    /// Returns the requested actual connection.
    #[must_use]
    pub const fn connection(self) -> LoopbackConnection {
        self.connection
    }
}

/// Reports the lifetime of the owned metadata session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataSessionState {
    /// The supplied generation remains current.
    Current(MetadataGeneration),
    /// The session cannot authorise another metadata read.
    Revoked(MetadataRefusal),
}

/// Reports an ownership observation without granting private-input permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataOwnership {
    /// The observer attributes this exact attempt to its owned generation.
    Owned(MetadataAttempt),
    /// The actual listener belongs to another owner.
    ForeignListener,
    /// Ownership could not be established.
    Unknown,
}

/// Refuses local metadata without carrying a remote response or diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataRefusal {
    /// The connected listener belongs to another owner.
    ForeignListener,
    /// Ownership could not be established.
    OwnershipUnknown,
    /// The owned generation was replaced.
    GenerationChanged,
    /// The observation identifies a different actual connection.
    TupleMismatch,
    /// The owned session was cancelled.
    Cancelled,
    /// The total operation budget expired.
    Deadline,
    /// The cumulative response body exceeded its explicit bound.
    ResponseTooLarge,
    /// The response was not supported version metadata.
    InvalidMetadata,
    /// The local transport failed.
    Transport,
    /// Explicit client configuration was invalid or unavailable.
    Configuration,
}

/// Observes actual connections for one owned generation and signals revocation.
pub trait MetadataOwnershipSession: Clone + Send + Sync + 'static {
    /// Returns the current owned-session state.
    fn state(&self) -> MetadataSessionState;

    /// Observes ownership of the supplied actual connection and generation.
    fn observe(&self, attempt: MetadataAttempt) -> impl Future<Output = MetadataOwnership> + Send;

    /// Completes when the supplied generation is cancelled, replaced or loses ownership.
    fn revoked(
        &self,
        generation: MetadataGeneration,
    ) -> impl Future<Output = MetadataRefusal> + Send;
}
