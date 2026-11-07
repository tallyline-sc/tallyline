// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Closed native OCR prerequisite outcomes, with no foreign handles.

/// One fixed module required by the accepted Windows OCR library.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOcrModule {
    /// The pinned ONNX Runtime library.
    OnnxRuntime,
    /// The main C++ standard library runtime.
    Msvcp140,
    /// The supplementary C++ standard library runtime.
    Msvcp140Supplement,
    /// The main Visual C++ runtime.
    Vcruntime140,
    /// The supplementary Visual C++ runtime.
    Vcruntime140Supplement,
}

impl NativeOcrModule {
    #[cfg(windows)]
    pub(crate) const fn filename(self) -> &'static str {
        match self {
            Self::OnnxRuntime => "onnxruntime.dll",
            Self::Msvcp140 => "msvcp140.dll",
            Self::Msvcp140Supplement => "msvcp140_1.dll",
            Self::Vcruntime140 => "vcruntime140.dll",
            Self::Vcruntime140Supplement => "vcruntime140_1.dll",
        }
    }
}

/// A mapped Visual C++ module whose file version meets the pinned 14.44 floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrtModuleVersion {
    module: NativeOcrModule,
    parts: [u16; 4],
}

impl CrtModuleVersion {
    /// Validates fixed file-version parts against ADR-0009's pinned floor.
    ///
    /// # Errors
    /// Rejects the ONNX module as a CRT module, or a version below14.44.
    pub fn checked(module: NativeOcrModule, parts: [u16; 4]) -> Result<Self, OcrNativeError> {
        if module == NativeOcrModule::OnnxRuntime {
            return Err(OcrNativeError::InvalidModule);
        }
        if parts < [14, 44, 0, 0] {
            return Err(OcrNativeError::CrtTooOld { module, parts });
        }
        Ok(Self { module, parts })
    }

    /// Returns the fixed CRT module identity.
    #[must_use]
    pub const fn module(self) -> NativeOcrModule {
        self.module
    }

    /// Returns major, minor, build and revision from the mapped file's fixed resource.
    #[must_use]
    pub const fn parts(self) -> [u16; 4] {
        self.parts
    }
}

/// Why the native OCR host could not establish the accepted prerequisites.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OcrNativeError {
    /// The target has no reviewed native host implementation.
    UnsupportedTarget,
    /// A path is relative, unreadable or cannot be resolved.
    InvalidPath,
    /// The Windows DLL search restriction failed.
    SearchRestriction,
    /// A runtime was already mapped before the verified private load.
    AlreadyMapped,
    /// Windows could not query a module handle safely.
    ModuleQuery(NativeOcrModule),
    /// A mapped module path could not be resolved.
    ModulePath(NativeOcrModule),
    /// One required module is absent from the process.
    MissingModule(NativeOcrModule),
    /// The loaded runtime is not the verified installation file.
    MappedFileMismatch,
    /// A mapped module has no valid bounded fixed file-version resource.
    InvalidVersion(NativeOcrModule),
    /// A mapped CRT file is below the pinned toolset floor.
    CrtTooOld {
        /// The affected mapped module.
        module: NativeOcrModule,
        /// Its actual major, minor, build and revision.
        parts: [u16; 4],
    },
    /// A CRT value was requested for the ONNX Runtime module.
    InvalidModule,
}

impl std::fmt::Display for OcrNativeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "native OCR prerequisite failed: {self:?}")
    }
}

impl std::error::Error for OcrNativeError {}

/// Restricts Windows dependency lookup before the private verified ORT load.
/// Targets without a reviewed mapped-file implementation fail before a native load.
///
/// # Errors
/// Refuses an already mapped ONNX module, unavailable Windows policy or unsupported target.
pub fn restrict_ocr_dll_search() -> Result<(), OcrNativeError> {
    #[cfg(windows)]
    {
        crate::windows_ocr::restrict()
    }
    #[cfg(not(windows))]
    {
        Err(OcrNativeError::UnsupportedTarget)
    }
}

/// Verifies the actual mapped OCR file and Windows CRT prerequisites.
/// Returns only checked numeric CRT versions, never native module handles.
///
/// # Errors
/// Rejects unresolved paths, a different or missing mapped library, missing/invalid
/// CRT resources, a version below14.44, or a target without a reviewed implementation.
pub fn verify_ocr_native_modules(
    expected: &std::path::Path,
) -> Result<Vec<CrtModuleVersion>, OcrNativeError> {
    if !expected.is_absolute() {
        return Err(OcrNativeError::InvalidPath);
    }
    let expected = expected
        .canonicalize()
        .map_err(|_| OcrNativeError::InvalidPath)?;
    if !expected.is_file() {
        return Err(OcrNativeError::InvalidPath);
    }
    #[cfg(windows)]
    {
        crate::windows_ocr::verify(&expected)
    }
    #[cfg(not(windows))]
    {
        Err(OcrNativeError::UnsupportedTarget)
    }
}
