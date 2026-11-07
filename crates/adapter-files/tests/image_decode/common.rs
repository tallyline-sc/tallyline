// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixture builders: a synthetic source image, hand-written ICC profiles and PNG, JPEG and BMP writers.

#![cfg(test)]
use std::borrow::Cow;

use tallyline_adapter_files::api::model::{
    ImageIntake as Intake, ImageIntakeSettings as IntakeLimits, SetAsideReason,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SrgbRaster {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 48;

pub const SRGB_D50: [[f64; 3]; 3] = [
    [0.436_074_7, 0.222_504_5, 0.013_932_2],
    [0.385_064_9, 0.716_878_6, 0.097_104_5],
    [0.143_080_4, 0.060_616_9, 0.714_173_3],
];

pub const DISPLAY_P3_D50: [[f64; 3]; 3] = [
    [0.515_102, 0.241_182, -0.001_049_41],
    [0.291_965, 0.692_236, 0.041_881_8],
    [0.157_153, 0.066_581_9, 0.784_378],
];

pub const LINEAR_SRGB_TO_LINEAR_P3: [[f64; 3]; 3] = [
    [0.822_462_1, 0.177_538_0, 0.0],
    [0.033_194_1, 0.966_805_8, 0.0],
    [0.017_082_7, 0.072_397_4, 0.910_519_9],
];

pub enum Trc {
    Gamma(f64),
    Srgb,
    Samples(Vec<u16>),
}

pub fn source() -> SrgbRaster {
    let mut rgb = Vec::with_capacity((WIDTH * HEIGHT * 3) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let r = (x * 255 / (WIDTH - 1)) as u8;
            let g = (y * 255 / (HEIGHT - 1)) as u8;
            let b = ((x + y) * 255 / (WIDTH + HEIGHT - 2)) as u8;
            rgb.extend_from_slice(&[r, g, 255 - b]);
        }
    }
    SrgbRaster {
        width: WIDTH,
        height: HEIGHT,
        rgb,
    }
}

pub fn srgb_eotf(v: f64) -> f64 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn srgb_oetf(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub fn map_samples_16(source: &SrgbRaster, f: impl Fn(f64) -> f64) -> Vec<u16> {
    source
        .rgb
        .iter()
        .map(|&v| {
            (f(f64::from(v) / 255.0) * 65535.0)
                .round()
                .clamp(0.0, 65535.0) as u16
        })
        .collect()
}

pub fn map_samples_8(source: &SrgbRaster, f: impl Fn(f64) -> f64) -> Vec<u8> {
    source
        .rgb
        .iter()
        .map(|&v| (f(f64::from(v) / 255.0) * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect()
}

pub fn to_display_p3_8(source: &SrgbRaster) -> Vec<u8> {
    convert_8(&source.rgb, LINEAR_SRGB_TO_LINEAR_P3)
}

pub const LINEAR_P3_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
    [1.224_940_2, -0.224_940_2, 0.0],
    [-0.042_056_9, 1.042_056_9, 0.0],
    [-0.019_637_6, -0.078_636_1, 1.098_273_7],
];

pub fn convert_8(rgb: &[u8], m: [[f64; 3]; 3]) -> Vec<u8> {
    rgb.as_chunks::<3>()
        .0
        .iter()
        .flat_map(|px| {
            let lin = [
                srgb_eotf(f64::from(px[0]) / 255.0),
                srgb_eotf(f64::from(px[1]) / 255.0),
                srgb_eotf(f64::from(px[2]) / 255.0),
            ];
            let mut out = [0u8; 3];
            for (row, o) in m.iter().zip(out.iter_mut()) {
                let v = row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2];
                *o = (srgb_oetf(v) * 255.0).round() as u8;
            }
            out
        })
        .collect()
}

pub fn exact_srgb_of_p3(p3: &[u8]) -> SrgbRaster {
    SrgbRaster {
        width: WIDTH,
        height: HEIGHT,
        rgb: convert_8(p3, LINEAR_P3_TO_LINEAR_SRGB),
    }
}

pub fn be_samples(samples: &[u16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_be_bytes()).collect()
}

fn s15f16(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(xyz: [f64; 3]) -> Vec<u8> {
    let mut tag = b"XYZ \0\0\0\0".to_vec();
    for v in xyz {
        tag.extend_from_slice(&s15f16(v));
    }
    tag
}

fn trc_tag(trc: &Trc) -> Vec<u8> {
    match trc {
        Trc::Samples(samples) => {
            let mut tag = b"curv\0\0\0\0".to_vec();
            tag.extend_from_slice(&u32::try_from(samples.len()).unwrap().to_be_bytes());
            for sample in samples {
                tag.extend_from_slice(&sample.to_be_bytes());
            }
            while !tag.len().is_multiple_of(4) {
                tag.push(0);
            }
            tag
        }
        Trc::Gamma(g) => {
            let mut tag = b"curv\0\0\0\0".to_vec();
            tag.extend_from_slice(&1u32.to_be_bytes());
            tag.extend_from_slice(&((g * 256.0).round() as u16).to_be_bytes());
            tag.extend_from_slice(&[0, 0]);
            tag
        }
        Trc::Srgb => {
            let mut tag = b"para\0\0\0\0".to_vec();
            tag.extend_from_slice(&3u16.to_be_bytes());
            tag.extend_from_slice(&[0, 0]);
            for v in [2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.040_45] {
                tag.extend_from_slice(&s15f16(v));
            }
            tag
        }
    }
}

fn desc_tag(text: &str) -> Vec<u8> {
    let mut tag = b"desc\0\0\0\0".to_vec();
    let len = u32::try_from(text.len() + 1).unwrap_or(u32::MAX);
    tag.extend_from_slice(&len.to_be_bytes());
    tag.extend_from_slice(text.as_bytes());
    tag.push(0);
    tag.extend_from_slice(&[0; 4 + 4 + 2 + 1 + 67]);
    while !tag.len().is_multiple_of(4) {
        tag.push(0);
    }
    tag
}

pub fn icc_matrix_trc(colorants: [[f64; 3]; 3], trc: &Trc) -> Vec<u8> {
    let trc = trc_tag(trc);
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc_tag("fixture")),
        (b"wtpt", xyz_tag([0.9642, 1.0, 0.8249])),
        (b"rXYZ", xyz_tag(colorants[0])),
        (b"gXYZ", xyz_tag(colorants[1])),
        (b"bXYZ", xyz_tag(colorants[2])),
        (b"rTRC", trc.clone()),
        (b"gTRC", trc.clone()),
        (b"bTRC", trc),
    ];
    let table_len = 4 + 12 * tags.len();
    let mut offset = 128 + table_len;
    let mut table = Vec::new();
    let mut data = Vec::new();
    table.extend_from_slice(&u32::try_from(tags.len()).unwrap_or(0).to_be_bytes());
    for (sig, body) in &tags {
        table.extend_from_slice(*sig);
        table.extend_from_slice(&u32::try_from(offset).unwrap_or(0).to_be_bytes());
        table.extend_from_slice(&u32::try_from(body.len()).unwrap_or(0).to_be_bytes());
        data.extend_from_slice(body);
        offset += body.len();
    }
    let total = u32::try_from(offset).unwrap_or(0);
    let mut header = Vec::with_capacity(128);
    header.extend_from_slice(&total.to_be_bytes());
    header.extend_from_slice(&[0; 4]);
    header.extend_from_slice(&[2, 0x10, 0, 0]);
    header.extend_from_slice(b"mntrRGB XYZ ");
    header.extend_from_slice(&[0; 12]);
    header.extend_from_slice(b"acsp");
    header.extend_from_slice(&[0; 4 + 4 + 4 + 4 + 8 + 4]);
    for v in [0.9642, 1.0, 0.8249] {
        header.extend_from_slice(&s15f16(v));
    }
    header.extend_from_slice(&[0; 4 + 16 + 28]);
    assert_eq!(header.len(), 128);
    [header, table, data].concat()
}

pub struct PngSpec<'a> {
    pub depth: png::BitDepth,
    pub color: png::ColorType,
    pub width: u32,
    pub height: u32,
    pub data: &'a [u8],
    pub icc: Option<Vec<u8>>,
    pub chunks: Vec<([u8; 4], Vec<u8>)>,
}

impl<'a> PngSpec<'a> {
    pub fn rgb8(data: &'a [u8]) -> Self {
        Self {
            depth: png::BitDepth::Eight,
            color: png::ColorType::Rgb,
            width: WIDTH,
            height: HEIGHT,
            data,
            icc: None,
            chunks: Vec::new(),
        }
    }

    pub fn rgb16(data: &'a [u8]) -> Self {
        Self {
            depth: png::BitDepth::Sixteen,
            ..Self::rgb8(data)
        }
    }

    pub fn chunk(mut self, kind: &[u8; 4], body: &[u8]) -> Self {
        self.chunks.push((*kind, body.to_vec()));
        self
    }

    pub fn icc(mut self, profile: Vec<u8>) -> Self {
        self.icc = Some(profile);
        self
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut info = png::Info::with_size(self.width, self.height);
        info.bit_depth = self.depth;
        info.color_type = self.color;
        info.icc_profile = self.icc.clone().map(Cow::Owned);
        let encoder = match png::Encoder::with_info(&mut out, info) {
            Ok(e) => e,
            Err(e) => panic!("encoder: {e}"),
        };
        let mut writer = match encoder.write_header() {
            Ok(w) => w,
            Err(e) => panic!("header: {e}"),
        };
        for (kind, body) in &self.chunks {
            if let Err(e) = writer.write_chunk(png::chunk::ChunkType(*kind), body) {
                panic!("chunk: {e}");
            }
        }
        if let Err(e) = writer.write_image_data(self.data) {
            panic!("data: {e}");
        }
        if let Err(e) = writer.finish() {
            panic!("finish: {e}");
        }
        out
    }
}

pub fn chromaticities(
    white: (u32, u32),
    red: (u32, u32),
    green: (u32, u32),
    blue: (u32, u32),
) -> Vec<u8> {
    [white, red, green, blue]
        .iter()
        .flat_map(|(x, y)| [x.to_be_bytes(), y.to_be_bytes()].concat())
        .collect()
}

pub fn bt709_chromaticities() -> Vec<u8> {
    chromaticities(
        (31270, 32900),
        (64000, 33000),
        (30000, 60000),
        (15000, 6000),
    )
}

pub fn p3_chromaticities() -> Vec<u8> {
    chromaticities(
        (31270, 32900),
        (68000, 32000),
        (26500, 69000),
        (15000, 6000),
    )
}

pub fn jpeg(
    data: &[u8],
    sampling: jpeg_encoder::SamplingFactor,
    progressive: bool,
    icc: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, 95);
    encoder.set_sampling_factor(sampling);
    encoder.set_progressive(progressive);
    if let Some(profile) = icc
        && let Err(e) = encoder.add_icc_profile(profile)
    {
        panic!("icc: {e}");
    }
    if let Err(e) = encoder.encode(
        data,
        WIDTH as u16,
        HEIGHT as u16,
        jpeg_encoder::ColorType::Rgb,
    ) {
        panic!("jpeg: {e}");
    }
    out
}

pub fn read_default(bytes: &[u8]) -> Intake {
    tallyline_adapter_files::api::read_image(bytes, &IntakeLimits::default())
}

pub fn set_aside(bytes: &[u8]) -> SetAsideReason {
    match read_default(bytes) {
        Intake::SetAside(reason) => reason,
        Intake::Decoded(_) => panic!("expected the file to be set aside"),
    }
}

pub fn decoded(intake: Intake) -> SrgbRaster {
    match intake {
        Intake::Decoded(image) => SrgbRaster {
            width: image.raster.width(),
            height: image.raster.height(),
            rgb: image
                .raster
                .argb()
                .iter()
                .flat_map(|pixel| pixel.to_be_bytes().into_iter().skip(1))
                .collect(),
        },
        Intake::SetAside(reason) => panic!("expected a decoded raster, got set aside: {reason:?}"),
    }
}

pub fn max_abs_diff(a: &SrgbRaster, b: &SrgbRaster) -> u8 {
    assert_eq!((a.width, a.height), (b.width, b.height));
    a.rgb
        .iter()
        .zip(&b.rgb)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

pub fn mean_abs_diff(a: &SrgbRaster, b: &SrgbRaster) -> f64 {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let sum: u64 = a
        .rgb
        .iter()
        .zip(&b.rgb)
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    sum as f64 / a.rgb.len() as f64
}
