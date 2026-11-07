// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sole-reader bounded synchronous child pipes.

use super::handles::{self, Handle};
use std::io;
use std::mem;
use std::ptr;
use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};

pub(super) struct Capture {
    pub(super) reader: Handle,
    pub(super) bytes: Vec<u8>,
    pub(super) eof: Option<()>,
    pub(super) exceeded: Option<()>,
    cap: usize,
}
impl Capture {
    pub(super) fn new(reader: Handle, cap: usize) -> Self {
        Self {
            reader,
            bytes: Vec::new(),
            eof: None,
            exceeded: None,
            cap,
        }
    }
    /// Polls and reads at most one available bounded chunk on the sole-owned reader.
    ///
    /// # Safety
    /// No other reader accesses this synchronous pipe; Peek supplies availability and the
    /// initialised buffer and reported count are checked before retaining bytes.
    #[expect(
        unsafe_code,
        reason = "PeekNamedPipe and ReadFile access a sole-owned bounded pipe."
    )]
    pub(super) fn poll(&mut self) -> io::Result<()> {
        if self.eof.is_some() {
            return Ok(());
        }
        let mut available = 0;
        if unsafe {
            PeekNamedPipe(
                self.reader.raw(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                &mut available,
                ptr::null_mut(),
            )
        } == 0
        {
            let error = handles::last_error();
            if error.raw_os_error() == Some(ERROR_BROKEN_PIPE.cast_signed()) {
                self.eof = Some(());
                return Ok(());
            }
            return Err(error);
        }
        if available == 0 {
            return Ok(());
        }
        const CHUNK_BYTES: usize = 4096;
        let mut buffer = [0_u8; CHUNK_BYTES];
        let requested = available.min(u32::try_from(buffer.len()).map_err(io::Error::other)?);
        let mut read = 0;
        if unsafe {
            ReadFile(
                self.reader.raw(),
                buffer.as_mut_ptr(),
                requested,
                &mut read,
                ptr::null_mut(),
            )
        } == 0
        {
            let error = handles::last_error();
            if error.raw_os_error() == Some(ERROR_BROKEN_PIPE.cast_signed()) {
                self.eof = Some(());
                return Ok(());
            }
            return Err(error);
        }
        let read = usize::try_from(read).map_err(io::Error::other)?;
        let chunk = buffer
            .get(..read)
            .ok_or_else(|| io::Error::other("pipe returned invalid byte count"))?;
        let remaining = self.cap.saturating_sub(self.bytes.len());
        let retained = chunk
            .get(..remaining.min(read))
            .ok_or_else(|| io::Error::other("invalid capture bound"))?;
        self.bytes.extend_from_slice(retained);
        if read > remaining {
            self.exceeded = Some(());
        }
        Ok(())
    }
}

/// Creates one anonymous pipe with inherited child ends selected by the caller.
///
/// # Safety
/// Writable outputs and SECURITY_ATTRIBUTES have their exact ABI sizes; returned handles
/// are owned exactly once and failed calls transfer no handles.
#[expect(
    unsafe_code,
    reason = "CreatePipe returns two exclusively owned anonymous pipe ends."
)]
pub(super) fn pair() -> io::Result<(Handle, Handle)> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(mem::size_of::<SECURITY_ATTRIBUTES>()).map_err(io::Error::other)?,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };
    let mut reader = ptr::null_mut();
    let mut writer = ptr::null_mut();
    if unsafe { CreatePipe(&mut reader, &mut writer, &attributes, 0) } == 0 {
        return Err(handles::last_error());
    }
    Ok((Handle::new(reader), Handle::new(writer)))
}
