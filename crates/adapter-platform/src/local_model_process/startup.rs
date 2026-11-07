// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stationary create-time job and explicit inheritance attributes.

use super::handles;
use std::io;
use std::mem;
use std::ptr;
use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, HANDLE};
use windows_sys::Win32::System::Threading::{
    DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, UpdateProcThreadAttribute,
};

pub(super) struct Attributes {
    storage: Vec<u128>,
    values: Box<([HANDLE; 1], [HANDLE; 3])>,
}
impl Attributes {
    pub(super) fn raw(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_ptr().cast_mut().cast()
    }
    /// Initialises bounded aligned storage and two stationary handle-array attributes.
    ///
    /// # Safety
    /// The zero-size probe is expected to fail with ERROR_INSUFFICIENT_BUFFER; storage is
    /// pointer-aligned and initialised. Boxed values outlive the successfully initialised list.
    #[expect(
        unsafe_code,
        reason = "Initialise and update the exact two approved startup attributes."
    )]
    pub(super) fn new(job: HANDLE, inherited: [HANDLE; 3]) -> io::Result<Self> {
        let mut bytes = 0;
        let result =
            unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes) };
        let error = handles::last_error();
        const MAX_ATTRIBUTE_BYTES: usize = 64 * 1024;
        if result != 0
            || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER.cast_signed())
            || bytes == 0
            || bytes > MAX_ATTRIBUTE_BYTES
        {
            return Err(io::Error::other("invalid startup attribute sizing result"));
        }
        let mut storage = vec![0_u128; bytes.div_ceil(mem::size_of::<u128>())];
        if unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 2, 0, &mut bytes)
        } == 0
        {
            return Err(handles::last_error());
        }
        let attributes = Self {
            storage,
            values: Box::new(([job], inherited)),
        };
        for (kind, value, size) in [
            (
                PROC_THREAD_ATTRIBUTE_JOB_LIST,
                attributes.values.0.as_ptr().cast(),
                mem::size_of::<[HANDLE; 1]>(),
            ),
            (
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                attributes.values.1.as_ptr().cast(),
                mem::size_of::<[HANDLE; 3]>(),
            ),
        ] {
            if unsafe {
                UpdateProcThreadAttribute(
                    attributes.raw(),
                    0,
                    kind as usize,
                    value,
                    size,
                    ptr::null_mut(),
                    ptr::null(),
                )
            } == 0
            {
                return Err(handles::last_error());
            }
        }
        Ok(attributes)
    }
}
impl Drop for Attributes {
    /// Destroys a successfully initialised list before its arrays and allocation are freed.
    ///
    /// # Safety
    /// Construction returns this type only after initialisation and it is destroyed once.
    #[expect(
        unsafe_code,
        reason = "DeleteProcThreadAttributeList precedes owned storage destruction."
    )]
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.raw()) };
    }
}
