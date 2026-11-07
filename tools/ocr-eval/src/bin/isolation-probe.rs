// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Safe public-only Linux namespace and hard-limit probe.

#![forbid(unsafe_code)]

use std::{
    fs,
    io::{self, Read, Write},
    process::{Command, ExitCode},
    time::Duration,
};

fn emit(value: &str) -> Result<(), String> {
    writeln!(io::stdout(), "{value}").map_err(|_| "probe output failed".to_owned())
}
fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|_| "required Linux isolation metadata unavailable".to_owned())
}
fn exact(path: &str, expected: &str) -> Result<(), String> {
    if read(path)?.trim() != expected {
        return Err("kernel hard limit differs".to_owned());
    }
    Ok(())
}
fn snapshot() -> Result<(), String> {
    exact("/sys/fs/cgroup/cpu.max", "200000 100000")?;
    exact("/sys/fs/cgroup/memory.swap.max", "0")?;
    exact("/sys/fs/cgroup/pids.max", "1")?;
    let memory = read("/sys/fs/cgroup/memory.max")?;
    if !matches!(memory.trim(), "4294967296" | "8589934592") {
        return Err("memory hard limit differs".to_owned());
    }
    let limits = read("/proc/self/limits")?;
    for label in ["Max core file size", "Max msgqueue size"] {
        let line = limits
            .lines()
            .find(|line| line.starts_with(label))
            .ok_or("required stricter limit missing")?;
        let values = line
            .strip_prefix(label)
            .ok_or("invalid stricter limit")?
            .split_whitespace()
            .collect::<Vec<_>>();
        if values.first() != Some(&"0") || values.get(1) != Some(&"0") {
            return Err("stricter core or message-queue limit differs".to_owned());
        }
    }
    let stat = read("/proc/self/stat")?;
    if stat
        .rsplit_once(") ")
        .and_then(|(_, tail)| tail.split_whitespace().nth(16))
        != Some("5")
    {
        return Err("worker nice value differs".to_owned());
    }
    let status = read("/proc/self/status")?;
    if !status.lines().any(|line| line == "NoNewPrivs:\t1")
        || !status
            .lines()
            .any(|line| line == "CapEff:\t0000000000000000")
        || !status
            .lines()
            .any(|line| line == "Uid:\t65534\t65534\t65534\t65534")
    {
        return Err("privilege metadata differs".to_owned());
    }
    let mounts = read("/proc/self/mountinfo")?;
    for line in mounts.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let point = fields.get(4).ok_or("mount metadata invalid")?;
        let options = fields.get(5).ok_or("mount options unavailable")?;
        let readonly = options.split(',').any(|option| option == "ro");
        if matches!(*point, "/" | "/worker" | "/model" | "/dev/shm") && !readonly {
            return Err("required runtime mount is writable".to_owned());
        }
        if !readonly
            && !matches!(
                *point,
                "/proc"
                    | "/dev"
                    | "/dev/pts"
                    | "/dev/mqueue"
                    | "/proc/interrupts"
                    | "/proc/kcore"
                    | "/proc/keys"
                    | "/proc/latency_stats"
                    | "/proc/timer_list"
                    | "/proc/bus"
                    | "/proc/fs"
                    | "/proc/irq"
                    | "/proc/sys"
                    | "/proc/sysrq-trigger"
            )
        {
            return Err(format!(
                "unexpected writable runtime mount: {point} {options}"
            ));
        }
    }
    for path in [
        "/tmp/tallyline-public-isolation-probe",
        "/worker/tallyline-public-isolation-probe",
        "/model/tallyline-public-isolation-probe",
        "/dev/shm/tallyline-public-isolation-probe",
        "/dev/mqueue/tallyline-public-isolation-probe",
        "/etc/tallyline-public-isolation-probe",
    ] {
        if let Ok(file) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            drop(file);
            fs::remove_file(path).map_err(|_| "public probe cleanup failed")?;
            return Err(format!("runtime allowed a forbidden file write: {path}"));
        }
    }
    for path in ["/etc/hosts", "/etc/hostname", "/etc/resolv.conf"] {
        let _metadata = fs::metadata(path).map_err(|_| "Docker metadata file unavailable")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if _metadata.uid() != 0 || _metadata.mode() & 0o777 != 0o644 {
                return Err("Docker metadata file ownership or mode differs".to_owned());
            }
        }
        if fs::OpenOptions::new().write(true).open(path).is_ok() {
            return Err("Docker metadata file permits a runtime write".to_owned());
        }
    }
    let executable = std::env::current_exe().map_err(|_| "probe executable unavailable")?;
    match Command::new(executable)
        .arg("child")
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            child
                .kill()
                .map_err(|_| "unexpected probe child could not be killed")?;
            child
                .wait()
                .map_err(|_| "unexpected probe child could not be reaped")?;
            return Err("pids limit allowed a second process".to_owned());
        }
        Err(error) => {
            if error.raw_os_error() != Some(11) {
                return Err("second-process refusal was not the kernel EAGAIN contract".to_owned());
            }
        }
    }
    let route = read("/proc/net/route")?;
    if route.lines().skip(1).any(|line| {
        line.split_whitespace()
            .next()
            .is_some_and(|interface| interface != "lo")
    }) {
        return Err("non-loopback network route exists".to_owned());
    }
    let address = "192.0.2.1:9"
        .parse()
        .map_err(|_| "invalid fixed public probe address")?;
    if std::net::TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok() {
        return Err("external route was usable".to_owned());
    }
    emit(&format!(
        "isolation=passed\nmemory.max={}\nmemory.swap.max=0\npids.max=1\ncpu.max=200000 100000\nloopback=permitted\nexternal_route=absent\nread_only=passed\nchild_denial=EAGAIN",
        memory.trim()
    ))
}
struct PreprocessUnit {
    directory: String,
    arguments: Vec<String>,
}
fn preprocess_plan(path: &str) -> Result<Vec<PreprocessUnit>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "preprocessing plan unavailable")?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "preprocessing plan unreadable")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("preprocessing plan exceeds bounds".to_owned());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "preprocessing plan is not UTF-8")?;
    let mut lines = text.lines();
    if lines.next() != Some("tallyline-preprocess-v1") {
        return Err("preprocessing plan header differs".to_owned());
    }
    let mut units = Vec::new();
    for (index, line) in lines.enumerate() {
        let fields = line.split('\t').collect::<Vec<_>>();
        let directory = *fields.first().ok_or("preprocessing directory missing")?;
        let arguments = fields.get(1..).ok_or("preprocessing arguments missing")?;
        if !matches!(directory, "/work/build-lept/src" | "/work/build-tess")
            || arguments.len() > 128
            || !matches!(
                arguments.first().copied(),
                Some("/usr/bin/x86_64-linux-gnu-gcc-12" | "/usr/bin/x86_64-linux-gnu-g++-12")
            )
        {
            return Err("preprocessing compiler or directory differs".to_owned());
        }
        for arg in arguments {
            if arg.len() > 4096
                || arg.contains(['\r', '\n', '\0', '`', '$', ';', '|', '&', '<', '>'])
                || [
                    "-fplugin",
                    "-specs",
                    "--specs",
                    "--sysroot",
                    "-wrapper",
                    "-B",
                    "@",
                ]
                .iter()
                .any(|prefix| arg.starts_with(prefix))
            {
                return Err("preprocessing argument enables an unapproved route".to_owned());
            }
        }
        let outputs = arguments
            .iter()
            .enumerate()
            .filter(|(_, arg)| **arg == "-o")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let output_index = *outputs.first().ok_or("preprocessing output missing")?;
        let expected = format!("/work/preprocessed-{index}.ii");
        if outputs.len() != 1
            || arguments.get(output_index + 1).copied() != Some(expected.as_str())
            || !arguments.contains(&"-E")
            || !arguments.contains(&"-dD")
            || arguments.iter().any(|arg| matches!(*arg, "-c" | "-S"))
        {
            return Err("preprocessing output or mode differs".to_owned());
        }
        let sources = arguments
            .iter()
            .filter(|arg| arg.starts_with("/src/"))
            .copied()
            .collect::<Vec<_>>();
        if sources.len() != 1
            || sources.iter().any(|source| {
                !source.starts_with("/src/leptonica-1.87.0/src/")
                    && !source
                        .starts_with("/src/tesseract-7e548e6eff172f38f97664afe7f7ce40db36c8d6/src/")
                    || source.split('/').any(|part| part == "..")
            })
        {
            return Err("preprocessing source path differs".to_owned());
        }
        units.push(PreprocessUnit {
            directory: directory.to_owned(),
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        });
    }
    if units.len() != 334 {
        return Err("preprocessing plan must contain the exact 334 approved units".to_owned());
    }
    Ok(units)
}
fn preprocess() -> Result<(), String> {
    exact("/sys/fs/cgroup/cpu.max", "200000 100000")?;
    exact("/sys/fs/cgroup/memory.max", "4294967296")?;
    exact("/sys/fs/cgroup/memory.swap.max", "0")?;
    exact("/sys/fs/cgroup/pids.max", "64")?;
    for (index, unit) in preprocess_plan("/work/preprocess.plan")?
        .into_iter()
        .enumerate()
    {
        let output = format!("/work/preprocessed-{index}.ii");
        if std::path::Path::new(&output).exists() {
            return Err("preprocessing output must be fresh".to_owned());
        }
        let executable = unit
            .arguments
            .first()
            .ok_or("preprocessing compiler missing")?;
        let status = Command::new(executable)
            .args(unit.arguments.iter().skip(1))
            .current_dir(&unit.directory)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", "/nonexistent")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("TMPDIR", "/work/tmp")
            .env("SOURCE_DATE_EPOCH", "1793145600")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|_| "approved compiler preprocessing spawn failed")?;
        if !status.success() {
            return Err(format!("approved preprocessing failed for unit {index}"));
        }
    }
    emit("approved preprocessing batch completed")
}
fn run() -> Result<(), String> {
    match std::env::args().nth(1).as_deref() {
        Some("check-preprocess-plan") => {
            preprocess_plan(
                &std::env::args()
                    .nth(2)
                    .ok_or("preprocessing plan path missing")?,
            )?;
            emit("approved preprocessing plan validated; no process started")
        }
        Some("preprocess") => preprocess(),
        Some("baseline") => emit("safe-rust-no-write-control"),
        Some("stdin") => {
            snapshot()?;
            let mut bytes = Vec::new();
            io::stdin()
                .take(1024)
                .read_to_end(&mut bytes)
                .map_err(|_| "public stdin fixture unreadable")?;
            if bytes != b"PUBLIC_STDIN_PROOF_FIXTURE" {
                return Err("public stdin fixture differs".to_owned());
            }
            emit("public stdin proof fixture passed")
        }
        Some("snapshot") => snapshot(),
        Some("deadline") => {
            snapshot()?;
            std::thread::sleep(Duration::from_secs(300));
            Err("deadline probe was not stopped".to_owned())
        }
        Some("output") => {
            snapshot()?;
            let mut bytes = [b'x'; 8192];
            *bytes.last_mut().ok_or("public output fixture is empty")? = b'\n';
            for _ in 0..1100 {
                io::stdout()
                    .write_all(&bytes)
                    .map_err(|_| "bounded output sink closed".to_owned())?;
            }
            Ok(())
        }
        Some("oom") => {
            snapshot()?;
            let mut allocations = Vec::new();
            loop {
                let mut bytes = vec![0u8; 64 * 1024 * 1024];
                for byte in bytes.iter_mut().step_by(4096) {
                    *byte = 1;
                }
                allocations.push(bytes);
                std::hint::black_box(&allocations);
            }
        }
        Some("child") => Ok(()),
        _ => Err("usage: isolation-probe snapshot | deadline | output | oom".to_owned()),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => match writeln!(io::stdout(), "isolation-probe: {error}") {
            Ok(()) | Err(_) => ExitCode::FAILURE,
        },
    }
}
