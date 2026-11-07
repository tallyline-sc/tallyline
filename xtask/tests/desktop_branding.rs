// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Packaged desktop identity without invoking installer tools.

#![expect(
    non_snake_case,
    reason = "Requirement tags use the prescribed separator."
)]

use std::path::Path;
use xtask::package;

#[test]
fn r_ui_9__desktop_metadata_selects_the_own_line_artwork() {
    let entry = package::debian_desktop_entry();
    assert!(entry.lines().any(|line| line == "Icon=tallyline"));
    assert!(entry.lines().any(|line| line == "Exec=tallyline"));
    let icon = package::msi_package_authoring(
        Path::new("C:/review & source/payload"),
        "1.2.3",
        &[
            "tallyline.exe".into(),
            "assets/branding/tallyline.ico".into(),
        ],
    )
    .unwrap();
    assert!(icon.find("<Icon ").unwrap() < icon.find("<StandardDirectory ").unwrap());
    assert!(
        icon.replace('\\', "/").contains(
            "SourceFile=\"C:/review &amp; source/payload/assets/branding/tallyline.ico\""
        )
    );
    assert!(
        icon.replace('\\', "/")
            .contains("<Property Id=\"ARPPRODUCTICON\" Value=\"TallylineIcon.exe\"/>")
    );
    assert!(icon.contains("<Icon Id=\"TallylineIcon.exe\" SourceFile=\""));
    let shortcut = &icon;
    assert!(shortcut.contains("Icon=\"TallylineIcon.exe\" IconIndex=\"0\""));
    assert!(shortcut.contains("Advertise=\"yes\""));
}

#[test]
fn r_ui_9_r_sec_10__packaged_branding_matches_the_reviewed_exports() {
    assert_eq!(
        xtask::hashlist::sha256_hex(include_bytes!("../../branding/tallyline-icon-256.png")),
        "d0ed9a91e6e5fd79633352b1f319e8f2c96c0ff1159c371a9354857ab5d8108f"
    );
    assert_eq!(
        xtask::hashlist::sha256_hex(include_bytes!("../../branding/tallyline.ico")),
        "13c3bf7fd049940f4ae4ec656ca548491ad22aa5760ef2562a2d072cad4b2879"
    );
}
