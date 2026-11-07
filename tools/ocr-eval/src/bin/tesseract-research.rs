// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source-bound Linux-only developer research controller.

#![forbid(unsafe_code)]

mod research;

use std::{
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let result = research::run(&std::env::args().skip(1).collect::<Vec<_>>());

    match result {
        Ok(value) => match writeln!(io::stdout(), "{value}") {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
        Err(error) => {
            let result = writeln!(io::stderr(), "tesseract-research: {error}");
            match result {
                Ok(()) | Err(_) => ExitCode::FAILURE,
            }
        }
    }
}
