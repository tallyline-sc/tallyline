// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Configured unnamed jobs and checked complete membership snapshots.

use super::handles::{self, Handle};
use super::model::OwnedProcessLimits;
use std::io;
use std::mem;
use std::ptr;
use windows_sys::Win32::System::JobObjects as api;
use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;

trait Information: Default {
    const CLASS: api::JOBOBJECTINFOCLASS;
}
impl Information for api::JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    const CLASS: i32 = api::JobObjectExtendedLimitInformation;
}
impl Information for api::JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
    const CLASS: i32 = api::JobObjectCpuRateControlInformation;
}
impl Information for api::JOBOBJECT_BASIC_ACCOUNTING_INFORMATION {
    const CLASS: i32 = api::JobObjectBasicAccountingInformation;
}

/// Reads exactly one private ABI-matched initialised information type.
///
/// # Safety
/// The private Information implementations bind each class to its exact Windows layout;
/// capacity is size_of of that type and successful returned lengths must match it.
#[expect(
    unsafe_code,
    reason = "QueryInformationJobObject writes an exact registered information type."
)]
fn query<T: Information>(job: &Handle) -> io::Result<T> {
    let mut value = T::default();
    let capacity = u32::try_from(mem::size_of::<T>()).map_err(io::Error::other)?;
    let mut returned = 0;
    if unsafe {
        api::QueryInformationJobObject(
            job.raw(),
            T::CLASS,
            ptr::from_mut(&mut value).cast(),
            capacity,
            &mut returned,
        )
    } == 0
    {
        return Err(handles::last_error());
    }
    if returned != capacity {
        return Err(io::Error::other("incomplete job information"));
    }
    Ok(value)
}

/// Sets one private ABI-matched information type without transferring any ownership.
///
/// # Safety
/// The Information implementations bind exact class, initialised layout and byte length.
#[expect(
    unsafe_code,
    reason = "SetInformationJobObject receives exact private ABI-matched values."
)]
fn set<T: Information>(job: &Handle, value: &T) -> io::Result<()> {
    let capacity = u32::try_from(mem::size_of::<T>()).map_err(io::Error::other)?;
    if unsafe {
        api::SetInformationJobObject(job.raw(), T::CLASS, ptr::from_ref(value).cast(), capacity)
    } == 0
    {
        Err(handles::last_error())
    } else {
        Ok(())
    }
}

const LIMIT_FLAGS: u32 = api::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    | api::JOB_OBJECT_LIMIT_JOB_MEMORY
    | api::JOB_OBJECT_LIMIT_ACTIVE_PROCESS
    | api::JOB_OBJECT_LIMIT_PRIORITY_CLASS;
const CPU_FLAGS: u32 =
    api::JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | api::JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;

/// Creates a new unnamed non-inheritable job and configures its limits before child creation.
///
/// # Safety
/// Null security/name parameters request an unnamed job with default non-inheritable security;
/// a successful handle is immediately owned and failures transfer no resources.
#[expect(
    unsafe_code,
    reason = "CreateJobObjectW creates only a new unnamed owned job."
)]
pub(super) fn create(limits: OwnedProcessLimits) -> io::Result<Handle> {
    let raw = unsafe { api::CreateJobObjectW(ptr::null(), ptr::null()) };
    if raw.is_null() {
        return Err(handles::last_error());
    }
    let job = Handle::new(raw);
    job.clear_inheritance()?;
    let extended = api::JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        BasicLimitInformation: api::JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: LIMIT_FLAGS,
            ActiveProcessLimit: limits.processes,
            PriorityClass: BELOW_NORMAL_PRIORITY_CLASS,
            ..Default::default()
        },
        JobMemoryLimit: limits.commit,
        ..Default::default()
    };
    set(&job, &extended)?;
    let cpu = api::JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
        ControlFlags: CPU_FLAGS,
        Anonymous: api::JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 {
            CpuRate: u32::from(limits.cpu) * 100,
        },
    };
    set(&job, &cpu)?;
    verify(&job, limits)?;
    Ok(job)
}

/// Verifies current job flags, process/commit/priority limits and the selected CPU union member.
///
/// # Safety
/// CpuRate is read only after its exact ENABLE|HARD_CAP discriminator has been verified.
#[expect(
    unsafe_code,
    reason = "Read the CPU union member selected by verified hard-cap flags."
)]
pub(super) fn verify(job: &Handle, limits: OwnedProcessLimits) -> io::Result<()> {
    let extended: api::JOBOBJECT_EXTENDED_LIMIT_INFORMATION = query(job)?;
    let basic = extended.BasicLimitInformation;
    let cpu: api::JOBOBJECT_CPU_RATE_CONTROL_INFORMATION = query(job)?;
    if basic.LimitFlags != LIMIT_FLAGS
        || basic.ActiveProcessLimit != limits.processes
        || basic.PriorityClass != BELOW_NORMAL_PRIORITY_CLASS
        || extended.JobMemoryLimit != limits.commit
        || cpu.ControlFlags != CPU_FLAGS
    {
        return Err(io::Error::other("owned job limits differ"));
    }
    if unsafe { cpu.Anonymous.CpuRate } != u32::from(limits.cpu) * 100 {
        return Err(io::Error::other("owned job CPU cap differs"));
    }
    Ok(())
}

/// Observes membership using a retained root and its exact owned job.
///
/// # Safety
/// Both handles are live and the BOOL output is initialised and valid for the call.
#[expect(
    unsafe_code,
    reason = "IsProcessInJob inspects only the exact retained owned process."
)]
pub(super) fn includes(job: &Handle, root: &Handle) -> io::Result<bool> {
    let mut included = 0;
    if unsafe { api::IsProcessInJob(root.raw(), job.raw(), &mut included) } == 0 {
        return Err(handles::last_error());
    }
    Ok(included != 0)
}

/// Queries bounded complete member identifiers with checked header/count arithmetic.
///
/// # Safety
/// Pointer-aligned initialised usize storage holds the two-u32 header plus 256 rows;
/// returned lengths, counts and every range are checked before safe integer decoding.
#[expect(
    unsafe_code,
    reason = "Query a capped variable job member buffer without unchecked row access."
)]
pub(super) fn members(job: &Handle) -> io::Result<Vec<usize>> {
    const MEMBER_CAP: usize = 256;
    const HEADER_BYTES: usize = 2 * mem::size_of::<u32>();
    let mut buffer = vec![0_usize; 1 + MEMBER_CAP];
    let capacity = u32::try_from(mem::size_of_val(buffer.as_slice())).map_err(io::Error::other)?;
    let mut returned = 0;
    if unsafe {
        api::QueryInformationJobObject(
            job.raw(),
            api::JobObjectBasicProcessIdList,
            buffer.as_mut_ptr().cast(),
            capacity,
            &mut returned,
        )
    } == 0
    {
        return Err(handles::last_error());
    }
    let header = buffer
        .first()
        .ok_or_else(|| io::Error::other("missing job header"))?
        .to_ne_bytes();
    let assigned = u32::from_ne_bytes(
        header
            .get(..4)
            .ok_or_else(|| io::Error::other("invalid job header"))?
            .try_into()
            .map_err(io::Error::other)?,
    );
    let count = u32::from_ne_bytes(
        header
            .get(4..8)
            .ok_or_else(|| io::Error::other("invalid job header"))?
            .try_into()
            .map_err(io::Error::other)?,
    );
    let count = usize::try_from(count).map_err(io::Error::other)?;
    let required = count
        .checked_mul(mem::size_of::<usize>())
        .and_then(|size| HEADER_BYTES.checked_add(size))
        .ok_or_else(|| io::Error::other("job member size overflow"))?;
    if assigned as usize != count
        || count > MEMBER_CAP
        || required > returned as usize
        || returned > capacity
    {
        return Err(io::Error::other(
            "inconsistent or incomplete job membership snapshot",
        ));
    }
    let rows = buffer
        .get(1..1 + count)
        .ok_or_else(|| io::Error::other("job members exceed buffer"))?;
    Ok(rows.to_vec())
}

pub(super) fn counts(job: &Handle) -> io::Result<(u32, u32)> {
    let before: api::JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = query(job)?;
    let members = members(job)?;
    let after: api::JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = query(job)?;
    let count = u32::try_from(members.len()).map_err(io::Error::other)?;
    if before.ActiveProcesses != after.ActiveProcesses || after.ActiveProcesses != count {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "job accounting changed during membership query",
        ));
    }
    Ok((after.ActiveProcesses, count))
}

/// Terminates only the exact owned job; no PID lookup or foreign process is used.
///
/// # Safety
/// The handle remains live throughout this operation and is never transferred.
#[expect(
    unsafe_code,
    reason = "TerminateJobObject affects only the exact owned job tree."
)]
pub(super) fn terminate(job: &Handle) -> io::Result<()> {
    if unsafe { api::TerminateJobObject(job.raw(), 1) } == 0 {
        Err(handles::last_error())
    } else {
        Ok(())
    }
}
