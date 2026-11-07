// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proxy contracts isolated from the parent process's environment and OS settings.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn r_api_3__https_proxy_environment_routes_connect_without_changing_system_settings() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let proxy = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(client) => break client,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "proxy was not contacted");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("local proxy accept failed: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            assert!(read > 0 && request.len() + read <= 4096);
            request.extend_from_slice(&buffer[..read]);
        }
        stream
            .write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        String::from_utf8(request).unwrap()
    });
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "proxy_contract_tests::proxy_client", "--ignored"])
        .env_remove("HTTP_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .env_remove("no_proxy")
        .env("NO_PROXY", "")
        .env("HTTPS_PROXY", format!("http://{address}"))
        .output()
        .unwrap();
    let request = proxy.join().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(request.starts_with("CONNECT proxy-target.invalid:443 HTTP/1.1\r\n"));
}

#[tokio::test]
#[ignore = "Private subprocess entrypoint invoked by the Cargo-run proxy contract."]
async fn proxy_client() {
    use crate::api::{ClientIdentity, ReaderSettings, UexReader};
    let settings = ReaderSettings::new(
        "https://proxy-target.invalid/2.0/",
        ClientIdentity::new("tallyline-test", "1.2.3"),
    )
    .unwrap();
    assert!(
        UexReader::new(&settings)
            .unwrap()
            .game_versions()
            .await
            .is_err()
    );
}
