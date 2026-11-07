// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Approval-bound input preparation and immutable campaign identity.

use super::{files, formats};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) const SCOPE: &str = "1c6180099f49e239624c4b85fb427ef10b73093d7635cadc9abe5d7a9857a2ee";
pub(super) const MANIFEST: &str =
    "84297112e18d9785e24a5032ccbdfe28eca05ca75cb743d14197a3c8282902d5";
pub(super) const PREPARED: &str =
    "723d6b490b72151b8aacdd1233a1299b433f9a219bdf5965abaa8b0d9624d380";
pub(super) const CONDITIONS: &str =
    "5adbdf2b8fbfa0ab9e711091b025e0cb3fb4006701d7cf70d64468e58d840c31";
pub(super) const IMAGE: &str =
    "rust@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6";
const MAX_METADATA: usize = 16 * 1024 * 1024;

pub(super) fn json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&files::read(path, MAX_METADATA)?)
        .map_err(|_| "metadata is not valid JSON".to_owned())
}

pub(super) fn digest(path: &Path, expected: &str, label: &str) -> Result<Vec<u8>, String> {
    let bytes = files::read(path, 256 * 1024 * 1024)?;
    if files::hash(&bytes) != expected {
        return Err(format!("{label} digest mismatch"));
    }
    Ok(bytes)
}

pub(super) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("metadata requires {key}"))
}

pub(super) fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("metadata requires {key}"))
}

fn relative(path: &str) -> Result<&Path, String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || path.to_string_lossy().contains('\\')
    {
        return Err("manifest path must be a normal relative path".to_owned());
    }
    Ok(path)
}

fn inventory(directory: &Path, prefix: &Path, actual: &mut BTreeSet<String>) -> Result<(), String> {
    files::reject_aliases(directory)?;
    for entry in fs::read_dir(directory).map_err(|_| "prepared directory unavailable")? {
        let entry = entry.map_err(|_| "prepared directory entry unavailable")?;
        files::reject_aliases(&entry.path())?;
        let kind = entry
            .file_type()
            .map_err(|_| "prepared file type unavailable")?;
        if kind.is_dir() {
            inventory(&entry.path(), prefix, actual)?;
        } else if kind.is_file() {
            let path = entry.path();
            let normal = path
                .strip_prefix(prefix)
                .map_err(|_| "prepared directory escape")?
                .to_str()
                .ok_or("non-UTF8 prepared path")?
                .replace('\\', "/");
            actual.insert(normal);
        } else {
            return Err("prepared tree contains a non-regular entry".to_owned());
        }
    }
    Ok(())
}

fn verify_tree(directory: &Path, entries: &[Value]) -> Result<(), String> {
    let mut expected = BTreeSet::new();
    for entry in entries {
        let name = text(entry, "path")?;
        if !expected.insert(name.to_owned()) {
            return Err("duplicate prepared file".to_owned());
        }
        let data = digest(
            &directory.join(relative(name)?),
            text(entry, "sha256")?,
            "prepared file",
        )?;
        if entry.get("bytes").and_then(Value::as_u64) != Some(data.len() as u64) {
            return Err("prepared file size mismatch".to_owned());
        }
    }
    let mut actual = BTreeSet::new();
    inventory(directory, directory, &mut actual)?;
    if actual != expected {
        return Err("prepared file inventory differs".to_owned());
    }
    Ok(())
}

pub(super) fn validate(campaign: &Path, proposal: &Path) -> Result<Value, String> {
    digest(&proposal.join("scope.json"), SCOPE, "scope")?;
    let manifest: Value = serde_json::from_slice(&digest(
        &proposal.join("artifact-manifest.json"),
        MANIFEST,
        "approval manifest",
    )?)
    .map_err(|_| "invalid approval manifest")?;
    for entry in manifest
        .as_array()
        .ok_or("approval manifest is not an array")?
    {
        let bytes = digest(
            &proposal.join(relative(text(entry, "path")?)?),
            text(entry, "sha256")?,
            "approved artefact",
        )?;
        if entry.get("bytes").and_then(Value::as_u64) != Some(bytes.len() as u64) {
            return Err("approved artefact size mismatch".to_owned());
        }
    }
    let approval = json(&campaign.join("owner-approval.json"))?;
    if text(&approval, "scopeSha256")? != SCOPE || text(&approval, "manifestSha256")? != MANIFEST {
        return Err("approval does not bind the exact scope".to_owned());
    }
    let prepared: Value = serde_json::from_slice(&digest(
        &campaign.join("prepared-files.json"),
        PREPARED,
        "prepared manifest",
    )?)
    .map_err(|_| "invalid prepared manifest")?;
    let entries = prepared
        .as_array()
        .ok_or("prepared manifest is not an array")?;
    if entries.len() != 10_578 {
        return Err("prepared inventory count differs".to_owned());
    }
    verify_tree(&campaign.join("prepared"), entries)?;
    let conditions: Value = serde_json::from_slice(&digest(
        &campaign.join("conditions.frozen.json"),
        CONDITIONS,
        "condition manifest",
    )?)
    .map_err(|_| "invalid conditions")?;
    if text(&conditions, "originalOwnerScopeSha256")? != SCOPE
        || array(&conditions, "privateInputs")?.len() != 5
        || array(&conditions, "models")?.len() != 2
        || array(&conditions, "profiles")?.len() != 2
        || conditions.get("autoRetry") != Some(&Value::Bool(false))
    {
        return Err("conditions differ from frozen scope".to_owned());
    }
    for input in array(&conditions, "privateInputs")?
        .iter()
        .chain(array(&conditions, "models")?)
    {
        let bytes = digest(
            Path::new(text(input, "input")?),
            text(input, "sha256")?,
            "frozen input",
        )?;
        if let Some(length) = input.get("bytes").and_then(Value::as_u64)
            && bytes.len() as u64 != length
        {
            return Err("frozen input size differs".to_owned());
        }
    }
    Ok(conditions)
}

pub(super) fn prepare(campaign: &Path, proposal: &Path) -> Result<(), String> {
    let conditions = validate(campaign, proposal)?;
    files::reject_aliases(campaign)?;
    let owned = campaign.join("controller");
    fs::create_dir(&owned).map_err(|_| "controller directory must be fresh")?;
    fs::create_dir(owned.join("docker-config"))
        .map_err(|_| "owned Docker config creation failed")?;
    files::write(&owned.join("docker-config/config.json"), b"{}")?;
    for name in ["work", "inputs", "models", "worker", "evidence"] {
        fs::create_dir(owned.join(name)).map_err(|_| "owned directory creation failed")?;
    }
    fs::create_dir(owned.join("work/tmp")).map_err(|_| "owned temp directory creation failed")?;
    let mut inputs = Vec::new();
    for input in array(&conditions, "privateInputs")? {
        let id = text(input, "case_id")?;
        if !matches!(
            id,
            "case-082" | "case-087" | "case-098" | "case-108" | "case-144"
        ) {
            return Err("unknown frozen case".to_owned());
        }
        let png = digest(
            Path::new(text(input, "input")?),
            text(input, "sha256")?,
            "frozen PNG",
        )?;
        let bitmap = formats::encode_png(&png)?;
        let (width, height) = formats::validate_bmp(&bitmap)?;
        files::write(&owned.join("inputs").join(format!("{id}.bmp")), &bitmap)?;
        inputs.push(json!({"id":id,"png_sha256":text(input,"sha256")?,"bmp_sha256":files::hash(&bitmap),"width":width,"height":height}));
    }
    for model in array(&conditions, "models")? {
        let id = text(model, "id")?;
        if !matches!(id, "best" | "fast") {
            return Err("unknown model".to_owned());
        }
        fs::create_dir(owned.join("models").join(id))
            .map_err(|_| "owned model directory creation failed")?;
        files::write(
            &owned.join("models").join(id).join("eng.traineddata"),
            &model_bytes(Path::new(text(model, "input")?), id)?,
        )?;
    }
    let executable = std::env::current_exe().map_err(|_| "controller identity unavailable")?;
    let identity = files::hash(&files::read(&executable, 256 * 1024 * 1024)?);
    let plan = json!({"schema":1,"scope_sha256":SCOPE,"manifest_sha256":MANIFEST,"prepared_sha256":PREPARED,"conditions_sha256":CONDITIONS,"controller_sha256":identity,"image":IMAGE,"inputs":inputs,"models":conditions.get("models"),"profiles":conditions.get("profiles"),"psm":11,"attempts":20,"automatic_retry":false,"physical_weak_pc":false,"native_executed":false,"proposal":fs::canonicalize(proposal).map_err(|_| "proposal directory unavailable")?.to_string_lossy()});
    files::write_json(&owned.join("plan.json"), &plan)?;
    files::write(
        &owned.join("plan.sha256"),
        files::hash(&files::read(&owned.join("plan.json"), MAX_METADATA)?).as_bytes(),
    )
}

pub(super) fn load(campaign: &Path) -> Result<(PathBuf, Value), String> {
    let owned = campaign.join("controller");
    let plan_bytes = files::read(&owned.join("plan.json"), MAX_METADATA)?;
    let expected = files::read(&owned.join("plan.sha256"), 64)?;
    if files::hash(&plan_bytes).as_bytes() != expected {
        return Err("plan digest mismatch".to_owned());
    }
    let plan: Value = serde_json::from_slice(&plan_bytes).map_err(|_| "invalid plan")?;
    let executable = std::env::current_exe().map_err(|_| "controller identity unavailable")?;
    digest(&executable, text(&plan, "controller_sha256")?, "controller")?;
    if text(&plan, "scope_sha256")? != SCOPE || text(&plan, "conditions_sha256")? != CONDITIONS {
        return Err("plan scope mismatch".to_owned());
    }
    let conditions = validate(campaign, Path::new(text(&plan, "proposal")?))?;
    if plan.get("models") != conditions.get("models")
        || plan.get("profiles") != conditions.get("profiles")
        || plan.get("psm") != Some(&serde_json::json!(11))
    {
        return Err("plan conditions differ from frozen authority".to_owned());
    }
    let inputs = array(&plan, "inputs")?;
    let frozen_inputs = array(&conditions, "privateInputs")?;
    if inputs.len() != frozen_inputs.len() {
        return Err("plan input count differs".to_owned());
    }
    for (input, frozen) in inputs.iter().zip(frozen_inputs) {
        if text(input, "id")? != text(frozen, "case_id")?
            || input.get("png_sha256") != frozen.get("sha256")
        {
            return Err("plan input binding differs".to_owned());
        }
        let actual = digest(
            &owned
                .join("inputs")
                .join(format!("{}.bmp", text(input, "id")?)),
            text(input, "bmp_sha256")?,
            "prepared bitmap",
        )?;
        let expected = formats::encode_png(&digest(
            Path::new(text(frozen, "input")?),
            text(frozen, "sha256")?,
            "frozen PNG",
        )?)?;
        if actual != expected {
            return Err("prepared bitmap differs from frozen source pixels".to_owned());
        }
    }
    for model in array(&plan, "models")? {
        if fs::read_dir(owned.join("models").join(text(model, "id")?))
            .map_err(|_| "model directory unavailable")?
            .count()
            != 1
        {
            return Err("model directory contains unexpected files".to_owned());
        }
        digest(
            &owned
                .join("models")
                .join(text(model, "id")?)
                .join("eng.traineddata"),
            text(model, "sha256")?,
            "prepared model",
        )?;
    }
    Ok((owned, plan))
}

pub(super) fn model_bytes(path: &Path, id: &str) -> Result<Vec<u8>, String> {
    let expected = match id {
        "best" => "8280aed0782fe27257a68ea10fe7ef324ca0f8d85bd2fd145d1c2b560bcb66ba",
        "fast" => "7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2",
        _ => return Err("unknown frozen model".to_owned()),
    };
    digest(path, expected, "model")
}
