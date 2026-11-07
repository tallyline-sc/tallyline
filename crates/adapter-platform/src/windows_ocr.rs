// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Owned mapped-module checks for the accepted ONNX/CRT boundary.

use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{ERROR_MOD_NOT_FOUND, FreeLibrary, HMODULE};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, LOAD_LIBRARY_SEARCH_APPLICATION_DIR,
    LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories,
};

use crate::ocr_native::{CrtModuleVersion, NativeOcrModule, OcrNativeError};

const MAX_MODULE_PATH: usize = 32768;
const MAX_VERSION_BYTES: u32 = 1024 * 1024;

struct Module {
    handle: HMODULE,
    name: NativeOcrModule,
}

impl Module {
    /// Owns one reference to an existing mapped module; no library is loaded.
    ///
    /// # Safety
    /// The fixed UTF-16 name is terminated and lives through the call. Flags zero
    /// acquire one reference into initialised handle storage; Drop releases it once.
    #[expect(
        unsafe_code,
        reason = "GetModuleHandleExW owns an existing mapped-module reference."
    )]
    fn acquire(name: NativeOcrModule) -> Result<Self, OcrNativeError> {
        let text: Vec<_> = name.filename().encode_utf16().chain([0]).collect();
        let mut handle = std::ptr::null_mut();
        if unsafe { GetModuleHandleExW(0, text.as_ptr(), &mut handle) } == 0 || handle.is_null() {
            return Err(
                if std::io::Error::last_os_error().raw_os_error()
                    == Some(ERROR_MOD_NOT_FOUND as i32)
                {
                    OcrNativeError::MissingModule(name)
                } else {
                    OcrNativeError::ModuleQuery(name)
                },
            );
        }
        Ok(Self { handle, name })
    }

    /// Copies the path while this wrapper owns a live module reference.
    ///
    /// # Safety
    /// The handle remains mapped, and the initialised writable UTF-16 buffer has
    /// exactly the advertised capacity. Zero/truncation are rejected before slicing.
    #[expect(
        unsafe_code,
        reason = "Mapped file identity requires GetModuleFileNameW."
    )]
    fn path(&self) -> Result<PathBuf, OcrNativeError> {
        let mut buffer = vec![0_u16; MAX_MODULE_PATH];
        let capacity = u32::try_from(buffer.len()).map_err(|_| OcrNativeError::InvalidPath)?;
        let length = unsafe { GetModuleFileNameW(self.handle, buffer.as_mut_ptr(), capacity) };
        if length == 0 || length >= capacity {
            return Err(OcrNativeError::ModulePath(self.name));
        }
        let path = buffer
            .get(..length as usize)
            .ok_or(OcrNativeError::ModulePath(self.name))?;
        let path = PathBuf::from(std::ffi::OsString::from_wide(path));
        if !path.is_absolute() {
            return Err(OcrNativeError::ModulePath(self.name));
        }
        path.canonicalize()
            .map_err(|_| OcrNativeError::ModulePath(self.name))
    }
}

impl Drop for Module {
    /// Balances only this wrapper's successful GetModuleHandleExW reference.
    ///
    /// # Safety
    /// The uniquely owned live reference is released exactly once, after path and
    /// resource inspection; no borrowed or uncounted module handle is freed.
    #[expect(
        unsafe_code,
        reason = "The owned mapped-module reference requires FreeLibrary."
    )]
    fn drop(&mut self) {
        if unsafe { FreeLibrary(self.handle) } == 0 {
            tracing::error!(module = ?self.name, "native OCR module reference release failed");
        }
    }
}

/// Restricts dependency lookup to the executable directory and Windows System32.
///
/// # Safety
/// Only documented directory flags are passed. The process-wide Windows API is
/// thread-safe; it reduces future implicit searches and is called before ORT loads.
#[expect(
    unsafe_code,
    reason = "ADR-0007 requires Windows DLL search-path restriction."
)]
pub(super) fn restrict() -> Result<(), OcrNativeError> {
    match Module::acquire(NativeOcrModule::OnnxRuntime) {
        Ok(_) => return Err(OcrNativeError::AlreadyMapped),
        Err(OcrNativeError::MissingModule(NativeOcrModule::OnnxRuntime)) => {}
        Err(error) => return Err(error),
    }
    if unsafe {
        SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32)
    } == 0
    {
        return Err(OcrNativeError::SearchRestriction);
    }
    Ok(())
}

pub(super) fn verify(expected: &Path) -> Result<Vec<CrtModuleVersion>, OcrNativeError> {
    let module = Module::acquire(NativeOcrModule::OnnxRuntime)?;
    if module.path()? != expected {
        return Err(OcrNativeError::MappedFileMismatch);
    }
    let mut versions = Vec::with_capacity(4);
    for name in [
        NativeOcrModule::Msvcp140,
        NativeOcrModule::Msvcp140Supplement,
        NativeOcrModule::Vcruntime140,
        NativeOcrModule::Vcruntime140Supplement,
    ] {
        let module = Module::acquire(name)?;
        let version = file_version(&module.path()?, name)?;
        versions.push(CrtModuleVersion::checked(name, version)?);
    }
    Ok(versions)
}

/// Copies the fixed numeric version from a bounded owned resource buffer.
///
/// # Safety
/// Terminated UTF-16 pathname and initialised output storage outlive the calls.
/// The query result must lie fully inside the owned allocation and contain at
/// least VS_FIXEDFILEINFO bytes before read_unaligned copies its all-integer value.
#[expect(
    unsafe_code,
    reason = "Mapped CRT file versions require Win32 version-resource APIs."
)]
fn file_version(path: &Path, name: NativeOcrModule) -> Result<[u16; 4], OcrNativeError> {
    use std::os::windows::ffi::OsStrExt;
    let invalid = OcrNativeError::InvalidVersion(name);
    let mut path: Vec<_> = path.as_os_str().encode_wide().collect();
    if path.contains(&0) {
        return Err(invalid);
    }
    path.push(0);
    let bytes = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), std::ptr::null_mut()) };
    if bytes == 0 || bytes > MAX_VERSION_BYTES {
        return Err(invalid);
    }
    let words = usize::try_from(bytes.div_ceil(4)).map_err(|_| invalid)?;
    let mut buffer = Vec::<u32>::new();
    buffer.try_reserve_exact(words).map_err(|_| invalid)?;
    buffer.resize(words, 0);
    if unsafe { GetFileVersionInfoW(path.as_ptr(), 0, bytes, buffer.as_mut_ptr().cast()) } == 0 {
        return Err(invalid);
    }
    let root = [0x005c_u16, 0];
    let mut fixed = std::ptr::null_mut();
    let mut length = 0;
    if unsafe {
        VerQueryValueW(
            buffer.as_ptr().cast(),
            root.as_ptr(),
            &mut fixed,
            &mut length,
        )
    } == 0
    {
        return Err(invalid);
    }
    let size = std::mem::size_of::<VS_FIXEDFILEINFO>();
    let start = buffer.as_ptr() as usize;
    let end = start.checked_add(bytes as usize).ok_or(invalid)?;
    let pointer = fixed as usize;
    if fixed.is_null()
        || (length as usize) < size
        || pointer < start
        || pointer.checked_add(size).is_none_or(|limit| limit > end)
    {
        return Err(invalid);
    }
    let fixed = unsafe { std::ptr::read_unaligned(fixed.cast::<VS_FIXEDFILEINFO>()) };
    if fixed.dwSignature != 0xfeef04bd {
        return Err(invalid);
    }
    Ok([
        (fixed.dwFileVersionMS >> 16) as u16,
        fixed.dwFileVersionMS as u16,
        (fixed.dwFileVersionLS >> 16) as u16,
        fixed.dwFileVersionLS as u16,
    ])
}
