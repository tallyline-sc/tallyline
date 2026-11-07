// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact research worker ELF target and import constraints.

pub(super) fn validate(bytes: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "ELF evidence is not UTF-8")?;
    if !text.contains("ELF64")
        || !text.contains("Advanced Micro Devices X86-64")
        || !text.contains("/lib64/ld-linux-x86-64.so.2")
    {
        return Err("worker ELF target or interpreter differs".to_owned());
    }
    for line in text.lines().filter(|line| line.contains("(NEEDED)")) {
        let library = line
            .split_once('[')
            .and_then(|(_, tail)| tail.split_once(']').map(|(name, _)| name))
            .ok_or("malformed ELF dependency")?;
        if !matches!(
            library,
            "libstdc++.so.6"
                | "libgcc_s.so.1"
                | "libm.so.6"
                | "libc.so.6"
                | "libpthread.so.0"
                | "ld-linux-x86-64.so.2"
        ) {
            return Err("worker ELF imports an unapproved library".to_owned());
        }
    }
    Ok(())
}
