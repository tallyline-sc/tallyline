// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-file supplied-geometry crop preparation contracts.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tallyline_adapter_files::api;
use tallyline_adapter_files::api::model::{ImageIntake, ImageIntakeSettings};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> std::io::Result<Self> {
        let root = std::env::temp_dir().join(format!("panel-dump-{}", std::process::id()));
        fs::create_dir(&root)?;
        Ok(Self(root))
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(fs::remove_dir_all(&self.0).is_ok());
    }
}

fn bmp() -> Vec<u8> {
    let mut bytes = [0; 246];
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&(246_u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&8_u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&8_u32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    for (index, pixel) in bytes[54..].as_chunks_mut::<3>().0.iter_mut().enumerate() {
        let y = 7 - index / 8;
        pixel.copy_from_slice(if y < 3 {
            &[255, 0, 255]
        } else {
            &[0x66, 0x44, 0x22]
        });
    }
    bytes.to_vec()
}
fn run(image: &Path, geometry: &Path, output: &Path) -> std::io::Result<std::process::Output> {
    Command::new(env!("CARGO_BIN_EXE_ocr-eval"))
        .arg("prepare-panels")
        .arg("--image")
        .arg(image)
        .arg("--geometry")
        .arg(geometry)
        .arg("--output")
        .arg(output)
        .output()
}

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]
fn r_ocr_2_r_cap_7__real_file_panel_dump_is_safe_bound_and_exclusive() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new().unwrap();
    let image = fixture.path("source.bmp");
    let bytes = bmp();
    fs::write(&image, &bytes).unwrap();
    let geometry = fixture.path("supplied.json");
    let manifest = serde_json::json!({
        "schemaVersion":2,"imageSha256":Sha256::digest(&bytes).iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "rasterDimensions":[8,8],"provenance":"manual",
        "shop":{"corners":[[2.,2.],[6.,2.],[6.,6.],[2.,6.]],"output":[4,4]},
        "location":{"corners":[[0.,2.],[2.,2.],[2.,5.],[0.,5.]],"output":[4,6]},
        "header":[[0.,0.],[8.,0.],[8.,2.25],[0.,2.25]],
        "limits":{"maxSide":64,"maxTotalPixels":1024,"maxSampledPixels":4096,"maxOutputBytes":65536}
    });
    fs::write(&geometry, serde_json::to_vec(&manifest).unwrap()).unwrap();
    for condition in 0..8 {
        let mut invalid = manifest.clone();
        match condition {
            0 => invalid["limits"]["maxOutputBytes"] = 1.into(),
            1 => invalid["imageSha256"] = "0".repeat(64).into(),
            2 => invalid["rasterDimensions"] = serde_json::json!([4, 4]),
            3 => invalid["provenance"] = "automatic".into(),
            4 => invalid["header"] = serde_json::json!([[0, 0], [0, 0], [0, 0], [0, 0]]),
            5 => invalid["limits"]["maxTotalPixels"] = 1.into(),
            6 => invalid["limits"]["maxSampledPixels"] = 1.into(),
            _ => invalid["verified"] = true.into(),
        }
        let bad_geometry = fixture.path("refused.json");
        fs::write(&bad_geometry, serde_json::to_vec(&invalid).unwrap()).unwrap();
        let refused_output = fixture.path("refused-output");
        let result = run(&image, &bad_geometry, &refused_output).unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!refused_output.exists());
        assert_eq!(fs::read(&image).unwrap(), bytes);
    }
    let mut legacy = manifest.clone();
    legacy["schemaVersion"] = 1.into();
    let legacy_geometry = fixture.path("legacy.json");
    fs::write(&legacy_geometry, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let legacy_output = fixture.path("legacy-output");
    let refused = run(&image, &legacy_geometry, &legacy_output).unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("regenerate and review"));
    assert!(!legacy_output.exists());
    let output = fixture.path("safe-dump");
    let result = run(&image, &geometry, &output).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let receipt: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["state"], "prepared");
    assert_eq!(receipt["automaticLocate"], "not_run");
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("geometry.json")).unwrap()).unwrap();
    assert_eq!(metadata["schemaVersion"], 2);
    assert_eq!(metadata["sourceSha256"], manifest["imageSha256"]);
    assert_eq!(
        metadata["geometrySha256"],
        Sha256::digest(fs::read(&geometry).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    assert_eq!(metadata["provenance"], "manual");
    let matrix: [[f64; 3]; 3] =
        serde_json::from_value(metadata["shop"]["outputToSource"].clone()).unwrap();
    assert_eq!(matrix, [[1., 0., 2.], [0., 1., 2.], [0., 0., 1.]]);
    assert_eq!(metadata["textSize"], "unavailable_not_measured");
    assert_eq!(metadata["tone"], "unavailable_uncalibrated");
    assert_eq!(metadata["privacyConfirmationRequired"], true);
    assert_eq!(
        metadata["privacyMask"],
        "full_width_upward_through_header_supporting_bottom_edge"
    );
    let ImageIntake::Decoded(decoded) = api::read_image_file(
        &output.join("location.png"),
        &ImageIntakeSettings::default(),
    ) else {
        panic!("safe output was not a PNG");
    };
    assert_eq!([decoded.raster.width(), decoded.raster.height()], [4, 6]);
    assert_eq!(decoded.raster.argb()[0], 0xff00_0000);
    assert_eq!(*decoded.raster.argb().last().unwrap(), 0xff22_4466);
    let frozen = fs::read(output.join("shop.png")).unwrap();
    assert!(!run(&image, &geometry, &output).unwrap().status.success());
    assert_eq!(fs::read(output.join("shop.png")).unwrap(), frozen);
    assert_eq!(fs::read(&image).unwrap(), bytes);
    let mut skew_bytes = bytes.clone();
    let sentinel = 54 + (5 * 8) * 3;
    skew_bytes[sentinel..sentinel + 3].copy_from_slice(&[0x56, 0x34, 0x12]);
    let skew_image = fixture.path("skew.bmp");
    fs::write(&skew_image, &skew_bytes).unwrap();
    let mut skew = manifest.clone();
    skew["imageSha256"] = Sha256::digest(&skew_bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .into();
    skew["shop"] = serde_json::json!({"corners":[[0,0],[8,0],[8,8],[0,8]],"output":[8,8]});
    skew["location"] = serde_json::json!({"corners":[[0,6],[2,6],[2,8],[0,8]],"output":[2,2]});
    skew["header"] = serde_json::json!([[2, 0], [6, 0], [6, 4], [2, 2]]);
    let skew_geometry = fixture.path("skew.json");
    fs::write(&skew_geometry, serde_json::to_vec(&skew).unwrap()).unwrap();
    let skew_output = fixture.path("skew-output");
    let result = run(&skew_image, &skew_geometry, &skew_output).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let ImageIntake::Decoded(decoded) = api::read_image_file(
        &skew_output.join("shop.png"),
        &ImageIntakeSettings::default(),
    ) else {
        panic!("skew output was not a PNG");
    };
    assert_eq!(decoded.raster.argb()[16], 0xff12_3456);
    assert_eq!(decoded.raster.argb()[17], 0xff00_0000);
    assert_eq!(fs::read(&skew_image).unwrap(), skew_bytes);
}
