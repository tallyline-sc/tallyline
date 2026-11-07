// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed research facade over the shared worker-local reader machinery.

use super::{SmallOcrReader, SmallReaderError, SmallSessionState, SmallTextLine};
use crate::assets::{OcrArtifact, SmallOcrAssets};
use crate::ctc::{CtcDecoder, CtcReading};
use crate::detection::DetectionBox;
use crate::research::{OcrDetector, OcrResearchProfile, ResearchOcrAssets};
use crate::runtime::{NativeSmokeError, OcrRuntime, OcrRuntimeSettings, RuntimeLoadError};
use ort::session::Session;
use std::time::Duration;
use tallyline_shared_kernel::api::model::ImageRaster;

pub(super) enum ReaderAssets<'a> {
    Small(&'a SmallOcrAssets),
    Research(&'a ResearchOcrAssets),
}

impl ReaderAssets<'_> {
    pub(super) fn decoder(&self) -> &CtcDecoder {
        match self {
            Self::Small(assets) => assets.decoder(),
            Self::Research(assets) => assets.decoder(),
        }
    }

    pub(super) fn detector(&self) -> OcrArtifact {
        match self {
            Self::Small(_) => OcrArtifact::SmallDetector,
            Self::Research(assets) => assets.detector().fingerprint().artifact,
        }
    }

    pub(super) fn recognizer(&self) -> OcrArtifact {
        match self {
            Self::Small(_) => OcrArtifact::SmallRecognizer,
            Self::Research(assets) => assets.recognizer().fingerprint().artifact,
        }
    }

    pub(super) fn detector_family(&self) -> OcrDetector {
        match self {
            Self::Small(_) => OcrDetector::Small,
            Self::Research(assets) => assets.profile().detector(),
        }
    }

    pub(super) fn create_session(
        &self,
        runtime: &OcrRuntime,
        artifact: OcrArtifact,
        threads: usize,
    ) -> Result<Session, NativeSmokeError> {
        match self {
            Self::Small(assets) => {
                crate::runtime::smoke::create_session(runtime, assets, artifact, threads)
            }
            Self::Research(assets) => {
                let model = if artifact == assets.detector().fingerprint().artifact {
                    assets.detector()
                } else if artifact == assets.recognizer().fingerprint().artifact {
                    assets.recognizer()
                } else {
                    return Err(NativeSmokeError::Interface(artifact));
                };
                crate::runtime::smoke::create_verified_session(runtime, model, threads)
            }
        }
    }
}

/// A research pair reusing the Small reader's bounded tensors, lifecycle and verified runtime.
pub struct ResearchOcrReader<'a> {
    inner: SmallOcrReader<'a>,
    profile: OcrResearchProfile,
}

impl<'a> ResearchOcrReader<'a> {
    /// Creates an empty worker-local reader without initialising the native runtime.
    pub fn new(
        assets: &'a ResearchOcrAssets,
        settings: OcrRuntimeSettings,
        clock: impl Fn() -> Duration + 'a,
        initialise: impl FnOnce() -> Result<OcrRuntime, RuntimeLoadError> + 'a,
    ) -> Self {
        Self {
            inner: SmallOcrReader {
                assets: ReaderAssets::Research(assets),
                runtime: None,
                initialise: Some(Box::new(initialise)),
                settings,
                clock: Box::new(clock),
                last_used: None,
                detector: None,
                recognizer: None,
                buffer: crate::preprocessing::OcrTensorBuffer::default(),
            },
            profile: assets.profile(),
        }
    }

    /// Returns the complete research pair and arithmetic identity.
    #[must_use]
    pub const fn profile(&self) -> OcrResearchProfile {
        self.profile
    }

    /// Returns the current worker-local native session residency.
    #[must_use]
    pub fn session_state(&self) -> SmallSessionState {
        self.inner.session_state()
    }

    /// Reads an original-raster text line with the selected recogniser's immutable alphabet.
    ///
    /// # Errors
    /// Refuses input bounds, runtime/model failures, invalid output or a regressing clock.
    pub fn read_line(&mut self, image: &ImageRaster) -> Result<CtcReading, SmallReaderError> {
        self.inner.read_line(image)
    }

    /// Detects original-raster regions using the selected detector's reviewed thresholds.
    ///
    /// # Errors
    /// Refuses oversized images, invalid native outputs, geometry and runtime failures.
    pub fn detect(&mut self, image: &ImageRaster) -> Result<Vec<DetectionBox>, SmallReaderError> {
        self.inner.detect(image)
    }

    /// Reads each detected original-raster crop sequentially without field interpretation.
    ///
    /// # Errors
    /// Refuses any failed detector, crop or recogniser without returning partial text.
    pub fn read_image(
        &mut self,
        image: &ImageRaster,
    ) -> Result<Vec<SmallTextLine>, SmallReaderError> {
        self.inner.read_image(image)
    }

    /// Releases sessions at the configured idle boundary, retaining the verified environment.
    ///
    /// # Errors
    /// Refuses a regressing monotonic clock.
    pub fn close_if_idle(&mut self) -> Result<bool, SmallReaderError> {
        self.inner.close_if_idle()
    }

    /// Releases native sessions and reusable pixel storage immediately.
    pub fn close_sessions(&mut self) {
        self.inner.close_sessions();
    }
}
