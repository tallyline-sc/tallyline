// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public OCR values.

pub use crate::assets::{
    AssetError, AssetFingerprint, OcrArtifact, SmallOcrAssets, VerifiedOcrModel,
};
pub use crate::ctc::{CtcCharacter, CtcDecoder, CtcError, CtcReading};
pub use crate::preprocessing::{
    OcrInputTensor, OcrTensorBuffer, RecognitionPreprocessing, TensorInputError,
};

pub use crate::runtime::{
    OcrRuntime, OcrRuntimeLoadToken, RuntimeHostChecks, RuntimeHostError, RuntimeLoadError,
};

pub use crate::detection::{DbError, DbPostprocessor, DetectionBox};
pub use crate::preprocessing::DetectionInputTensor;
pub use crate::research::{OcrDetector, OcrRecognizer, OcrResearchProfile, ResearchOcrAssets};
pub use crate::runtime::ResearchOcrReader;
pub use crate::runtime::{InvalidOcrRuntimeSettings, OcrRuntimeSettings};
pub use crate::runtime::{NativeSmokeError, SmallModelSmoke, SmokeTensor};
pub use crate::runtime::{
    SmallOcrReader, SmallReaderError, SmallReaderProfile, SmallSessionState, SmallTextLine,
};
