// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Native desktop metadata for the approved product artwork.

use std::io;
use std::path::Path;

/// Returns the Debian application entry with its installed icon name.
#[must_use]
pub fn debian_desktop_entry() -> &'static str {
    "[Desktop Entry]\nType=Application\nName=Tallyline\nComment=Star Citizen commodity terminal data client\nExec=tallyline\nIcon=tallyline\nTerminal=false\n"
}

/// Authors the Windows application icon and installed-programmes identity.
///
/// # Errors
/// Returns a non-UTF-8 icon source path.
pub(super) fn msi_icon_authoring(payload: &Path) -> io::Result<String> {
    let path = payload.join("assets/branding/tallyline.ico");
    let source = path
        .to_str()
        .ok_or_else(|| io::Error::other("MSI icon source path is not UTF-8"))?;
    let source = escape(source);
    Ok(format!(
        "<Icon Id=\"TallylineIcon.exe\" SourceFile=\"{source}\"/><Property Id=\"ARPPRODUCTICON\" Value=\"TallylineIcon.exe\"/>"
    ))
}

/// Authors the advertised Windows Start menu shortcut with its product icon.
#[must_use]
pub(super) fn msi_shortcut_authoring() -> &'static str {
    "<Shortcut Id=\"StartMenuShortcut\" Directory=\"ProgramMenuFolder\" Name=\"Tallyline\" Advertise=\"yes\" WorkingDirectory=\"INSTALLFOLDER\" Icon=\"TallylineIcon.exe\" IconIndex=\"0\"/>"
}

pub(super) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
