// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offline corpus inspection and separate measurement proposals.

#![forbid(unsafe_code)]

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(value) => {
            if writeln!(io::stdout(), "{value}").is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => match writeln!(io::stderr(), "ocr-eval: {error}") {
            Ok(()) => ExitCode::FAILURE,
            Err(_) => ExitCode::FAILURE,
        },
    }
}

fn run(arguments: &[String]) -> Result<String, String> {
    let private = std::env::var_os("TALLYLINE_CORPUS_DIR").map(PathBuf::from);
    let default_public = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/public");
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["witness", "--channel", channel] => serde_json::to_string_pretty(&tallyline_adapter_files::api::GameLogFiles::new(PathBuf::from(channel)).evaluate()).map_err(|_| "witness output could not be encoded".to_owned()),
        ["reader-replay", "--public", public, "--packet", packet, "--sha256", hash] => serde_json::to_string_pretty(&ocr_eval::reader_replay::analyse(Path::new(public), Path::new(packet), hash).map_err(|error| error.to_string())?).map_err(|_| "replay output could not be encoded".to_owned()),
        ["scu-policy", "--public", public] => serde_json::to_string_pretty(&ocr_eval::scu_policy::analyse(Path::new(public))?).map_err(|_| "policy output could not be encoded".to_owned()),
        ["inspect"] => inspect(&default_public, private.as_deref()),
        ["inspect", "--public", public] => inspect(Path::new(public), private.as_deref()),
        ["prepare-panels", "--image", image, "--geometry", geometry, "--output", output] => serde_json::to_string_pretty(&ocr_eval::panel_dump::prepare(Path::new(image),Path::new(geometry),Path::new(output))?).map_err(|_|"panel receipt encoding failed".to_owned()),
        ["annotate", expected, anchors, proposal] => {
            ocr_eval::corpus::annotate(Path::new(expected), Path::new(anchors), Path::new(proposal))?;
            Ok("Unverified proposal written; expected.json is unchanged.".to_owned())
        }
        _ => Err("usage: ocr-eval witness --channel <directory> | inspect [--public <directory>] | scu-policy --public <directory> | reader-replay --public <directory> --packet <file> --sha256 <digest> | prepare-panels --image <file> --geometry <manifest.json> --output <new-directory> | annotate <expected.json> <anchors.json> <proposal.json>".to_owned()),
    }
}

fn inspect(public: &Path, private: Option<&Path>) -> Result<String, String> {
    serde_json::to_string_pretty(&ocr_eval::corpus::inspect(public, private)?)
        .map_err(|error| error.to_string())
}
