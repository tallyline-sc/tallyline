// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Private JSON projection of the side-specific status vocabulary.

use crate::api::StorageError;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU8;
use tallyline_reference_data::api::model::{StatusLevel, StatusLevels};

#[derive(Serialize, Deserialize)]
struct LevelDocument {
    code: u8,
    name: String,
    short_name: String,
    abbreviation: String,
}

#[derive(Serialize, Deserialize)]
struct LevelsDocument {
    buy: Vec<LevelDocument>,
    sell: Vec<LevelDocument>,
}

pub(crate) fn decode(payload: &str) -> Result<StatusLevels, StorageError> {
    let document: LevelsDocument =
        serde_json::from_str(payload).map_err(|_| StorageError::InvalidCache)?;
    let levels = StatusLevels {
        buy: document
            .buy
            .into_iter()
            .map(level)
            .collect::<Result<_, _>>()?,
        sell: document
            .sell
            .into_iter()
            .map(level)
            .collect::<Result<_, _>>()?,
    };
    if valid(&levels) {
        Ok(levels)
    } else {
        Err(StorageError::InvalidCache)
    }
}

fn level(document: LevelDocument) -> Result<StatusLevel, StorageError> {
    Ok(StatusLevel {
        code: NonZeroU8::new(document.code).ok_or(StorageError::InvalidCache)?,
        name: document.name,
        short_name: document.short_name,
        abbreviation: document.abbreviation,
    })
}

pub(crate) fn encode(levels: &StatusLevels) -> Result<String, serde_json::Error> {
    serde_json::to_string(&LevelsDocument {
        buy: levels.buy.iter().map(level_document).collect(),
        sell: levels.sell.iter().map(level_document).collect(),
    })
}

fn level_document(level: &StatusLevel) -> LevelDocument {
    LevelDocument {
        code: level.code.get(),
        name: level.name.clone(),
        short_name: level.short_name.clone(),
        abbreviation: level.abbreviation.clone(),
    }
}

pub(crate) fn valid(levels: &StatusLevels) -> bool {
    [&levels.buy, &levels.sell].into_iter().all(|side| {
        let codes = side
            .iter()
            .map(|level| level.code)
            .collect::<std::collections::BTreeSet<_>>();
        codes.len() == side.len()
    })
}
