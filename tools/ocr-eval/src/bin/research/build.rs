// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact offline research build and generated-scope evidence.

use super::{
    docker::{Docker, Kind, Spec},
    files, plan,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::Path,
    time::{Duration, Instant},
};

const CMAKE: &str = "/tools/cmake-4.4.4-linux-x86_64/bin/cmake";
const TESS: &str = "tesseract-7e548e6eff172f38f97664afe7f7ce40db36c8d6";
const COMMON: &[&str] = &[
    "-G",
    "Unix Makefiles",
    "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_C_COMPILER=/usr/bin/x86_64-linux-gnu-gcc-12",
    "-DCMAKE_CXX_COMPILER=/usr/bin/x86_64-linux-gnu-g++-12",
    "-DCMAKE_AR=/usr/bin/x86_64-linux-gnu-ar",
    "-DCMAKE_RANLIB=/usr/bin/x86_64-linux-gnu-ranlib",
    "-DCMAKE_LINKER=/usr/bin/x86_64-linux-gnu-ld.bfd",
    "-DCMAKE_MAKE_PROGRAM=/usr/bin/make",
    "-DCMAKE_INSTALL_PREFIX=/work/stage",
    "-DCMAKE_INSTALL_LIBDIR=lib",
    "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
    "-DCMAKE_VERBOSE_MAKEFILE=ON",
    "-DCMAKE_DISABLE_FIND_PACKAGE_PkgConfig=TRUE",
    "-DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF",
    "-DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF",
    "-DCMAKE_FIND_USE_CMAKE_ENVIRONMENT_PATH=OFF",
    "-DCMAKE_FIND_USE_SYSTEM_ENVIRONMENT_PATH=OFF",
    "-DBUILD_SHARED_LIBS=OFF",
];
const LEPT: &[&str] = &[
    "-DBUILD_PROG=OFF",
    "-DSW_BUILD=OFF",
    "-DSYM_LINK=OFF",
    "-DSTRICT_CONF=ON",
    "-DENABLE_ZLIB=OFF",
    "-DENABLE_PNG=OFF",
    "-DENABLE_GIF=OFF",
    "-DENABLE_JPEG=OFF",
    "-DENABLE_TIFF=OFF",
    "-DENABLE_WEBP=OFF",
    "-DENABLE_OPENJPEG=OFF",
];
const TESS_FLAGS: &[&str] = &[
    "-DLeptonica_DIR=/work/stage/lib/cmake/leptonica",
    "-DCMAKE_CXX_FLAGS=-DTESSERACT_DISABLE_DEBUG_FONTS",
    "-DOPENMP_BUILD=OFF",
    "-DGRAPHICS_DISABLED=ON",
    "-DDISABLED_LEGACY_ENGINE=ON",
    "-DDISABLE_TIFF=ON",
    "-DDISABLE_ARCHIVE=ON",
    "-DDISABLE_CURL=ON",
    "-DBUILD_TRAINING_TOOLS=OFF",
    "-DBUILD_TESTS=OFF",
    "-DUSE_SYSTEM_GTEST=OFF",
    "-DUSE_SYSTEM_ICU=OFF",
    "-DINSTALL_CONFIGS=OFF",
    "-DENABLE_NATIVE=OFF",
    "-DENABLE_LTO=OFF",
    "-DENABLE_UNITY_BUILD=OFF",
    "-DENABLE_PRECOMPILED_HEADERS=OFF",
    "-DENABLE_CCACHE=OFF",
    "-DENABLE_NINJA_POOL=OFF",
    "-DFAST_FLOAT=ON",
];

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|value| (*value).to_owned()).collect()
}
fn configure(source: &str, build: &str, extra: &[&str]) -> Vec<String> {
    let mut argv = strings(&[CMAKE, "-S", source, "-B", build]);
    argv.extend(strings(COMMON));
    argv.extend(strings(extra));
    argv
}
fn spec(campaign: &Path, owned: &Path, argv: Vec<String>, started: Instant) -> Spec {
    Spec {
        workdir: "/",
        kind: Kind::Build,
        memory: 4_294_967_296,
        mounts: vec![
            (campaign.join("prepared/sources"), "/src", false),
            (campaign.join("prepared/tools"), "/tools", false),
            (owned.join("work"), "/work", true),
        ],
        argv,
        started,
    }
}
fn checked(
    docker: &Docker,
    spec: &Spec,
    deadline: Instant,
    owned: &Path,
    label: &str,
) -> Result<Vec<u8>, String> {
    let attempt = docker.run(spec, Vec::new(), deadline, 128 * 1024 * 1024)?;
    files::write(
        &owned.join("evidence").join(format!("{label}.stdout")),
        &attempt.output.bytes,
    )?;
    files::write(
        &owned.join("evidence").join(format!("{label}.stderr")),
        &attempt.output.stderr,
    )?;
    files::write_json(
        &owned.join("evidence").join(format!("{label}.json")),
        &attempt.receipt,
    )?;
    if attempt.output.code != Some(0)
        || attempt.output.failure.is_some()
        || attempt.receipt.pointer("/final_state/OOMKilled") == Some(&json!(true))
    {
        return Err(format!("bounded research build step failed: {label}"));
    }
    Ok(attempt.output.bytes)
}

pub(super) fn build(campaign: &Path, public_probe: &Path) -> Result<(), String> {
    let (owned, plan) = plan::load(campaign)?;
    let probe = super::probe::verify(public_probe, plan::text(&plan, "controller_sha256")?)?;
    let runner = plan::digest(
        &public_probe.join("worker/probe"),
        plan::text(&probe, "probe_sha256")?,
        "public build runner",
    )?;
    files::write(&owned.join("work/preprocess-runner"), &runner)?;
    files::write(
        &owned.join("build.started"),
        b"one attempt; no automatic retry",
    )?;
    let docker = Docker::new(&owned)?;
    let started = Instant::now();
    let deadline = started + Duration::from_secs(1800);
    preflight(
        campaign,
        &owned,
        Path::new(plan::text(&plan, "proposal")?),
        &docker,
        started,
        deadline,
    )?;
    let steps = [
        configure("/src/leptonica-1.87.0", "/work/build-lept", LEPT),
        strings(&[
            CMAKE,
            "--build",
            "/work/build-lept",
            "--target",
            "leptonica",
            "--parallel",
            "2",
        ]),
        strings(&[CMAKE, "--install", "/work/build-lept"]),
        configure(&format!("/src/{TESS}"), "/work/build-tess", TESS_FLAGS),
        strings(&[
            CMAKE,
            "--build",
            "/work/build-tess",
            "--target",
            "tesseract",
            "--parallel",
            "2",
        ]),
    ];
    for (index, argv) in steps.into_iter().enumerate() {
        checked(
            &docker,
            &spec(campaign, &owned, argv, started),
            deadline,
            &owned,
            &format!("build-{index}"),
        )?;
    }
    let graph = graph(campaign, &owned, Path::new(plan::text(&plan, "proposal")?))?;
    let readelf = checked(
        &docker,
        &spec(
            campaign,
            &owned,
            strings(&[
                "/usr/bin/readelf",
                "-h",
                "-d",
                "-l",
                "--version-info",
                "/work/build-tess/bin/tesseract",
            ]),
            started,
        ),
        deadline,
        &owned,
        "worker-elf",
    )?;
    super::elf::validate(&readelf)?;
    let mut batch = String::from("tallyline-preprocess-v1\n");
    for (index, unit) in graph.iter().enumerate() {
        if Instant::now() >= deadline {
            return Err("total research build deadline exceeded".to_owned());
        }
        batch.push_str(&preprocess_record(unit, index)?);
    }
    files::write(&owned.join("work/preprocess.plan"), batch.as_bytes())?;
    plan::digest(
        &owned.join("work/preprocess-runner"),
        plan::text(&probe, "probe_sha256")?,
        "build runner",
    )?;
    checked(
        &docker,
        &spec(
            campaign,
            &owned,
            strings(&["/work/preprocess-runner", "preprocess"]),
            started,
        ),
        deadline,
        &owned,
        "preprocess-batch",
    )?;
    let mut preprocessed = Vec::new();
    for (index, unit) in graph.iter().enumerate() {
        if Instant::now() >= deadline {
            return Err("total research build deadline exceeded".to_owned());
        }
        let file = owned.join("work").join(format!("preprocessed-{index}.ii"));
        let bytes = files::read(&file, 128 * 1024 * 1024)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "preprocessor output is not UTF-8")?;
        for forbidden in [
            "#define _OPENMP ",
            "#define HAVE_LIBCURL 1",
            "#define HAVE_LIBARCHIVE 1",
            "#define HAVE_LIBTIFF 1",
            "#define HAVE_LIBPNG 1",
            "#define HAVE_LIBJPEG 1",
            "#define HAVE_LIBZ 1",
            "#define HAVE_LIBGIF 1",
            "#define HAVE_LIBWEBP 1",
            "#define HAVE_LIBWEBP_ANIM 1",
            "#define HAVE_LIBJP2K 1",
        ] {
            if text.contains(forbidden) {
                return Err("preprocessor activated an excluded native feature".to_owned());
            }
        }
        preprocessed.push(
            json!({"unit":unit.get("file"),"sha256":files::hash(&bytes),"bytes":bytes.len()}),
        );
    }
    if Instant::now() >= deadline {
        return Err("total research build deadline exceeded".to_owned());
    }
    let worker = files::read(
        &owned.join("work/build-tess/bin/tesseract"),
        256 * 1024 * 1024,
    )?;
    files::write(&owned.join("worker/tesseract"), &worker)?;
    let summary = json!({"controller_sha256":plan.get("controller_sha256"),"plan_sha256":files::hash(&files::read(&owned.join("plan.json"),16*1024*1024)?),"worker_sha256":files::hash(&worker),"translation_units":graph,"generated_artifacts":generated_artifacts(&owned)?,"preprocessed":preprocessed,"elf_sha256":files::hash(&readelf),"automated_scope_gates":true,"actual_reachability_review_required":true,"native_synthetic_gates_pending":true,"private_inference":false});
    files::write_json(&owned.join("build-evidence.json"), &summary)?;
    files::write(
        &owned.join("build-evidence.sha256"),
        files::hash(&files::read(
            &owned.join("build-evidence.json"),
            16 * 1024 * 1024,
        )?)
        .as_bytes(),
    )
}

fn preprocess_record(unit: &Value, index: usize) -> Result<String, String> {
    let mut argv = tokenise(plan::text(unit, "command")?)?;
    let output = format!("/work/preprocessed-{index}.ii");
    let mut found = false;
    let mut i = 0;
    while i < argv.len() {
        if argv.get(i).is_some_and(|arg| arg == "-o") {
            *argv.get_mut(i + 1).ok_or("missing compiler output")? = output.clone();
            found = true;
            i += 1;
        } else if argv.get(i).is_some_and(|arg| arg == "-c") {
            *argv.get_mut(i).ok_or("missing compiler mode")? = "-E".to_owned();
        }
        i += 1;
    }
    if !found {
        return Err("compile command has no output".to_owned());
    }
    argv.push("-dD".to_owned());
    let directory = plan::text(unit, "directory")?;
    if !matches!(directory, "/work/build-lept/src" | "/work/build-tess")
        || argv
            .iter()
            .any(|arg| arg.contains(['\t', '\r', '\n', '\0']))
    {
        return Err("invalid preprocessing directory or argument".to_owned());
    }
    let mut record = directory.to_owned();
    for argument in &argv {
        record.push('\t');
        record.push_str(argument);
    }
    record.push('\n');
    Ok(record)
}

fn graph(campaign: &Path, owned: &Path, proposal: &Path) -> Result<Vec<Value>, String> {
    let expected = plan::json(&proposal.join("native-source-set.proposed.json"))?;
    let mut units = Vec::new();
    for (key, directory, source) in [
        ("leptonica", "build-lept", "leptonica-1.87.0"),
        ("tesseract", "build-tess", TESS),
    ] {
        let mut wanted = BTreeSet::new();
        for entry in plan::array(&expected, key)? {
            plan::digest(
                &campaign
                    .join("prepared/sources")
                    .join(source)
                    .join(plan::text(entry, "path")?),
                plan::text(entry, "sha256")?,
                "native translation unit",
            )?;
            wanted.insert(format!("/src/{source}/{}", plan::text(entry, "path")?));
        }
        let compiled = plan::json(
            &owned
                .join("work")
                .join(directory)
                .join("compile_commands.json"),
        )?;
        let mut actual = BTreeSet::new();
        for unit in compiled
            .as_array()
            .ok_or("compile_commands is not an array")?
        {
            let file = plan::text(unit, "file")?;
            if !actual.insert(file.to_owned()) || !wanted.contains(file) {
                return Err("actual native translation-unit graph differs from approval".to_owned());
            }
            let command = tokenise(plan::text(unit, "command")?)?;
            let compiler = command.first().ok_or("compiler command is empty")?;
            if compiler != "/usr/bin/x86_64-linux-gnu-gcc-12"
                && compiler != "/usr/bin/x86_64-linux-gnu-g++-12"
            {
                return Err("unexpected compiler in actual graph".to_owned());
            }
            if command.iter().any(|arg| {
                [
                    "openmp",
                    "march=native",
                    "-mfpu=neon",
                    "-march=rv",
                    "-lcurl",
                    "-larchive",
                    "-ltiff",
                    "-licu",
                    "-fplugin",
                    "-specs=",
                    "@",
                ]
                .iter()
                .any(|pattern| arg.to_ascii_lowercase().contains(pattern))
            }) {
                return Err("compile command activates an excluded feature or tool".to_owned());
            }
            units.push(unit.clone());
        }
        if actual != wanted {
            return Err("actual native translation-unit set is incomplete".to_owned());
        }
        let cache = String::from_utf8(files::read(
            &owned.join("work").join(directory).join("CMakeCache.txt"),
            4 * 1024 * 1024,
        )?)
        .map_err(|_| "CMake cache is not UTF-8")?;
        for argument in COMMON
            .iter()
            .chain(if key == "leptonica" { LEPT } else { TESS_FLAGS })
        {
            if let Some(definition) = argument.strip_prefix("-D") {
                let (name, value) = definition
                    .split_once('=')
                    .ok_or("invalid fixed CMake argument")?;
                if !cache.lines().any(|line| {
                    line.split_once(':').is_some_and(|(key, tail)| {
                        key == name
                            && tail
                                .split_once('=')
                                .is_some_and(|(_, actual)| actual == value)
                    })
                }) {
                    return Err(format!("generated CMake cache differs: {name}"));
                }
            }
        }
        let header = owned
            .join("work")
            .join(directory)
            .join(if key == "leptonica" {
                "src/config_auto.h"
            } else {
                "config_auto.h"
            });
        files::read(&header, 1024 * 1024)?;
    }
    for relative in [
        "build-tess/CMakeFiles/tesseract.dir/link.txt",
        "build-tess/CMakeFiles/libtesseract.dir/link.txt",
        "build-lept/src/CMakeFiles/leptonica.dir/link.txt",
    ] {
        let bytes = files::read(&owned.join("work").join(relative), 4 * 1024 * 1024)?;
        let text = String::from_utf8(bytes).map_err(|_| "link evidence is not UTF-8")?;
        if [
            "-fopenmp",
            "-march=native",
            "-lcurl",
            "-larchive",
            "-ltiff",
            "-lpng",
            "-ljpeg",
            "-lz ",
            "-lgomp",
            "-licu",
            "-ltesseract",
            "-llept ",
        ]
        .iter()
        .any(|needle| text.contains(needle))
        {
            return Err("link graph contains an excluded dependency".to_owned());
        }
    }
    Ok(units)
}

fn tokenise(command: &str) -> Result<Vec<String>, String> {
    if command.chars().any(|ch| {
        matches!(
            ch,
            '\n' | '\r' | '`' | '$' | ';' | '|' | '&' | '<' | '>' | '\0'
        )
    }) {
        return Err("compiler evidence contains a shell operation".to_owned());
    }
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in command.chars() {
        if escaped {
            token.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if ch == delimiter {
                quote = None;
            } else {
                token.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_ascii_whitespace() {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            token.push(ch);
        }
    }
    if quote.is_some() || escaped {
        return Err("compiler evidence has incomplete quoting".to_owned());
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn preflight(
    campaign: &Path,
    owned: &Path,
    proposal: &Path,
    docker: &Docker,
    started: Instant,
    deadline: Instant,
) -> Result<(), String> {
    let value = plan::json(&proposal.join("compiler-sdk-file-hashes.json"))?;
    let entries = value
        .as_array()
        .ok_or("compiler SDK manifest must be an array")?;
    if entries.len() != 4140 {
        return Err("compiler SDK checksum count differs".to_owned());
    }
    let mut checks = String::new();
    let mut paths = BTreeSet::new();
    for entry in entries {
        let path = plan::text(entry, "path")?;
        let hash = plan::text(entry, "sha256")?;
        if !path.starts_with('/')
            || path.contains(['\n', '\r', '\\', '\0'])
            || path.split('/').any(|part| part == "..")
            || hash.len() != 64
            || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !paths.insert(path.to_owned())
        {
            return Err("invalid fixed compiler checksum entry".to_owned());
        }
        checks.push_str(&format!("{hash}  {path}\n"));
    }
    let bootstrap = String::from_utf8(files::read(
        &proposal.join("existing-bootstrap-tool-identities.txt"),
        1024 * 1024,
    )?)
    .map_err(|_| "bootstrap identities are not UTF-8")?;
    let mut tools = 0;
    for line in bootstrap.lines() {
        let Some((hash, path)) = line.split_once("  ") else {
            break;
        };
        if hash.len() != 64
            || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !path.starts_with('/')
            || path.contains(['\n', '\r', '\\', '\0'])
        {
            return Err("invalid bootstrap checksum entry".to_owned());
        }
        checks.push_str(&format!("{hash}  {path}\n"));
        tools += 1;
    }
    if tools != 8 {
        return Err("bootstrap checksum count differs".to_owned());
    }
    files::write(
        &owned.join("evidence/compiler-preflight.sha256sums"),
        checks.as_bytes(),
    )?;
    let attempt = docker.run(
        &spec(
            campaign,
            owned,
            strings(&["/usr/bin/sha256sum", "--check", "--strict", "--status", "-"]),
            started,
        ),
        checks.into_bytes(),
        deadline,
        1024 * 1024,
    )?;
    files::write(
        &owned.join("evidence/compiler-preflight.stderr"),
        &attempt.output.stderr,
    )?;
    files::write_json(
        &owned.join("evidence/compiler-preflight.json"),
        &attempt.receipt,
    )?;
    if attempt.output.code != Some(0) || attempt.output.failure.is_some() {
        return Err(
            "approved compiler/SDK/bootstrap file revalidation failed before CMake".to_owned(),
        );
    }
    Ok(())
}

fn generated_artifacts(owned: &Path) -> Result<Vec<Value>, String> {
    let names = [
        "work/build-lept/compile_commands.json",
        "work/build-tess/compile_commands.json",
        "work/build-lept/CMakeCache.txt",
        "work/build-tess/CMakeCache.txt",
        "work/build-lept/src/config_auto.h",
        "work/build-tess/config_auto.h",
        "work/build-tess/CMakeFiles/tesseract.dir/link.txt",
        "work/build-tess/CMakeFiles/libtesseract.dir/link.txt",
        "work/build-lept/src/CMakeFiles/leptonica.dir/link.txt",
        "evidence/worker-elf.stdout",
        "evidence/compiler-preflight.sha256sums",
        "evidence/compiler-preflight.json",
        "work/preprocess.plan",
        "work/preprocess-runner",
        "evidence/preprocess-batch.stdout",
        "evidence/preprocess-batch.stderr",
        "evidence/preprocess-batch.json",
    ];
    names
        .into_iter()
        .map(|name| {
            let bytes = files::read(&owned.join(name), 16 * 1024 * 1024)?;
            Ok(json!({"path":name,"sha256":files::hash(&bytes),"bytes":bytes.len()}))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::preprocess_record;
    use serde_json::Value;

    #[test]
    fn recorded_cmake_commands_generate_exact_preprocessing_records() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/preprocessing-commands.json"
        ))
        .unwrap();
        let mut index = 0;
        for case in cases {
            let directory = case["directory"].as_str().unwrap();
            let arguments = case["preprocessed_arguments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap())
                .collect::<Vec<_>>()
                .join("\t");
            for _ in 0..case["copies"].as_u64().unwrap() {
                let expected =
                    format!("{directory}\t{arguments}\n").replace("INDEX", &index.to_string());
                assert_eq!(preprocess_record(&case, index).unwrap(), expected);
                index += 1;
            }
        }
        assert_eq!(index, 334);
    }

    #[test]
    fn preprocessing_records_refuse_unapproved_directories_and_shell_evidence() {
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/preprocessing-commands.json"
        ))
        .unwrap();
        for mut case in cases {
            let approved = case["directory"].as_str().unwrap().to_owned();
            for directory in [
                "/work/build-lept".to_owned(),
                format!("{approved}/.."),
                format!("{approved}/child"),
                format!("{approved}-extra"),
                "/tmp/build-lept/src".to_owned(),
            ] {
                case["directory"] = directory.into();
                assert_eq!(
                    preprocess_record(&case, 0).unwrap_err(),
                    "invalid preprocessing directory or argument"
                );
            }
            case["directory"] = approved.into();
            let command = case["command"].as_str().unwrap().to_owned();
            case["command"] = format!("{command}; touch /tmp/foreign").into();
            assert_eq!(
                preprocess_record(&case, 0).unwrap_err(),
                "compiler evidence contains a shell operation"
            );
            case["command"] = format!("{command} 'embedded\targument'").into();
            assert_eq!(
                preprocess_record(&case, 0).unwrap_err(),
                "invalid preprocessing directory or argument"
            );
        }
    }
}
