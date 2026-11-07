// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public runtime resource settings contracts.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use std::time::Duration;
use tallyline_adapter_ocr::api::model::OcrRuntimeSettings;

#[test]
fn r_nf_3__runtime_settings_bound_native_threads_and_idle_sessions() {
    let settings = OcrRuntimeSettings::default();
    assert_eq!(settings.intra_op_threads(), 1);
    assert_eq!(settings.idle_close_after(), Duration::from_secs(300));
    assert!(OcrRuntimeSettings::new(0, Duration::from_secs(300)).is_err());
    assert!(OcrRuntimeSettings::new(1, Duration::ZERO).is_err());
    assert!(OcrRuntimeSettings::new(i32::MAX as usize + 1, Duration::from_secs(300)).is_err());
    let settings = OcrRuntimeSettings::new(2, Duration::from_secs(60)).unwrap();
    assert_eq!(settings.intra_op_threads(), 2);
    assert_eq!(settings.idle_close_after(), Duration::from_secs(60));
}
