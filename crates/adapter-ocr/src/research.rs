// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit data-only research choices bound to reviewed public model identities.

use crate::assets::{
    AssetError, AssetFingerprint, OcrArtifact, Pin, VerifiedOcrModel, verify_path,
};
use crate::ctc::CtcDecoder;
use std::path::Path;

/// A reviewed detector family, independent of the recogniser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrDetector {
    /// The official Tiny detector with box threshold 0.4.
    Tiny,
    /// The existing Small detector with box threshold 0.45.
    Small,
    /// The official Medium detector with box threshold 0.45.
    Medium,
}

/// A reviewed recogniser family with its exact ordered dictionary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcrRecognizer {
    /// Tiny, with 6,904 dictionary entries.
    Tiny,
    /// Small, with 18,708 dictionary entries.
    Small,
    /// Medium, with the same canonical dictionary as Small.
    Medium,
}

/// A frozen portable DB/bilinear research pair; this selects no production default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OcrResearchProfile {
    detector: OcrDetector,
    recognizer: OcrRecognizer,
}

impl OcrResearchProfile {
    /// Selects one of nine bounded detector/recogniser research pairs.
    #[must_use]
    pub const fn new(detector: OcrDetector, recognizer: OcrRecognizer) -> Self {
        Self {
            detector,
            recognizer,
        }
    }

    /// Returns the selected detector and its fixed thresholds.
    #[must_use]
    pub const fn detector(self) -> OcrDetector {
        self.detector
    }

    /// Returns the selected recogniser and dictionary binding.
    #[must_use]
    pub const fn recognizer(self) -> OcrRecognizer {
        self.recognizer
    }

    /// Returns the complete arithmetic and pair identifier for experiment receipts.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match (self.detector, self.recognizer) {
            (OcrDetector::Tiny, OcrRecognizer::Tiny) => {
                "ppocr-tiny-det-tiny-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Tiny, OcrRecognizer::Small) => {
                "ppocr-tiny-det-small-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Tiny, OcrRecognizer::Medium) => {
                "ppocr-tiny-det-medium-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Small, OcrRecognizer::Tiny) => {
                "ppocr-small-det-tiny-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Small, OcrRecognizer::Small) => {
                "ppocr-small-det-small-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Small, OcrRecognizer::Medium) => {
                "ppocr-small-det-medium-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Medium, OcrRecognizer::Tiny) => {
                "ppocr-medium-det-tiny-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Medium, OcrRecognizer::Small) => {
                "ppocr-medium-det-small-rec-portable-db-bilinear-v1"
            }
            (OcrDetector::Medium, OcrRecognizer::Medium) => {
                "ppocr-medium-det-medium-rec-portable-db-bilinear-v1"
            }
        }
    }
}

impl OcrDetector {
    /// Returns the reviewed DB box threshold; mask 0.2, unclip 1.4 and cap 1,000 are shared.
    #[must_use]
    pub const fn box_threshold(self) -> f64 {
        match self {
            Self::Tiny => 0.4,
            Self::Small | Self::Medium => 0.45,
        }
    }

    fn pin(self) -> Pin {
        match self {
            Self::Small => crate::assets::DETECTOR,
            Self::Tiny => Pin {
                artifact: OcrArtifact::TinyDetector,
                file: "inference.onnx",
                sha256: "193bab7a04fca699a6c82e6abb5b81bdb28177f0abd4062552b04908dafb19f8",
                bytes: "1780590",
            },
            Self::Medium => Pin {
                artifact: OcrArtifact::MediumDetector,
                file: "inference.onnx",
                sha256: "eb13b44b25bb36f89528b68720af8a61d9cf381176107f465db1757b65d086e1",
                bytes: "62032837",
            },
        }
    }
}

impl OcrRecognizer {
    /// Returns the output class count, including blank zero and appended space.
    #[must_use]
    pub const fn classes(self) -> usize {
        match self {
            Self::Tiny => 6906,
            Self::Small | Self::Medium => 18710,
        }
    }

    fn pin(self) -> Pin {
        match self {
            Self::Small => crate::assets::RECOGNIZER,
            Self::Tiny => Pin {
                artifact: OcrArtifact::TinyRecognizer,
                file: "inference.onnx",
                sha256: "9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6",
                bytes: "4462639",
            },
            Self::Medium => Pin {
                artifact: OcrArtifact::MediumRecognizer,
                file: "inference.onnx",
                sha256: "9c09abf0957f7968c7586464b7397b84ad2387a0497a351af40e9acc71b673ba",
                bytes: "76554979",
            },
        }
    }

    fn dictionary(self) -> Pin {
        match self {
            Self::Tiny => Pin {
                artifact: OcrArtifact::TinyDictionary,
                file: "ppocrv6_tiny_dict.txt",
                sha256: "c5cbe34ef40c29c4df07ed012bf96569cb69a2d2a01a07027e9f13cb832bd9cd",
                bytes: "27156",
            },
            Self::Small | Self::Medium => crate::assets::DICTIONARY,
        }
    }
}

/// Immutable verified model snapshots and the selected recogniser's exact CTC alphabet.
/// Paths identify data only; no runtime, plugin or backend can be supplied here.
pub struct ResearchOcrAssets {
    profile: OcrResearchProfile,
    detector: VerifiedOcrModel,
    recognizer: VerifiedOcrModel,
    fingerprints: [AssetFingerprint; 3],
    decoder: CtcDecoder,
}

impl ResearchOcrAssets {
    /// Hashes each absolute data file once, retaining the same model bytes for every session.
    ///
    /// # Errors
    /// Rejects any relative path before file access, then unreadable or changed data in
    /// detector, recogniser, dictionary order, before any native runtime is initialised.
    pub fn load(
        profile: OcrResearchProfile,
        detector: &Path,
        recognizer: &Path,
        dictionary: &Path,
    ) -> Result<Self, AssetError> {
        if [detector, recognizer, dictionary]
            .iter()
            .any(|path| !path.is_absolute())
        {
            return Err(AssetError::NotAbsolute);
        }
        let detector = verify_path(detector, profile.detector.pin(), true)?;
        let recognizer = verify_path(recognizer, profile.recognizer.pin(), true)?;
        let dictionary = verify_path(dictionary, profile.recognizer.dictionary(), true)?;
        let text = String::from_utf8(dictionary.contents.ok_or(AssetError::InvalidBuildPins)?)
            .map_err(|_| AssetError::InvalidDictionary)?;
        let mut alphabet: Vec<_> = text.lines().map(str::to_owned).collect();
        if alphabet.len() + 2 != profile.recognizer.classes() {
            return Err(AssetError::InvalidDictionary);
        }
        alphabet.push(" ".to_owned());
        let decoder = CtcDecoder::new(alphabet).map_err(AssetError::Dictionary)?;
        let fingerprints = [
            detector.fingerprint,
            recognizer.fingerprint,
            dictionary.fingerprint,
        ];
        Ok(Self {
            profile,
            detector: VerifiedOcrModel {
                fingerprint: detector.fingerprint,
                bytes: detector.contents.ok_or(AssetError::InvalidBuildPins)?,
            },
            recognizer: VerifiedOcrModel {
                fingerprint: recognizer.fingerprint,
                bytes: recognizer.contents.ok_or(AssetError::InvalidBuildPins)?,
            },
            fingerprints,
            decoder,
        })
    }

    /// Returns the selected pair and arithmetic identity.
    #[must_use]
    pub const fn profile(&self) -> OcrResearchProfile {
        self.profile
    }

    /// Returns detector, recogniser and dictionary identities in load order.
    #[must_use]
    pub const fn fingerprints(&self) -> &[AssetFingerprint; 3] {
        &self.fingerprints
    }

    /// Returns the exact model-specific alphabet.
    #[must_use]
    pub const fn decoder(&self) -> &CtcDecoder {
        &self.decoder
    }

    /// Borrows the immutable detector snapshot verified during load.
    #[must_use]
    pub const fn detector(&self) -> &VerifiedOcrModel {
        &self.detector
    }

    /// Borrows the immutable recogniser snapshot verified during load.
    #[must_use]
    pub const fn recognizer(&self) -> &VerifiedOcrModel {
        &self.recognizer
    }
}
