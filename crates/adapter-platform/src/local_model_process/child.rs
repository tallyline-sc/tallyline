// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Suspended fixed-command creation and retained-root identity checks.

use super::handles::{self, Handle};
use super::model::{MAX_WINDOWS_UNITS, OwnedProcessCommand};
use super::startup::Attributes;
use std::io;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use windows_sys::Win32::Foundation::{FILETIME, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading as api;

pub(super) struct Child {
    pub(super) root: Handle,
    pub(super) thread: Handle,
    pub(super) pid: usize,
}

fn wide(value: &std::ffi::OsStr) -> io::Result<Vec<u16>> {
    let mut result: Vec<u16> = value.encode_wide().take(MAX_WINDOWS_UNITS).collect();
    if result.contains(&0) || result.len() >= MAX_WINDOWS_UNITS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Windows command string",
        ));
    }
    result.push(0);
    Ok(result)
}

fn command_line(command: &OwnedProcessCommand) -> io::Result<Vec<u16>> {
    const QUOTE: u16 = 34;
    const BACKSLASH: u16 = 92;
    const SPACE: u16 = 32;
    let mut result = Vec::new();
    for argument in std::iter::once(command.executable.as_os_str())
        .chain(command.arguments.iter().map(AsRef::as_ref))
    {
        if !result.is_empty() {
            result.push(SPACE);
        }
        result.push(QUOTE);
        let mut backslashes = 0;
        for unit in argument.encode_wide() {
            if unit == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "NUL in child argument",
                ));
            }
            if unit == BACKSLASH {
                backslashes += 1;
                continue;
            }
            let copies = if unit == QUOTE {
                backslashes * 2 + 1
            } else {
                backslashes
            };
            result.extend(std::iter::repeat_n(BACKSLASH, copies));
            result.push(unit);
            backslashes = 0;
            if result.len() >= MAX_WINDOWS_UNITS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "child arguments exceed limit",
                ));
            }
        }
        result.extend(std::iter::repeat_n(BACKSLASH, backslashes * 2));
        result.push(QUOTE);
        if result.len() >= MAX_WINDOWS_UNITS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child arguments exceed limit",
            ));
        }
    }
    result.push(0);
    Ok(result)
}

fn environment(command: &OwnedProcessCommand) -> io::Result<Vec<u16>> {
    const MAX_ENVIRONMENT_UNITS: usize = 32767;
    let mut rows = Vec::new();
    for (name, value) in &command.environment {
        let key = name.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "child environment name is not ASCII",
            )
        })?;
        if key.is_empty() || !key.is_ascii() || key.contains(['=', '\0']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid child environment name",
            ));
        }
        rows.push((key.to_ascii_uppercase(), name, value));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    if rows
        .windows(2)
        .any(|pair| pair.first().map(|row| &row.0) == pair.last().map(|row| &row.0))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "duplicate child environment name",
        ));
    }
    let mut block = Vec::new();
    for (_, name, value) in rows {
        let mut row = wide(name)?;
        row.pop();
        row.push(u16::from(b'='));
        row.extend(wide(value)?);
        if block
            .len()
            .checked_add(row.len())
            .is_none_or(|length| length >= MAX_ENVIRONMENT_UNITS)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child environment exceeds limit",
            ));
        }
        block.extend(row);
    }
    block.push(0);
    Ok(block)
}

/// Creates only the supplied absolute command suspended in its job at creation time.
///
/// # Safety
/// All UTF-16 buffers are bounded and terminated and the environment is double terminated;
/// extended startup storage/handle arrays remain live and stationary through CreateProcessW.
/// Only HANDLE_LIST's three child pipe ends are inheritable; returned handles transfer once.
#[expect(
    unsafe_code,
    reason = "CreateProcessW uses the exact job and explicit stdio inheritance list."
)]
pub(super) fn spawn(
    command: &OwnedProcessCommand,
    job: &Handle,
    stdin: &Handle,
    stdout: &Handle,
    stderr: &Handle,
) -> io::Result<Child> {
    let executable = wide(command.executable.as_os_str())?;
    let directory = wide(command.private.as_os_str())?;
    let mut arguments = command_line(command)?;
    let environment = environment(command)?;
    let attributes = Attributes::new(job.raw(), [stdin.raw(), stdout.raw(), stderr.raw()])?;
    let startup = api::STARTUPINFOEXW {
        StartupInfo: api::STARTUPINFOW {
            cb: u32::try_from(mem::size_of::<api::STARTUPINFOEXW>()).map_err(io::Error::other)?,
            dwFlags: api::STARTF_USESTDHANDLES,
            hStdInput: stdin.raw(),
            hStdOutput: stdout.raw(),
            hStdError: stderr.raw(),
            ..Default::default()
        },
        lpAttributeList: attributes.raw(),
    };
    let mut process = api::PROCESS_INFORMATION::default();
    let flags = api::CREATE_SUSPENDED
        | api::BELOW_NORMAL_PRIORITY_CLASS
        | api::CREATE_UNICODE_ENVIRONMENT
        | api::CREATE_NO_WINDOW
        | api::EXTENDED_STARTUPINFO_PRESENT;
    if unsafe {
        api::CreateProcessW(
            executable.as_ptr(),
            arguments.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            flags,
            environment.as_ptr().cast(),
            directory.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    } == 0
    {
        return Err(handles::last_error());
    }
    Ok(Child {
        root: Handle::new(process.hProcess),
        thread: Handle::new(process.hThread),
        pid: process.dwProcessId as usize,
    })
}

/// Reads creation time from the exact retained process handle.
///
/// # Safety
/// The four FILETIME outputs are initialised and live; the handle is not reopened by PID.
#[expect(
    unsafe_code,
    reason = "GetProcessTimes binds observations to a retained process identity."
)]
pub(super) fn creation_time(root: &Handle) -> io::Result<u64> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { api::GetProcessTimes(root.raw(), &mut creation, &mut exit, &mut kernel, &mut user) }
        == 0
    {
        return Err(handles::last_error());
    }
    Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
}

/// Checks current process liveness without blocking.
///
/// # Safety
/// The exact retained process handle stays live for the zero-duration wait.
#[expect(
    unsafe_code,
    reason = "WaitForSingleObject observes only the retained owned process."
)]
pub(super) fn exited(root: &Handle) -> io::Result<bool> {
    match unsafe { api::WaitForSingleObject(root.raw(), 0) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        WAIT_FAILED => Err(handles::last_error()),
        _ => Err(io::Error::other("unexpected owned process wait result")),
    }
}

/// Checks the retained process's actual below-normal priority class.
///
/// # Safety
/// The exact live retained handle is borrowed without ownership transfer.
#[expect(
    unsafe_code,
    reason = "GetPriorityClass verifies the owned root's configured priority."
)]
pub(super) fn verify_priority(root: &Handle) -> io::Result<()> {
    let priority = unsafe { api::GetPriorityClass(root.raw()) };
    if priority == 0 {
        return Err(handles::last_error());
    }
    if priority != api::BELOW_NORMAL_PRIORITY_CLASS {
        return Err(io::Error::other("owned process priority differs"));
    }
    Ok(())
}

/// Resumes the initial thread only when its previous suspend count is exactly one.
///
/// # Safety
/// The handle identifies only the exclusively retained initial thread; no foreign thread opens.
#[expect(
    unsafe_code,
    reason = "ResumeThread resumes the exact suspended initial thread once."
)]
pub(super) fn resume(thread: &Handle) -> io::Result<()> {
    let previous = unsafe { api::ResumeThread(thread.raw()) };
    if previous == u32::MAX {
        return Err(handles::last_error());
    }
    if previous != 1 {
        return Err(io::Error::other("unexpected owned thread suspend count"));
    }
    Ok(())
}

/// Reconciles a failed post-create check on the exact returned root handle only.
///
/// # Safety
/// This handle comes from the successful owned CreateProcessW call and is never PID-opened.
#[expect(
    unsafe_code,
    reason = "TerminateProcess is limited to failed owned-root startup reconciliation."
)]
pub(super) fn terminate_failed_root(root: &Handle) -> io::Result<()> {
    if unsafe { api::TerminateProcess(root.raw(), 1) } == 0 {
        Err(handles::last_error())
    } else {
        Ok(())
    }
}
