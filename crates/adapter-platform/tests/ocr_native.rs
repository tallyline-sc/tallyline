// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public mapped-runtime prerequisite policy.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use tallyline_adapter_platform::api::model::{CrtModuleVersion, NativeOcrModule, OcrNativeError};

#[test]
fn r_nf_13__every_mapped_crt_module_must_meet_the_pinned_toolset_floor() {
    for module in [
        NativeOcrModule::Msvcp140,
        NativeOcrModule::Msvcp140Supplement,
        NativeOcrModule::Vcruntime140,
        NativeOcrModule::Vcruntime140Supplement,
    ] {
        assert!(matches!(
            CrtModuleVersion::checked(module, [14, 43, 65535, 0]),
            Err(OcrNativeError::CrtTooOld { .. })
        ));
        assert!(CrtModuleVersion::checked(module, [14, 44, 0, 0]).is_ok());
        let newer = CrtModuleVersion::checked(module, [14, 50, 123, 0]).unwrap();
        assert_eq!(newer.module(), module);
        assert_eq!(newer.parts(), [14, 50, 123, 0]);
    }
    assert!(matches!(
        CrtModuleVersion::checked(NativeOcrModule::OnnxRuntime, [14, 44, 0, 0]),
        Err(OcrNativeError::InvalidModule)
    ));
}

#[test]
fn r_sec_8__mapped_runtime_verification_refuses_a_relative_or_missing_file() {
    use tallyline_adapter_platform::api::verify_ocr_native_modules;
    assert_eq!(
        verify_ocr_native_modules(std::path::Path::new("onnxruntime.dll")),
        Err(OcrNativeError::InvalidPath)
    );
    let missing = std::env::temp_dir().join(format!(
        "tallyline-unavailable-native-{}.dll",
        std::process::id()
    ));
    assert!(!missing.exists());
    assert_eq!(
        verify_ocr_native_modules(&missing),
        Err(OcrNativeError::InvalidPath)
    );
}
