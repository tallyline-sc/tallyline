// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Isolated Linux credential-bus transport contracts.

#![cfg(target_os = "linux")]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

use tallyline_adapter_platform::api::LinuxCredentialBus;
use tallyline_submission::api::model::StorageFailure;

const GUID: &str = "0123456789abcdef0123456789abcdef";

fn address(path: &Path) -> String {
    format!("unix:path={}", path.display())
}

fn peer(
    listener: UnixListener,
    accept_auth: bool,
) -> std::thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    std::thread::spawn(move || {
        let (stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        let mut stream = BufReader::new(stream);
        let mut line = String::new();
        stream.read_line(&mut line)?;
        assert!(line.starts_with("\0AUTH EXTERNAL "), "{line:?}");
        if !accept_auth {
            stream.get_mut().write_all(b"REJECTED ANONYMOUS\r\n")?;
            line.clear();
            assert_eq!(stream.read_line(&mut line)?, 0);
            return Ok(());
        }
        write!(stream.get_mut(), "OK {GUID}\r\n")?;
        line.clear();
        stream.read_line(&mut line)?;
        assert_eq!(line, "NEGOTIATE_UNIX_FD\r\n");
        stream.get_mut().write_all(b"AGREE_UNIX_FD\r\n")?;
        line.clear();
        stream.read_line(&mut line)?;
        assert_eq!(line, "BEGIN\r\n");
        hello(&mut stream)?;
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte)?, 0);
        Ok(())
    })
}

fn hello(
    stream: &mut BufReader<UnixStream>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut header = [0; 16];
    stream.read_exact(&mut header)?;
    assert_eq!(&header[..4], b"l\x01\0\x01");
    let serial: [u8; 4] = header[8..12].try_into()?;
    let fields = u32::from_le_bytes(header[12..16].try_into()?) as usize;
    let body = u32::from_le_bytes(header[4..8].try_into()?) as usize;
    let mut rest = vec![0; fields.next_multiple_of(8) + body];
    stream.read_exact(&mut rest)?;
    assert!(rest.windows(5).any(|part| part == b"Hello"));
    let mut reply = b"l\x02\0\x01\x09\0\0\0\x01\0\0\0\x0f\0\0\0\x05\x01u\0".to_vec();
    reply.extend_from_slice(&serial);
    reply.extend_from_slice(b"\x08\x01g\0\x01s\0\0\x04\0\0\0:1.1\0");
    stream.get_mut().write_all(&reply)?;
    Ok(())
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__unsupported_or_ambiguous_addresses_fail_before_connecting() {
    for value in [
        "",
        "tcp:host=localhost,port=1234",
        "unixexec:path=/bin/false",
        "autolaunch:",
        "unix:abstract=dummy",
        "unix:path=relative",
        "unix:path=/tmp/a;unix:path=/tmp/b",
        "unix:path=/tmp/a,path=/tmp/b",
        "unix:path=/tmp/a,guid=bad",
        "unix:path=/tmp/%00",
        "unix:path=/tmp/%zz",
        "unix:path=/tmp/a,unknown=b",
    ] {
        assert_eq!(
            LinuxCredentialBus::connect(value).await.err(),
            Some(StorageFailure::Unavailable),
            "{value}"
        );
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__private_same_uid_socket_completes_external_and_hello_with_guid() {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.path().join("bus");
    let listener = UnixListener::bind(&path).unwrap();
    let server = peer(listener, true);
    let bus = LinuxCredentialBus::connect(&format!("{},guid={GUID}", address(&path)))
        .await
        .unwrap();
    assert!(bus.is_connected());
    drop(bus);
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__anonymous_authentication_and_wrong_guids_are_never_accepted() {
    for (accept_auth, guid) in [(false, GUID), (true, "ffffffffffffffffffffffffffffffff")] {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.path().join("bus");
        let server = peer(UnixListener::bind(&path).unwrap(), accept_auth);
        assert_eq!(
            LinuxCredentialBus::connect(&format!("{},guid={guid}", address(&path)))
                .await
                .err(),
            Some(StorageFailure::Unavailable)
        );
        server.join().unwrap().unwrap();
    }
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__public_parent_symlink_and_regular_file_are_rejected_without_authentication() {
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let path = private.join("bus");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o750)).unwrap();
    assert_eq!(
        LinuxCredentialBus::connect(&address(&path)).await.err(),
        Some(StorageFailure::Unavailable)
    );
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let alias = directory.path().join("alias");
    symlink(&private, &alias).unwrap();
    for rejected in [
        alias.join("bus"),
        private.join("../private/bus"),
        private.join("regular"),
    ] {
        fs::write(private.join("regular"), b"dummy").unwrap();
        assert_eq!(
            LinuxCredentialBus::connect(&address(&rejected)).await.err(),
            Some(StorageFailure::Unavailable)
        );
    }
    assert_eq!(
        listener.accept().err().unwrap().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__percent_encoded_filesystem_paths_use_the_same_validated_socket() {
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private space");
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).unwrap();
    let path = private.join("bus");
    let server = peer(UnixListener::bind(&path).unwrap(), true);
    let bus = LinuxCredentialBus::connect(&address(&path).replace(' ', "%20"))
        .await
        .unwrap();
    assert!(bus.is_connected());
    drop(bus);
    server.join().unwrap().unwrap();
}

#[tokio::test]
#[ignore = "Requires an isolated root-owned fixture and an unprivileged child."]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_4__foreign_uid_peer_is_rejected_before_any_authentication_bytes() {
    use std::os::unix::process::CommandExt;

    const CHILD_SOCKET: &str = "TALLYLINE_ISOLATED_FOREIGN_UID_SOCKET";
    const CHILD_UID: u32 = 65534;
    if let Some(path) = std::env::var_os(CHILD_SOCKET) {
        assert_eq!(rustix::process::geteuid().as_raw(), CHILD_UID);
        assert_eq!(
            LinuxCredentialBus::connect(&address(Path::new(&path)))
                .await
                .err(),
            Some(StorageFailure::Unavailable)
        );
        return;
    }
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    rustix::fs::chown(
        directory.path(),
        Some(rustix::process::Uid::from_raw(CHILD_UID)),
        None,
    )
    .unwrap();
    let path = directory.path().join("bus");
    let listener = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    rustix::fs::chown(&path, Some(rustix::process::Uid::from_raw(CHILD_UID)), None).unwrap();
    listener.set_nonblocking(true).unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "r_nf_4__foreign_uid_peer_is_rejected_before_any_authentication_bytes",
        ])
        .env(CHILD_SOCKET, &path)
        .uid(CHILD_UID)
        .gid(CHILD_UID)
        .spawn()
        .unwrap();
    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match listener.accept() {
                Ok(accepted) => break accepted,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => panic!("isolated listener failed: {error}"),
            }
        }
    })
    .await
    .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    assert!(child.wait_with_output().unwrap().status.success());
}
