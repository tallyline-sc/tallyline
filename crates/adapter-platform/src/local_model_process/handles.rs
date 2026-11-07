// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exclusive Windows handle ownership and immediate OS errors.

use std::io;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};

pub(super) struct Handle(Option<HANDLE>);
impl Handle {
    pub(super) fn new(raw: HANDLE) -> Self {
        Self(Some(raw))
    }
    pub(super) fn raw(&self) -> HANDLE {
        self.0.unwrap_or(std::ptr::null_mut())
    }
    pub(super) fn is_closed(&self) -> bool {
        self.0.is_none()
    }
    /// Closes the exclusive live handle, retaining ownership if closure fails.
    ///
    /// # Safety
    /// Only successful owned API returns construct this type; no handle is duplicated.
    #[expect(
        unsafe_code,
        reason = "CloseHandle releases one exclusively owned handle."
    )]
    pub(super) fn close(&mut self) -> io::Result<()> {
        if let Some(raw) = self.0 {
            if unsafe { CloseHandle(raw) } == 0 {
                return Err(last_error());
            }
            self.0 = None;
        }
        Ok(())
    }
    /// Clears inheritance for one retained parent or job handle.
    ///
    /// # Safety
    /// The owned handle is live and the flag mask is the documented inheritance bit.
    #[expect(
        unsafe_code,
        reason = "SetHandleInformation prevents unintended child inheritance."
    )]
    pub(super) fn clear_inheritance(&self) -> io::Result<()> {
        if unsafe { SetHandleInformation(self.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
            Err(last_error())
        } else {
            Ok(())
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            tracing::error!(?error, "owned handle fallback close failed");
        }
    }
}

/// Captures the calling thread's last error before another OS operation.
///
/// # Safety
/// GetLastError has no parameters and transfers no resource ownership.
#[expect(
    unsafe_code,
    reason = "GetLastError captures an immediate documented failure."
)]
pub(super) fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { GetLastError() }.cast_signed())
}
