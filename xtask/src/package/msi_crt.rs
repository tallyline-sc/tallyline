// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core-only x64 CRT registry prerequisite rows, without numeric string coercion.

use std::io;

const CRT_MAJOR: u32 = 14;
const CRT_MINOR: u32 = 44;
const MSI_CONDITION_LIMIT: usize = 255;
const MEMBERSHIP_CHUNK: usize = 8;

pub(super) fn authoring() -> io::Result<String> {
    let mut xml = String::new();
    for (view, bitness) in [("3", "always32"), ("6", "always64")] {
        for (letter, name) in [("I", "Installed"), ("M", "Major"), ("N", "Minor")] {
            xml.push_str(&format!("<Property Id=\"V{letter}{view}\" Secure=\"yes\"><RegistrySearch Id=\"VC{letter}{view}\" Root=\"HKLM\" Key=\"SOFTWARE\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x64\" Name=\"{name}\" Type=\"raw\" Bitness=\"{bitness}\"/></Property>"));
        }
    }
    for left in predicates("3") {
        for right in predicates("6") {
            let condition = format!("Installed OR ({left}) OR ({right})");
            if condition.len() > MSI_CONDITION_LIMIT {
                return Err(io::Error::other(
                    "CRT condition exceeds the MSI table limit",
                ));
            }
            xml.push_str(&format!("<Launch Condition=\"{}\" Message=\"Tallyline OCR requires the separately installed Microsoft Visual C++ Redistributable x64 version 14.44 or newer. Download information: https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/\"/>", escape(&condition)));
        }
    }
    Ok(xml)
}

fn predicates(view: &str) -> Vec<String> {
    let installed = format!("VI{view}");
    let major = format!("VM{view}");
    let minor = format!("VN{view}");
    let mut required = vec![format!("{installed} = \"#1\" OR {installed} = \"#+1\"")];
    required.extend(dword(&major));
    required.extend(below(&major, CRT_MAJOR));
    let skip_minor = format!("{major} <> \"#14\" AND {major} <> \"#+14\"");
    for predicate in dword(&minor).into_iter().chain(below(&minor, CRT_MINOR)) {
        required.push(format!("({skip_minor}) OR ({predicate})"));
    }
    required
}

fn dword(property: &str) -> Vec<String> {
    let mut required = vec![format!("{property} << \"#\"")];
    required
        .extend(["##", "#x", "#%", "#-"].map(|prefix| format!("NOT ({property} << \"{prefix}\")")));
    required
}

fn below(property: &str, floor: u32) -> Vec<String> {
    (0..floor)
        .collect::<Vec<_>>()
        .chunks(MEMBERSHIP_CHUNK)
        .map(|chunk| {
            let values = chunk
                .iter()
                .map(|value| format!("#{value}#+{value}"))
                .collect::<String>();
            format!("NOT (\"{values}\" >< {property})")
        })
        .collect()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
}
