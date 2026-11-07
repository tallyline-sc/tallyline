// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conservative BMP input and literal TSV evidence.

use serde_json::{Value, json};
use tallyline_adapter_files::api::{
    model::{ImageFormat, ImageIntake, ImageIntakeSettings},
    read_image,
};

pub(super) const MAX_TSV: usize = 8 * 1024 * 1024;
const MAX_ROW: usize = 1024 * 1024;
const MAX_PIXELS: u64 = 16_777_216;
const MAX_EDGE: u32 = 16_384;
const HEADER: &str =
    "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext";

fn dimensions(width: u32, height: u32) -> Result<usize, String> {
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err("bitmap dimensions exceed bounds".to_owned());
    }
    usize::try_from((u64::from(width) * 3 + 3) & !3)
        .map_err(|_| "bitmap stride overflow".to_owned())
}

fn little(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let array: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or("truncated bitmap")?
        .try_into()
        .map_err(|_| "truncated bitmap")?;
    Ok(u32::from_le_bytes(array))
}

pub(super) fn validate_bmp(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.get(..2) != Some(b"BM")
        || little(bytes, 10)? != 54
        || little(bytes, 14)? != 40
        || little(bytes, 26)? != 0x0018_0001
        || little(bytes, 30)? != 0
        || bytes.get(6..10) != Some(&[0; 4])
    {
        return Err("only canonical 24bpp BI_RGB BMP is accepted".to_owned());
    }
    let (width, height) = (little(bytes, 18)?, little(bytes, 22)?);
    let stride = dimensions(width, height)?;
    let payload = stride
        .checked_mul(height as usize)
        .ok_or("bitmap size overflow")?;
    if little(bytes, 2)? as usize != bytes.len()
        || bytes.len() != 54 + payload
        || little(bytes, 34)? as usize != payload
        || little(bytes, 46)? != 0
        || little(bytes, 50)? != 0
    {
        return Err("bitmap length or palette is invalid".to_owned());
    }
    let used = width as usize * 3;
    for row in bytes
        .get(54..)
        .ok_or("truncated bitmap")?
        .chunks_exact(stride)
    {
        if row
            .get(used..)
            .ok_or("truncated bitmap")?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err("bitmap padding must be zero".to_owned());
        }
    }
    Ok((width, height))
}

pub(super) fn encode_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err("research input must be PNG".to_owned());
    }
    let settings = ImageIntakeSettings::new(200 * 1024 * 1024, MAX_PIXELS, 1024 * 1024, MAX_PIXELS)
        .map_err(|_| "invalid fixed image settings")?;
    let image = match read_image(bytes, &settings) {
        ImageIntake::Decoded(image)
            if image.downscaled.is_none() && image.format == ImageFormat::Png =>
        {
            image
        }
        ImageIntake::Decoded(_) | ImageIntake::SetAside(_) => {
            return Err("input image failed bounded unscaled SDR decoding".to_owned());
        }
    };
    let (width, height) = (image.raster.width(), image.raster.height());
    let stride = dimensions(width, height)?;
    let payload = stride * height as usize;
    let mut bmp = Vec::with_capacity(54 + payload);
    bmp.extend(b"BM");
    bmp.extend(
        u32::try_from(54 + payload)
            .map_err(|_| "bitmap size overflow")?
            .to_le_bytes(),
    );
    bmp.extend([0; 4]);
    for value in [
        54u32,
        40,
        width,
        height,
        0x0018_0001,
        0,
        payload as u32,
        0,
        0,
        0,
        0,
    ] {
        bmp.extend(value.to_le_bytes());
    }
    for row in image.raster.argb().chunks_exact(width as usize).rev() {
        for pixel in row {
            let [_, r, g, b] = pixel.to_be_bytes();
            bmp.extend([b, g, r]);
        }
        bmp.resize(bmp.len() + stride - width as usize * 3, 0);
    }
    validate_bmp(&bmp)?;
    Ok(bmp)
}

pub(super) fn parse_tsv(bytes: &[u8], width: u32, height: u32) -> Result<Value, String> {
    if bytes.len() > MAX_TSV || !bytes.ends_with(b"\n") {
        return Err("TSV is oversized or incomplete".to_owned());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "TSV is not UTF-8")?;
    let mut lines = text
        .split_terminator('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    if lines.next() != Some(HEADER) {
        return Err("TSV header differs from the fixed contract".to_owned());
    }
    let mut words = Vec::new();
    let mut page_seen = false;
    for line in lines {
        if line.len() > MAX_ROW {
            return Err("TSV row exceeds bounds".to_owned());
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [
            level,
            page,
            block,
            paragraph,
            line_number,
            word,
            left,
            top,
            box_width,
            box_height,
            confidence,
            literal,
        ] = fields.as_slice()
        else {
            return Err("TSV row must have twelve columns".to_owned());
        };
        let numbers = [
            *level,
            *page,
            *block,
            *paragraph,
            *line_number,
            *word,
            *left,
            *top,
            *box_width,
            *box_height,
        ]
        .into_iter()
        .map(|value| {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("TSV integer is invalid".to_owned());
            }
            value
                .parse::<u32>()
                .map_err(|_| "TSV integer overflow".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
        let [
            level,
            page,
            block,
            paragraph,
            line_number,
            word,
            left,
            top,
            box_width,
            box_height,
        ] = numbers.as_slice()
        else {
            return Err("TSV integer count is invalid".to_owned());
        };
        let conf = confidence
            .parse::<f64>()
            .map_err(|_| "TSV confidence is invalid")?;
        if !(1..=5).contains(level)
            || *page != 1
            || !conf.is_finite()
            || left.checked_add(*box_width).is_none_or(|edge| edge > width)
            || top
                .checked_add(*box_height)
                .is_none_or(|edge| edge > height)
            || *box_width == 0
            || *box_height == 0
            || literal.chars().any(char::is_control)
        {
            return Err("TSV geometry, level or text is invalid".to_owned());
        }
        if *level == 1 {
            if page_seen
                || *block != 0
                || *paragraph != 0
                || *line_number != 0
                || *word != 0
                || *left != 0
                || *top != 0
                || *box_width != width
                || *box_height != height
            {
                return Err("TSV page does not match input".to_owned());
            }
            page_seen = true;
        } else if !page_seen {
            return Err("TSV page row must precede content".to_owned());
        }
        let hierarchy_valid = match *level {
            1 => true,
            2 => *block > 0 && *paragraph == 0 && *line_number == 0 && *word == 0,
            3 => *block > 0 && *paragraph > 0 && *line_number == 0 && *word == 0,
            4 => *block > 0 && *paragraph > 0 && *line_number > 0 && *word == 0,
            5 => *block > 0 && *paragraph > 0 && *line_number > 0 && *word > 0,
            _ => false,
        };
        if !hierarchy_valid {
            return Err("TSV hierarchy is inconsistent with its level".to_owned());
        }
        if *level == 5 {
            if *block == 0
                || *paragraph == 0
                || *line_number == 0
                || *word == 0
                || !(0.0..=100.0).contains(&conf)
                || literal.is_empty()
            {
                return Err("TSV word evidence is invalid".to_owned());
            }
            words.push(json!({"text": literal, "left":left,"top":top,"width":box_width,"height":box_height,"confidence":confidence,"block":block,"paragraph":paragraph,"line":line_number,"word":word}));
        } else if conf != -1.0 || !literal.is_empty() {
            return Err("TSV structural row has unexpected evidence".to_owned());
        }
    }
    if !page_seen {
        return Err("TSV page row is missing".to_owned());
    }
    Ok(
        json!({"width":width,"height":height,"words":words,"raw_sha256":super::files::hash(bytes),"calibrated":false}),
    )
}
