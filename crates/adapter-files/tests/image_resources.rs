// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit large-image resource contracts, run in a separately measured process.

#![cfg(test)]

use tallyline_adapter_files::api::model::{Downscaled, ImageIntake, ImageIntakeSettings};

use tallyline_adapter_files::api::read_image;

const WIDTH: u32 = 7680;
const HEIGHT: u32 = 4320;

fn assert_reduced(bytes: &[u8]) {
    let ImageIntake::Decoded(image) = read_image(bytes, &ImageIntakeSettings::default()) else {
        panic!("large image failed")
    };
    assert_eq!((image.raster.width(), image.raster.height()), (3840, 2160));
    assert_eq!(
        image.downscaled,
        Some(Downscaled {
            original_width: WIDTH,
            original_height: HEIGHT,
            factor: 2
        })
    );
    assert!(
        image
            .raster
            .argb()
            .iter()
            .all(|pixel| *pixel == 0xff80_8080)
    );
}

#[test]
#[ignore = "run in a separately measured process for R-OCR-17 and R-NF-3"]
fn large_sixteen_bit_png_resource_contract() {
    use std::io::Write;
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, WIDTH, HEIGHT);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder.write_header().unwrap();
        let mut stream = writer.stream_writer().unwrap();
        let row = vec![0x80; WIDTH as usize * 6];
        for _ in 0..HEIGHT {
            stream.write_all(&row).unwrap();
        }
        stream.finish().unwrap();
    }
    assert_reduced(&bytes);
}

#[test]
#[ignore = "run in a separately measured process for R-OCR-17 and R-NF-3"]
fn large_jpeg_resource_contract() {
    let source = vec![128; WIDTH as usize * HEIGHT as usize * 3];
    let mut bytes = Vec::new();
    jpeg_encoder::Encoder::new(&mut bytes, 100)
        .encode(
            &source,
            WIDTH as u16,
            HEIGHT as u16,
            jpeg_encoder::ColorType::Rgb,
        )
        .unwrap();
    drop(source);
    assert_reduced(&bytes);
}

#[test]
#[ignore = "run in a separately measured process for R-OCR-17 and R-NF-3"]
fn large_bmp_resource_contract() {
    let source = vec![128; WIDTH as usize * HEIGHT as usize * 3];
    let mut bytes = Vec::new();
    image::codecs::bmp::BmpEncoder::new(&mut bytes)
        .encode(&source, WIDTH, HEIGHT, image::ExtendedColorType::Rgb8)
        .unwrap();
    drop(source);
    assert_reduced(&bytes);
}
