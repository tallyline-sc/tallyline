// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit headless native load acceptance probe; never starts the desktop shell.

use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use tallyline_adapter_ocr::api::model::{
    OcrRuntime, OcrRuntimeLoadToken, RuntimeHostChecks, RuntimeHostError, RuntimeLoadError,
    SmallOcrAssets,
};
use tallyline_adapter_platform::api::model::{
    CrtModuleVersion, OcrChildExit, OcrNativeError, OcrProcessStartup,
};
use tallyline_adapter_platform::api::{restrict_ocr_dll_search, verify_ocr_native_modules};

fn main() -> Result<ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    match tallyline_adapter_platform::api::prepare_ocr_process()? {
        OcrProcessStartup::Prepared => run(),
        OcrProcessStartup::Relaunched(status) => Ok(match OcrChildExit::from_status(&status) {
            OcrChildExit::Code(code) => ExitCode::from(code),
            OcrChildExit::Abnormal => ExitCode::FAILURE,
        }),
    }
}

fn run_worker(
    mut token: OcrRuntimeLoadToken,
) -> Result<ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    let mut arguments = std::env::args_os().skip(1);
    let mode = arguments.next();
    let research = ResearchSelection::parse(mode.as_deref(), &mut arguments)?;
    let reader_mode = match mode.as_deref() {
        Some(value) if value == std::ffi::OsStr::new("--small-reader") => Some(false),
        Some(value) if value == std::ffi::OsStr::new("--small-textline") => Some(true),
        _ if research.is_some() => research.as_ref().and_then(|selection| selection.line_mode),
        _ => None,
    };
    let reader_path = if reader_mode.is_some() {
        arguments.next()
    } else {
        None
    };
    let reader_selftest = mode.as_deref() == Some(std::ffi::OsStr::new("--small-reader-selftest"))
        || research
            .as_ref()
            .is_some_and(|selection| selection.line_mode.is_none());
    let tensors = match mode.as_deref() {
        Some(value) if value == std::ffi::OsStr::new("--load-only") => false,
        Some(value) if value == std::ffi::OsStr::new("--small-tensors") => true,
        _ if reader_mode.is_some() || reader_selftest => false,
        _ => {
            return Err(
                std::io::Error::other("expected an explicit bounded OCR probe mode").into(),
            );
        }
    };
    let raw_output = match arguments.next() {
        None => None,
        Some(flag) if reader_mode.is_some() && flag == std::ffi::OsStr::new("--raw-output") => {
            Some(arguments.next().ok_or_else(|| {
                std::io::Error::other("raw output requires one absolute private destination")
            })?)
        }
        Some(_) => return Err(std::io::Error::other("unexpected probe arguments").into()),
    };
    if arguments.next().is_some() {
        return Err(std::io::Error::other("unexpected probe arguments").into());
    }
    let versions = Arc::new(Mutex::new(Vec::<CrtModuleVersion>::new()));
    let failure = Arc::new(Mutex::new(None::<OcrNativeError>));
    let before_failure = Arc::clone(&failure);
    let after_failure = Arc::clone(&failure);
    let after_versions = Arc::clone(&versions);
    let checks = RuntimeHostChecks::new(
        move |_| {
            restrict_ocr_dll_search().map_err(|error| {
                if let Ok(mut last) = before_failure.lock() {
                    *last = Some(error);
                }
                RuntimeHostError::SearchPolicy
            })
        },
        move |path| match verify_ocr_native_modules(path) {
            Ok(observed) => {
                *after_versions
                    .lock()
                    .map_err(|_| RuntimeHostError::MappedLibrary)? = observed;
                Ok(())
            }
            Err(error) => {
                if let Ok(mut last) = after_failure.lock() {
                    *last = Some(error);
                }
                Err(match error {
                    OcrNativeError::CrtTooOld { .. }
                    | OcrNativeError::InvalidVersion(_)
                    | OcrNativeError::MissingModule(_) => RuntimeHostError::CrtPrerequisite,
                    OcrNativeError::UnsupportedTarget
                    | OcrNativeError::InvalidPath
                    | OcrNativeError::SearchRestriction
                    | OcrNativeError::AlreadyMapped
                    | OcrNativeError::ModuleQuery(_)
                    | OcrNativeError::ModulePath(_)
                    | OcrNativeError::MappedFileMismatch
                    | OcrNativeError::InvalidModule => RuntimeHostError::MappedLibrary,
                })
            }
        },
    );
    if reader_mode.is_some() || reader_selftest {
        return run_reader(
            token,
            checks,
            reader_path,
            reader_mode,
            reader_selftest,
            raw_output,
            research,
        );
    }
    let mut runtime = OcrRuntime::load(&mut token, checks);
    let duplicate_refused = if token.native_load_attempted() {
        let retry = RuntimeHostChecks::new(
            |_| Err(RuntimeHostError::SearchPolicy),
            |_| Err(RuntimeHostError::MappedLibrary),
        );
        matches!(
            OcrRuntime::load(&mut token, retry),
            Err(RuntimeLoadError::AlreadyAttempted)
        )
    } else {
        false
    };
    if token.native_load_attempted() && !duplicate_refused {
        return Err(std::io::Error::other("native load token did not retain its attempt").into());
    }
    let output = match &mut runtime {
        Ok(runtime) => {
            let versions = versions
                .lock()
                .map_err(|_| std::io::Error::other("probe version result unavailable"))?;
            let versions: Vec<_> = versions.iter().map(|entry| serde_json::json!({"module": format!("{:?}", entry.module()), "version": entry.parts()})).collect();
            let tensor_report = if tensors {
                let executable = std::env::current_exe()?.canonicalize()?;
                let directory = executable
                    .parent()
                    .ok_or_else(|| std::io::Error::other("probe executable has no directory"))?
                    .join("models/small");
                let assets = SmallOcrAssets::load(&directory)?;
                let smoke = runtime.smoke_small_models(&assets)?;
                serde_json::json!({"profile": smoke.profile, "detector": {"input_shape": smoke.detector.input_shape, "output_shape": smoke.detector.output_shape, "output_elements": smoke.detector.output_elements, "probability_range": smoke.detector.probability_range}, "recognizer": {"input_shape": smoke.recognizer.input_shape, "output_shape": smoke.recognizer.output_shape, "output_elements": smoke.recognizer.output_elements, "probability_range": smoke.recognizer.probability_range}, "dictionary_classes": smoke.dictionary_classes, "ctc_emitted_labels": smoke.ctc_emitted_labels, "requested_session_options": {"intra_op_threads":1,"inter_op_threads":1,"sequential":true,"spinning":false,"cpu_arena":false,"memory_pattern":false,"graph_optimisation":3}})
            } else {
                serde_json::Value::String("not_run".to_owned())
            };
            serde_json::json!({"runtime": runtime.version(), "native_load_attempted": token.native_load_attempted(), "duplicate_native_load_refused": duplicate_refused, "mapped_library": "verified", "crt": versions, "tensor_smoke": tensor_report})
        }
        Err(error) => {
            let failure = failure
                .lock()
                .map_err(|_| std::io::Error::other("probe host result unavailable"))?;
            serde_json::json!({"runtime_load": "failed", "native_load_attempted": token.native_load_attempted(), "duplicate_native_load_refused": duplicate_refused, "reason": error.to_string(), "host_failure": format!("{failure:?}")})
        }
    };
    serde_json::to_writer_pretty(std::io::stdout().lock(), &output)?;
    Ok(if runtime.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    let token = OcrRuntimeLoadToken::new();
    tallyline_app::api::CpuWorker::start("tallyline-ocr-probe", move || run_worker(token))?
        .finish()?
}

fn run_reader(
    mut token: OcrRuntimeLoadToken,
    checks: RuntimeHostChecks,
    path: Option<std::ffi::OsString>,
    line_mode: Option<bool>,
    selftest: bool,
    raw_output: Option<std::ffi::OsString>,
    research: Option<ResearchSelection>,
) -> Result<ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    use std::time::{Duration, Instant};
    use tallyline_adapter_files::api::model::{ImageIntake, ImageIntakeSettings};
    use tallyline_adapter_ocr::api::model::{
        OcrRuntimeSettings, ResearchOcrAssets, ResearchOcrReader, SmallOcrReader, SmallSessionState,
    };
    use tallyline_shared_kernel::api::model::ImageRaster;
    let executable = std::env::current_exe()?.canonicalize()?;
    let directory = executable
        .parent()
        .ok_or_else(|| std::io::Error::other("probe executable has no directory"))?
        .join("models/small");
    let research_assets = research
        .as_ref()
        .map(|selection| {
            ResearchOcrAssets::load(
                selection.profile,
                &selection.detector,
                &selection.recognizer,
                &selection.dictionary,
            )
        })
        .transpose()?;
    let assets = if research_assets.is_none() {
        Some(SmallOcrAssets::load(&directory)?)
    } else {
        None
    };
    let identities = research_assets.as_ref().map(|assets| assets.fingerprints().iter().map(|identity| serde_json::json!({"artifact":format!("{:?}", identity.artifact), "sha256":identity.sha256, "bytes":identity.bytes})).collect::<Vec<_>>());
    let synthetic_clock = std::cell::Cell::new(Duration::ZERO);
    let started = Instant::now();
    let clock = || {
        if selftest {
            synthetic_clock.get()
        } else {
            started.elapsed()
        }
    };
    let initialise = || OcrRuntime::load(&mut token, checks);
    let mut reader = match (&assets, &research_assets) {
        (Some(assets), None) => ProbeReader::Small(SmallOcrReader::new(
            assets,
            OcrRuntimeSettings::default(),
            clock,
            initialise,
        )),
        (None, Some(assets)) => ProbeReader::Research(ResearchOcrReader::new(
            assets,
            OcrRuntimeSettings::default(),
            clock,
            initialise,
        )),
        _ => return Err(std::io::Error::other("invalid explicit reader selection").into()),
    };
    if reader.session_state() != SmallSessionState::Empty {
        return Err(std::io::Error::other("reader did not start empty").into());
    }
    let mut output = if selftest {
        let image = ImageRaster::new(64, 64, vec![0xff80_8080; 64 * 64])
            .map_err(|_| std::io::Error::other("invalid synthetic raster"))?;
        let first = reader.read_line(&image)?;
        let reused = reader.read_line(&image)?;
        if first != reused || reader.session_state() != SmallSessionState::Recognizer {
            return Err(std::io::Error::other("recogniser reuse contract failed").into());
        }
        let boxes = reader.detect(&image)?;
        if reader.session_state() != SmallSessionState::Both {
            return Err(std::io::Error::other("detector did not join resident recogniser").into());
        }
        synthetic_clock.set(Duration::from_secs(1));
        let _completion_read = reader.read_line(&image)?;
        synthetic_clock.set(Duration::ZERO);
        if !matches!(
            reader.close_if_idle(),
            Err(tallyline_adapter_ocr::api::model::SmallReaderError::ClockRegression)
        ) || reader.session_state() != SmallSessionState::Both
        {
            return Err(std::io::Error::other("monotonic clock regression contract failed").into());
        }
        synthetic_clock.set(Duration::from_millis(300_999));
        if reader.close_if_idle()? {
            return Err(std::io::Error::other("sessions closed before idle boundary").into());
        }
        synthetic_clock.set(Duration::from_secs(301));
        if !reader.close_if_idle()? || reader.session_state() != SmallSessionState::Empty {
            return Err(std::io::Error::other("sessions did not close at idle boundary").into());
        }
        let reopened = reader.read_line(&image)?;
        if reopened != first {
            return Err(std::io::Error::other("recogniser changed after session reopening").into());
        }
        reader.close_sessions();
        serde_json::json!({"profile":reader.profile(),"mode":"synthetic-reader-lifecycle","labels":first.characters.len(),"detected_boxes":boxes.len(),"recogniser_repeat_equal":true,"both_sessions_resident":true,"idle_before_boundary_retained":true,"idle_at_boundary_released":true,"reopened_read_equal":true,"monotonic_regression_refused":true,"explicit_shutdown_empty":reader.session_state()==SmallSessionState::Empty})
    } else {
        let path =
            path.ok_or_else(|| std::io::Error::other("reader mode requires one image path"))?;
        let image = match tallyline_adapter_files::api::read_image_file(
            std::path::Path::new(&path),
            &ImageIntakeSettings::default(),
        ) {
            ImageIntake::Decoded(image) => image,
            ImageIntake::SetAside(_) => {
                return Err(std::io::Error::other("image intake refused the probe image").into());
            }
        };
        let (lines, labels, raw_lines) = if line_mode == Some(true) {
            let reading = reader.read_line(&image.raster)?;
            let raw = if raw_output.is_some() {
                let width = f64::from(image.raster.width());
                let height = f64::from(image.raster.height());
                vec![raw_line(
                    [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]],
                    None,
                    &reading,
                )]
            } else {
                Vec::new()
            };
            (1, reading.characters.len(), raw)
        } else {
            let readings = reader.read_image(&image.raster)?;
            let raw = if raw_output.is_some() {
                readings
                    .iter()
                    .map(|line| {
                        raw_line(
                            line.region.corners,
                            Some(line.region.probability),
                            &line.reading,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            (
                readings.len(),
                readings
                    .iter()
                    .map(|line| line.reading.characters.len())
                    .sum(),
                raw,
            )
        };
        if let Some(path) = &raw_output {
            let mut artifact = serde_json::json!({"schema_version":1,"profile":reader.profile(),"mode":if line_mode == Some(true) {"raw-textline"} else {"raw-image"},"dimensions":[image.raster.width(),image.raster.height()],"downscaled_at_intake":image.downscaled.is_some(),"lines":raw_lines});
            if let Some(identities) = &identities {
                artifact
                    .as_object_mut()
                    .ok_or_else(|| std::io::Error::other("raw receipt must be an object"))?
                    .insert("model_identities".to_owned(), serde_json::json!(identities));
            }
            let bytes = serde_json::to_vec_pretty(&artifact)?;
            tallyline_adapter_files::api::write_private_artifact(
                std::path::Path::new(path),
                &bytes,
            )?;
        }
        serde_json::json!({"profile":reader.profile(),"mode":if line_mode == Some(true) { "raw-textline" } else { "raw-image" },"image_dimensions":[image.raster.width(),image.raster.height()],"downscaled_at_intake":image.downscaled.is_some(),"lines":lines,"labels":labels,"private_raw_artifact_written":raw_output.is_some(),"session_state":format!("{:?}",reader.session_state())})
    };
    if let Some(identities) = &identities {
        output
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("probe receipt must be an object"))?
            .insert("model_identities".to_owned(), serde_json::json!(identities));
    }
    drop(reader);
    if !token.native_load_attempted() {
        return Err(std::io::Error::other("reader did not initialise the native runtime").into());
    }
    let retry = RuntimeHostChecks::new(
        |_| Err(RuntimeHostError::SearchPolicy),
        |_| Err(RuntimeHostError::MappedLibrary),
    );
    if !matches!(
        OcrRuntime::load(&mut token, retry),
        Err(RuntimeLoadError::AlreadyAttempted)
    ) {
        return Err(std::io::Error::other("reader did not retain the single load attempt").into());
    }
    serde_json::to_writer_pretty(std::io::stdout().lock(), &output)?;
    Ok(ExitCode::SUCCESS)
}

fn raw_line(
    corners: [[f64; 2]; 4],
    detection_probability: Option<f32>,
    reading: &tallyline_adapter_ocr::api::model::CtcReading,
) -> serde_json::Value {
    let characters: Vec<_> = reading.characters.iter().map(|character| serde_json::json!({"symbol":character.symbol,"probability":character.probability,"frames":[character.frames.start,character.frames.end]})).collect();
    serde_json::json!({"corners":corners,"detection_probability":detection_probability,"text":reading.text,"characters":characters})
}

struct ResearchSelection {
    profile: tallyline_adapter_ocr::api::model::OcrResearchProfile,
    detector: std::path::PathBuf,
    recognizer: std::path::PathBuf,
    dictionary: std::path::PathBuf,
    line_mode: Option<bool>,
}

impl ResearchSelection {
    fn parse(
        mode: Option<&std::ffi::OsStr>,
        arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    ) -> Result<Option<Self>, std::io::Error> {
        use tallyline_adapter_ocr::api::model::{OcrDetector, OcrRecognizer, OcrResearchProfile};
        let line_mode = match mode.and_then(std::ffi::OsStr::to_str) {
            Some("--research-reader") => Some(false),
            Some("--research-textline") => Some(true),
            Some("--research-reader-selftest") => None,
            _ => return Ok(None),
        };
        let mut next = || {
            arguments.next().ok_or_else(|| std::io::Error::other("research mode requires detector family, recogniser family and three absolute data paths"))
        };
        let detector = match next()?.to_str() {
            Some("tiny") => OcrDetector::Tiny,
            Some("small") => OcrDetector::Small,
            Some("medium") => OcrDetector::Medium,
            _ => return Err(std::io::Error::other("unknown reviewed detector family")),
        };
        let recognizer = match next()?.to_str() {
            Some("tiny") => OcrRecognizer::Tiny,
            Some("small") => OcrRecognizer::Small,
            Some("medium") => OcrRecognizer::Medium,
            _ => return Err(std::io::Error::other("unknown reviewed recogniser family")),
        };
        Ok(Some(Self {
            profile: OcrResearchProfile::new(detector, recognizer),
            detector: next()?.into(),
            recognizer: next()?.into(),
            dictionary: next()?.into(),
            line_mode,
        }))
    }
}

enum ProbeReader<'a> {
    Small(tallyline_adapter_ocr::api::model::SmallOcrReader<'a>),
    Research(tallyline_adapter_ocr::api::model::ResearchOcrReader<'a>),
}

impl ProbeReader<'_> {
    fn profile(&self) -> &'static str {
        match self {
            Self::Small(reader) => reader.profile().identifier(),
            Self::Research(reader) => reader.profile().identifier(),
        }
    }

    fn session_state(&self) -> tallyline_adapter_ocr::api::model::SmallSessionState {
        match self {
            Self::Small(reader) => reader.session_state(),
            Self::Research(reader) => reader.session_state(),
        }
    }

    fn close_if_idle(
        &mut self,
    ) -> Result<bool, tallyline_adapter_ocr::api::model::SmallReaderError> {
        match self {
            Self::Small(reader) => reader.close_if_idle(),
            Self::Research(reader) => reader.close_if_idle(),
        }
    }

    fn close_sessions(&mut self) {
        match self {
            Self::Small(reader) => reader.close_sessions(),
            Self::Research(reader) => reader.close_sessions(),
        }
    }

    fn read_line(
        &mut self,
        image: &tallyline_shared_kernel::api::model::ImageRaster,
    ) -> Result<
        tallyline_adapter_ocr::api::model::CtcReading,
        tallyline_adapter_ocr::api::model::SmallReaderError,
    > {
        match self {
            Self::Small(reader) => reader.read_line(image),
            Self::Research(reader) => reader.read_line(image),
        }
    }

    fn detect(
        &mut self,
        image: &tallyline_shared_kernel::api::model::ImageRaster,
    ) -> Result<
        Vec<tallyline_adapter_ocr::api::model::DetectionBox>,
        tallyline_adapter_ocr::api::model::SmallReaderError,
    > {
        match self {
            Self::Small(reader) => reader.detect(image),
            Self::Research(reader) => reader.detect(image),
        }
    }

    fn read_image(
        &mut self,
        image: &tallyline_shared_kernel::api::model::ImageRaster,
    ) -> Result<
        Vec<tallyline_adapter_ocr::api::model::SmallTextLine>,
        tallyline_adapter_ocr::api::model::SmallReaderError,
    > {
        match self {
            Self::Small(reader) => reader.read_image(image),
            Self::Research(reader) => reader.read_image(image),
        }
    }
}
