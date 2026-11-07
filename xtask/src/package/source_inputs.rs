// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Canonical legal-profile inputs and the exact workspace manifest inventory.

use std::collections::{BTreeMap, BTreeSet};
use std::io;

pub(super) fn compiled() -> io::Result<BTreeMap<&'static str, &'static [u8]>> {
    let sources = BTreeMap::from([
        (
            "Cargo.lock",
            include_bytes!("../../../Cargo.lock").as_slice(),
        ),
        ("NOTICE", include_bytes!("../../../NOTICE").as_slice()),
        (
            "supply-chain/notices.toml",
            include_bytes!("../../../supply-chain/notices.toml").as_slice(),
        ),
        (
            "Cargo.toml",
            include_bytes!("../../../Cargo.toml").as_slice(),
        ),
        (
            "crates/adapter-files/Cargo.toml",
            include_bytes!("../../../crates/adapter-files/Cargo.toml").as_slice(),
        ),
        (
            "crates/adapter-ocr/Cargo.toml",
            include_bytes!("../../../crates/adapter-ocr/Cargo.toml").as_slice(),
        ),
        (
            "crates/adapter-platform/Cargo.toml",
            include_bytes!("../../../crates/adapter-platform/Cargo.toml").as_slice(),
        ),
        (
            "crates/adapter-storage/Cargo.toml",
            include_bytes!("../../../crates/adapter-storage/Cargo.toml").as_slice(),
        ),
        (
            "crates/adapter-uex/Cargo.toml",
            include_bytes!("../../../crates/adapter-uex/Cargo.toml").as_slice(),
        ),
        (
            "crates/adapter-vlm/Cargo.toml",
            include_bytes!("../../../crates/adapter-vlm/Cargo.toml").as_slice(),
        ),
        (
            "crates/recognition/Cargo.toml",
            include_bytes!("../../../crates/recognition/Cargo.toml").as_slice(),
        ),
        (
            "crates/app/Cargo.toml",
            include_bytes!("../../../crates/app/Cargo.toml").as_slice(),
        ),
        (
            "crates/egui/Cargo.toml",
            include_bytes!("../../../crates/egui/Cargo.toml").as_slice(),
        ),
        (
            "crates/game/Cargo.toml",
            include_bytes!("../../../crates/game/Cargo.toml").as_slice(),
        ),
        (
            "crates/reference-data/Cargo.toml",
            include_bytes!("../../../crates/reference-data/Cargo.toml").as_slice(),
        ),
        (
            "crates/reporting/Cargo.toml",
            include_bytes!("../../../crates/reporting/Cargo.toml").as_slice(),
        ),
        (
            "crates/shared-kernel/Cargo.toml",
            include_bytes!("../../../crates/shared-kernel/Cargo.toml").as_slice(),
        ),
        (
            "crates/submission/Cargo.toml",
            include_bytes!("../../../crates/submission/Cargo.toml").as_slice(),
        ),
        (
            "crates/ui/Cargo.toml",
            include_bytes!("../../../crates/ui/Cargo.toml").as_slice(),
        ),
        (
            "crates/workflows/Cargo.toml",
            include_bytes!("../../../crates/workflows/Cargo.toml").as_slice(),
        ),
        (
            "tools/ocr-eval/Cargo.toml",
            include_bytes!("../../../tools/ocr-eval/Cargo.toml").as_slice(),
        ),
        (
            "xtask/Cargo.toml",
            include_bytes!("../../../xtask/Cargo.toml").as_slice(),
        ),
    ]);
    let root = sources.get("Cargo.toml").ok_or_else(invalid)?;
    let manifest: toml::Table =
        toml::from_str(std::str::from_utf8(root).map_err(io::Error::other)?)
            .map_err(io::Error::other)?;
    let members = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(invalid)?;
    let mut expected = BTreeSet::new();
    for member in members {
        let name = member.as_str().ok_or_else(invalid)?;
        if !name.split('/').all(super::archive::portable_name)
            || !expected.insert(format!("{name}/Cargo.toml"))
        {
            return Err(invalid());
        }
    }
    let actual = sources
        .keys()
        .filter(|name| **name != "Cargo.toml" && name.ends_with("/Cargo.toml"))
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid());
    }
    Ok(sources)
}

fn invalid() -> io::Error {
    io::Error::other("packaging workspace manifest inventory is incomplete or unsupported")
}
