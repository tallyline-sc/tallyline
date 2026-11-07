// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Supplied panel geometry and bounded preparation outcomes.

use tallyline_shared_kernel::api::model::{ImageRaster, ProjectiveMap};

/// The origin of caller-supplied geometry, without automatic locate evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryProvenance {
    /// Explicitly supplied corners, without evidence of owner approval.
    Manual,
    /// Corners supplied by a layout profile.
    Profile,
}

/// Clockwise raster-edge corners and the requested working pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelGeometry {
    /// Top-left, top-right, bottom-right and bottom-left corners.
    pub corners: [[f64; 2]; 4],
    /// Working width and height, with no claim about physical resolution or source text height.
    pub output: [u32; 2],
}

/// Caller-supplied geometry in the decoded raster's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SuppliedPanels {
    /// Shop extent including its title and tabs.
    pub shop: PanelGeometry,
    /// Location field extent.
    pub location: PanelGeometry,
    /// Supplied terminal-header/balance quadrilateral in clockwise raster-edge coordinates.
    /// Its supported upright bottom edge defines a full-width upward half-plane; contact cells are black.
    pub header: [[f64; 2]; 4],
    /// Geometry origin retained for preview confirmation.
    pub provenance: GeometryProvenance,
}

/// Tightenable validated release limits for supplied-panel preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelLimits {
    max_side: u32,
    max_total_pixels: u64,
    max_sampled_pixels: u64,
}

/// A panel limit is zero or exceeds its hard release maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidPanelLimits;

impl PanelLimits {
    /// Accepts positive limits no larger than 4,096 per side, four megapixels and 128 million samples.
    ///
    /// # Errors
    /// Refuses zero or raised limits.
    pub fn new(
        max_side: u32,
        max_total_pixels: u64,
        max_sampled_pixels: u64,
    ) -> Result<Self, InvalidPanelLimits> {
        let hard = Self::default();
        if max_side == 0
            || max_side > hard.max_side
            || max_total_pixels == 0
            || max_total_pixels > hard.max_total_pixels
            || max_sampled_pixels == 0
            || max_sampled_pixels > hard.max_sampled_pixels
        {
            return Err(InvalidPanelLimits);
        }
        Ok(Self {
            max_side,
            max_total_pixels,
            max_sampled_pixels,
        })
    }

    /// Returns the maximum output side in pixels.
    #[must_use]
    pub const fn max_side(self) -> u32 {
        self.max_side
    }

    /// Returns the maximum combined shop and location pixels.
    #[must_use]
    pub const fn max_total_pixels(self) -> u64 {
        self.max_total_pixels
    }

    /// Returns the maximum candidate source cells visited across both filters.
    #[must_use]
    pub const fn max_sampled_pixels(self) -> u64 {
        self.max_sampled_pixels
    }
}

impl Default for PanelLimits {
    fn default() -> Self {
        Self {
            max_side: 4096,
            max_total_pixels: 4_194_304,
            max_sampled_pixels: 128_000_000,
        }
    }
}

/// An unsafe or unsupported supplied-panel request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelRefusal {
    /// Invalid panel or exclusion geometry.
    Geometry,
    /// At least one panel has no geometric extent below the conservative header boundary.
    PrivacyExtent,
    /// Requested output exceeds the validated allocation bounds.
    OutputLimit,
    /// Source-cell traversal would exceed the supplied work budget.
    SamplingLimit,
}

/// An upright safe working raster with observable forward and inverse coordinates.
#[derive(Debug, PartialEq)]
pub struct PreparedPanel {
    raster: ImageRaster,
    transform: ProjectiveMap,
}

impl PreparedPanel {
    pub(crate) const fn new(raster: ImageRaster, transform: ProjectiveMap) -> Self {
        Self { raster, transform }
    }

    /// Returns safe un-stretched pixels for layout inspection.
    #[must_use]
    pub const fn raster(&self) -> &ImageRaster {
        &self.raster
    }

    /// Returns the output-edge/source-edge mapping with half-integer pixel centres.
    #[must_use]
    pub const fn transform(&self) -> &ProjectiveMap {
        &self.transform
    }
}

/// Safe supplied-geometry preparation, without automatic locate or calibrated measurements.
#[derive(Debug, PartialEq)]
pub struct PreparedPanels {
    shop: PreparedPanel,
    location: PreparedPanel,
    provenance: GeometryProvenance,
}

impl PreparedPanels {
    pub(crate) const fn new(
        shop: PreparedPanel,
        location: PreparedPanel,
        provenance: GeometryProvenance,
    ) -> Self {
        Self {
            shop,
            location,
            provenance,
        }
    }

    /// Returns the prepared shop panel.
    #[must_use]
    pub const fn shop(&self) -> &PreparedPanel {
        &self.shop
    }

    /// Returns the prepared location panel.
    #[must_use]
    pub const fn location(&self) -> &PreparedPanel {
        &self.location
    }

    /// Returns the supplied geometry origin.
    #[must_use]
    pub const fn provenance(&self) -> GeometryProvenance {
        self.provenance
    }

    /// Requires full-size privacy preview confirmation before an upload.
    #[must_use]
    pub const fn requires_privacy_confirmation(&self) -> bool {
        true
    }
}
