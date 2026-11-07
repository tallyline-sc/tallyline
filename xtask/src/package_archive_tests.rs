// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Portable archive permissions checked by independent format readers.

#![expect(
    non_snake_case,
    reason = "requirement traceability uses double underscores"
)]

use std::process::Command;

use crate::scratch::Scratch;

#[test]
fn r_nf_2_q_installers__archives_keep_shared_libraries_as_data() {
    let directory = Scratch::new();
    directory.write("payload/tallyline", "application fixture");
    directory.write("payload/libonnxruntime.so.1.31.0", "library fixture");
    let input = directory.path().join("payload");
    #[cfg(unix)]
    {
        use std::fs;
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        for (name, mode) in [("tallyline", 0o755), ("libonnxruntime.so.1.31.0", 0o644)] {
            fs::set_permissions(input.join(name), Permissions::from_mode(mode))
                .expect("set fixture permissions");
        }
    }
    directory.write(
        "inspect.py",
        r#"import sys, tarfile, zipfile
expected = {'tallyline/tallyline': 0o755, 'tallyline/libonnxruntime.so.1.31.0': 0o644}
if sys.argv[1] == 'zip':
    with zipfile.ZipFile(sys.argv[2]) as archive:
        actual = {name: archive.getinfo(name).external_attr >> 16 & 0o777 for name in expected}
else:
    with tarfile.open(sys.argv[2], 'r:gz') as archive:
        actual = {name: archive.getmember(name).mode for name in expected}
assert actual == expected, (actual, expected)
"#,
    );
    for format in ["zip", "tar.gz"] {
        let output = directory.path().join(format!("application.{format}"));
        crate::package::write_archive(format, &input, &output).expect("write portable archive");
        let inspected = Command::new(if cfg!(windows) { "python" } else { "python3" })
            .arg(directory.path().join("inspect.py"))
            .arg(format)
            .arg(output)
            .output()
            .expect("run independent archive reader");
        assert!(
            inspected.status.success(),
            "independent {format} permission inspection failed: {}",
            String::from_utf8_lossy(&inspected.stderr)
        );
    }
}
