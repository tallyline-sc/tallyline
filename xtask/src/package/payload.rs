// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact approved initial Small/native/font/branding application payload identities.

use super::Target;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const MAX_PINNED_ASSET_BYTES: u64 = 32 * 1024 * 1024;
const ASSETS: [(&str, &str); 15] = [
    (
        "assets/fonts/material-symbols/MaterialSymbolsRounded-Tallyline.ttf",
        "40c8c3a472c6978cedd5028194fa174b2c63edae4165f7686d88fbf174602234",
    ),
    (
        "assets/fonts/material-symbols/LICENSE",
        "58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd",
    ),
    (
        "assets/fonts/material-symbols/NOTICE",
        "b8f97f88268442a9c60afa6f8defbdd6b7ab568707621f4fb9264d887ea5ccc6",
    ),
    (
        "assets/branding/tallyline-icon-256.png",
        "d0ed9a91e6e5fd79633352b1f319e8f2c96c0ff1159c371a9354857ab5d8108f",
    ),
    (
        "assets/branding/tallyline.ico",
        "13c3bf7fd049940f4ae4ec656ca548491ad22aa5760ef2562a2d072cad4b2879",
    ),
    (
        "assets/ocr/small/PP-OCRv6_small_det.onnx",
        "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e",
    ),
    (
        "assets/ocr/small/PP-OCRv6_small_rec.onnx",
        "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634",
    ),
    (
        "assets/ocr/small/ppocrv6_dict.txt",
        "b5f2bfe2bdd9448429e3e82b51c789775d9b42f2403d082b00662eb77e401c5d",
    ),
    (
        "assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf",
        "975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5",
    ),
    (
        "assets/fonts/ibm-plex/IBMPlexSans-SemiBold.ttf",
        "a20caf8286023a6a7a85e40b1d2a4ae9fc3e3b1f9eda8f4c542dd4986af67bb1",
    ),
    (
        "assets/fonts/ibm-plex/IBMPlexSans-Bold.ttf",
        "9e6c74a889a700d707613d24548fe4ffa6bc59559a0689d2cf9e133bdcdafb2f",
    ),
    (
        "assets/fonts/ibm-plex/OFL.txt",
        "7e6b2818edbd8f6a01ae80641cc8f16a51080d08fb4e532be3a0b6f74adb07da",
    ),
    (
        "assets/fonts/jetbrains-mono/JetBrainsMonoNL-Regular.ttf",
        "fb3b2575d7b0657359707993288f12a7360344d39387bb26050e276d61f6bd2a",
    ),
    (
        "assets/fonts/jetbrains-mono/JetBrainsMonoNL-Bold.ttf",
        "0198e841824025f8876e5c297f0b9b497ee8d6eb9969710a3328e1303f996ec3",
    ),
    (
        "assets/fonts/jetbrains-mono/OFL.txt",
        "30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad",
    ),
];

pub(super) fn verify(root: &Path, target: Target) -> io::Result<()> {
    binary(&root.join(target.executable()), target)?;
    for (path, expected) in ASSETS
        .into_iter()
        .chain([(target.runtime(), target.runtime_hash())])
    {
        let path = root.join(path);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_PINNED_ASSET_BYTES
            || crate::hashlist::sha256_hex(&fs::read(path)?) != expected
        {
            return Err(io::Error::other(
                "payload asset differs from its approved identity",
            ));
        }
    }
    let expected = ASSETS
        .into_iter()
        .map(|(path, _)| path.to_owned())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    collect_assets(root, &root.join("assets"), &mut actual)?;
    actual.remove("assets/README.md");
    if actual != expected {
        return Err(io::Error::other(
            "payload contains missing or unapproved asset paths",
        ));
    }
    super::manifest::legal_catalogue(root)?;
    super::legal::verify_profile(root, target)?;
    let project_licence: &[u8] = include_bytes!("../../../LICENSE");
    if fs::read(root.join("LICENSE"))?.as_slice() != project_licence
        || fs::read(root.join("NOTICE"))? != fs::read(root.join("legal/fan-kit/NOTICE"))?
    {
        return Err(io::Error::other(
            "project licence or notice differs from the delivered legal material",
        ));
    }
    for name in ["LICENSE", "NOTICE"] {
        if fs::read(root.join("assets/fonts/material-symbols").join(name))?
            != fs::read(root.join("legal/material-symbols").join(name))?
        {
            return Err(io::Error::other(
                "icon licence or modification notice differs from its bundled legal notice",
            ));
        }
    }
    for family in ["ibm-plex", "jetbrains-mono"] {
        if fs::read(root.join("assets/fonts").join(family).join("OFL.txt"))?
            != fs::read(root.join("legal").join(family).join("OFL.txt"))?
        {
            return Err(io::Error::other(
                "font licence differs from its bundled legal notice",
            ));
        }
    }
    Ok(())
}

fn collect_assets(root: &Path, directory: &Path, paths: &mut BTreeSet<String>) -> io::Result<()> {
    for (child, _) in super::archive::children(directory)? {
        let path = child.path();
        let kind = fs::symlink_metadata(&path)?.file_type();
        if kind.is_symlink() {
            return Err(io::Error::other("payload assets reject symbolic links"));
        }
        if kind.is_dir() {
            collect_assets(root, &path, paths)?;
        } else if kind.is_file() {
            paths.insert(
                path.strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_str()
                    .ok_or_else(|| io::Error::other("payload asset path is not UTF-8"))?
                    .replace('\\', "/"),
            );
        } else {
            return Err(io::Error::other("payload asset is not a regular file"));
        }
    }
    Ok(())
}

fn binary(path: &Path, target: Target) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "prebuilt application must be a regular file",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut header = [0_u8; 64];
    file.read_exact(&mut header)?;
    let valid = match target {
        Target::Windows => {
            let offset: [u8; 4] = header
                .get(60..64)
                .ok_or_else(|| io::Error::other("invalid PE header"))?
                .try_into()
                .map_err(io::Error::other)?;
            file.seek(SeekFrom::Start(u64::from(u32::from_le_bytes(offset))))?;
            let mut signature = [0_u8; 6];
            file.read_exact(&mut signature)?;
            header.starts_with(b"MZ") && signature == [b'P', b'E', 0, 0, 0x64, 0x86]
        }
        Target::Linux => {
            header.starts_with(&[0x7f, b'E', b'L', b'F', 2, 1, 1])
                && matches!(header.get(16..20), Some([2 | 3, 0, 62, 0]))
        }
    };
    if !valid {
        return Err(io::Error::other(
            "prebuilt application does not match the supported x64 target",
        ));
    }
    Ok(())
}

pub(super) fn forbidden_runtime(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy().to_ascii_lowercase();
        ["msvcp140", "vcruntime140", "api-ms-win-", "vc_redist"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
            || name == "ucrtbase.dll"
    })
}
