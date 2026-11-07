// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proxy environment isolation through the same harmless test executable.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, TcpListener};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_vlm_6__proxy_environment_cannot_redirect_local_metadata() {
    let proxy = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind forbidden proxy");
    proxy
        .set_nonblocking(true)
        .expect("inspect proxy without blocking");
    let address = format!(
        "http://127.0.0.1:{}",
        proxy.local_addr().expect("proxy address").port()
    );
    let mut command = Command::new(std::env::current_exe().expect("same test executable"));
    command
        .args([
            "--exact",
            "proxy::metadata_proxy_probe",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("TALLYLINE_METADATA_PROXY_PROBE", "1")
        .env("TEMP", "D:/Coding/Agents/Temp/codex/http6")
        .env("TMP", "D:/Coding/Agents/Temp/codex/http6")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    for variable in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(variable, &address);
    }
    command.env("NO_PROXY", "").env("no_proxy", "");
    let mut child = command.spawn().expect("start isolated proxy contract");
    let deadline = Instant::now() + Duration::from_secs(15);
    while child.try_wait().expect("poll owned test child").is_none() {
        if Instant::now() >= deadline {
            child.kill().expect("stop only timed-out owned child");
            child.wait().expect("reap timed-out child");
            panic!("isolated proxy contract exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child
        .wait_with_output()
        .expect("collect owned child result");
    assert!(output.stdout.len() + output.stderr.len() <= 32 * 1024);
    assert!(
        output.status.success(),
        "isolated child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        proxy
            .accept()
            .expect_err("proxy receives no connection")
            .kind(),
        ErrorKind::WouldBlock
    );
}

#[test]
fn metadata_proxy_probe() {
    if std::env::var_os("TALLYLINE_METADATA_PROXY_PROBE").is_none() {
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread fixture runtime");
    runtime
        .block_on(super::only_the_owned_current_connection_receives_version_metadata())
        .expect("isolated admission succeeds");
}
