// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed research command dispatch.

mod build;
mod contracts;
mod docker;
mod elf;
mod files;
mod formats;
mod native;
mod plan;
mod probe;

use std::path::Path;

pub(super) fn run(arguments: &[String]) -> Result<String, String> {
    match arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["resource", recorded, profile, state]=>serde_json::to_string(&docker::recorded_resource_state(Path::new(recorded),profile,Path::new(state))?).map_err(|_|"resource output encoding failed".to_owned()),
        ["resource", recorded, profile]=>serde_json::to_string(&docker::recorded_resource(Path::new(recorded),profile)?).map_err(|_|"resource output encoding failed".to_owned()),
        ["verify-model", id, input]=>{plan::model_bytes(Path::new(input),id)?;Ok("Frozen model identity validated; no process started.".to_owned())},
        ["verify-evidence", owned]=>{native::verify_recorded(Path::new(owned))?;Ok("Recorded generated evidence validated; no process started.".to_owned())},
        ["accept-build-review", campaign, digest] => { native::review(Path::new(campaign),digest)?; Ok("Root review bound to exact generated evidence; native synthetic gates remain required.".to_owned()) },
        ["synthetic", campaign, probe] => { native::synthetic(Path::new(campaign),Path::new(probe))?; Ok("Public native synthetic gates passed.".to_owned()) },
        ["run", campaign] => { native::run(Path::new(campaign))?; Ok("Frozen research campaign completed; no calibrated or golden metric claimed.".to_owned()) },
        ["contract", mode] => { contracts::run(mode)?; Ok("Recorded process contract passed.".to_owned()) },
        ["fixture", mode] => contracts::fixture(mode),
        ["probe", fresh] => { probe::run(Path::new(fresh))?; Ok("Public safe Rust isolation contracts passed.".to_owned()) },
        ["build", campaign, probe] => { build::build(Path::new(campaign),Path::new(probe))?; Ok("Native build evidence captured; root reachability review and synthetic gates required.".to_owned()) },
        ["prepare", campaign, proposal] => { plan::prepare(Path::new(campaign), Path::new(proposal))?; Ok("Research inputs frozen; no native work executed.".to_owned()) },
        ["tsv", bitmap, recorded, output] => {
            let bmp = files::read(Path::new(bitmap), 64 * 1024 * 1024)?;
            let (width, height) = formats::validate_bmp(&bmp)?;
            let tsv = files::read(Path::new(recorded), formats::MAX_TSV)?;
            let parsed = formats::parse_tsv(&tsv, width, height)?;
            files::write_json(Path::new(output), &parsed)?;
            Ok("Recorded TSV validated; private output created.".to_owned())
        }
        ["bmp", source, output] => {
            let encoded = formats::encode_png(&files::read(Path::new(source), 200 * 1024 * 1024)?)?;
            files::write(Path::new(output), &encoded)?;
            Ok("Bounded BMP created.".to_owned())
        }
        _ => Err("usage: tesseract-research probe <fresh-public-dir> | prepare <campaign> <approved-proposal> | build <campaign> <public-probe> | accept-build-review <campaign> <sha256> | synthetic <campaign> <public-probe> | run <campaign> | bmp <png> <fresh-bmp> | tsv <bmp> <recorded-tsv> <private-json> | verify-evidence <owned-dir> | verify-model <best|fast> <data> | contract <mode>".to_owned()),
    }
}
