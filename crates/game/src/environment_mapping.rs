// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Channel mapping independent of the local witness.

use tallyline_shared_kernel::api::model::UexEnvironment;

/// An installed Star Citizen game channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEnvironment {
    /// Production channel.
    Live,
    /// Production hotfix channel.
    Hotfix,
    /// Public test channel.
    Ptu,
    /// Experimental public test channel.
    Eptu,
    /// Technology preview channel.
    TechPreview,
}

/// The user's independent UEX choice for each game channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentMapping {
    live: UexEnvironment,
    hotfix: UexEnvironment,
    ptu: UexEnvironment,
    eptu: UexEnvironment,
    tech_preview: UexEnvironment,
}

impl Default for EnvironmentMapping {
    fn default() -> Self {
        Self {
            live: UexEnvironment::Live,
            hotfix: UexEnvironment::Live,
            ptu: UexEnvironment::Ptu,
            eptu: UexEnvironment::Ptu,
            tech_preview: UexEnvironment::Ptu,
        }
    }
}

impl EnvironmentMapping {
    /// Returns the mapped UEX universe without testing submission acceptance.
    #[must_use]
    pub const fn environment(&self, channel: GameEnvironment) -> UexEnvironment {
        match channel {
            GameEnvironment::Live => self.live,
            GameEnvironment::Hotfix => self.hotfix,
            GameEnvironment::Ptu => self.ptu,
            GameEnvironment::Eptu => self.eptu,
            GameEnvironment::TechPreview => self.tech_preview,
        }
    }

    /// Changes one channel, preserving the original mapping and other choices.
    #[must_use]
    pub fn with_environment(&self, channel: GameEnvironment, environment: UexEnvironment) -> Self {
        let mut changed = self.clone();
        match channel {
            GameEnvironment::Live => changed.live = environment,
            GameEnvironment::Hotfix => changed.hotfix = environment,
            GameEnvironment::Ptu => changed.ptu = environment,
            GameEnvironment::Eptu => changed.eptu = environment,
            GameEnvironment::TechPreview => changed.tech_preview = environment,
        }
        changed
    }
}
