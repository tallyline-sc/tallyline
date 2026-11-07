// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit bounded native tensor acceptance, separate from the production OCR pool.

use super::OcrRuntime;
use crate::assets::{AssetError, OcrArtifact, SmallOcrAssets};
use crate::ctc::CtcError;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::TensorRef;

/// Numeric evidence from one fixed synthetic native input, without image or decoded text.
#[derive(Debug)]
pub struct SmokeTensor {
    /// The exact fixed Small model.
    pub artifact: OcrArtifact,
    /// Synthetic batch/channel/height/width dimensions.
    pub input_shape: [usize; 4],
    /// The actual native output shape after inference.
    pub output_shape: Vec<i64>,
    /// The number of native output elements.
    pub output_elements: usize,
    /// The finite output probability minimum and maximum.
    pub probability_range: [f32; 2],
}

/// Bounded model execution evidence; this establishes no corpus accuracy or pool acceptance.
#[derive(Debug)]
pub struct SmallModelSmoke {
    /// Immutable diagnostic profile, including zero input and conservative session options.
    pub profile: &'static str,
    /// Actual detector output evidence.
    pub detector: SmokeTensor,
    /// Actual recogniser output evidence.
    pub recognizer: SmokeTensor,
    /// The exact immutable dictionary's classes, including blank and appended space.
    pub dictionary_classes: usize,
    /// Number of CTC labels on the synthetic input; the text is deliberately not returned.
    pub ctc_emitted_labels: usize,
}

/// Why fixed native tensor acceptance could not establish the expected contract.
#[derive(Debug)]
pub enum NativeSmokeError {
    /// A model could not be captured into the same immutable bytes that were hashed.
    Asset(AssetError),
    /// A native API failed with only its closed code, never a raw message or path.
    Native {
        /// The fixed model concerned.
        artifact: OcrArtifact,
        /// The fixed operation label.
        stage: &'static str,
        /// A closed native error-code label.
        code: &'static str,
    },
    /// The fixed model has an unexpected input/output interface.
    Interface(OcrArtifact),
    /// An actual native output shape does not fit its fixed model/dictionary contract.
    Shape {
        /// The fixed model concerned.
        artifact: OcrArtifact,
        /// Actual bounded-rank dimensions; no user data.
        actual: Vec<i64>,
    },
    /// Native output contains nonfinite or out-of-range probabilities.
    Values(OcrArtifact),
    /// The actual recogniser tensor cannot be decoded by the immutable CTC alphabet.
    Ctc(CtcError),
}

impl OcrRuntime {
    /// Runs detector64x64 then recogniser48x320 with normalised-zero input, one thread each.
    /// Sessions are sequential and local to this explicit smoke, without global pools,
    /// spinning, CPU arenas or memory patterns. Production pool/inactivity remain separate.
    ///
    /// # Errors
    /// Rejects changed model bytes, native failures, extra interfaces, mismatched actual
    /// shapes/classes or invalid probabilities. Only the verified in-memory model is passed to ORT.
    pub fn smoke_small_models(
        &mut self,
        assets: &SmallOcrAssets,
    ) -> Result<SmallModelSmoke, NativeSmokeError> {
        let (detector, _) = run_tensor(self, assets, OcrArtifact::SmallDetector, [1, 3, 64, 64])?;
        let (recognizer, ctc_emitted_labels) =
            run_tensor(self, assets, OcrArtifact::SmallRecognizer, [1, 3, 48, 320])?;
        Ok(SmallModelSmoke {
            profile: "ort131-small-zero-1x1-level3-noarena-nopattern-nospin-v1",
            detector,
            recognizer,
            dictionary_classes: assets.decoder().class_count(),
            ctc_emitted_labels,
        })
    }
}

fn run_tensor(
    runtime: &OcrRuntime,
    assets: &SmallOcrAssets,
    artifact: OcrArtifact,
    input_shape: [usize; 4],
) -> Result<(SmokeTensor, usize), NativeSmokeError> {
    let mut session = create_session(runtime, assets, artifact, 1)?;
    let input = vec![0.0_f32; input_shape.into_iter().product()];
    let tensor = TensorRef::from_array_view((input_shape, input.as_slice()))
        .map_err(|error| failure(artifact, "input", error.code()))?;
    let output = session
        .run([tensor.into()])
        .map_err(|error| failure(artifact, "run", error.code()))?;
    let value = output
        .values()
        .next()
        .ok_or(NativeSmokeError::Interface(artifact))?;
    let (shape, values) = value
        .try_extract_tensor::<f32>()
        .map_err(|error| failure(artifact, "output_type", error.code()))?;
    if shape.len() > 4 {
        return Err(NativeSmokeError::Shape {
            artifact,
            actual: Vec::new(),
        });
    }
    let shape: Vec<i64> = shape.iter().copied().collect();
    let frames = validate_shape(artifact, &shape, assets.decoder().class_count())?;
    if values.is_empty()
        || values
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(NativeSmokeError::Values(artifact));
    }
    let range = values
        .iter()
        .fold([f32::INFINITY, f32::NEG_INFINITY], |[min, max], value| {
            [min.min(*value), max.max(*value)]
        });
    let labels = if artifact == OcrArtifact::SmallRecognizer {
        assets
            .decoder()
            .decode(values, [1, frames, assets.decoder().class_count()])
            .map_err(NativeSmokeError::Ctc)?
            .characters
            .len()
    } else {
        0
    };
    Ok((
        SmokeTensor {
            artifact,
            input_shape,
            output_shape: shape,
            output_elements: values.len(),
            probability_range: range,
        },
        labels,
    ))
}

fn validate_shape(
    artifact: OcrArtifact,
    shape: &[i64],
    classes: usize,
) -> Result<usize, NativeSmokeError> {
    match (artifact, shape) {
        (OcrArtifact::SmallDetector, [1, 1, 64, 64]) => Ok(0),
        (OcrArtifact::SmallRecognizer, [1, frames, actual_classes])
            if *frames > 0 && usize::try_from(*actual_classes).ok() == Some(classes) =>
        {
            usize::try_from(*frames).map_err(|_| NativeSmokeError::Shape {
                artifact,
                actual: shape.to_vec(),
            })
        }
        _ => Err(NativeSmokeError::Shape {
            artifact,
            actual: shape.to_vec(),
        }),
    }
}

pub(super) fn failure(
    artifact: OcrArtifact,
    stage: &'static str,
    code: ort::ErrorCode,
) -> NativeSmokeError {
    let code = match code {
        ort::ErrorCode::Ok => "ok",
        ort::ErrorCode::GenericFailure => "failure",
        ort::ErrorCode::InvalidArgument => "invalid_argument",
        ort::ErrorCode::NoSuchFile => "no_such_file",
        ort::ErrorCode::NoModel => "no_model",
        ort::ErrorCode::EngineError => "engine",
        ort::ErrorCode::RuntimeException => "runtime_exception",
        ort::ErrorCode::InvalidProtobuf => "invalid_protobuf",
        ort::ErrorCode::ModelLoaded => "model_loaded",
        ort::ErrorCode::NotImplemented => "not_implemented",
        ort::ErrorCode::InvalidGraph => "invalid_graph",
        ort::ErrorCode::ExecutionProviderFailure => "execution_provider",
        _ => "unrecognised",
    };
    NativeSmokeError::Native {
        artifact,
        stage,
        code,
    }
}

impl std::fmt::Display for NativeSmokeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "native Small tensor smoke failed: {self:?}")
    }
}

impl std::error::Error for NativeSmokeError {}

pub(super) fn create_session(
    runtime: &OcrRuntime,
    assets: &SmallOcrAssets,
    artifact: OcrArtifact,
    intra_op_threads: usize,
) -> Result<Session, NativeSmokeError> {
    let model = assets
        .open_model(artifact)
        .map_err(NativeSmokeError::Asset)?;
    create_verified_session(runtime, &model, intra_op_threads)
}

pub(super) fn create_verified_session(
    runtime: &OcrRuntime,
    model: &crate::assets::VerifiedOcrModel,
    intra_op_threads: usize,
) -> Result<Session, NativeSmokeError> {
    let artifact = model.fingerprint().artifact;
    let builder = Session::builder(&runtime._environment)
        .map_err(|error| failure(artifact, "builder", error.code()))?;
    let mut builder = builder
        .with_intra_threads(intra_op_threads)
        .map_err(|error| failure(artifact, "intra_threads", error.code()))?
        .with_inter_threads(1)
        .map_err(|error| failure(artifact, "inter_threads", error.code()))?
        .with_parallel_execution(false)
        .map_err(|error| failure(artifact, "sequential", error.code()))?
        .with_intra_op_spinning(false)
        .map_err(|error| failure(artifact, "intra_spinning", error.code()))?
        .with_inter_op_spinning(false)
        .map_err(|error| failure(artifact, "inter_spinning", error.code()))?
        .with_memory_pattern(false)
        .map_err(|error| failure(artifact, "memory_pattern", error.code()))?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|error| failure(artifact, "optimisation", error.code()))?
        .with_execution_providers([ort::ep::CPU::default().with_arena_allocator(false).build()])
        .map_err(|error| failure(artifact, "cpu_arena", error.code()))?;
    let session = builder
        .commit_from_memory(model.bytes())
        .map_err(|error| failure(artifact, "model", error.code()))?;
    if session.inputs().len() != 1 || session.outputs().len() != 1 {
        return Err(NativeSmokeError::Interface(artifact));
    }
    Ok(session)
}
