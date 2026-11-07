// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        for attempt in 0..10_000 {
            let path =
                std::env::temp_dir().join(format!("ocr-eval-{}-{attempt}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
                    ) =>
                {
                    continue;
                }
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        }
        panic!("cannot allocate a test directory")
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn write(&self, relative: &str, contents: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
