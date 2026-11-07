// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict independent supplied-geometry manifest parsing.

use serde_json::Value;
use tallyline_recognition::api::model::{
    GeometryProvenance, PanelGeometry, PanelLimits, SuppliedPanels,
};

pub(super) const MAX_METADATA_BYTES: u64 = 65_536;
pub(super) struct Manifest {
    pub(super) supplied: SuppliedPanels,
    pub(super) limits: PanelLimits,
    pub(super) max_output_bytes: usize,
    pub(super) dimensions: [u32; 2],
    pub(super) image_hash: String,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Manifest, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid geometry JSON")?;
    fields(
        &value,
        &[
            "schemaVersion",
            "imageSha256",
            "rasterDimensions",
            "provenance",
            "shop",
            "location",
            "header",
            "limits",
        ],
    )?;
    if field(&value, "schemaVersion")? == 1 {
        return Err(
            "geometry schema 1 is obsolete; regenerate and review source-bound schema 2 geometry"
                .into(),
        );
    }
    if field(&value, "schemaVersion")? != 2 {
        return Err("unsupported geometry schema".into());
    }
    let image_hash = field(&value, "imageSha256")?
        .as_str()
        .filter(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or("invalid source hash")?
        .to_owned();
    let provenance = match field(&value, "provenance")?.as_str() {
        Some("manual") => GeometryProvenance::Manual,
        Some("profile") => GeometryProvenance::Profile,
        _ => return Err("supplied geometry must be manual or profile".into()),
    };
    let limits = field(&value, "limits")?;
    fields(
        limits,
        &[
            "maxSide",
            "maxTotalPixels",
            "maxSampledPixels",
            "maxOutputBytes",
        ],
    )?;
    let max_output_bytes = usize::try_from(integer(limits, "maxOutputBytes")?)
        .map_err(|_| "output byte limit is excessive")?;
    if max_output_bytes == 0 || max_output_bytes > 32 * 1024 * 1024 {
        return Err("output byte limit is excessive".into());
    }
    Ok(Manifest {
        supplied: SuppliedPanels {
            shop: panel(field(&value, "shop")?)?,
            location: panel(field(&value, "location")?)?,
            header: quad(field(&value, "header")?)?,
            provenance,
        },
        limits: PanelLimits::new(
            u32::try_from(integer(limits, "maxSide")?).map_err(|_| "side limit is excessive")?,
            integer(limits, "maxTotalPixels")?,
            integer(limits, "maxSampledPixels")?,
        )
        .map_err(|_| "invalid panel limits")?,
        max_output_bytes,
        dimensions: dimensions(field(&value, "rasterDimensions")?)?,
        image_hash,
    })
}

fn panel(value: &Value) -> Result<PanelGeometry, String> {
    fields(value, &["corners", "output"])?;
    Ok(PanelGeometry {
        corners: quad(field(value, "corners")?)?,
        output: dimensions(field(value, "output")?)?,
    })
}
fn dimensions(value: &Value) -> Result<[u32; 2], String> {
    let values = value
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or("dimensions need two integers")?;
    let mut output = [0; 2];
    for (out, value) in output.iter_mut().zip(values) {
        *out = u32::try_from(value.as_u64().ok_or("invalid dimension")?)
            .map_err(|_| "dimension is excessive")?;
    }
    Ok(output)
}
fn quad(value: &Value) -> Result<[[f64; 2]; 4], String> {
    let values = value
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or("a quad needs four corners")?;
    let mut output = [[0.; 2]; 4];
    for (out, value) in output.iter_mut().zip(values) {
        let point = value
            .as_array()
            .filter(|v| v.len() == 2)
            .ok_or("a corner needs two coordinates")?;
        for (out, value) in out.iter_mut().zip(point) {
            *out = value
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or("coordinate is not finite")?;
        }
    }
    Ok(output)
}
fn integer(value: &Value, key: &str) -> Result<u64, String> {
    field(value, key)?
        .as_u64()
        .ok_or_else(|| format!("invalid {key}"))
}
fn fields(value: &Value, keys: &[&str]) -> Result<(), String> {
    let object = value.as_object().ok_or("geometry object required")?;
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err("geometry fields do not match the schema".into());
    }
    Ok(())
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing geometry field {key}"))
}
