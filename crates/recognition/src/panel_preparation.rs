// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pure preparation of caller-supplied shop and location geometry.

use crate::panel_geometry::{
    PanelGeometry, PanelLimits, PanelRefusal, PreparedPanel, PreparedPanels, SuppliedPanels,
};
use crate::privacy_mask::PrivacyMask;
use tallyline_shared_kernel::api;
use tallyline_shared_kernel::api::model::{AreaSampleError, ImageRaster, ProjectiveMap};

/// Prepares bounded upright working panels from supplied geometry without altering the source.
///
/// # Errors
/// Refuses invalid geometry, output allocation and work limits without returning partial panels.
pub fn prepare_panels(
    image: &ImageRaster,
    supplied: &SuppliedPanels,
    limits: PanelLimits,
) -> Result<PreparedPanels, PanelRefusal> {
    validate_output(supplied, limits)?;
    let source = [image.width(), image.height()];
    let shop = ProjectiveMap::new(supplied.shop.corners, source, supplied.shop.output)
        .map_err(|_| PanelRefusal::Geometry)?;
    let location = ProjectiveMap::new(supplied.location.corners, source, supplied.location.output)
        .map_err(|_| PanelRefusal::Geometry)?;
    ProjectiveMap::new(supplied.header, source, [1, 1]).map_err(|_| PanelRefusal::Geometry)?;
    let mask = PrivacyMask::try_new(supplied.header, source).ok_or(PanelRefusal::Geometry)?;
    if mask.covers(supplied.shop.corners) || mask.covers(supplied.location.corners) {
        return Err(PanelRefusal::PrivacyExtent);
    }
    let mut remaining = limits.max_sampled_pixels();
    Ok(PreparedPanels::new(
        prepare(image, supplied.shop, shop, &mask, &mut remaining)?,
        prepare(image, supplied.location, location, &mask, &mut remaining)?,
        supplied.provenance,
    ))
}

fn validate_output(supplied: &SuppliedPanels, limits: PanelLimits) -> Result<(), PanelRefusal> {
    let dimensions = [supplied.shop.output, supplied.location.output];
    if dimensions
        .iter()
        .flatten()
        .any(|value| *value == 0 || *value > limits.max_side())
    {
        return Err(PanelRefusal::OutputLimit);
    }
    let total: u64 = dimensions
        .iter()
        .map(|[width, height]| u64::from(*width) * u64::from(*height))
        .sum();
    if total > limits.max_total_pixels() {
        return Err(PanelRefusal::OutputLimit);
    }
    Ok(())
}
fn prepare(
    image: &ImageRaster,
    geometry: PanelGeometry,
    transform: ProjectiveMap,
    mask: &PrivacyMask,
    remaining: &mut u64,
) -> Result<PreparedPanel, PanelRefusal> {
    let [width, height] = geometry.output;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(width as usize * height as usize)
        .map_err(|_| PanelRefusal::OutputLimit)?;
    for y in 0..height {
        for x in 0..width {
            pixels.push(sample(image, &transform, mask, [x, y], remaining)?);
        }
    }
    let raster = ImageRaster::new(width, height, pixels).map_err(|_| PanelRefusal::Geometry)?;
    Ok(PreparedPanel::new(raster, transform))
}

fn sample(
    image: &ImageRaster,
    transform: &ProjectiveMap,
    mask: &PrivacyMask,
    [x, y]: [u32; 2],
    remaining: &mut u64,
) -> Result<u32, PanelRefusal> {
    let pixel = |x, y| {
        if mask.excludes(x, y) {
            Some(0xff00_0000)
        } else {
            image
                .argb()
                .get(y as usize * image.width() as usize + x as usize)
                .copied()
        }
    };
    let mut footprint = [[0.; 2]; 4];
    for (point, [x, y]) in
        footprint
            .iter_mut()
            .zip([[x, y], [x + 1, y], [x + 1, y + 1], [x, y + 1]])
    {
        *point = transform
            .to_source([f64::from(x), f64::from(y)])
            .ok_or(PanelRefusal::Geometry)?;
        point[0] = point[0].clamp(0., f64::from(image.width()));
        point[1] = point[1].clamp(0., f64::from(image.height()));
    }
    let distance = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let minifying = footprint
        .iter()
        .zip(footprint.iter().cycle().skip(1))
        .any(|(a, b)| distance(*a, *b) > 1. + 1e-9);
    if minifying {
        api::sample_area(image, footprint, remaining, pixel).map_err(|error| match error {
            AreaSampleError::Geometry => PanelRefusal::Geometry,
            AreaSampleError::WorkLimit => PanelRefusal::SamplingLimit,
        })
    } else {
        *remaining = remaining
            .checked_sub(4)
            .ok_or(PanelRefusal::SamplingLimit)?;
        let source = transform
            .to_source([f64::from(x) + 0.5, f64::from(y) + 0.5])
            .ok_or(PanelRefusal::Geometry)?;
        api::sample_bilinear(image, [source[0] - 0.5, source[1] - 0.5], pixel)
            .map_err(|_| PanelRefusal::Geometry)
    }
}
