// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed local Docker endpoint and bounded owned-process lifecycle.

use super::{files, plan};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const DOCKER: &str = "C:/Program Files/Docker/Docker/resources/bin/docker.exe";
const WSL: &str = "C:/Windows/System32/wsl.exe";
const ENDPOINT: &str = "npipe:////./pipe/dockerDesktopLinuxEngine";
const KERNEL: &str = "6.18.33.2-microsoft-standard-WSL2";
const POLL: Duration = Duration::from_millis(25);
const CONTROL_TIMEOUT: Duration = Duration::from_secs(15);

pub(super) struct Output {
    pub(super) bytes: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) input_bytes: Option<usize>,
    pub(super) code: Option<i32>,
    pub(super) failure: Option<String>,
}

fn reap(child: &mut Child) -> Result<(), String> {
    match child.try_wait().map_err(|_| "child status unavailable")? {
        Some(_) => Ok(()),
        None => {
            child.kill().map_err(|_| "owned child termination failed")?;
            let end = Instant::now() + CONTROL_TIMEOUT;
            while Instant::now() < end {
                if child
                    .try_wait()
                    .map_err(|_| "owned child status unavailable")?
                    .is_some()
                {
                    return Ok(());
                }
                thread::sleep(POLL);
            }
            Err("owned child did not terminate by cleanup deadline".to_owned())
        }
    }
}

fn read_stream(
    mut stdout: impl Read,
    cap: usize,
    row_cap: Option<usize>,
) -> (Vec<u8>, Option<String>) {
    let mut bytes = Vec::new();
    let mut row = 0usize;
    let mut buffer = [0u8; 8192];
    loop {
        let count = match stdout.read(&mut buffer) {
            Ok(0) => return (bytes, None),
            Ok(count) => count,
            Err(_) => return (bytes, Some("owned stdout read failed".to_owned())),
        };
        for byte in buffer.iter().take(count) {
            bytes.push(*byte);
            if bytes.len() > cap {
                return (bytes, Some("stdout limit exceeded".to_owned()));
            }
            row = if *byte == b'\n' { 0 } else { row + 1 };
            if row_cap.is_some_and(|maximum| row > maximum) {
                return (bytes, Some("stdout row limit exceeded".to_owned()));
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Diagnostics {
    Null,
    SourceBuild,
}

pub(super) enum Admission {
    Permit,
    Refuse,
}
pub(super) enum Input {
    Immediate(Vec<u8>),
    AfterLocalProof {
        bytes: Vec<u8>,
        decision: mpsc::Receiver<Admission>,
    },
}
enum InputWritten {
    Complete(usize),
    Refused,
}
struct StopInput(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for StopInput {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
}
fn send_input(
    mut stdin: std::process::ChildStdin,
    input: Input,
    deadline: Instant,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<InputWritten, String> {
    let bytes = match input {
        Input::Immediate(bytes) => bytes,
        Input::AfterLocalProof { bytes, decision } => loop {
            if stop.load(std::sync::atomic::Ordering::Acquire) || Instant::now() >= deadline {
                return Ok(InputWritten::Refused);
            }
            match decision.recv_timeout(POLL) {
                Ok(Admission::Permit) => break bytes,
                Ok(Admission::Refuse) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Ok(InputWritten::Refused);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => thread::yield_now(),
            }
        },
    };
    if stop.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(InputWritten::Refused);
    }
    stdin
        .write_all(&bytes)
        .map_err(|_| "owned stdin write failed".to_owned())?;
    Ok(InputWritten::Complete(bytes.len()))
}

pub(super) fn bounded(
    mut command: Command,
    input: Input,
    cap: usize,
    row_cap: Option<usize>,
    diagnostics: Diagnostics,
    deadline: Instant,
    cancelled: &Path,
) -> Result<Output, String> {
    if cancelled.exists() || Instant::now() >= deadline {
        return Ok(Output {
            bytes: Vec::new(),
            stderr: Vec::new(),
            input_bytes: Some(0),
            code: None,
            failure: Some(
                if cancelled.exists() {
                    "operator cancellation"
                } else {
                    "wall deadline exceeded"
                }
                .to_owned(),
            ),
        });
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(match diagnostics {
            Diagnostics::Null => Stdio::null(),
            Diagnostics::SourceBuild => Stdio::piped(),
        });
    let mut child = command.spawn().map_err(|_| "owned process spawn failed")?;
    let execution = (|| {
        let stdin = child.stdin.take().ok_or("owned stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("owned stdout unavailable")?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || sender.send(read_stream(stdout, cap, row_cap)));
        let (diagnostic_sender, diagnostic_receiver) = mpsc::sync_channel(1);
        let diagnostic_reader = match diagnostics {
            Diagnostics::Null => {
                drop(diagnostic_sender);
                None
            }
            Diagnostics::SourceBuild => {
                let stderr = child
                    .stderr
                    .take()
                    .ok_or("source build stderr unavailable")?;
                Some(thread::spawn(move || {
                    diagnostic_sender.send(read_stream(stderr, 16 * 1024 * 1024, None))
                }))
            }
        };
        let mut diagnostic = if matches!(diagnostics, Diagnostics::Null) {
            Some(Vec::new())
        } else {
            None
        };
        let (write_sender, write_receiver) = mpsc::sync_channel(1);
        let input_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_guard = StopInput(input_stop.clone());
        let writer_stop = input_stop.clone();
        let awaiting_proof = matches!(&input, Input::AfterLocalProof { .. });
        let writer = thread::spawn(move || {
            write_sender.send(send_input(stdin, input, deadline, writer_stop))
        });
        let mut input_bytes = None;
        let mut refusal_deadline = None;
        let mut output = None;
        let mut written = false;
        let mut code = None;
        let mut failure = None;
        loop {
            if output.is_none() {
                match receiver.try_recv() {
                    Ok((bytes, error)) => {
                        output = Some(bytes);
                        failure = error;
                    }
                    Err(mpsc::TryRecvError::Empty) => thread::yield_now(),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        failure = Some("stdout worker disconnected".to_owned())
                    }
                }
            }
            if diagnostic.is_none() {
                match diagnostic_receiver.try_recv() {
                    Ok((bytes, error)) => {
                        diagnostic = Some(bytes);
                        if let Some(error) = error {
                            failure = Some(error.replace("stdout", "source build stderr"));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => thread::yield_now(),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        failure = Some("source build stderr worker disconnected".to_owned())
                    }
                }
            }
            if !written {
                match write_receiver.try_recv() {
                    Ok(result) => {
                        written = true;
                        match result {
                            Ok(InputWritten::Complete(count)) => input_bytes = Some(count),
                            Ok(InputWritten::Refused) => {
                                input_bytes = Some(0);
                                refusal_deadline =
                                    Some(Instant::now() + Duration::from_millis(250));
                            }
                            Err(error) => failure = Some(error),
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => thread::yield_now(),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        failure = Some("stdin worker disconnected".to_owned())
                    }
                }
            }
            if code.is_none()
                && let Some(status) = child
                    .try_wait()
                    .map_err(|_| "owned process status unavailable")?
            {
                code = Some(status.code());
            }
            if code.is_some() && !written && awaiting_proof {
                input_stop.store(true, std::sync::atomic::Ordering::Release);
            }
            if refusal_deadline.is_some_and(|end| Instant::now() >= end) {
                failure = Some("private stdin admission refused".to_owned());
            }
            if cancelled.exists() {
                failure = Some("operator cancellation".to_owned());
            }
            if Instant::now() >= deadline {
                failure = Some("wall deadline exceeded".to_owned());
            }
            if failure.is_some()
                || (code.is_some() && output.is_some() && diagnostic.is_some() && written)
            {
                break;
            }
            thread::sleep(POLL);
        }
        input_stop.store(true, std::sync::atomic::Ordering::Release);
        drop(stop_guard);
        if refusal_deadline.is_some() && failure.is_none() {
            failure = Some("private stdin admission refused".to_owned());
        }
        reap(&mut child)?;
        if output.is_none() {
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok((bytes, error)) => {
                    output = Some(bytes);
                    if failure.is_none() {
                        failure = error;
                    }
                }
                Err(_) => failure = Some("stdout drain did not terminate".to_owned()),
            }
        }
        if !written {
            match write_receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(InputWritten::Complete(count))) => input_bytes = Some(count),
                Ok(Ok(InputWritten::Refused)) => {
                    input_bytes = Some(0);
                    if failure.is_none() {
                        failure = Some("private stdin admission refused".to_owned());
                    }
                }
                Ok(Err(error)) => {
                    if failure.is_none() {
                        failure = Some(error);
                    }
                }
                Err(_) => failure = Some("stdin write did not terminate".to_owned()),
            }
        }
        if diagnostic.is_none() {
            match diagnostic_receiver.recv_timeout(Duration::from_secs(1)) {
                Ok((bytes, error)) => {
                    diagnostic = Some(bytes);
                    if failure.is_none() {
                        failure = error.map(|error| error.replace("stdout", "source build stderr"));
                    }
                }
                Err(_) => failure = Some("source build stderr drain did not terminate".to_owned()),
            }
        }
        if let Some(reader) = diagnostic_reader {
            if reader.is_finished() {
                reader
                    .join()
                    .map_err(|_| "source build stderr worker panicked")?
                    .map_err(|_| "source build stderr receiver unavailable")?;
            } else {
                failure = Some("source build stderr worker shutdown incomplete".to_owned());
            }
        }
        if reader.is_finished() {
            reader
                .join()
                .map_err(|_| "stdout worker panicked")?
                .map_err(|_| "stdout result receiver unavailable")?;
        } else {
            failure = Some("stdout worker shutdown incomplete".to_owned());
        }
        if writer.is_finished() {
            writer
                .join()
                .map_err(|_| "stdin worker panicked")?
                .map_err(|_| "stdin result receiver unavailable")?;
        } else {
            failure = Some("stdin worker shutdown incomplete".to_owned());
        }
        Ok(Output {
            bytes: output.unwrap_or_default(),
            stderr: diagnostic.unwrap_or_default(),
            input_bytes,
            code: code.flatten(),
            failure,
        })
    })();
    let cleanup = reap(&mut child);
    cleanup?;
    execution
}

fn clean_command(executable: &str) -> Command {
    let mut command = Command::new(executable);
    command
        .env_clear()
        .env("SystemRoot", "C:/Windows")
        .env("WINDIR", "C:/Windows")
        .env("PATH", "C:/Windows/System32");
    command
}

pub(super) struct Docker {
    owned: PathBuf,
}

impl Docker {
    pub(super) fn new(owned: &Path) -> Result<Self, String> {
        if !cfg!(windows) {
            return Err(
                "the reviewed research controller host is Windows with local Docker Desktop Linux"
                    .to_owned(),
            );
        }
        let config = owned.join("docker-config");
        if files::read(&config.join("config.json"), 2)? != b"{}"
            || fs::read_dir(&config)
                .map_err(|_| "owned Docker config unavailable")?
                .count()
                != 1
        {
            return Err("Docker configuration must remain empty and owned".to_owned());
        }
        let docker = Self {
            owned: owned.to_owned(),
        };
        let info = docker.control(&["info", "--format", "{{json .}}"], 1024 * 1024)?;
        let info: Value = serde_json::from_slice(&info).map_err(|_| "Docker info invalid")?;
        if info.get("OSType").and_then(Value::as_str) != Some("linux")
            || info.get("Architecture").and_then(Value::as_str) != Some("x86_64")
            || info.get("CgroupDriver").and_then(Value::as_str) != Some("cgroupfs")
            || info.get("CgroupVersion").and_then(Value::as_str) != Some("2")
            || info.get("KernelVersion").and_then(Value::as_str) != Some(KERNEL)
        {
            return Err(
                "Docker engine does not match the reviewed local Linux environment".to_owned(),
            );
        }
        let kernel = docker.wsl(&["/bin/cat", "/proc/sys/kernel/osrelease"])?;
        if kernel.trim() != KERNEL {
            return Err("local WSL kernel does not corroborate Docker engine".to_owned());
        }
        let image: Value = serde_json::from_slice(
            &docker.control(&["image", "inspect", plan::IMAGE], 1024 * 1024)?,
        )
        .map_err(|_| "image metadata invalid")?;
        if image.pointer("/0/Id").and_then(Value::as_str)
            != Some("sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6")
        {
            return Err("installed image ID differs from exact approval".to_owned());
        }
        Ok(docker)
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = clean_command(DOCKER);
        command
            .arg("--host")
            .arg(ENDPOINT)
            .arg("--config")
            .arg(self.owned.join("docker-config"))
            .args(args);
        command
    }
    pub(super) fn control(&self, args: &[&str], cap: usize) -> Result<Vec<u8>, String> {
        let result = bounded(
            self.command(args),
            Input::Immediate(Vec::new()),
            cap,
            None,
            Diagnostics::Null,
            Instant::now() + CONTROL_TIMEOUT,
            &self.owned.join("control-cancel-never-created"),
        )?;
        if result.failure.is_some() || result.code != Some(0) {
            return Err(format!(
                "Docker control failed: {}",
                args.first().copied().unwrap_or("unknown")
            ));
        }
        Ok(result.bytes)
    }
    fn wsl(&self, args: &[&str]) -> Result<String, String> {
        let mut command = clean_command(WSL);
        command
            .args(["--distribution", "docker-desktop", "--exec"])
            .args(args);
        let result = bounded(
            command,
            Input::Immediate(Vec::new()),
            1024 * 1024,
            None,
            Diagnostics::Null,
            Instant::now() + CONTROL_TIMEOUT,
            &self.owned.join("control-cancel-never-created"),
        )?;
        if result.failure.is_some() || result.code != Some(0) {
            return Err("local WSL metadata unavailable".to_owned());
        }
        String::from_utf8(result.bytes).map_err(|_| "local WSL metadata is not UTF-8".to_owned())
    }
    pub(super) fn run(
        &self,
        spec: &Spec,
        input: Vec<u8>,
        deadline: Instant,
        cap: usize,
    ) -> Result<Attempt, String> {
        if self.owned.join("CANCEL").exists() || Instant::now() >= deadline {
            return Err(
                "attempt cancelled or deadline elapsed before container creation".to_owned(),
            );
        }
        let arguments = spec.arguments()?;
        let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
        let id = String::from_utf8(self.control(&arguments, 128)?)
            .map_err(|_| "invalid owned container ID")?
            .trim()
            .to_owned();
        if id.len() != 64
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("Docker did not return an exact container ID".to_owned());
        }
        let owned = Owned {
            docker: self,
            id,
            removed: std::cell::Cell::new(false),
        };
        let before = owned.inspect()?;
        spec.validate(&before)?;
        let before_diff = self.control(&["diff", &owned.id], 1024 * 1024)?;

        let mut attach = self.command(&["start", "--attach", "--interactive", &owned.id]);
        attach.current_dir(&self.owned);
        let (sample_tx, sample_rx) = mpsc::channel();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sample_stop = stop.clone();
        let id_copy = owned.id.clone();
        let directory = self.owned.clone();
        let memory = spec.memory;
        let pids = if spec.kind.runtime() { 1 } else { 64 };
        let requires_proof = spec.kind == Kind::Runtime && !input.is_empty();
        let (admission_tx, admission_rx) = mpsc::sync_channel(1);
        let (proof_tx, proof_rx) = mpsc::sync_channel(1);
        let mut admission_sender = if requires_proof {
            Some(admission_tx)
        } else {
            None
        };
        let sampler = thread::spawn(move || {
            let docker = Self { owned: directory };
            let mut evidence = json!({"state":"unknown","reason":"owned cgroup not yet available"});
            while Instant::now() < deadline
                && !sample_stop.load(std::sync::atomic::Ordering::Acquire)
            {
                match docker.sample(&id_copy, memory, pids, false) {
                    Ok(sample) => {
                        if sample.get("sample_population").and_then(Value::as_u64) == Some(1)
                            && let Err(error) = decide_input(
                                &mut admission_sender,
                                &proof_tx,
                                Admission::Permit,
                                json!({"state":"admitted","owned_cgroup":sample}),
                            )
                        {
                            if sample_tx
                                .send(json!({"state":"unknown","reason":error}))
                                .is_err()
                            {
                                return;
                            }
                            break;
                        }
                        evidence = sample;
                    }
                    Err(error) => {
                        if !matches!(
                            error.as_str(),
                            "local WSL metadata unavailable"
                                | "owned container has not entered running state"
                                | "owned cgroup has no live population"
                                | "owned cgroup population changed during snapshot"
                        ) && let Err(admission_error) = decide_input(
                            &mut admission_sender,
                            &proof_tx,
                            Admission::Refuse,
                            json!({"state":"refused","reason":error}),
                        ) {
                            if sample_tx
                                .send(json!({"state":"unknown","reason":admission_error}))
                                .is_err()
                            {
                                return;
                            }
                            break;
                        }
                        if evidence.get("state").and_then(Value::as_str) != Some("measured") {
                            evidence = json!({"state":"unknown","reason":error});
                        }
                    }
                }
                if sample_tx.send(evidence.clone()).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        let input = if requires_proof {
            Input::AfterLocalProof {
                bytes: input,
                decision: admission_rx,
            }
        } else {
            Input::Immediate(input)
        };
        let execution = bounded(
            attach,
            input,
            cap,
            if spec.kind.runtime() {
                Some(1024 * 1024)
            } else {
                None
            },
            if spec.kind.runtime() {
                Diagnostics::Null
            } else {
                Diagnostics::SourceBuild
            },
            deadline,
            &self.owned.join("CANCEL"),
        );
        let kill = self.control(&["kill", &owned.id], 128);
        let wait = self.control(&["wait", &owned.id], 128);
        let after = owned.inspect();
        stop.store(true, std::sync::atomic::Ordering::Release);
        let mut resource = json!({"state":"unknown","reason":"no live owned cgroup sample"});
        while let Ok(sample) = sample_rx.try_recv() {
            resource = sample;
        }

        let sample_end = Instant::now() + Duration::from_secs(50);
        while !sampler.is_finished() && Instant::now() < sample_end {
            thread::sleep(POLL);
        }
        if !sampler.is_finished() {
            return Err("owned resource sampler exceeded its shutdown deadline".to_owned());
        }
        sampler.join().map_err(|_| "resource sampler panicked")?;
        while let Ok(sample) = sample_rx.try_recv() {
            resource = sample;
        }
        drop(sample_rx);
        match self.sample(&owned.id, memory, pids, true) {
            Ok(final_sample) => resource = final_sample,
            Err(error) => {
                resource
                    .as_object_mut()
                    .ok_or("resource receipt invalid")?
                    .insert("final_read_failure".to_owned(), json!(error));
            }
        }
        let diff = self.control(&["diff", &owned.id], 1024 * 1024);
        let removed = owned.remove();
        let mut output = execution?;
        wait?;
        let after = after?;
        let state = after
            .get("State")
            .ok_or("container final state unavailable")?;
        if kill.is_err() && state.get("Running") != Some(&Value::Bool(false)) {
            return Err("owned container termination unverified".to_owned());
        }
        let diff = diff?;
        if spec.kind == Kind::Runtime
            && normalise_diff(&diff)?
                != normalise_diff(&files::read(
                    &self.owned.join("runtime-startup-diff.txt"),
                    1024 * 1024,
                )?)?
        {
            output.failure = Some("runtime changed the read-only filesystem".to_owned());
        }
        removed?;
        let admission_proof = if requires_proof {
            proof_rx
                .try_recv()
                .unwrap_or_else(|_| json!({"state":"not-established"}))
        } else {
            json!({"state":"not-required"})
        };
        let receipt = json!({"stdin_admission":admission_proof,"input_bytes_written":output.input_bytes,"container_id":owned.id,"configuration":before,"final_state":state,"resource":resource,"elapsed_millis":spec.started.elapsed().as_millis(),"removed":true,"filesystem_diff":String::from_utf8_lossy(&diff),"filesystem_diff_before_start":String::from_utf8_lossy(&before_diff),"failure":output.failure,"stdout_bytes":output.bytes.len(),"source_build_stderr_bytes":output.stderr.len(),"stderr_policy":if spec.kind.runtime(){"os-null"}else{"bounded-source-build"}});
        files::write_json(
            &self
                .owned
                .join("evidence")
                .join(format!("container-{}.json", owned.id)),
            &receipt,
        )?;
        Ok(Attempt { output, receipt })
    }
    fn inspect(&self, id: &str) -> Result<Value, String> {
        let raw = self.control(&["inspect", id], 1024 * 1024)?;
        let value: Value = serde_json::from_slice(&raw).map_err(|_| "owned inspection invalid")?;
        value
            .get(0)
            .cloned()
            .ok_or("owned container inspection missing".to_owned())
    }
    fn sample(&self, id: &str, memory: u64, pids: u64, final_read: bool) -> Result<Value, String> {
        let before = if final_read {
            None
        } else {
            Some(running_state(&self.inspect(id)?)?)
        };
        let base = format!("/sys/fs/cgroup/docker/{id}");
        let paths = [
            "cgroup.events",
            "cpu.max",
            "memory.max",
            "memory.swap.max",
            "pids.max",
            "cgroup.procs",
            "memory.peak",
            "memory.events",
            "cgroup.events",
        ]
        .map(|name| format!("{base}/{name}"));
        let mut arguments = vec!["/bin/cat"];
        arguments.extend(paths.iter().map(String::as_str));
        let mut sample = parse_cgroup(&self.wsl(&arguments)?, memory, pids, final_read)?;
        if let Some(before) = before {
            let after = running_state(&self.inspect(id)?)?;
            let evidence = sample
                .as_object_mut()
                .ok_or("owned cgroup evidence invalid")?;
            evidence.insert("engine_running_before".to_owned(), before);
            evidence.insert("engine_running_after".to_owned(), after);
        }
        Ok(sample)
    }
}

fn parse_cgroup(text: &str, memory: u64, pids: u64, final_read: bool) -> Result<Value, String> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut cursor = 0;
    let mut populated = None;
    let mut frozen = false;
    while let Some(line) = lines.get(cursor) {
        if let Some(value) = line.strip_prefix("populated ") {
            if populated.is_some() || !matches!(value, "0" | "1") {
                return Err("owned cgroup population metadata invalid".to_owned());
            }
            populated = Some(value == "1");
            cursor += 1;
        } else if let Some(value) = line.strip_prefix("frozen ") {
            if value != "0" || frozen {
                return Err("owned cgroup is unexpectedly frozen".to_owned());
            }
            frozen = true;
            cursor += 1;
        } else {
            break;
        }
    }
    let populated = populated.ok_or("owned cgroup population metadata missing")?;
    if !final_read && !populated {
        return Err("owned cgroup has no live population".to_owned());
    }
    let limit_cursor = cursor;
    cursor += 4;
    let mut numeric = Vec::new();
    while let Some(line) = lines.get(cursor) {
        if line.bytes().all(|byte| byte.is_ascii_digit()) && !line.is_empty() {
            numeric.push(
                line.parse::<u64>()
                    .map_err(|_| "owned cgroup numeric metadata invalid")?,
            );
            cursor += 1;
        } else {
            break;
        }
    }
    let peak = numeric.pop().ok_or("owned cgroup memory.peak missing")?;
    let mut seen = std::collections::BTreeSet::new();
    if numeric.len() as u64 > pids
        || numeric.iter().any(|pid| *pid == 0 || !seen.insert(*pid))
        || populated == numeric.is_empty()
        || final_read && populated
    {
        return Err("owned cgroup population is inconsistent or not final".to_owned());
    }
    let mut events = serde_json::Map::new();
    for line in lines.iter().skip(cursor) {
        if line.starts_with("populated ") || line.starts_with("frozen ") {
            break;
        }
        cursor += 1;
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let [key, count] = fields.as_slice() else {
            return Err("invalid memory.events metadata".to_owned());
        };
        if !matches!(
            *key,
            "low" | "high" | "max" | "oom" | "oom_kill" | "oom_group_kill" | "sock_throttled"
        ) || events
            .insert(
                (*key).to_owned(),
                json!(
                    count
                        .parse::<u64>()
                        .map_err(|_| "invalid memory event count")?
                ),
            )
            .is_some()
        {
            return Err("unknown or repeated memory.events field".to_owned());
        }
    }
    let second_populated = lines
        .get(cursor)
        .and_then(|line| line.strip_prefix("populated "))
        .ok_or("owned cgroup population recheck missing")?;
    if !matches!(second_populated, "0" | "1") {
        return Err("owned cgroup population recheck invalid".to_owned());
    }
    if (second_populated == "1") != populated {
        return Err("owned cgroup population changed during snapshot".to_owned());
    }
    cursor += 1;
    if lines
        .get(cursor)
        .is_some_and(|line| line.starts_with("frozen "))
    {
        if lines.get(cursor) != Some(&"frozen 0") {
            return Err("owned cgroup unexpectedly froze during snapshot".to_owned());
        }
        cursor += 1;
    }
    if cursor != lines.len() {
        return Err("owned cgroup has unexpected trailing metadata".to_owned());
    }
    if lines.get(limit_cursor) != Some(&"200000 100000")
        || lines.get(limit_cursor + 1) != Some(&memory.to_string().as_str())
        || lines.get(limit_cursor + 2) != Some(&"0")
        || lines.get(limit_cursor + 3) != Some(&pids.to_string().as_str())
    {
        return Err(format!(
            "owned cgroup hard limits differ: cpu={:?}, memory={:?}, swap={:?}, pids={:?}",
            lines.get(limit_cursor),
            lines.get(limit_cursor + 1),
            lines.get(limit_cursor + 2),
            lines.get(limit_cursor + 3)
        ));
    }

    if ["low", "high", "max", "oom", "oom_kill", "oom_group_kill"]
        .iter()
        .any(|key| !events.contains_key(*key))
    {
        return Err("owned memory.events fields missing".to_owned());
    }
    let evidence = json!({"state":"measured","observed_memory_peak_bytes":peak,"whole_attempt_peak_known":false,"memory_events":events,"sample_population":numeric.len(),"kernel_hard_limits":{"cpu_max":"200000 100000","memory_max":memory,"memory_swap_max":0,"pids_max":pids},"origin":"read-only exact returned-ID cgroupfs path in corroborated local WSL namespace","sample_stage":if final_read{"post-exit-read-attempt"}else{"live"},"final_sample":false});
    Ok(evidence)
}
struct Owned<'a> {
    docker: &'a Docker,
    id: String,
    removed: std::cell::Cell<bool>,
}
impl Owned<'_> {
    fn inspect(&self) -> Result<Value, String> {
        let raw = self.docker.control(&["inspect", &self.id], 1024 * 1024)?;
        let value: Value =
            serde_json::from_slice(&raw).map_err(|_| "container inspection invalid")?;
        value
            .get(0)
            .cloned()
            .ok_or("owned container missing".to_owned())
    }
    fn remove(&self) -> Result<(), String> {
        self.docker.control(&["rm", &self.id], 128)?;
        self.removed.set(true);
        Ok(())
    }
}
impl Drop for Owned<'_> {
    fn drop(&mut self) {
        if self.removed.get() {
            return;
        }
        let kill = self.docker.control(&["kill", &self.id], 128);
        let wait = self.docker.control(&["wait", &self.id], 128);
        let removed = self.docker.control(&["rm", &self.id], 128);
        let evidence = json!({"owned_container_id":self.id,"fallback_cleanup":true,"kill_succeeded":kill.is_ok(),"wait_succeeded":wait.is_ok(),"removed":removed.is_ok()});
        match files::write_json(
            &self
                .docker
                .owned
                .join("evidence")
                .join(format!("cleanup-{}.json", self.id)),
            &evidence,
        ) {
            Ok(()) => {
                self.removed.set(removed.is_ok());
            }
            Err(error) => {
                let result = writeln!(
                    std::io::stderr(),
                    "tesseract-research: cleanup evidence could not be retained: {error}"
                );
                if result.is_err() {
                    self.removed.set(false);
                }
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Build,
    ProbeBuild,
    ControlRuntime,
    Runtime,
}
impl Kind {
    fn runtime(self) -> bool {
        matches!(self, Self::ControlRuntime | Self::Runtime)
    }
}
pub(super) struct Spec {
    pub(super) kind: Kind,
    pub(super) memory: u64,
    pub(super) mounts: Vec<(PathBuf, &'static str, bool)>,
    pub(super) argv: Vec<String>,
    pub(super) started: Instant,
    pub(super) workdir: &'static str,
}
pub(super) struct Attempt {
    pub(super) output: Output,
    pub(super) receipt: Value,
}
impl Spec {
    fn arguments(&self) -> Result<Vec<String>, String> {
        if !matches!(self.memory, 4_294_967_296 | 8_589_934_592) || self.argv.is_empty() {
            return Err("invalid fixed container profile".to_owned());
        }
        let mut args = [
            "create",
            "--pull",
            "never",
            "--network",
            "none",
            "--cpus",
            "2",
            "--pids-limit",
            if self.kind.runtime() { "1" } else { "64" },
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--ipc",
            "none",
            "--user",
            "65534:65534",
            "--log-driver",
            "none",
            "--interactive",
            "--workdir",
            self.workdir,
            "--entrypoint",
            "/usr/bin/env",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        if self.kind.runtime() {
            args.extend(
                ["--ulimit", "core=0:0", "--ulimit", "msgqueue=0:0"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        for option in ["--memory", "--memory-swap"] {
            args.extend([option.to_owned(), self.memory.to_string()]);
        }
        for (source, target, writable) in &self.mounts {
            files::reject_aliases(source)?;
            let source = fs::canonicalize(source).map_err(|_| "container mount unavailable")?;
            let source = source
                .to_str()
                .ok_or("container mount is not UTF-8")?
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");
            if source.contains(',') || self.kind.runtime() && *writable {
                return Err("container mount violates read-only policy".to_owned());
            }
            args.extend([
                "--mount".to_owned(),
                format!(
                    "type=bind,source={source},target={target}{}",
                    if *writable { "" } else { ",readonly" }
                ),
            ]);
        }
        args.push(plan::IMAGE.to_owned());
        args.extend(
            [
                "-i",
                if self.kind.runtime() {
                    "PATH=/worker"
                } else {
                    "PATH=/usr/bin:/bin"
                },
                "LANG=C",
                "LC_ALL=C",
                "HOME=/nonexistent",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        if !self.kind.runtime() {
            args.extend(
                ["TMPDIR=/work/tmp", "SOURCE_DATE_EPOCH=1793145600"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        if self.kind == Kind::ProbeBuild {
            args.extend(
                [
                    "RUSTUP_HOME=/usr/local/rustup",
                    "RUSTUP_TOOLCHAIN=1.99.0",
                    "RUSTUP_AUTO_INSTALL=0",
                    "CARGO_HOME=/work/cargo",
                    "PATH=/usr/local/cargo/bin:/usr/bin:/bin",
                ]
                .into_iter()
                .map(str::to_owned),
            );
        }
        args.extend(self.argv.clone());
        Ok(args)
    }
    fn validate(&self, value: &Value) -> Result<(), String> {
        let host = value.get("HostConfig").ok_or("HostConfig missing")?;
        if host.get("CgroupParent").and_then(Value::as_str) != Some("") {
            return Err("custom cgroup parent is forbidden".to_owned());
        }
        for (key, expected) in [
            ("NetworkMode", json!("none")),
            ("IpcMode", json!("none")),
            ("ReadonlyRootfs", json!(true)),
            ("PidsLimit", json!(if self.kind.runtime() { 1 } else { 64 })),
            ("Memory", json!(self.memory)),
            ("MemorySwap", json!(self.memory)),
            ("NanoCpus", json!(2_000_000_000u64)),
            ("Privileged", json!(false)),
        ] {
            if host.get(key) != Some(&expected) {
                return Err(format!("container policy mismatch: {key}"));
            }
        }
        if host.pointer("/LogConfig/Type").and_then(Value::as_str) != Some("none")
            || host.get("CapDrop") != Some(&json!(["ALL"]))
            || host.get("SecurityOpt") != Some(&json!(["no-new-privileges"]))
            || host.get("Init").is_some_and(|value| value == &json!(true))
            || host
                .get("DeviceRequests")
                .and_then(Value::as_array)
                .is_some_and(|value| !value.is_empty())
            || host
                .get("Devices")
                .and_then(Value::as_array)
                .is_some_and(|value| !value.is_empty())
            || host
                .get("PortBindings")
                .and_then(Value::as_object)
                .is_some_and(|value| !value.is_empty())
            || host
                .get("Tmpfs")
                .and_then(Value::as_object)
                .is_some_and(|value| !value.is_empty())
        {
            return Err(
                "unexpected Docker privileges, log, device, port or tmpfs settings".to_owned(),
            );
        }
        if value.pointer("/Config/WorkingDir").and_then(Value::as_str) != Some(self.workdir) {
            return Err("container working directory differs".to_owned());
        }
        if value.pointer("/Config/User").and_then(Value::as_str) != Some("65534:65534")
            || value.pointer("/Config/Image").and_then(Value::as_str) != Some(plan::IMAGE)
            || value.pointer("/Config/Entrypoint") != Some(&json!(["/usr/bin/env"]))
        {
            return Err("container execution identity mismatch".to_owned());
        }
        if self.kind.runtime() {
            let limits = host
                .get("Ulimits")
                .and_then(Value::as_array)
                .ok_or("runtime ulimits missing")?;
            if limits.len() != 2
                || ["core", "msgqueue"].iter().any(|name| {
                    !limits.iter().any(|limit| {
                        limit.get("Name").and_then(Value::as_str) == Some(*name)
                            && limit.get("Hard") == Some(&json!(0))
                            && limit.get("Soft") == Some(&json!(0))
                    })
                })
            {
                return Err("stricter runtime ulimits differ".to_owned());
            }
        }
        let expected = self.arguments()?;
        let image_index = expected
            .iter()
            .position(|argument| argument == plan::IMAGE)
            .ok_or("fixed image argument missing")?;
        if value.get("Args")
            != Some(&json!(
                expected
                    .get(image_index + 1..)
                    .ok_or("fixed child arguments missing")?
            ))
        {
            return Err("container child argv or cleared environment differs".to_owned());
        }
        let mounts = value
            .get("Mounts")
            .and_then(Value::as_array)
            .ok_or("container mounts missing")?;
        if mounts.len() != self.mounts.len() {
            return Err("unexpected container mount".to_owned());
        }
        for (source, destination, writable) in &self.mounts {
            let mount = mounts
                .iter()
                .find(|mount| {
                    mount.get("Destination").and_then(Value::as_str) == Some(*destination)
                })
                .ok_or("required mount missing")?;
            let expected_source = fs::canonicalize(source)
                .map_err(|_| "mount source unavailable")?
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");
            if mount.get("Source").and_then(Value::as_str) != Some(expected_source.as_str()) {
                return Err("actual mount source differs".to_owned());
            }
            if mount.get("RW") != Some(&json!(writable))
                || mount.get("Type").and_then(Value::as_str) != Some("bind")
            {
                return Err("mount type or read-only policy mismatch".to_owned());
            }
        }
        Ok(())
    }
}

pub(super) fn normalise_diff(bytes: &[u8]) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "container diff is not UTF-8")?;
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    lines.sort();
    if lines.windows(2).any(|pair| pair.first() == pair.get(1)) {
        return Err("container diff contains duplicate metadata".to_owned());
    }
    Ok(lines)
}

pub(super) fn recorded_resource(path: &Path, profile: &str) -> Result<Value, String> {
    let memory = match profile {
        "4g" => 4_294_967_296,
        "8g" => 8_589_934_592,
        _ => return Err("unknown resource profile".to_owned()),
    };
    let bytes = files::read(path, 1024 * 1024)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| "recorded resource metadata is not UTF-8")?;
    parse_cgroup(text, memory, 1, false)
}

fn decide_input(
    sender: &mut Option<mpsc::SyncSender<Admission>>,
    proof: &mpsc::SyncSender<Value>,
    decision: Admission,
    evidence: Value,
) -> Result<(), String> {
    let Some(sender) = sender.take() else {
        return Ok(());
    };
    sender
        .send(decision)
        .map_err(|_| "stdin admission receiver unavailable")?;
    proof
        .send(evidence)
        .map_err(|_| "stdin admission proof receiver unavailable".to_owned())
}

fn running_state(value: &Value) -> Result<Value, String> {
    let state = value.get("State").ok_or("owned container state missing")?;
    match state.get("Status").and_then(Value::as_str) {
        Some("created") => Err("owned container has not entered running state".to_owned()),
        Some("running") => {
            if state.get("Running") != Some(&json!(true))
                || ["Paused", "Restarting", "Dead"]
                    .iter()
                    .any(|name| state.get(name) != Some(&json!(false)))
            {
                return Err("owned container running state is inconsistent".to_owned());
            }
            Ok(state.clone())
        }
        Some("exited" | "dead" | "removing" | "restarting" | "paused") => {
            Err("owned container is not an active permitted worker".to_owned())
        }
        Some(_) | None => Err("owned container status is unknown".to_owned()),
    }
}
pub(super) fn recorded_resource_state(
    path: &Path,
    profile: &str,
    state: &Path,
) -> Result<Value, String> {
    running_state(&plan::json(state)?)?;
    recorded_resource(path, profile)
}
