// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Game Environment application API.

pub mod model;

use model::{GameSettings, WitnessMode, WitnessSnapshot};

/// Supplies sanitised facts without exposing raw game data.
pub trait GameLogSource {
    /// Reads the selected channel's validated witness facts.
    fn read(&self) -> WitnessSnapshot;
}

/// Inspects the witness only after explicit opt-in.
#[must_use]
pub fn inspect_witness(settings: &GameSettings, source: &impl GameLogSource) -> WitnessSnapshot {
    match settings.witness {
        WitnessMode::Off => WitnessSnapshot::Off,
        WitnessMode::On => source.read(),
    }
}

/// Persists game preferences through the single configuration writer.
pub trait GameSettingsStore {
    /// A persistence failure.
    type Error;
    /// Saves channel mapping and witness preferences atomically.
    ///
    /// # Errors
    /// Returns the failure without applying the proposed preferences.
    fn save_game_settings(&mut self, settings: &GameSettings) -> Result<(), Self::Error>;
}

/// Applies game preferences only after storage succeeds.
pub struct GameSettingsApplication<S> {
    settings: GameSettings,
    store: S,
}
impl<S: GameSettingsStore> GameSettingsApplication<S> {
    /// Creates the workflow from loaded preferences and their store.
    #[must_use]
    pub const fn new(settings: GameSettings, store: S) -> Self {
        Self { settings, store }
    }
    /// Returns durable active game preferences.
    #[must_use]
    pub const fn settings(&self) -> &GameSettings {
        &self.settings
    }
    /// Persists and applies the user's game preferences.
    ///
    /// # Errors
    /// Returns a storage failure and keeps the active preferences unchanged.
    pub fn change_settings(&mut self, next: GameSettings) -> Result<(), S::Error> {
        self.store.save_game_settings(&next)?;
        self.settings = next;
        Ok(())
    }
}
