// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Ordered private native loading; no foreign types cross the adapter boundary.

use std::path::Path;

mod library;
mod smoke;
pub use smoke::{NativeSmokeError, SmallModelSmoke, SmokeTensor};

type HostCheck = dyn Fn(&Path) -> Result<(), RuntimeHostError> + Send + Sync;

/// Closed infrastructure failures returned by composition-root platform callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHostError {
    /// Windows dependency search could not be restricted before loading.
    SearchPolicy,
    /// The mapped module is not the verified installation file.
    MappedLibrary,
    /// A mapped Visual C++ module is absent, invalid or below the pinned floor.
    CrtPrerequisite,
}

/// Required native host checks wired by the application, without adapter dependencies.
pub struct RuntimeHostChecks {
    before_load: Box<HostCheck>,
    after_load: Box<HostCheck>,
}

impl RuntimeHostChecks {
    /// Binds search preparation and mapped-file/CRT proof around the private load.
    /// The callbacks inspect platform state; the OCR adapter owns hashing and ordering.
    pub fn new<B, A>(before_load: B, after_load: A) -> Self
    where
        B: Fn(&Path) -> Result<(), RuntimeHostError> + Send + Sync + 'static,
        A: Fn(&Path) -> Result<(), RuntimeHostError> + Send + Sync + 'static,
    {
        Self {
            before_load: Box::new(before_load),
            after_load: Box::new(after_load),
        }
    }
}

/// Why the private runtime could not establish its required load boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeLoadError {
    /// Startup did not prepare the required telemetry environment before threads.
    TelemetryNotPrepared,
    /// The executable or fixed sibling library cannot establish a safe installation path.
    Location,
    /// The fixed regular library file cannot be read safely.
    Read,
    /// The library bytes differ from the exact compile-time identity.
    HashMismatch,
    /// A required host check refused the load or environment creation.
    Host(RuntimeHostError),
    /// A native load was already attempted; restarting is required after a native failure.
    AlreadyAttempted,
    /// The verified library could not supply the accepted native API.
    Load,
    /// The native environment could not be built with telemetry disabled.
    Environment,
}

/// One process-owned native environment, ready for bounded worker sessions.
/// Construction is blocking and belongs on an owned CPU worker, never the UI/Tokio executor.
/// No native runtime is initialised merely by linking or importing this adapter.
pub struct OcrRuntime {
    pub(super) _environment: ort::environment::Environment,
    _verified_library: std::fs::File,
}

impl OcrRuntime {
    /// Checks startup, sibling library identity, host policy and mapped modules before the environment.
    /// The composition root owns one non-clone token and passes it exclusively to this loader.
    ///
    /// # Errors
    /// Returns a closed error for missing telemetry preparation, altered/missing files, failed
    /// host checks, a previous native attempt, or a native load/environment failure. No fallback
    /// path, environment-selected library, alternate provider or download is attempted.
    pub fn load(
        token: &mut OcrRuntimeLoadToken,
        checks: RuntimeHostChecks,
    ) -> Result<Self, RuntimeLoadError> {
        if std::env::var_os("ORT_DISABLE_TELEMETRY").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return Err(RuntimeLoadError::TelemetryNotPrepared);
        }
        if token.attempted {
            return Err(RuntimeLoadError::AlreadyAttempted);
        }
        let mut library = library::VerifiedLibrary::open_sibling()?;
        (checks.before_load)(&library.path).map_err(RuntimeLoadError::Host)?;
        library.recheck()?;
        if std::env::var_os("ORT_DISABLE_TELEMETRY").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return Err(RuntimeLoadError::TelemetryNotPrepared);
        }
        token.attempted = true;
        let builder = ort::init_from(&library.path).map_err(|_| RuntimeLoadError::Load)?;
        (checks.after_load)(&library.path).map_err(RuntimeLoadError::Host)?;
        let environment = builder
            .with_name("tallyline-ocr")
            .with_telemetry(false)
            .with_logger(std::sync::Arc::new(|level, _, _, _, _| {
                tracing::debug!(?level, "ONNX Runtime diagnostic");
            }))
            .build()
            .map_err(|_| RuntimeLoadError::Environment)?;
        Ok(Self {
            _environment: environment,
            _verified_library: library.file,
        })
    }

    /// Returns the accepted runtime release identity checked before native loading.
    #[must_use]
    pub const fn version(&self) -> &'static str {
        "1.31.0"
    }
}

impl std::fmt::Display for RuntimeLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "OCR runtime could not start: {self:?}")
    }
}

impl std::error::Error for RuntimeLoadError {}

/// App-owned non-clone state for one process-lifetime native load attempt.
/// Construct once at the composition root before workers and move it to the owning
/// OCR runtime manager. Exclusive mutable access serialises native initialisation.
#[derive(Default)]
pub struct OcrRuntimeLoadToken {
    attempted: bool,
}

impl OcrRuntimeLoadToken {
    /// Creates the composition root's unattempted native load token.
    #[must_use]
    pub const fn new() -> Self {
        Self { attempted: false }
    }

    /// Reports whether a native load was attempted, including failed native loads.
    #[must_use]
    pub const fn native_load_attempted(&self) -> bool {
        self.attempted
    }
}
mod settings;
pub use settings::{InvalidOcrRuntimeSettings, OcrRuntimeSettings};
mod reader;
pub use reader::ResearchOcrReader;
pub use reader::{
    SmallOcrReader, SmallReaderError, SmallReaderProfile, SmallSessionState, SmallTextLine,
};
