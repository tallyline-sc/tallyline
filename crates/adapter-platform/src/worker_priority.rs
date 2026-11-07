// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Calling-thread scheduling controls for owned CPU workers.

use std::io;

#[cfg(target_os = "linux")]
const BELOW_NORMAL_NICE: i32 = 1;

/// An observed OS scheduling level for the calling thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerPriority(i32);

impl WorkerPriority {
    /// Reports a Windows level below normal or a positive Linux nice value.
    #[must_use]
    pub const fn is_below_normal(self) -> bool {
        #[cfg(windows)]
        {
            self.0 < 0
        }
        #[cfg(target_os = "linux")]
        {
            self.0 > 0
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            false
        }
    }
}

/// Reads only the calling thread's scheduling priority.
///
/// # Errors
/// Returns the OS query failure or an unsupported-target error.
pub fn worker_priority() -> io::Result<WorkerPriority> {
    #[cfg(windows)]
    {
        windows::query().map(WorkerPriority)
    }
    #[cfg(target_os = "linux")]
    {
        rustix::process::getpriority_process(None)
            .map(WorkerPriority)
            .map_err(Into::into)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "worker priority is unavailable",
        ))
    }
}

/// Lowers only the calling thread below normal and verifies the resulting level.
/// An already lower priority is preserved; this function never raises priority.
///
/// # Errors
/// Returns an OS failure, failed verification or unsupported-target error.
pub fn lower_worker_priority() -> io::Result<WorkerPriority> {
    let before = worker_priority()?;
    if !before.is_below_normal() {
        #[cfg(windows)]
        windows::lower()?;
        #[cfg(target_os = "linux")]
        rustix::process::setpriority_process(None, BELOW_NORMAL_NICE)?;
    }
    let after = worker_priority()?;
    if after.is_below_normal() {
        Ok(after)
    } else {
        Err(io::Error::other("worker priority was not lowered"))
    }
}

#[cfg(windows)]
mod windows {
    use std::io;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, GetThreadPriority, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };

    /// Obtains the calling thread's borrowed pseudo-handle without acquiring a resource.
    ///
    /// # Safety
    /// GetCurrentThread has no parameters; the returned pseudo-handle stays on this thread
    /// and is neither closed nor retained after its immediate priority operation.
    #[expect(
        unsafe_code,
        reason = "GetCurrentThread returns a borrowed calling-thread pseudo-handle."
    )]
    fn current() -> HANDLE {
        unsafe { GetCurrentThread() }
    }

    /// Reads one live calling-thread pseudo-handle and rejects the documented error sentinel.
    ///
    /// # Safety
    /// The pseudo-handle belongs to the calling thread and remains valid through this call.
    #[expect(
        unsafe_code,
        reason = "GetThreadPriority inspects the calling-thread pseudo-handle."
    )]
    pub(super) fn query() -> io::Result<i32> {
        let level = unsafe { GetThreadPriority(current()) };
        if level == i32::MAX {
            Err(io::Error::last_os_error())
        } else {
            Ok(level)
        }
    }

    /// Sets the fixed below-normal level on the calling thread only.
    ///
    /// # Safety
    /// The live calling-thread pseudo-handle and named below-normal level are valid;
    /// failure is checked immediately and no handle ownership is transferred.
    #[expect(
        unsafe_code,
        reason = "SetThreadPriority lowers only the calling CPU worker."
    )]
    pub(super) fn lower() -> io::Result<()> {
        if unsafe { SetThreadPriority(current(), THREAD_PRIORITY_BELOW_NORMAL) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
