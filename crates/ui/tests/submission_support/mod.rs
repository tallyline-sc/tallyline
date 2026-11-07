// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Existing approved font resources for headless submission views.

pub fn font_files() -> std::io::Result<tallyline_ui::api::model::FontFiles> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let read = |name: &str| std::fs::read(root.join(name));
    Ok(tallyline_ui::api::model::FontFiles {
        material_symbols: read("material-symbols/MaterialSymbolsRounded-Tallyline.ttf")?
            .try_into()
            .map_err(std::io::Error::other)?,
        plex_regular: read("ibm-plex/IBMPlexSans-Regular.ttf")?,
        plex_semibold: read("ibm-plex/IBMPlexSans-SemiBold.ttf")?,
        plex_bold: read("ibm-plex/IBMPlexSans-Bold.ttf")?,
        mono_regular: read("jetbrains-mono/JetBrainsMonoNL-Regular.ttf")?,
        mono_bold: read("jetbrains-mono/JetBrainsMonoNL-Bold.ttf")?,
    })
}
