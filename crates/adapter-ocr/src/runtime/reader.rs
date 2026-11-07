// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Worker-owned lazy Small sessions and reusable image tensors.

use std::time::Duration;
mod detect;
mod research;
use super::{NativeSmokeError, OcrRuntime, OcrRuntimeSettings, RuntimeLoadError};
use crate::assets::SmallOcrAssets;
use crate::ctc::CtcReading;
use crate::detection::{DbError, DetectionBox};
use crate::preprocessing::OcrTensorBuffer;
use crate::preprocessing::TensorInputError;
use ort::session::Session;
use research::ReaderAssets;
use tallyline_shared_kernel::api::model::ImageRaster;

/// A frozen complete raw-reader arithmetic profile for independent evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmallReaderProfile {
    /// BGR detector, portable DB rectangles, bilinear projective crops and Linear8uV1 recognition.
    PortableDbBilinearV1,
}

impl SmallReaderProfile {
    /// Returns the immutable identifier recorded with raw-reader results.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::PortableDbBilinearV1 => "ppocr-small-portable-db-bilinear-v1",
        }
    }
}
/// Which worker-local native sessions currently retain model memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmallSessionState {
    /// No native session has been created or both were released.
    Empty,
    /// Only detection has run.
    Detector,
    /// Only text-line recognition has run.
    Recognizer,
    /// Both models are ready for reuse.
    Both,
}

/// A Small reader refused its input, native output or injected clock.
#[derive(Debug)]
pub enum SmallReaderError {
    /// The injected monotonic time moved backwards.
    ClockRegression,
    /// A model failed its verified native execution contract.
    Native(NativeSmokeError),
    /// The image cannot fit an aspect-preserving bounded input tensor.
    Input(TensorInputError),
    /// The app-supplied runtime initialiser refused its first attempt.
    Runtime(RuntimeLoadError),
    /// The one-shot app initialiser was consumed, including a pre-native refusal.
    RuntimeInitialiserConsumed,
    /// Native detector probabilities or original-raster crop geometry were refused.
    Detection(DbError),
}

/// One raw model text line and its original-raster source quadrilateral.
pub struct SmallTextLine {
    /// The detector region, without layout or field interpretation.
    pub region: DetectionBox,
    /// Unrepaired CTC text and character probabilities/frame spans.
    pub reading: CtcReading,
}

/// One worker's lazy reusable sessions; runtime/environment ownership stays with the app.
/// Construction and idle release never initialise or reload the native library.
pub struct SmallOcrReader<'a> {
    assets: ReaderAssets<'a>,
    runtime: Option<OcrRuntime>,
    initialise: Option<Box<dyn FnOnce() -> Result<OcrRuntime, RuntimeLoadError> + 'a>>,
    settings: OcrRuntimeSettings,
    clock: Box<dyn Fn() -> Duration + 'a>,
    last_used: Option<Duration>,
    detector: Option<Session>,
    recognizer: Option<Session>,
    buffer: OcrTensorBuffer,
}

impl<'a> SmallOcrReader<'a> {
    /// Creates an empty reader with an injected monotonic elapsed-time source.
    pub fn new(
        assets: &'a SmallOcrAssets,
        settings: OcrRuntimeSettings,
        clock: impl Fn() -> Duration + 'a,
        initialise: impl FnOnce() -> Result<OcrRuntime, RuntimeLoadError> + 'a,
    ) -> Self {
        Self {
            assets: ReaderAssets::Small(assets),
            runtime: None,
            initialise: Some(Box::new(initialise)),
            settings,
            clock: Box::new(clock),
            last_used: None,
            detector: None,
            recognizer: None,
            buffer: OcrTensorBuffer::default(),
        }
    }

    /// Recognises one original-raster text line with the exact Small dictionary.
    /// Native sessions are lazy, reused and run sequentially on the calling CPU worker.
    ///
    /// # Errors
    /// Refuses oversized input before runtime initialisation and rejects runtime/model,
    /// output-shape, probability or monotonic-clock failures without repairing text.
    pub fn read_line(&mut self, image: &ImageRaster) -> Result<CtcReading, SmallReaderError> {
        let width = (u64::from(image.width()) * crate::preprocessing::HEIGHT as u64)
            .div_ceil(u64::from(image.height()));
        let width = usize::try_from(width)
            .map_err(|_| SmallReaderError::Input(TensorInputError))?
            .max(320);
        if width > crate::preprocessing::MAX_WIDTH {
            return Err(SmallReaderError::Input(TensorInputError));
        }
        self.close_if_idle()?;
        Self::ensure_runtime(&mut self.runtime, &mut self.initialise)?;
        if self.recognizer.is_none() {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or(SmallReaderError::RuntimeInitialiserConsumed)?;
            self.recognizer = Some(
                self.assets
                    .create_session(
                        runtime,
                        self.assets.recognizer(),
                        self.settings.intra_op_threads(),
                    )
                    .map_err(SmallReaderError::Native)?,
            );
        }
        self.last_used = Some((self.clock)());
        let input = self
            .buffer
            .recognition(image, width)
            .map_err(SmallReaderError::Input)?;
        let tensor = ort::value::TensorRef::from_array_view((input.shape, input.values)).map_err(
            |error| {
                SmallReaderError::Native(super::smoke::failure(
                    self.assets.recognizer(),
                    "input",
                    error.code(),
                ))
            },
        )?;
        let session = self
            .recognizer
            .as_mut()
            .ok_or(SmallReaderError::RuntimeInitialiserConsumed)?;
        let output = session.run([tensor.into()]).map_err(|error| {
            SmallReaderError::Native(super::smoke::failure(
                self.assets.recognizer(),
                "run",
                error.code(),
            ))
        })?;
        let value =
            output
                .values()
                .next()
                .ok_or(SmallReaderError::Native(NativeSmokeError::Interface(
                    self.assets.recognizer(),
                )))?;
        let (shape, probabilities) = value.try_extract_tensor::<f32>().map_err(|error| {
            SmallReaderError::Native(super::smoke::failure(
                self.assets.recognizer(),
                "output_type",
                error.code(),
            ))
        })?;
        let dimensions: Vec<_> = shape.iter().copied().collect();
        let [1, frames, classes] = dimensions.as_slice() else {
            return Err(SmallReaderError::Native(NativeSmokeError::Shape {
                artifact: self.assets.recognizer(),
                actual: dimensions,
            }));
        };
        let frames = usize::try_from(*frames).map_err(|_| {
            SmallReaderError::Native(NativeSmokeError::Interface(self.assets.recognizer()))
        })?;
        let classes = usize::try_from(*classes).map_err(|_| {
            SmallReaderError::Native(NativeSmokeError::Interface(self.assets.recognizer()))
        })?;
        let reading = self
            .assets
            .decoder()
            .decode(probabilities, [1, frames, classes])
            .map_err(|error| SmallReaderError::Native(NativeSmokeError::Ctc(error)))?;
        self.last_used = Some((self.clock)());
        Ok(reading)
    }

    fn ensure_runtime(
        runtime: &mut Option<OcrRuntime>,
        initialise: &mut Option<Box<dyn FnOnce() -> Result<OcrRuntime, RuntimeLoadError> + 'a>>,
    ) -> Result<(), SmallReaderError> {
        if runtime.is_none() {
            let initialise = initialise
                .take()
                .ok_or(SmallReaderError::RuntimeInitialiserConsumed)?;
            *runtime = Some(initialise().map_err(SmallReaderError::Runtime)?);
        }
        Ok(())
    }
    /// Returns the complete source-qualified raw-reader arithmetic identity.
    #[must_use]
    pub const fn profile(&self) -> SmallReaderProfile {
        SmallReaderProfile::PortableDbBilinearV1
    }

    /// Reports native session residency for bounded lifecycle diagnostics.
    #[must_use]
    pub fn session_state(&self) -> SmallSessionState {
        match (self.detector.is_some(), self.recognizer.is_some()) {
            (false, false) => SmallSessionState::Empty,
            (true, false) => SmallSessionState::Detector,
            (false, true) => SmallSessionState::Recognizer,
            (true, true) => SmallSessionState::Both,
        }
    }

    /// Releases both sessions when elapsed inactivity reaches the configured interval.
    /// The app's verified runtime environment and load token are retained.
    ///
    /// # Errors
    /// Returns `ClockRegression` if the injected monotonic clock moved backwards.
    pub fn close_if_idle(&mut self) -> Result<bool, SmallReaderError> {
        let Some(last_used) = self.last_used else {
            return Ok(false);
        };
        let elapsed = (self.clock)()
            .checked_sub(last_used)
            .ok_or(SmallReaderError::ClockRegression)?;
        if elapsed < self.settings.idle_close_after() {
            return Ok(false);
        }
        let released = self.session_state() != SmallSessionState::Empty;
        self.close_sessions();
        Ok(released)
    }

    /// Releases worker sessions and reusable pixel storage without reloading the environment.
    pub fn close_sessions(&mut self) {
        self.detector = None;
        self.recognizer = None;
        self.buffer = OcrTensorBuffer::default();
        self.last_used = None;
    }
}

impl std::fmt::Display for SmallReaderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Small image reader failed: {self:?}")
    }
}

impl std::error::Error for SmallReaderError {}

impl Drop for SmallOcrReader<'_> {
    fn drop(&mut self) {
        self.close_sessions();
    }
}
pub use research::ResearchOcrReader;
