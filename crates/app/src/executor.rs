// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Named composition-root workers that inherit the caller's logging context.

use std::io;
use std::thread::JoinHandle;

pub(crate) fn spawn<T: Send + 'static>(
    name: &str,
    task: impl FnOnce() -> T + Send + 'static,
) -> io::Result<JoinHandle<T>> {
    let span = tracing::Span::current();
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || span.in_scope(task))
}
