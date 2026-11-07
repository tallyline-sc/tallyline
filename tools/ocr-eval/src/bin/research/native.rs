// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Root-operated native synthetic gates and frozen independent attempts.

use super::{
    docker::{Docker, Kind, Spec},
    files, formats, plan,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn identity(owned: &Path, plan: &Value) -> Result<Value, String> {
    let evidence = plan::json(&owned.join("build-evidence.json"))?;
    let bytes = files::read(&owned.join("build-evidence.json"), 16 * 1024 * 1024)?;
    let expected = files::read(&owned.join("build-evidence.sha256"), 64)?;
    if files::hash(&bytes).as_bytes() != expected
        || evidence.get("controller_sha256") != plan.get("controller_sha256")
    {
        return Err("build evidence identity changed".to_owned());
    }
    plan::digest(
        &owned.join("worker/tesseract"),
        plan::text(&evidence, "worker_sha256")?,
        "worker",
    )?;
    verify_artifacts(owned, &evidence)?;
    let entries = fs::read_dir(owned.join("worker"))
        .map_err(|_| "worker directory unavailable")?
        .count();
    if entries != 1 {
        return Err("runtime worker directory must contain only the official CLI".to_owned());
    }
    for item in plan::array(&evidence, "preprocessed")?.iter().enumerate() {
        plan::digest(
            &owned
                .join("work")
                .join(format!("preprocessed-{}.ii", item.0)),
            plan::text(item.1, "sha256")?,
            "preprocessed evidence",
        )?;
    }
    Ok(evidence)
}

pub(super) fn review(campaign: &Path, evidence_sha256: &str) -> Result<(), String> {
    let (owned, plan) = plan::load(campaign)?;
    let evidence = identity(&owned, &plan)?;
    plan::digest(
        &owned.join("build-evidence.json"),
        evidence_sha256,
        "root-reviewed generated evidence",
    )?;
    files::write_json(
        &owned.join("root-build-review.json"),
        &json!({"build_evidence_sha256":evidence_sha256,"worker_sha256":evidence.get("worker_sha256"),"controller_sha256":plan.get("controller_sha256"),"operator_review":"actual generated/preprocessed native reachability and link scope reviewed against exact owner approval; no product adoption"}),
    )
}

fn reviewed(owned: &Path, plan: &Value) -> Result<Value, String> {
    let evidence = identity(owned, plan)?;
    let review = plan::json(&owned.join("root-build-review.json"))?;
    plan::digest(
        &owned.join("build-evidence.json"),
        plan::text(&review, "build_evidence_sha256")?,
        "reviewed build evidence",
    )?;
    if review.get("worker_sha256") != evidence.get("worker_sha256")
        || review.get("controller_sha256") != plan.get("controller_sha256")
    {
        return Err("root build review is stale".to_owned());
    }
    Ok(evidence)
}

fn argv() -> Vec<String> {
    let mut args = [
        "/usr/bin/nice",
        "-n",
        "5",
        "/worker/tesseract",
        "stdin",
        "stdout",
        "--tessdata-dir",
        "/model",
        "-l",
        "eng",
        "--psm",
        "11",
        "--loglevel",
        "OFF",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for (key, value) in [
        ("tessedit_create_tsv", "1"),
        ("tessedit_create_txt", "0"),
        ("tessedit_create_hocr", "0"),
        ("tessedit_create_alto", "0"),
        ("tessedit_create_page_xml", "0"),
        ("tessedit_create_pdf", "0"),
        ("tessedit_create_lstmbox", "0"),
        ("tessedit_create_boxfile", "0"),
        ("tessedit_create_wordstrbox", "0"),
        ("tessedit_write_unlv", "0"),
        ("tessedit_write_images", "0"),
        ("tessedit_dump_pageseg_images", "0"),
        ("stream_filelist", "0"),
        ("tessedit_write_params_to_file", ""),
        ("tessedit_load_sublangs", ""),
    ] {
        args.extend(["-c".to_owned(), format!("{key}={value}")]);
    }
    args
}

fn attempt(
    docker: &Docker,
    owned: &Path,
    model: &str,
    memory: u64,
    bitmap: Vec<u8>,
    label: &str,
) -> Result<Value, String> {
    if !matches!(model, "best" | "fast") || !matches!(memory, 4_294_967_296 | 8_589_934_592) {
        return Err("unknown frozen runtime condition".to_owned());
    }
    let (width, height) = formats::validate_bmp(&bitmap)?;
    let started = Instant::now();
    let spec = Spec {
        workdir: "/",
        kind: Kind::Runtime,
        memory,
        mounts: vec![
            (owned.join("worker"), "/worker", false),
            (owned.join("models").join(model), "/model", false),
        ],
        argv: argv(),
        started,
    };
    files::write(
        &owned.join("evidence").join(format!("{label}.started")),
        b"single attempt; no automatic retry",
    )?;
    let attempt = docker.run(
        &spec,
        bitmap,
        started + Duration::from_secs(120),
        formats::MAX_TSV,
    )?;
    files::write(
        &owned.join("evidence").join(format!("{label}.tsv")),
        attempt
            .output
            .bytes
            .get(..attempt.output.bytes.len().min(formats::MAX_TSV))
            .ok_or("bounded output unavailable")?,
    )?;
    let mut receipt = attempt.receipt;
    receipt.as_object_mut().ok_or("container receipt is not an object")?.insert("worker_condition".to_owned(), json!({"model":model,"memory_bytes":memory,"psm":11,"input_width":width,"input_height":height,"automatic_retry":false}));
    let parsed = if attempt.output.failure.is_none()
        && attempt.output.code == Some(0)
        && receipt.pointer("/final_state/OOMKilled") != Some(&json!(true))
    {
        formats::parse_tsv(&attempt.output.bytes, width, height)
    } else {
        Err("native attempt refused, exceeded a bound or exited unsuccessfully".to_owned())
    };
    match parsed {
        Ok(parsed) => {
            files::write_json(
                &owned.join("evidence").join(format!("{label}.words.json")),
                &parsed,
            )?;
            receipt
                .as_object_mut()
                .ok_or("container receipt is not an object")?
                .insert("tsv_valid".to_owned(), json!(true));
        }
        Err(error) => {
            receipt
                .as_object_mut()
                .ok_or("container receipt is not an object")?
                .insert("tsv_valid".to_owned(), json!(false));
            receipt
                .as_object_mut()
                .ok_or("container receipt is not an object")?
                .insert("refusal".to_owned(), json!(error));
        }
    }
    files::write_json(
        &owned.join("evidence").join(format!("{label}.json")),
        &receipt,
    )?;
    Ok(receipt)
}

fn blank_bmp() -> Vec<u8> {
    let payload = 128u32 * 64 * 3;
    let mut bytes = Vec::new();
    bytes.extend(b"BM");
    bytes.extend((54 + payload).to_le_bytes());
    bytes.extend([0; 4]);
    for value in [54u32, 40, 128, 64, 0x0018_0001, 0, payload, 0, 0, 0, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.resize(54 + payload as usize, 255);
    bytes
}

pub(super) fn synthetic(campaign: &Path, public_probe: &Path) -> Result<(), String> {
    let (owned, plan) = plan::load(campaign)?;
    let evidence = reviewed(&owned, &plan)?;
    super::probe::verify(public_probe, plan::text(&plan, "controller_sha256")?)?;
    let startup = files::read(&public_probe.join("runtime-startup-diff.txt"), 1024)?;
    if super::docker::normalise_diff(&startup)?
        != vec!["A /model".to_owned(), "A /worker".to_owned()]
    {
        return Err("public startup baseline differs".to_owned());
    }
    files::write(&owned.join("runtime-startup-diff.txt"), &startup)?;
    files::write(
        &owned.join("synthetic.started"),
        b"root-operated public native fixture; no private input",
    )?;
    let docker = Docker::new(&owned)?;
    let mut conditions = Vec::new();
    for memory in [4_294_967_296u64, 8_589_934_592] {
        for model in ["best", "fast"] {
            let label = format!("synthetic-{model}-{memory}");
            let receipt = attempt(&docker, &owned, model, memory, blank_bmp(), &label)?;
            if receipt.get("tsv_valid") != Some(&json!(true)) {
                return Err(format!("native public synthetic fixture failed: {label}"));
            }
            conditions.push(json!({"condition":label,"receipt_sha256":files::hash(&files::read(&owned.join("evidence").join(format!("{label}.json")),16*1024*1024)?)}));
        }
    }
    files::write_json(
        &owned.join("synthetic.passed.json"),
        &json!({"controller_sha256":plan.get("controller_sha256"),"worker_sha256":evidence.get("worker_sha256"),"build_evidence_sha256":files::hash(&files::read(&owned.join("build-evidence.json"),16*1024*1024)?),"public_probe_receipt_sha256":files::hash(&files::read(&public_probe.join("probe.passed.json"),16*1024*1024)?),"conditions":conditions,"private_inputs":false}),
    )
}

pub(super) fn run(campaign: &Path) -> Result<(), String> {
    let (owned, plan) = plan::load(campaign)?;
    let evidence = reviewed(&owned, &plan)?;
    let synthetic = plan::json(&owned.join("synthetic.passed.json"))?;
    if synthetic.get("controller_sha256") != plan.get("controller_sha256")
        || synthetic.get("worker_sha256") != evidence.get("worker_sha256")
    {
        return Err("matching public native synthetic gates have not passed".to_owned());
    }
    for condition in plan::array(&synthetic, "conditions")? {
        plan::digest(
            &owned
                .join("evidence")
                .join(format!("{}.json", plan::text(condition, "condition")?)),
            plan::text(condition, "receipt_sha256")?,
            "native synthetic receipt",
        )?;
    }
    files::write(
        &owned.join("private-campaign.started"),
        b"twenty independently frozen attempts; no automatic retry",
    )?;
    let docker = Docker::new(&owned)?;
    let mut results = Vec::new();
    for profile in plan::array(&plan, "profiles")? {
        let memory = profile
            .get("memoryBytes")
            .and_then(Value::as_u64)
            .ok_or("profile memory missing")?;
        for model in ["best", "fast"] {
            for input in plan::array(&plan, "inputs")? {
                let id = plan::text(input, "id")?;
                let bitmap = plan::digest(
                    &owned.join("inputs").join(format!("{id}.bmp")),
                    plan::text(input, "bmp_sha256")?,
                    "private bitmap",
                )?;
                let label = format!("private-{id}-{model}-{memory}");
                let result = attempt(&docker, &owned, model, memory, bitmap, &label)?;
                let failed = result.get("tsv_valid") != Some(&json!(true));
                results.push(json!({"condition":label,"tsv_valid":!failed,"receipt_sha256":files::hash(&files::read(&owned.join("evidence").join(format!("{label}.json")),16*1024*1024)?)}));
                if failed {
                    files::write_json(
                        &owned.join("campaign.incomplete.json"),
                        &json!({"attempts":results,"complete":false,"automatic_retry":false}),
                    )?;
                    return Err("frozen private campaign stopped on a failed condition; separate profiles are not retries".to_owned());
                }
            }
        }
    }
    files::write_json(
        &owned.join("campaign.completed.json"),
        &json!({"attempts":results,"complete":true,"accuracy_claimed":false,"golden_verification_changed":false,"physical_weak_pc":false,"automatic_retry":false}),
    )
}

fn verify_artifacts(owned: &Path, evidence: &Value) -> Result<(), String> {
    for artifact in plan::array(evidence, "generated_artifacts")? {
        let name = plan::text(artifact, "path")?;
        if !name.starts_with("work/") && !name.starts_with("evidence/")
            || Path::new(name)
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || name.contains('\\')
        {
            return Err("generated evidence path is invalid".to_owned());
        }
        plan::digest(
            &owned.join(name),
            plan::text(artifact, "sha256")?,
            "generated build evidence",
        )?;
    }
    Ok(())
}
pub(super) fn verify_recorded(owned: &Path) -> Result<(), String> {
    verify_artifacts(owned, &plan::json(&owned.join("build-evidence.json"))?)
}
