// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded offline supplied-geometry inspection without model execution.

mod bundle;
mod manifest;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tallyline_adapter_files::api as files;
use tallyline_adapter_files::api::model::{ImageIntake, ImageIntakeSettings};
use tallyline_recognition::api as recognition;
use tallyline_recognition::api::model::{PanelGeometry, PreparedPanel};

/// Dumps safe shop/location PNGs and separate geometry evidence from one bound supplied manifest.
///
/// # Errors
/// Refuses invalid or changed inputs, bounded preparation failures and non-exclusive destinations.
pub fn prepare(image: &Path, geometry: &Path, destination: &Path) -> Result<Value, String> {
    let geometry_bytes = read_bounded(geometry, manifest::MAX_METADATA_BYTES)?;
    let manifest = manifest::parse(&geometry_bytes)?;
    let image_bytes = read_bounded(image, 200 * 1024 * 1024)?;
    if digest(&image_bytes) != manifest.image_hash {
        return Err("source hash does not match supplied geometry".into());
    }
    let ImageIntake::Decoded(decoded) =
        files::read_image(&image_bytes, &ImageIntakeSettings::default())
    else {
        return Err("source image was set aside by bounded intake".into());
    };
    if [decoded.raster.width(), decoded.raster.height()] != manifest.dimensions {
        return Err("geometry dimensions do not match the decoded raster".into());
    }
    let prepared =
        recognition::prepare_panels(&decoded.raster, &manifest.supplied, manifest.limits)
            .map_err(|error| format!("panel preparation refused: {error:?}"))?;
    let reduction=decoded.downscaled.map(|value|json!({"originalDimensions":[value.original_width,value.original_height],"factor":value.factor}));
    let metadata = json!({
        "schemaVersion":2,"sourceSha256":manifest.image_hash,"geometrySha256":digest(&geometry_bytes),
        "rasterDimensions":manifest.dimensions,"downscaled":reduction,
        "provenance":match prepared.provenance() { tallyline_recognition::api::model::GeometryProvenance::Manual=>"manual",tallyline_recognition::api::model::GeometryProvenance::Profile=>"profile" },
        "automaticLocate":"not_run","privacyConfirmationRequired":prepared.requires_privacy_confirmation(),
        "textSize":"unavailable_not_measured","tone":"unavailable_uncalibrated",
        "recognition":"not_run","sourceTextEvidence":"original_raster_only",
        "coordinateConvention":"raster edges; clockwise TL/TR/BR/BL; half-integer pixel centres; upright panels",
        "headerExclusion":manifest.supplied.header,"privacyMask":"full_width_upward_through_header_supporting_bottom_edge",
        "shop":panel_metadata(prepared.shop(),manifest.supplied.shop),"location":panel_metadata(prepared.location(),manifest.supplied.location),
    });
    let metadata =
        serde_json::to_vec_pretty(&metadata).map_err(|_| "geometry output encoding failed")?;
    if metadata.len() > manifest::MAX_METADATA_BYTES as usize {
        return Err("geometry output exceeds its metadata bound".into());
    }
    let remaining = manifest
        .max_output_bytes
        .checked_sub(metadata.len())
        .ok_or("metadata exceeds the output byte budget")?;
    let shop = files::encode_raster_png(prepared.shop().raster(), remaining)
        .map_err(|_| "shop encoding exceeds the output byte budget")?;
    let remaining = remaining
        .checked_sub(shop.len())
        .ok_or("shop exceeds the output byte budget")?;
    let location = files::encode_raster_png(prepared.location().raster(), remaining)
        .map_err(|_| "location encoding exceeds the output byte budget")?;
    bundle::write(
        destination,
        [
            ("shop.png", &shop),
            ("location.png", &location),
            ("geometry.json", &metadata),
        ],
    )?;
    Ok(
        json!({"state":"prepared","captures":1,"crops":2,"automaticLocate":"not_run","recognition":"not_run"}),
    )
}

fn panel_metadata(prepared: &PreparedPanel, supplied: PanelGeometry) -> Value {
    json!({"corners":supplied.corners,"outputDimensions":supplied.output,
        "outputToSource":prepared.transform().output_to_source_matrix(),
        "sourceToOutput":prepared.transform().source_to_output_matrix()})
}
fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| "input is unavailable")?;
    if !file
        .metadata()
        .map_err(|_| "input metadata is unavailable")?
        .is_file()
    {
        return Err("input must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "input read failed")?;
    if bytes.len() as u64 > maximum {
        return Err("input exceeds its byte bound".into());
    }
    Ok(bytes)
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
