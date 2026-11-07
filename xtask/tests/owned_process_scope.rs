// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public declaration contracts for packet features without JobObjects.

use serde_json::{Value, json};
use xtask::owned_process_scope;

fn registry_declaration(owner: &str, version: &str, requirement: &str, features: &[&str]) -> Value {
    json!({
        "packages": [{
            "id": "owner", "name": owner, "version": version,
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "dependencies": [{
                "name": "windows-sys", "source": "registry+https://github.com/rust-lang/crates.io-index",
                "req": requirement, "kind": null, "rename": null, "optional": false,
                "uses_default_features": true, "features": features,
                "target": "cfg(windows)", "registry": null
            }]
        }],
        "workspace_members": [], "resolve": {"nodes": []}
    })
}

#[test]
fn existing_security_pipe_and_io_declarations_remain_permitted() {
    for input in [
        registry_declaration(
            "os_pipe",
            "1.2.3",
            ">=0.28, <=0.61",
            &["Win32_Foundation", "Win32_System_Pipes", "Win32_Security"],
        ),
        registry_declaration(
            "socket2",
            "0.6.5",
            ">=0.60, <0.62",
            &[
                "Win32_Foundation",
                "Win32_Networking_WinSock",
                "Win32_System_IO",
                "Win32_System_Threading",
                "Win32_System_WindowsProgramming",
            ],
        ),
    ] {
        assert!(
            owned_process_scope::violations(&input.to_string(), "x86_64-pc-windows-msvc")
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn existing_older_sdk_security_declaration_remains_permitted() {
    let mut input = registry_declaration(
        "winit",
        "0.30.13",
        "^0.52.0",
        &[
            "Win32_Devices_HumanInterfaceDevice",
            "Win32_Foundation",
            "Win32_Globalization",
            "Win32_Graphics_Dwm",
            "Win32_Graphics_Gdi",
            "Win32_Media",
            "Win32_System_Com_StructuredStorage",
            "Win32_System_Com",
            "Win32_System_LibraryLoader",
            "Win32_System_Ole",
            "Win32_Security",
            "Win32_System_SystemInformation",
            "Win32_System_SystemServices",
            "Win32_System_Threading",
            "Win32_System_WindowsProgramming",
            "Win32_UI_Accessibility",
            "Win32_UI_Controls",
            "Win32_UI_HiDpi",
            "Win32_UI_Input_Ime",
            "Win32_UI_Input_KeyboardAndMouse",
            "Win32_UI_Input_Pointer",
            "Win32_UI_Input_Touch",
            "Win32_UI_Shell",
            "Win32_UI_TextServices",
            "Win32_UI_WindowsAndMessaging",
        ],
    );
    input["packages"][0]["dependencies"][0]["target"] = json!("cfg(target_os = \"windows\")");
    assert!(
        owned_process_scope::violations(&input.to_string(), "x86_64-pc-windows-msvc")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_existing_owner_cannot_change_its_pipe_declaration() {
    let input = registry_declaration(
        "os_pipe",
        "1.2.3",
        ">=0.28, <=0.61",
        &["Win32_Foundation", "Win32_System_Pipes", "Win32_Security"],
    );
    for (field, bad) in [
        ("target", json!("x86_64-pc-windows-msvc")),
        ("req", json!("=0.61.2")),
        ("uses_default_features", json!(false)),
        ("rename", json!("aliased_sdk")),
        ("optional", json!(true)),
        ("kind", json!("build")),
        ("source", json!("git+https://example.invalid/sdk")),
        ("features", json!(["Win32_System_Pipes"])),
    ] {
        let mut changed = input.clone();
        changed["packages"][0]["dependencies"][0][field] = bad;
        assert!(
            !owned_process_scope::violations(&changed.to_string(), "x86_64-pc-windows-msvc")
                .unwrap()
                .is_empty(),
            "{field}"
        );
    }
    for (field, bad) in [
        ("name", json!("foreign-owner")),
        ("version", json!("1.2.4")),
        ("source", Value::Null),
    ] {
        let mut changed = input.clone();
        changed["packages"][0][field] = bad;
        assert!(
            !owned_process_scope::violations(&changed.to_string(), "x86_64-pc-windows-msvc")
                .unwrap()
                .is_empty(),
            "{field}"
        );
    }
}
