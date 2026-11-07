// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bundled artwork decoding against an independent image entry point.

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_ui_19_q_app__bundled_logo_decoding_preserves_every_rgba_pixel()
-> Result<(), Box<dyn std::error::Error>> {
    for bytes in [
        include_bytes!("../../../docs/images/fankit/made-by-the-community.png").as_slice(),
        include_bytes!("../../../docs/images/fankit/MadeByTheCommunity_White.png").as_slice(),
    ] {
        let actual = tallyline_adapter_files::api::decode_logo(bytes)?;
        let expected =
            image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.to_rgba8();
        assert_eq!(actual.width, expected.width());
        assert_eq!(actual.height, expected.height());
        assert_eq!(actual.rgba, *expected.as_raw());
    }
    Ok(())
}
