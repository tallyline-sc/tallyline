// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-process contracts for writer ownership and local activation.

use crate::api::{Election, InstancePaths, elect};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn paths(root: &Path, mode: &str) -> InstancePaths {
    InstancePaths::new(
        root.join(mode).join("instance.lock"),
        root.join("user/instance.lock"),
        root.join("user/run/a.sock"),
    )
}

struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        if self.0.try_wait().unwrap().is_none() {
            self.0.kill().unwrap();
            self.0.wait().unwrap();
        }
    }
}

fn start(root: &Path, mode: &str) -> Process {
    Process(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "contract_tests::subprocess_probe", "--ignored"])
            .env("TALLYLINE_TEST_INSTANCE_ROOT", root)
            .env("TALLYLINE_TEST_INSTANCE_MODE", mode)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "child readiness/result missing: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn publish_probe_result(
    path: &Path,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let pending = path.with_extension("pending");
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    write(&mut file)?;
    drop(file);
    std::fs::rename(pending, path)
}

#[test]
#[ignore = "Private subprocess entrypoint invoked by the Cargo-run acceptance tests."]
fn subprocess_probe() {
    let root = PathBuf::from(std::env::var_os("TALLYLINE_TEST_INSTANCE_ROOT").unwrap());
    let mode = std::env::var("TALLYLINE_TEST_INSTANCE_MODE").unwrap();
    if mode == "partial-result" {
        publish_probe_result(&root.join("secondary"), |file| {
            file.write_all(b"Act")?;
            std::fs::write(root.join("partial-ready"), b"")?;
            wait_for(&root.join("finish-write"));
            file.write_all(b"ivated")
        })
        .unwrap();
        return;
    }
    if mode == "drip" {
        let client = crate::socket::LocalSocket::new().unwrap();
        client
            .connect(
                &socket2::SockAddr::unix(root.join("user/run/a.sock")).unwrap(),
                Duration::from_millis(250),
            )
            .unwrap();
        for byte in b"activate" {
            match client.write(&[*byte]) {
                Ok(1) => (),
                Ok(count) => panic!("unexpected one-byte fixture write: {count}"),
                Err(error) => {
                    std::fs::write(root.join("drip-done"), format!("{:?}", error.kind())).unwrap();
                    return;
                }
            }
            std::fs::write(root.join("dripping"), "started").unwrap();
            std::thread::sleep(Duration::from_millis(80));
        }
        std::fs::write(root.join("drip-done"), "all bytes sent").unwrap();
        return;
    }
    match elect(paths(&root, &mode)).unwrap() {
        Election::Primary(mut primary) => {
            std::fs::write(root.join("ready"), mode).unwrap();
            loop {
                if primary.poll_activation().unwrap() {
                    std::fs::write(root.join("activated"), "activate").unwrap();
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        Election::Secondary(outcome) => {
            publish_probe_result(&root.join("secondary"), |file| {
                file.write_all(format!("{outcome:?}").as_bytes())
            })
            .unwrap();
        }
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__subprocess_results_are_visible_only_after_complete_writes() {
    let root = tempfile::tempdir().unwrap();
    let mut child = start(root.path(), "partial-result");
    wait_for(&root.path().join("partial-ready"));
    assert!(!root.path().join("secondary").exists());
    std::fs::write(root.path().join("finish-write"), b"").unwrap();
    wait_for(&root.path().join("secondary"));
    assert_eq!(
        std::fs::read_to_string(root.path().join("secondary")).unwrap(),
        "Activated"
    );
    assert!(child.0.wait().unwrap().success());
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__installed_and_portable_contend_for_one_user() {
    let root = std::env::temp_dir().join(format!("tl95-pair-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let primary = start(&root, "installed");
    wait_for(&root.join("ready"));
    let mut secondary = start(&root, "portable");
    wait_for(&root.join("secondary"));
    assert_eq!(
        std::fs::read_to_string(root.join("secondary")).unwrap(),
        "Activated"
    );
    assert!(secondary.0.wait().unwrap().success());
    wait_for(&root.join("activated"));
    assert!(!root.join("portable/instance.lock").exists());
    drop(primary);
    drop(secondary);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__portable_primary_activates_from_installed_and_recovers_after_crash() {
    let root = std::env::temp_dir().join(format!("tl95-crash-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let primary = start(&root, "portable");
    wait_for(&root.join("ready"));
    let mut secondary = start(&root, "installed");
    wait_for(&root.join("secondary"));
    assert_eq!(
        std::fs::read_to_string(root.join("secondary")).unwrap(),
        "Activated"
    );
    assert!(secondary.0.wait().unwrap().success());
    drop(primary);
    drop(secondary);
    assert!(root.join("user/run/a.sock").exists());
    let recovered = elect(paths(&root, "installed")).unwrap();
    assert!(matches!(recovered, Election::Primary(_)));
    drop(recovered);
    assert!(!root.join("user/run/a.sock").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__missing_listener_never_grants_a_second_writer() {
    let root = std::env::temp_dir().join(format!("tl95-missing-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("user")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("user"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
    }
    let lock = std::fs::File::create(root.join("user/instance.lock")).unwrap();
    lock.try_lock().unwrap();
    let started = Instant::now();
    assert!(matches!(
        elect(paths(&root, "portable")).unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(!root.join("portable").exists());
    drop(lock);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__malformed_oversized_and_unfinished_tokens_do_not_activate() {
    use crate::socket::LocalSocket;
    let root = std::env::temp_dir().join(format!("tl95-frame-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let Election::Primary(mut primary) = elect(paths(&root, "installed")).unwrap() else {
        panic!("first election must win");
    };
    for payload in [
        b"".as_slice(),
        b"activat",
        b"activateX",
        b"activate\n",
        b"\xffctivate",
        b"activateactivate",
    ] {
        let client = LocalSocket::new().unwrap();
        client
            .connect(
                &socket2::SockAddr::unix(root.join("user/run/a.sock")).unwrap(),
                Duration::from_millis(250),
            )
            .unwrap();
        if !payload.is_empty() {
            assert_eq!(client.write(payload).unwrap(), payload.len());
        }
        client.finish_request().unwrap();
        assert!(
            !primary.poll_activation().unwrap(),
            "invalid frame must not activate"
        );
    }
    let unfinished = LocalSocket::new().unwrap();
    unfinished
        .connect(
            &socket2::SockAddr::unix(root.join("user/run/a.sock")).unwrap(),
            Duration::from_millis(250),
        )
        .unwrap();
    assert_eq!(unfinished.write(b"activate").unwrap(), 8);
    let started = Instant::now();
    assert!(!primary.poll_activation().unwrap());
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(unfinished);
    drop(primary);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__silent_listener_is_bounded_and_never_opens_portable_data() {
    let root = std::env::temp_dir().join(format!("tl95-silent-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let primary = elect(paths(&root, "installed")).unwrap();
    let started = Instant::now();
    assert!(matches!(
        elect(paths(&root, "portable")).unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(!root.join("portable").exists());
    drop(primary);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__windows_profile_election_does_not_follow_portable_data() {
    let root = std::env::temp_dir().join(format!("tl95-mapping-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let primary = elect(InstancePaths::windows(&root.join("installed"), &root)).unwrap();
    assert!(root.join("Tallyline/instance.lock").exists());
    assert!(root.join("Tallyline/run/a").exists());
    assert!(matches!(
        elect(InstancePaths::windows(&root.join("portable"), &root)).unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    assert!(!root.join("portable").exists());
    drop(primary);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__writer_lease_survives_listener_exit() {
    let root = std::env::temp_dir().join(format!("tl95-lease-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let Election::Primary(primary) = elect(paths(&root, "installed")).unwrap() else {
        panic!("first election must win");
    };
    let (lease, listener) = primary.into_parts();
    drop(listener);
    assert!(matches!(
        elect(paths(&root, "portable")).unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    assert!(!root.join("portable").exists());
    drop(lease);
    let next = elect(paths(&root, "portable")).unwrap();
    assert!(matches!(next, Election::Primary(_)));
    drop(next);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__linux_runtime_and_private_state_fallback_keep_portable_lock_fixed() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("tl95-xdg-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let runtime = root.join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let primary = elect(InstancePaths::linux(
        &root.join("installed"),
        &root.join("state"),
        Some(&runtime),
    ))
    .unwrap();
    assert!(runtime.join("tallyline.lock").exists());
    assert!(runtime.join("tl-a").exists());
    assert!(matches!(
        elect(InstancePaths::linux(
            &root.join("portable"),
            &root.join("state"),
            Some(&runtime)
        ))
        .unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    drop(primary);
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755)).unwrap();
    let fallback = elect(InstancePaths::linux(
        &root.join("installed"),
        &root.join("state"),
        Some(&runtime),
    ))
    .unwrap();
    assert!(root.join("state/tallyline/instance.lock").exists());
    assert!(root.join("state/tallyline/run/a").exists());
    drop(fallback);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__backlog_pressure_keeps_failed_activation_bounded() {
    let root = std::env::temp_dir().join(format!("tl95-backlog-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let primary = elect(paths(&root, "installed")).unwrap();
    let address = socket2::SockAddr::unix(root.join("user/run/a.sock")).unwrap();
    let mut peers = Vec::new();
    let mut pressure = None;
    for _ in 0..256 {
        let client = crate::socket::LocalSocket::new().unwrap();
        match client.connect(&address, Duration::from_millis(50)) {
            Ok(()) => peers.push(client),
            Err(error) => {
                pressure = Some(error.kind());
                break;
            }
        }
    }
    assert!(
        !peers.is_empty(),
        "at least one real AF_UNIX connection must fill the backlog"
    );
    let started = Instant::now();
    assert!(matches!(
        elect(paths(&root, "portable")).unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(!root.join("portable").exists());
    assert!(
        pressure.is_some(),
        "the real backlog must reach connection refusal or timeout"
    );
    drop(peers);
    drop(primary);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__lock_and_endpoint_io_failures_refuse_startup_and_release_partial_locks() {
    let root = std::env::temp_dir().join(format!("tl95-io-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let first = elect(paths(&root, "installed")).unwrap();
    drop(first);
    std::fs::create_dir(root.join("user/run/a.sock")).unwrap();
    assert!(elect(paths(&root, "installed")).is_err());
    std::fs::remove_dir(root.join("user/run/a.sock")).unwrap();
    std::fs::remove_file(root.join("user/instance.lock")).unwrap();
    std::fs::create_dir(root.join("user/instance.lock")).unwrap();
    assert!(elect(paths(&root, "portable")).is_err());
    assert!(!root.join("portable").exists());
    std::fs::remove_dir(root.join("user/instance.lock")).unwrap();
    let recovered = elect(paths(&root, "installed")).unwrap();
    assert!(matches!(recovered, Election::Primary(_)));
    drop(recovered);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__simultaneous_modes_elect_exactly_one_writer() {
    let root = std::env::temp_dir().join(format!("tl95-race-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let installed = start(&root, "installed");
    let portable = start(&root, "portable");
    wait_for(&root.join("ready"));
    wait_for(&root.join("secondary"));
    assert_eq!(
        std::fs::read_to_string(root.join("secondary")).unwrap(),
        "Activated"
    );
    let writers = usize::from(root.join("installed/instance.lock").exists())
        + usize::from(root.join("portable/instance.lock").exists());
    assert_eq!(writers, 1);
    wait_for(&root.join("activated"));
    drop(installed);
    drop(portable);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__data_lock_contention_also_refuses_a_writer() {
    let root = std::env::temp_dir().join(format!("tl95-data-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let first = elect(paths(&root, "installed")).unwrap();
    let data = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join("installed/instance.lock"))
        .unwrap();
    drop(first);
    data.try_lock().unwrap();
    assert!(matches!(
        elect(paths(&root, "installed")).unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    drop(data);
    let next = elect(paths(&root, "installed")).unwrap();
    assert!(matches!(next, Election::Primary(_)));
    drop(next);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__different_runtime_environments_still_share_the_state_election_lock() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("tl95-runtime-race-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for name in ["runtime-a", "runtime-b"] {
        let path = root.join(name);
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let state = root.join("state");
    let primary = elect(InstancePaths::linux(
        &root.join("installed"),
        &state,
        Some(&root.join("runtime-a")),
    ))
    .unwrap();
    assert!(matches!(primary, Election::Primary(_)));
    assert!(matches!(
        elect(InstancePaths::linux(
            &root.join("portable"),
            &state,
            Some(&root.join("runtime-b"))
        ))
        .unwrap(),
        Election::Secondary(crate::api::ActivationOutcome::Unavailable)
    ));
    assert!(!root.join("portable").exists());
    assert!(!root.join("runtime-b/tallyline.lock").exists());
    drop(primary);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__set_and_unset_runtime_environments_never_elect_two_writers() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("tl95-runtime-unset-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let runtime = root.join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let state = root.join("state");
    for (first, second) in [
        (Some(runtime.as_path()), None),
        (None, Some(runtime.as_path())),
    ] {
        let primary = elect(InstancePaths::linux(&root.join("installed"), &state, first)).unwrap();
        assert!(matches!(primary, Election::Primary(_)));
        assert!(matches!(
            elect(InstancePaths::linux(&root.join("portable"), &state, second)).unwrap(),
            Election::Secondary(crate::api::ActivationOutcome::Unavailable)
        ));
        assert!(!root.join("portable").exists());
        drop(primary);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_10__byte_dripping_cannot_extend_the_overall_request_deadline() {
    let root = std::env::temp_dir().join(format!("tl95-drip-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let Election::Primary(mut primary) = elect(paths(&root, "installed")).unwrap() else {
        panic!("first election must win");
    };
    let mut peer = start(&root, "drip");
    wait_for(&root.join("dripping"));
    let started = Instant::now();
    assert!(!primary.poll_activation().unwrap());
    assert!(started.elapsed() < Duration::from_secs(1));
    wait_for(&root.join("drip-done"));
    assert!(peer.0.wait().unwrap().success());
    drop(peer);
    drop(primary);
    std::fs::remove_dir_all(root).unwrap();
}
