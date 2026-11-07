// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded local read-only Game.log access.

use crate::game_log_mapping::{BRANCH, BUILD};
use crate::game_log_parser::LogParser;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use tallyline_game::api::model::{GameSettings, WitnessFacts, WitnessSnapshot, WitnessUnavailable};
use tallyline_game::api::{GameLogSource, inspect_witness};

struct ReadLimits {
    manifest_bytes: u64,
    log_bytes: u64,
    line_bytes: u64,
}
const LIMITS: ReadLimits = ReadLimits {
    manifest_bytes: 65_536,
    log_bytes: 134_217_728,
    line_bytes: 16_384,
};

/// The selected channel's read-only witness source.
pub struct GameLogFiles {
    channel: PathBuf,
}
impl GameLogFiles {
    /// Selects a channel directory without accessing its files.
    #[must_use]
    pub const fn new(channel: PathBuf) -> Self {
        Self { channel }
    }
    /// Inspects allowlisted facts only after explicit opt-in.
    #[must_use]
    pub fn inspect(&self, settings: &GameSettings) -> WitnessSnapshot {
        inspect_witness(settings, self)
    }
    fn read_validated(&self) -> Result<WitnessFacts, WitnessUnavailable> {
        if self.channel.as_os_str().is_empty() {
            return Err(WitnessUnavailable::NotConfigured);
        }
        let manifest = File::open(self.channel.join("build_manifest.id"))
            .map_err(|_| WitnessUnavailable::ReadFailure)?;
        validate_manifest(manifest)?;
        let log = File::open(self.channel.join("Game.log"))
            .map_err(|_| WitnessUnavailable::ReadFailure)?;
        read_log(log)
    }
}
impl GameLogSource for GameLogFiles {
    fn read(&self) -> WitnessSnapshot {
        self.read_validated()
            .map_or_else(WitnessSnapshot::Unavailable, WitnessSnapshot::Available)
    }
}
fn validate_manifest(file: File) -> Result<(), WitnessUnavailable> {
    if file
        .metadata()
        .map_err(|_| WitnessUnavailable::ReadFailure)?
        .len()
        > LIMITS.manifest_bytes
    {
        return Err(WitnessUnavailable::InputLimit);
    }
    let document: serde_json::Value = serde_json::from_reader(file.take(LIMITS.manifest_bytes + 1))
        .map_err(|_| WitnessUnavailable::UnvalidatedBuild)?;
    let data = document
        .get("Data")
        .ok_or(WitnessUnavailable::UnvalidatedBuild)?;
    if data.get("Version").and_then(serde_json::Value::as_str) != Some(BUILD)
        || data.get("Branch").and_then(serde_json::Value::as_str) != Some(BRANCH)
    {
        return Err(WitnessUnavailable::UnvalidatedBuild);
    }
    Ok(())
}
fn read_log(file: File) -> Result<WitnessFacts, WitnessUnavailable> {
    if file
        .metadata()
        .map_err(|_| WitnessUnavailable::ReadFailure)?
        .len()
        > LIMITS.log_bytes
    {
        return Err(WitnessUnavailable::InputLimit);
    }
    let mut reader = BufReader::new(file.take(LIMITS.log_bytes + 1));
    let mut line = Vec::new();
    let mut parser = LogParser::default();
    let mut total_bytes = 0_u64;
    loop {
        line.clear();
        let count = reader
            .by_ref()
            .take(LIMITS.line_bytes + 1)
            .read_until(b'\n', &mut line)
            .map_err(|_| WitnessUnavailable::ReadFailure)?;
        if count == 0 {
            break;
        }
        let bytes = u64::try_from(count).map_err(|_| WitnessUnavailable::InputLimit)?;
        total_bytes += bytes;
        if total_bytes > LIMITS.log_bytes || bytes > LIMITS.line_bytes {
            return Err(WitnessUnavailable::InputLimit);
        }
        let text = std::str::from_utf8(&line)
            .map_err(|_| WitnessUnavailable::UnvalidatedFormat)?
            .trim_end();
        parser.line(text)?;
    }
    parser.finish()
}
