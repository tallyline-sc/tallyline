// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared capture image intake contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_files::api::model::{
    HdrContainer, ImageIntake, ImageIntakeSettings, SetAsideReason,
};
use tallyline_adapter_files::api::read_image;

#[test]
fn hdr_originals_and_unknown_content_are_set_aside_from_bytes() {
    for (bytes, container) in [
        (b"II\xbc\x01".as_slice(), HdrContainer::JpegXr),
        (b"\x76\x2f\x31\x01".as_slice(), HdrContainer::OpenExr),
    ] {
        assert_eq!(
            read_image(bytes, &ImageIntakeSettings::default()),
            ImageIntake::SetAside(SetAsideReason::HdrFormat(container))
        );
    }
    assert_eq!(
        read_image(b"GIF89a", &ImageIntakeSettings::default()),
        ImageIntake::SetAside(SetAsideReason::UnsupportedFormat)
    );
}
#[test]
fn r_cap_8__sixteen_bit_png_samples_use_full_precision() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[1, 255, 127, 128, 255, 255])
            .unwrap();
    }
    let ImageIntake::Decoded(image) = read_image(&bytes, &ImageIntakeSettings::default()) else {
        panic!("supported PNG was set aside")
    };
    assert_eq!(image.raster.argb(), &[0xff02_7fff]);
    assert_eq!(image.downscaled, None);
}
#[test]
fn plain_bmp_is_decoded_from_content() {
    let mut bytes = Vec::new();
    let image = image::RgbImage::from_raw(1, 1, vec![10, 20, 30]).unwrap();
    image::DynamicImage::ImageRgb8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Bmp,
        )
        .unwrap();
    let ImageIntake::Decoded(decoded) = read_image(&bytes, &ImageIntakeSettings::default()) else {
        panic!("supported BMP was set aside")
    };
    assert_eq!(decoded.raster.argb(), &[0xff0a_141e]);
}
#[test]
fn jpeg_is_decoded_in_strict_mode() {
    let mut bytes = Vec::new();
    jpeg_encoder::Encoder::new(&mut bytes, 100)
        .encode(&[10, 20, 30], 1, 1, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    assert!(matches!(
        read_image(&bytes, &ImageIntakeSettings::default()),
        ImageIntake::Decoded(_)
    ));
    bytes.truncate(bytes.len() / 2);
    assert!(matches!(
        read_image(&bytes, &ImageIntakeSettings::default()),
        ImageIntake::SetAside(SetAsideReason::DecodeFailed(_))
    ));
}
#[test]
fn r_ocr_17__integer_boxes_include_edge_pixels_and_record_original_dimensions() {
    let pixels: Vec<u8> = [0, 60, 120, 180, 240]
        .into_iter()
        .cycle()
        .take(15)
        .flat_map(|v| [v, v, v])
        .collect();
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 5, 3);
        encoder.set_color(png::ColorType::Rgb);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    }
    let settings =
        ImageIntakeSettings::new(200 * 1024 * 1024, 100_000_000, 1024 * 1024, 4).unwrap();
    let ImageIntake::Decoded(image) = read_image(&bytes, &settings) else {
        panic!("supported PNG was set aside")
    };
    assert_eq!((image.raster.width(), image.raster.height()), (2, 1));
    assert_eq!(image.raster.argb(), &[0xff3c_3c3c, 0xffd2_d2d2]);
    assert_eq!(
        image.downscaled,
        Some(tallyline_adapter_files::api::model::Downscaled {
            original_width: 5,
            original_height: 3,
            factor: 3
        })
    );
}
#[test]
fn image_files_are_size_checked_before_their_contents_are_read() {
    let directory = std::env::temp_dir().join(format!("tallyline-image-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("capture.png");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(1024 * 1024).unwrap();
    let settings = ImageIntakeSettings::new(100, 100_000_000, 1024 * 1024, 20_000_000).unwrap();
    assert_eq!(
        tallyline_adapter_files::api::read_image_file(&path, &settings),
        ImageIntake::SetAside(SetAsideReason::FileTooLarge)
    );
    drop(file);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
#[test]
fn oversized_headers_are_rejected_before_pixel_decode() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_chunk(
                png::chunk::ChunkType(*b"tEXt"),
                &vec![b'x'; 4 * 1024 * 1024],
            )
            .unwrap();
        writer.write_image_data(&[0, 0, 0]).unwrap();
    }
    assert_eq!(
        read_image(&bytes, &ImageIntakeSettings::default()),
        ImageIntake::SetAside(SetAsideReason::HeaderTooLarge)
    );
}
#[test]
fn oversized_jpeg_icc_segments_are_rejected_before_codec_header_allocation() {
    let mut bytes = vec![255, 216, 255, 226];
    let mut profile = b"ICC_PROFILE\0\x01\x01".to_vec();
    profile.extend_from_slice(&[0; 2048]);
    bytes.extend_from_slice(&u16::try_from(profile.len() + 2).unwrap().to_be_bytes());
    bytes.extend_from_slice(&profile);
    let settings =
        ImageIntakeSettings::new(200 * 1024 * 1024, 100_000_000, 1024, 20_000_000).unwrap();
    assert_eq!(
        read_image(&bytes, &settings),
        ImageIntake::SetAside(SetAsideReason::ProfileTooLarge)
    );
}
#[test]
fn bmp_dimensions_are_bounded_before_the_codec_reads_metadata() {
    let mut bytes = vec![0; 54];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&20_000u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&20_000u32.to_le_bytes());
    assert_eq!(
        read_image(&bytes, &ImageIntakeSettings::default()),
        ImageIntake::SetAside(SetAsideReason::TooManyPixels)
    );
}

#[test]
fn public_corpus_png_bit_depth_variants_differ_by_at_most_one() {
    let ImageIntake::Decoded(source) = read_image(
        include_bytes!("../../../corpus/public/pyro-gateway-stanton-01/buy-1.jpg"),
        &ImageIntakeSettings::default(),
    ) else {
        panic!(
            "public corpus JPEG failed: {:?}",
            read_image(
                include_bytes!("../../../corpus/public/pyro-gateway-stanton-01/buy-1.jpg"),
                &ImageIntakeSettings::default()
            )
        )
    };
    let rgb: Vec<u8> = source
        .raster
        .argb()
        .iter()
        .flat_map(|pixel| pixel.to_be_bytes().into_iter().skip(1))
        .collect();
    let make_png = |depth, pixels: &[u8]| {
        let mut bytes = Vec::new();
        {
            let mut encoder =
                png::Encoder::new(&mut bytes, source.raster.width(), source.raster.height());
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(depth);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        }
        bytes
    };
    let sixteen: Vec<u8> = rgb
        .iter()
        .flat_map(|&v| (u16::from(v) * 257).to_be_bytes())
        .collect();
    let ImageIntake::Decoded(eight) = read_image(
        &make_png(png::BitDepth::Eight, &rgb),
        &ImageIntakeSettings::default(),
    ) else {
        panic!("eight bit PNG failed")
    };
    let ImageIntake::Decoded(sixteen) = read_image(
        &make_png(png::BitDepth::Sixteen, &sixteen),
        &ImageIntakeSettings::default(),
    ) else {
        panic!("sixteen bit PNG failed")
    };
    for (&a, &b) in eight.raster.argb().iter().zip(sixteen.raster.argb()) {
        assert!(
            a.to_be_bytes()
                .into_iter()
                .zip(b.to_be_bytes())
                .all(|(x, y)| x.abs_diff(y) <= 1)
        );
    }
}

#[test]
fn colour_metadata_after_png_pixels_is_set_aside() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0, 0, 0]).unwrap();
        writer
            .write_chunk(png::chunk::ChunkType(*b"cICP"), &[9, 16, 0, 1])
            .unwrap();
        writer.finish().unwrap();
    }
    assert!(matches!(
        read_image(&bytes, &ImageIntakeSettings::default()),
        ImageIntake::SetAside(SetAsideReason::HdrOrUnsupportedEncoding(_))
    ));
}

#[test]
fn unsupported_colour_explains_the_sdr_copy_and_is_not_retryable() {
    let reason = SetAsideReason::HdrOrUnsupportedEncoding(
        tallyline_adapter_files::api::model::Encoding::IccHdr,
    );
    assert_eq!(
        reason.message(),
        "HDR or unsupported colour encoding – use an SDR copy"
    );
    assert!(!reason.retryable());
    assert!(SetAsideReason::DecodeFailed("partial image".into()).retryable());
}

#[test]
fn compatible_hdr_brands_and_both_jpeg_xr_headers_are_recognised() {
    let cases: [(&[u8], HdrContainer); 5] = [
        (b"II\xbc\x00\x08\0\0\0", HdrContainer::JpegXr),
        (b"II\xbc\x01\x08\0\0\0", HdrContainer::JpegXr),
        (b"\0\0\0\x14ftypmif1\0\0\0\0avif", HdrContainer::Avif),
        (b"\0\0\0\x10ftypavis\0\0\0\0", HdrContainer::Avif),
        (b"\0\0\0\x14ftypmif1\0\0\0\0heic", HdrContainer::Heif),
    ];
    for (bytes, container) in cases {
        assert_eq!(
            read_image(bytes, &ImageIntakeSettings::default()),
            ImageIntake::SetAside(SetAsideReason::HdrFormat(container))
        );
    }
}

#[test]
fn jpeg_file_with_a_png_extension_decodes_as_jpeg() {
    let directory = std::env::temp_dir().join(format!("tallyline-magic-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("capture.png");
    let mut bytes = Vec::new();
    jpeg_encoder::Encoder::new(&mut bytes, 100)
        .encode(&[10, 20, 30], 1, 1, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    std::fs::write(&path, bytes).unwrap();
    let result =
        tallyline_adapter_files::api::read_image_file(&path, &ImageIntakeSettings::default());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
    let ImageIntake::Decoded(image) = result else {
        panic!("misnamed JPEG was not decoded")
    };
    assert_eq!(
        image.format,
        tallyline_adapter_files::api::model::ImageFormat::Jpeg
    );
}
