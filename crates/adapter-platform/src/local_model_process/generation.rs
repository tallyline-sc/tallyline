// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Retained allocation identity for one owned-child lifetime.

use std::fmt;
use std::sync::Arc;

/// An opaque lifetime identity that cannot be reused while any token remains retained.
#[derive(Clone)]
pub struct OwnedProcessGeneration(Arc<()>);

impl OwnedProcessGeneration {
    pub(super) fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl PartialEq for OwnedProcessGeneration {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for OwnedProcessGeneration {}

impl fmt::Debug for OwnedProcessGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnedProcessGeneration")
    }
}
