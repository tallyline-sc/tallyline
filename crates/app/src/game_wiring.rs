// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only local witness wiring on the settings worker.

use std::io;
use std::path::PathBuf;
use tallyline_adapter_files::api::{ConfigFile, GameLogFiles};
use tallyline_game::api::GameSettingsApplication;
use tallyline_game::api::model::WitnessMode;
use tallyline_ui::api::model::GameWitnessView;

pub(crate) fn view(store: &ConfigFile) -> io::Result<(GameWitnessView, Vec<String>)> {
    let loaded = store.load_game_preferences()?;
    let source = GameLogFiles::new(
        loaded
            .channel_directory
            .as_deref()
            .map_or_else(PathBuf::new, PathBuf::from),
    );
    let snapshot = source.inspect(&loaded.settings);
    let issues = loaded
        .errors
        .into_iter()
        .map(|issue| format!("{}: {} → {}", issue.key, issue.rejected, issue.default))
        .collect();
    Ok((
        GameWitnessView {
            settings: loaded.settings,
            channel_directory: loaded.channel_directory,
            snapshot,
        },
        issues,
    ))
}

pub(crate) fn change_mode(path: PathBuf, mode: WitnessMode) -> io::Result<()> {
    let store = ConfigFile::new(path);
    let loaded = store.load_game_preferences()?;
    let mut next = loaded.settings.clone();
    next.witness = mode;
    GameSettingsApplication::new(loaded.settings, store).change_settings(next)
}

pub(crate) fn change_mapping(
    path: PathBuf,
    channel: tallyline_game::api::model::GameEnvironment,
    environment: tallyline_shared_kernel::api::model::UexEnvironment,
) -> io::Result<()> {
    let store = ConfigFile::new(path);
    let loaded = store.load_game_preferences()?;
    let mut next = loaded.settings.clone();
    next.environment_mapping = next
        .environment_mapping
        .with_environment(channel, environment);
    GameSettingsApplication::new(loaded.settings, store).change_settings(next)
}
