// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `cargo xtask` entry point: parses the subcommand and runs the steps of `xtask::check`.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::architecture;
use xtask::check::{Step, plan};
use xtask::dco;
use xtask::deny::unjustified_suppressions;
use xtask::dependabot;
use xtask::english;
use xtask::exit::exit_code;
use xtask::guard::scan;
use xtask::header;
use xtask::licence;
use xtask::listing::check_listing;
use xtask::lockfile;
use xtask::notices::{
    ComponentFile, Inputs, NOTICES_DIRECTORY, REVIEWED_LIST_PATH, Roots, assemble,
    parse_deny_report, parse_reviewed_list, vendored_crates, write_catalogue, write_directory,
};
use xtask::osv::{judge, shipped_packages, shipped_roots};
use xtask::registry::Registry;
use xtask::sources::collect;
use xtask::tracked;
use xtask::vendor::{actual_list, regenerate, verify};
use xtask::vet::violations;

const REQUIREMENTS: &str = "docs/plan/01-requirements.md";
const ROADMAP: &str = "docs/plan/04-roadmap.md";
const ENGINEERING_PRINCIPLES: &str = "docs/plan/09-engineering-principles.md";
const DENY_CONFIG: &str = "deny.toml";
const VET_CONFIG: &str = "supply-chain/config.toml";
const VET_BASELINE: &str = "supply-chain/bootstrap-exemptions.txt";
const LOCKFILE: &str = "Cargo.lock";
const HASH_LIST: &str = "supply-chain/vendor-hashes.txt";
const SHIPPED_TARGETS: [&str; 2] = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"];
const OSV_EDGES: &str = "normal";
const LINKED_EDGES: &str = "normal,no-proc-macro";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "package")
    {
        return exit_code(
            arguments
                .get(1..)
                .ok_or_else(|| io::Error::other("missing package arguments"))
                .and_then(xtask::package::run)
                .map(|()| ExitCode::SUCCESS),
            &mut io::stderr(),
        );
    }
    let root = workspace_root();
    let outcome = match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check"] => run_check(&root),
        ["install-cargo-mutants"] => {
            xtask::install_cargo_mutants::run().map(|()| ExitCode::SUCCESS)
        }
        ["guard"] => run_guard(&root),
        ["architecture"] => architecture_passes(&root).map(exit_status),
        ["build-code"] => run_build_code(&root),
        ["native-scope"] => run_native_scope(&root),
        ["property-scope", metadata, lock, target] => {
            run_property_scope(Path::new(metadata), Path::new(lock), target, None)
        }
        ["property-scope", metadata, lock, target, review] => run_property_scope(
            Path::new(metadata),
            Path::new(lock),
            target,
            Some(Path::new(review)),
        ),
        ["sqlite-native"] => run_sqlite_native(&root),
        ["native-assembly"] => {
            run_native_assembly(&root, Path::new(xtask::native_assembly::REVIEWED_PATH))
        }
        ["native-assembly", review] => run_native_assembly(&root, Path::new(review)),
        ["mutation", options @ ..] => xtask::mutation::Options::parse(options)
            .map_err(io::Error::other)
            .and_then(|options| run_mutation(&root, &options)),
        ["mutation-aggregate", directory, head, run, attempt] => {
            run_mutation_aggregate(Path::new(directory), head, run, attempt)
        }
        ["dco", base, head] => run_dco(&root, base, head),
        ["osv-gate", report] => run_osv_gate(&root, Path::new(report)),
        ["vendor-hashes", "write"] => write_hash_list(&root),
        ["vendor-hashes", "verify"] => verify_hash_list(&root),
        ["notices"] => write_notices(&root),
        _ => usage(),
    };
    exit_code(outcome, &mut io::stderr())
}

fn usage() -> io::Result<ExitCode> {
    writeln!(
        io::stderr(),
        "usage: cargo xtask check | install-cargo-mutants | guard | architecture | build-code | native-scope | property-scope <metadata.json> <Cargo.lock> <target> [<approved.json>] | sqlite-native | native-assembly [<approved.json>] | mutation [--in-diff <diff>] [--shard <k/n>] | mutation-aggregate <directory> <head> <run> <attempt> | dco <base> <head> | osv-gate <report.json> | vendor-hashes write | vendor-hashes verify | notices"
    )?;
    Ok(ExitCode::FAILURE)
}

fn workspace_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(manifest_dir).to_path_buf()
}

fn cargo_program() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn run_check(root: &Path) -> io::Result<ExitCode> {
    let mut failed = Vec::new();
    for step in plan() {
        writeln!(io::stderr(), "==> {}", step.name())?;
        if !run_step(&step, root)? {
            failed.push(step.name());
            if matches!(
                step,
                Step::NativeAssembly
                    | Step::SqliteNative
                    | Step::NativeScope
                    | Step::BuildCode
                    | Step::PackageLegalSources
            ) {
                writeln!(
                    io::stderr(),
                    "native, build-code or packaging source-binding gate failed before test/build work"
                )?;
                return Ok(ExitCode::FAILURE);
            }
        }
    }
    if failed.is_empty() {
        writeln!(io::stderr(), "check passed")?;
        Ok(ExitCode::SUCCESS)
    } else {
        writeln!(io::stderr(), "check failed: {}", failed.join(", "))?;
        Ok(ExitCode::FAILURE)
    }
}

fn run_step(step: &Step, root: &Path) -> io::Result<bool> {
    match step {
        Step::SupplyChainGuard => guard_passes(root),
        Step::SqliteNative => sqlite_native_passes(root),
        Step::NativeScope => native_scope_passes(root),
        Step::NativeAssembly => {
            native_assembly_passes(root, Path::new(xtask::native_assembly::REVIEWED_PATH))
        }
        Step::BuildCode => build_code_passes(root),
        Step::PackageLegalSources => match xtask::package::verify_source_bindings(root) {
            Ok(()) => Ok(true),
            Err(error) => {
                writeln!(io::stderr(), "{error}")?;
                Ok(false)
            }
        },
        Step::DenySuppressions => deny_suppressions_pass(root),
        Step::VetExemptions => vet_exemptions_pass(root),
        Step::Dependabot => dependabot_passes(root),
        Step::LicenceCopy => licence_copy_passes(root),
        Step::SourceHeaders => source_headers_pass(root),
        Step::English => english_passes(root),
        Step::Architecture => architecture_passes(root),
        Step::CoreCoverage => coverage_passes(root),
        Step::Cargo { args, env, .. } => Ok(work_paths(root)?
            .check_command(&cargo_program())?
            .args(args)
            .envs(env.iter().copied())
            .current_dir(root)
            .status()?
            .success()),
        Step::RequirementTags { list } => check_requirement_tags(list, root),
    }
}

fn run_mutation(root: &Path, options: &xtask::mutation::Options) -> io::Result<ExitCode> {
    let paths = work_paths(root)?;
    let output = if let Some(shard) = options.shard {
        paths.prepare_child(
            xtask::work_paths::OutputKind::Mutation,
            OsStr::new(&shard.report_directory()),
        )?
    } else {
        paths.prepare(xtask::work_paths::OutputKind::Mutation)?
    };
    let report = output.join("mutants.out/outcomes.json");
    if report.exists() {
        fs::remove_file(&report)?;
    }
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
    )?;
    let packages = xtask::quality::core_packages(&metadata).map_err(io::Error::other)?;
    if packages.is_empty() {
        fs::write(output.join("all-selected-mutants.json"), "[]")?;
        fs::write(output.join("selected-mutants.json"), "[]")?;
        writeln!(
            io::stderr(),
            "SKIP core mutation: no core crates; no baseline measured"
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut args = vec![
        OsString::from("mutants"),
        OsString::from("--cargo-arg=--locked"),
        OsString::from("--test-workspace=true"),
        OsString::from("--minimum-test-timeout=120"),
        OsString::from("--output"),
        output.clone().into_os_string(),
    ];
    for package in packages {
        args.extend([OsString::from("--package"), package.into()]);
    }
    if let Some(diff) = &options.diff {
        args.extend([OsString::from("--in-diff"), diff.as_os_str().to_owned()]);
    }
    let complete_selection = list_mutants(root, &args)?;
    fs::write(
        output.join("all-selected-mutants.json"),
        &complete_selection,
    )?;
    let selection = if let Some(shard) = options.shard {
        args.extend([
            OsString::from("--shard"),
            shard.to_string().into(),
            OsString::from("--sharding=round-robin"),
        ]);
        list_mutants(root, &args)?
    } else {
        complete_selection
    };
    fs::write(output.join("selected-mutants.json"), &selection)?;
    let mutants: serde_json::Value = serde_json::from_str(&selection).map_err(io::Error::other)?;
    let mutants = mutants
        .as_array()
        .ok_or_else(|| io::Error::other("mutation list is not an array"))?;
    if mutants.is_empty() {
        writeln!(
            io::stderr(),
            "SKIP core mutation: no selected mutants; no baseline measured"
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    let status = work_paths(root)?
        .mutation_command(&cargo_program())
        .args(args)
        .current_dir(root)
        .status()?;
    if !matches!(status.code(), Some(0 | 2)) {
        return Ok(ExitCode::FAILURE);
    }
    let report_text = fs::read_to_string(report)?;
    let counts =
        xtask::mutation::selected_counts(&report_text, &selection).map_err(io::Error::other)?;
    xtask::mutation_evidence::verify_cause_logs(&output.join("mutants.out"), &report_text)
        .map_err(io::Error::other)?;
    if options.shard.is_some() && counts.caught == 0 && counts.missed == 0 {
        writeln!(
            io::stderr(),
            "completed unviable-only shard: {} unviable; no score measured; complete selection requires aggregate",
            counts.unviable
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    report_mutation_score(counts.score().map(Some))
}

fn list_mutants(root: &Path, arguments: &[OsString]) -> io::Result<String> {
    let output = work_paths(root)?
        .mutation_command(&cargo_program())
        .args(arguments)
        .args(["--list", "--json"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "mutation listing failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    xtask::mutation_evidence::listing_json(output.stdout, &output.stderr).map_err(io::Error::other)
}

fn run_mutation_aggregate(
    directory: &Path,
    head: &str,
    run: &str,
    attempt: &str,
) -> io::Result<ExitCode> {
    let identity = xtask::mutation::RunIdentity { head, run, attempt };
    report_mutation_score(xtask::mutation::aggregate(directory, &identity))
}

fn report_mutation_score(result: Result<Option<f64>, String>) -> io::Result<ExitCode> {
    match result {
        Ok(Some(score)) => {
            writeln!(
                io::stderr(),
                "core mutation score: {score:.2}% (frozen floor {:.0}%)",
                xtask::quality::MUTATION_SCORE_FLOOR
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(None) => {
            writeln!(
                io::stderr(),
                "SKIP core mutation: complete selection is empty; no baseline measured"
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            writeln!(io::stderr(), "{error}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}
fn coverage_passes(root: &Path) -> io::Result<bool> {
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
    )?;
    let packages = xtask::quality::core_packages(&metadata).map_err(io::Error::other)?;
    if packages.is_empty() {
        writeln!(
            io::stderr(),
            "SKIP core coverage: no core crates in the workspace"
        )?;
        return Ok(true);
    }
    let mut args = vec![
        "llvm-cov".to_owned(),
        "--locked".to_owned(),
        "--fail-under-lines".to_owned(),
        "85".to_owned(),
    ];
    for package in packages {
        args.extend(["--package".to_owned(), package]);
    }
    Ok(work_paths(root)?
        .check_command(&cargo_program())?
        .args(args)
        .current_dir(root)
        .status()?
        .success())
}
fn run_build_code(root: &Path) -> io::Result<ExitCode> {
    if !guard_passes(root)? {
        return Ok(ExitCode::FAILURE);
    }
    Ok(exit_status(build_code_passes(root)?))
}

fn build_code_passes(root: &Path) -> io::Result<bool> {
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--offline", "--format-version", "1"],
    )?;
    let hashes = xtask::build_code::source_hashes(&metadata).map_err(io::Error::other)?;
    let findings = xtask::build_code::violations(
        &metadata,
        &fs::read_to_string(root.join(LOCKFILE))?,
        &fs::read_to_string(root.join(xtask::build_code::REVIEWED_PATH))?,
        &hashes,
    )
    .map_err(io::Error::other)?;
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}
fn run_native_scope(root: &Path) -> io::Result<ExitCode> {
    if !guard_passes(root)? {
        return Ok(ExitCode::FAILURE);
    }
    Ok(exit_status(native_scope_passes(root)?))
}

fn native_scope_passes(root: &Path) -> io::Result<bool> {
    let mut passed = true;
    for target in SHIPPED_TARGETS {
        let metadata = cargo_stdout(
            root,
            &[
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
                target,
            ],
        )?;
        let mut findings = xtask::native::violations(&metadata).map_err(io::Error::other)?;
        findings.extend(xtask::property_scope_command::workspace_violations(
            root,
            target,
            |args| cargo_stdout(root, args),
        )?);
        for finding in &findings {
            writeln!(io::stderr(), "{target}: {finding}")?;
        }
        passed &= findings.is_empty();
    }
    Ok(passed)
}

fn run_property_scope(
    metadata: &Path,
    lock: &Path,
    target: &str,
    review: Option<&Path>,
) -> io::Result<ExitCode> {
    let findings = xtask::property_scope_command::inspect(metadata, lock, target, review)?;
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(exit_status(findings.is_empty()))
}
fn architecture_passes(root: &Path) -> io::Result<bool> {
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
    )?;
    let mut findings = architecture::violations(&metadata);
    let paths: Vec<_> = tracked::paths(root)?
        .into_iter()
        .filter(|path| path.starts_with("crates/") && path.ends_with(".rs"))
        .collect();
    findings.extend(
        xtask::reconstitution::violations(&tracked::read_text(root, &paths)?)
            .into_iter()
            .map(architecture::Violation),
    );
    for finding in &findings {
        writeln!(io::stderr(), "{}", finding.0)?;
    }
    Ok(findings.is_empty())
}

fn run_guard(root: &Path) -> io::Result<ExitCode> {
    Ok(exit_status(guard_passes(root)?))
}

fn program_stdout(program: &OsStr, root: &Path, args: &[&str]) -> io::Result<String> {
    let output = work_paths(root)?
        .command(program)
        .args(args)
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "{} {} failed",
            program.to_string_lossy(),
            args.join(" ")
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn cargo_stdout(root: &Path, args: &[&str]) -> io::Result<String> {
    program_stdout(&cargo_program(), root, args)
}

fn shipped_graph(
    root: &Path,
    edges: &str,
    targets: &[&str],
) -> io::Result<(Vec<String>, BTreeSet<xtask::osv::PackageId>)> {
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
    )?;
    let roots = shipped_roots(&metadata).map_err(io::Error::other)?;
    let mut shipped = BTreeSet::new();
    for name in &roots {
        for target in targets {
            let tree = cargo_stdout(
                root,
                &[
                    "tree",
                    "--locked",
                    "--package",
                    name,
                    "--target",
                    target,
                    "--edges",
                    edges,
                    "--all-features",
                    "--prefix",
                    "none",
                    "--format",
                    "{p}",
                ],
            )?;
            shipped.extend(shipped_packages(&tree));
        }
    }
    Ok((roots, shipped))
}

fn run_osv_gate(root: &Path, report: &Path) -> io::Result<ExitCode> {
    let (roots, shipped) = shipped_graph(root, OSV_EDGES, &SHIPPED_TARGETS)?;
    let text = fs::read_to_string(report)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", report.display())))?;
    let verdict = judge(&text, &shipped).map_err(io::Error::other)?;
    writeln!(
        io::stderr(),
        "shipped packages: {} in {} workspace members ({})",
        shipped.len(),
        roots.len(),
        if roots.is_empty() {
            "none yet".to_owned()
        } else {
            roots.join(", ")
        }
    )?;
    for finding in &verdict.reported {
        writeln!(io::stderr(), "reported: {finding}")?;
    }
    for finding in &verdict.blocking {
        writeln!(io::stderr(), "BLOCKING: {finding}")?;
    }
    Ok(exit_status(verdict.blocking.is_empty()))
}

fn exit_status(passed: bool) -> ExitCode {
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn guard_passes(root: &Path) -> io::Result<bool> {
    let findings = scan(&collect(root)?);
    for finding in &findings {
        writeln!(io::stderr(), "{}: {}", finding.file, finding.problem)?;
    }
    Ok(findings.is_empty())
}

fn deny_suppressions_pass(root: &Path) -> io::Result<bool> {
    let findings = unjustified_suppressions(
        &fs::read_to_string(root.join(DENY_CONFIG))?,
        &fs::read_to_string(root.join(ENGINEERING_PRINCIPLES))?,
    );
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn licence_copy_passes(root: &Path) -> io::Result<bool> {
    let read = |path: &str| {
        fs::read(root.join(path))
            .map_err(|error| io::Error::new(error.kind(), format!("{path}: {error}")))
    };
    let finding = licence::difference(&read(licence::LICENSE_PATH)?, &read(licence::COPY_PATH)?);
    if let Some(finding) = &finding {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(finding.is_none())
}

fn source_headers_pass(root: &Path) -> io::Result<bool> {
    let paths: Vec<String> = tracked::paths(root)?
        .into_iter()
        .filter(|path| path.ends_with(".rs"))
        .collect();
    let findings = header::violations(&tracked::read_text(root, &paths)?);
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn english_passes(root: &Path) -> io::Result<bool> {
    let allowlist =
        english::parse_allowlist(&fs::read_to_string(root.join(english::ALLOWLIST_PATH))?)
            .map_err(io::Error::other)?;
    let files = tracked::read_text(root, &tracked::paths(root)?)?;
    let findings = english::violations(&files, &allowlist);
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn run_dco(root: &Path, base: &str, head: &str) -> io::Result<ExitCode> {
    let range = format!("{base}..{head}");
    let output = tracked::git_command(root)
        .args(["log", dco::LOG_FORMAT, &range])
        .output()?;
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!("git log {range} failed")));
    }
    let commits =
        dco::parse_log(&String::from_utf8_lossy(&output.stdout)).map_err(io::Error::other)?;
    if commits.is_empty() {
        writeln!(
            io::stderr(),
            "no commits in {range}; refusing to pass an empty range"
        )?;
        return Ok(ExitCode::FAILURE);
    }
    let findings = dco::violations(&commits);
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    writeln!(io::stderr(), "checked {} commits in {range}", commits.len())?;
    Ok(exit_status(findings.is_empty()))
}

fn dependabot_passes(root: &Path) -> io::Result<bool> {
    let findings = dependabot::violations(&collect(root)?);
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn vet_exemptions_pass(root: &Path) -> io::Result<bool> {
    let findings = violations(
        &fs::read_to_string(root.join(VET_CONFIG))?,
        &fs::read_to_string(root.join(VET_BASELINE))?,
        &fs::read_to_string(root.join(ENGINEERING_PRINCIPLES))?,
    );
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn with_vendor_tree<T>(
    root: &Path,
    action: impl FnOnce(&str, &Path) -> io::Result<T>,
) -> io::Result<T> {
    let lock_text = fs::read_to_string(root.join(LOCKFILE))?;
    let paths = work_paths(root)?;
    let work_dir = paths.reset(xtask::work_paths::OutputKind::Vendor)?;
    let outcome = regenerate(root, &cargo_program(), &work_dir)
        .and_then(|vendor_dir| action(&lock_text, &vendor_dir));
    let cleanup = paths.remove(xtask::work_paths::OutputKind::Vendor);
    match (outcome, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(io::Error::other(format!(
            "vendor work failed ({error}); cleanup failed ({cleanup})"
        ))),
    }
}

fn write_hash_list(root: &Path) -> io::Result<ExitCode> {
    let list = with_vendor_tree(root, |lock_text, vendor_dir| {
        actual_list(lock_text, vendor_dir).map_err(io::Error::other)
    })?;
    let target = root.join(HASH_LIST);
    if let Some(directory) = target.parent() {
        fs::create_dir_all(directory)?;
    }
    fs::write(&target, list.render())?;
    writeln!(
        io::stderr(),
        "wrote {HASH_LIST}: {} packages",
        list.entries().len()
    )?;
    Ok(ExitCode::SUCCESS)
}

fn verify_hash_list(root: &Path) -> io::Result<ExitCode> {
    let list_text = fs::read_to_string(root.join(HASH_LIST))?;
    let differences = with_vendor_tree(root, |lock_text, vendor_dir| {
        verify(lock_text, &list_text, vendor_dir).map_err(io::Error::other)
    })?;
    for difference in &differences {
        writeln!(io::stderr(), "{HASH_LIST}: {difference}")?;
    }
    if differences.is_empty() {
        writeln!(io::stderr(), "{HASH_LIST} matches the vendored tree")?;
    }
    Ok(exit_status(differences.is_empty()))
}

fn toolchain_sysroot(root: &Path) -> io::Result<PathBuf> {
    let output = program_stdout(OsStr::new("rustc"), root, &["--print", "sysroot"])?;
    Ok(PathBuf::from(output.trim()))
}

fn write_notices(root: &Path) -> io::Result<ExitCode> {
    let components = parse_reviewed_list(&fs::read_to_string(root.join(REVIEWED_LIST_PATH))?)
        .map_err(io::Error::other)?;
    let locked: BTreeSet<String> = lockfile::parse(&fs::read_to_string(root.join(LOCKFILE))?)
        .map_err(|error| io::Error::other(error.to_string()))?
        .into_iter()
        .map(|package| package.name)
        .collect();
    let licences = parse_deny_report(&cargo_stdout(
        root,
        &[
            "deny", "--locked", "list", "--format", "json", "--layout", "crate",
        ],
    )?)
    .map_err(io::Error::other)?;
    let sysroot = toolchain_sysroot(root)?;
    let mut graphs = Vec::new();
    for target in SHIPPED_TARGETS {
        let (roots, linked) = shipped_graph(root, LINKED_EDGES, &[target])?;
        let shipped: BTreeSet<_> = linked
            .into_iter()
            .filter(|package| !roots.contains(&package.name))
            .collect();
        graphs.push((target, shipped));
    }
    let paths = work_paths(root)?;
    let work_dir = paths.reset(xtask::work_paths::OutputKind::Notices)?;
    let passed = with_vendor_tree(root, |_, vendor_dir| {
        let mut passed = true;
        for (target, shipped) in &graphs {
            let vendored = vendored_crates(vendor_dir, shipped)?;
            let assembly = assemble(
                Inputs {
                    shipped,
                    locked: &locked,
                    vendored: &vendored,
                    licences: &licences,
                    components: &components,
                },
                |file| match file {
                    ComponentFile::Toolchain(path) => sysroot.join(path).is_file(),
                    ComponentFile::Repository(path) => root.join(path).is_file(),
                },
            );
            for finding in &assembly.findings {
                writeln!(io::stderr(), "{target}: {finding}")?;
            }
            if assembly.findings.is_empty() {
                write_directory(
                    &work_dir.join(target).join(NOTICES_DIRECTORY),
                    &assembly,
                    Roots {
                        vendor: vendor_dir,
                        toolchain: &sysroot,
                        repository: root,
                    },
                )?;
                if let Err(error) = write_catalogue(
                    &work_dir.join(target).join(NOTICES_DIRECTORY),
                    &assembly,
                    Roots {
                        vendor: vendor_dir,
                        toolchain: &sysroot,
                        repository: root,
                    },
                ) {
                    writeln!(io::stderr(), "{target}: {error}")?;
                    passed = false;
                    continue;
                }
                writeln!(
                    io::stderr(),
                    "wrote {}/{target}/{NOTICES_DIRECTORY}: {} files for {} shipped crates and the reviewed list",
                    work_dir.display(),
                    assembly.copies.len(),
                    shipped.len()
                )?;
            } else {
                passed = false;
            }
        }
        Ok(passed)
    })?;
    if !passed && work_dir.exists() {
        paths.remove(xtask::work_paths::OutputKind::Notices)?;
    }
    Ok(exit_status(passed))
}

fn check_requirement_tags(list: &[String], root: &Path) -> io::Result<bool> {
    let listing = work_paths(root)?
        .check_command(&cargo_program())?
        .args(list)
        .current_dir(root)
        .output()?;
    if !listing.status.success() {
        io::stderr().write_all(&listing.stderr)?;
        return Ok(false);
    }
    let registry = Registry::from_documents(
        &fs::read_to_string(root.join(REQUIREMENTS))?,
        &fs::read_to_string(root.join(ROADMAP))?,
    );
    let violations = check_listing(&String::from_utf8_lossy(&listing.stdout), &registry);
    for violation in &violations {
        writeln!(io::stderr(), "{}: {:?}", violation.test, violation.problem)?;
    }
    Ok(violations.is_empty())
}

fn run_native_assembly(root: &Path, review: &Path) -> io::Result<ExitCode> {
    if !guard_passes(root)? {
        return Ok(ExitCode::FAILURE);
    }
    Ok(exit_status(native_assembly_passes(root, review)?))
}

fn native_assembly_passes(root: &Path, review: &Path) -> io::Result<bool> {
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--offline", "--format-version", "1"],
    )?;
    let files = xtask::native_assembly::source_files(&metadata).map_err(io::Error::other)?;
    writeln!(
        io::stderr(),
        "native source inventory: {} Rust source files",
        files.len()
    )?;
    let cargo_version = cargo_stdout(root, &["--version", "--verbose"])?;
    if cargo_version.lines().next() != Some(xtask::target_graph::REVIEWED_CARGO) {
        return Err(io::Error::other(
            "target-tree evidence requires reviewed Cargo 1.99.0 commit 5f94df478",
        ));
    }
    let host = cargo_version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| io::Error::other("missing Cargo host provenance"))?;
    let mut targets = std::collections::BTreeMap::new();
    for target in SHIPPED_TARGETS {
        let tree = cargo_stdout(
            root,
            &[
                "tree",
                "--locked",
                "--offline",
                "--workspace",
                "--target",
                target,
                "--edges",
                "normal,build,dev",
                "--prefix",
                "depth",
                "--format",
                "{p}|{f}",
                "--color",
                "never",
            ],
        )?;
        let resolved =
            xtask::target_graph::resolved_metadata(&metadata, &tree).map_err(io::Error::other)?;
        let mut credential_findings =
            xtask::native::resolved_credential_violations(&resolved).map_err(io::Error::other)?;
        credential_findings.extend(
            xtask::owned_process_scope::violations(&resolved, target).map_err(io::Error::other)?,
        );
        if !credential_findings.is_empty() {
            return Err(io::Error::other(credential_findings.join("; ")));
        }
        let filtered = cargo_stdout(
            root,
            &[
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--filter-platform",
                target,
            ],
        )?;
        let mut filtered: serde_json::Value = serde_json::from_str(&filtered)?;
        filtered
            .as_object_mut()
            .ok_or_else(|| io::Error::other("invalid metadata document"))?
            .insert(
                "tallyline_target_tree".to_owned(),
                serde_json::json!({
                    "cargo": xtask::target_graph::REVIEWED_CARGO,
                    "host": host,
                    "target": target,
                    "graph": serde_json::from_str::<serde_json::Value>(&resolved)?,
                }),
            );
        targets.insert(target.to_owned(), serde_json::to_string(&filtered)?);
    }
    let findings = xtask::native_assembly::violations(
        &metadata,
        &fs::read_to_string(root.join(LOCKFILE))?,
        &fs::read_to_string(root.join(review))?,
        &files,
        &targets,
    )
    .map_err(io::Error::other)?;
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn run_sqlite_native(root: &Path) -> io::Result<ExitCode> {
    if !guard_passes(root)? {
        return Ok(ExitCode::FAILURE);
    }
    Ok(exit_status(sqlite_native_passes(root)?))
}

fn sqlite_native_passes(root: &Path) -> io::Result<bool> {
    let locked = xtask::lockfile::parse(&fs::read_to_string(root.join(LOCKFILE))?)
        .map_err(|error| io::Error::other(error.to_string()))?;
    if !locked
        .iter()
        .any(|package| package.name == "libsqlite3-sys")
    {
        return Ok(true);
    }
    let mut environment = std::env::vars_os()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect::<Vec<_>>();
    environment.extend(
        xtask::sqlite_native::configuration_environment(&fs::read_to_string(
            root.join(".cargo/config.toml"),
        )?)
        .map_err(io::Error::other)?,
    );
    let overrides = xtask::sqlite_native::environment_violations(&environment);
    if !overrides.is_empty() {
        for finding in overrides {
            writeln!(io::stderr(), "{finding}")?;
        }
        return Ok(false);
    }
    let metadata = cargo_stdout(
        root,
        &["metadata", "--locked", "--offline", "--format-version", "1"],
    )?;
    let hashes = xtask::sqlite_native::source_hashes(&metadata).map_err(io::Error::other)?;
    let mut graphs = std::collections::BTreeMap::from([("full".to_owned(), metadata)]);
    for target in SHIPPED_TARGETS {
        graphs.insert(
            target.to_owned(),
            cargo_stdout(
                root,
                &[
                    "metadata",
                    "--locked",
                    "--offline",
                    "--format-version",
                    "1",
                    "--filter-platform",
                    target,
                ],
            )?,
        );
    }
    let now_days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs()
        / 86_400;
    let findings = xtask::sqlite_native::violations(
        &fs::read_to_string(root.join("supply-chain/sqlite-native.json"))?,
        &locked,
        &graphs,
        &hashes,
        &environment,
        now_days,
    )
    .map_err(io::Error::other)?;
    for finding in &findings {
        writeln!(io::stderr(), "{finding}")?;
    }
    Ok(findings.is_empty())
}

fn work_paths(root: &Path) -> io::Result<xtask::work_paths::WorkPaths> {
    xtask::work_paths::WorkPaths::resolve(
        root,
        &std::env::current_dir()?,
        std::env::var_os("CARGO_TARGET_DIR").as_deref(),
    )
}
