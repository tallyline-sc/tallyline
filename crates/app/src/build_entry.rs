// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless identity dispatch before the native bootstrap port.

use crate::api::model::{BuildInformation, BuildInformationError, BuildKind};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

const MAX_ARGUMENTS: usize = 64;
const MAX_REPORT_BYTES: usize = 1024;

/// Handles the bounded build-info query before invoking any native application bootstrap.
///
/// The caller supplies immutable compiled identity and the sole desktop-start port.
/// Complete identity is required even for normal startup. Query output contains only
/// validated public build metadata; no environment, filesystem or provider is read.
///
/// # Errors
/// Returns invalid metadata/query arguments, output errors or the desktop port's error.
pub fn enter_application(
    arguments: impl IntoIterator<Item = OsString>,
    information: Result<BuildInformation, BuildInformationError>,
    output: &mut impl Write,
    desktop: impl FnOnce(BuildInformation) -> Result<ExitCode, Box<dyn std::error::Error>>,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let query = information_requested(arguments)?;
    let information = information?;
    if query {
        let kind = match information.kind() {
            BuildKind::Development => "development",
            BuildKind::Release => "release",
        };
        let tag = json_option(information.release_tag());
        let commit = json_option(information.source_commit());
        let report = format!(
            "{{\"schema_version\":1,\"product\":\"tallyline\",\"kind\":\"{kind}\",\"version\":\"{}\",\"release_tag\":{tag},\"source_commit\":{commit},\"target\":\"{}\"}}\n",
            information.version(),
            information.target().triple()
        );
        if report.len() > MAX_REPORT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "build information exceeds its output bound",
            )
            .into());
        }
        output.write_all(report.as_bytes())?;
        output.flush()?;
        return Ok(ExitCode::SUCCESS);
    }
    desktop(information)
}

fn information_requested(arguments: impl IntoIterator<Item = OsString>) -> io::Result<bool> {
    let mut count = 0;
    let mut query = false;
    for argument in arguments {
        if count == MAX_ARGUMENTS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "too many application arguments",
            ));
        }
        count += 1;
        query |= argument == "--build-info";
    }
    if query && count != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: tallyline --build-info",
        ));
    }
    Ok(query)
}

fn json_option(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{value}\""),
        None => "null".to_owned(),
    }
}
