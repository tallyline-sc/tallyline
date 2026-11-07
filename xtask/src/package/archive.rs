// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic application archive records.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

const TAR_BLOCK: usize = 512;
const TAR_NAME: usize = 100;
const TAR_PREFIX: usize = 155;
const TAR_CHECKSUM: std::ops::Range<usize> = 148..156;
const DIRECTORY_MODE: u32 = 0o755;

pub(super) fn tar_gzip(source: &Path, file: File, source_files: bool) -> io::Result<()> {
    tar_stream(source, &file, source_files)?;
    file.sync_all()
}

pub(super) fn tar_gzip_digest(source: &Path) -> io::Result<String> {
    let mut writer = HashWriter(Sha256::new());
    tar_stream(source, &mut writer, true)?;
    Ok(writer
        .0
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

struct HashWriter(Sha256);

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn tar_stream(source: &Path, writer: impl Write, source_files: bool) -> io::Result<()> {
    let mut gzip = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(3)
        .write(writer, flate2::Compression::default());
    let mode: fn(&Path, &fs::Metadata) -> u32 = if source_files {
        |_, _: &fs::Metadata| 0o644
    } else {
        file_mode
    };
    append(&mut gzip, source, "tallyline", mode)?;
    gzip.write_all(&[0; TAR_BLOCK * 2])?;
    gzip.finish()?;
    Ok(())
}

fn append(
    writer: &mut impl Write,
    source: &Path,
    name: &str,
    mode: fn(&Path, &fs::Metadata) -> u32,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "application archives reject symbolic links",
        ));
    }
    if metadata.is_dir() {
        writer.write_all(&header(&format!("{name}/"), DIRECTORY_MODE, 0, b'5')?)?;
        for (child, filename) in children(source)? {
            append(writer, &child.path(), &format!("{name}/{filename}"), mode)?;
        }
    } else if metadata.is_file() {
        writer.write_all(&header(
            name,
            mode(source, &metadata),
            metadata.len(),
            b'0',
        )?)?;
        io::copy(&mut File::open(source)?, writer)?;
        let padding = (TAR_BLOCK as u64 - metadata.len() % TAR_BLOCK as u64) % TAR_BLOCK as u64;
        writer.write_all(&vec![
            0;
            usize::try_from(padding).map_err(io::Error::other)?
        ])?;
    } else {
        return Err(io::Error::other(
            "application archive input is not a regular file",
        ));
    }
    Ok(())
}

fn file_mode(source: &Path, metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = source;
        metadata.permissions().mode() & 0o777
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        if source
            .extension()
            .is_some_and(|extension| extension == "exe")
            || source
                .file_name()
                .is_some_and(|filename| filename == "tallyline")
        {
            0o755
        } else {
            0o644
        }
    }
}

fn field(output: &mut Vec<u8>, value: &[u8], width: usize) -> io::Result<()> {
    if value.len() > width {
        return Err(io::Error::other(
            "application archive field exceeds its format limit",
        ));
    }
    output.extend_from_slice(value);
    output.resize(output.len() + width - value.len(), 0);
    Ok(())
}

fn octal(output: &mut Vec<u8>, value: u64, width: usize) -> io::Result<()> {
    let value = format!("{value:0digits$o}", digits = width - 1);
    field(output, value.as_bytes(), width)
}

fn header(name: &str, mode: u32, size: u64, kind: u8) -> io::Result<Vec<u8>> {
    linked_header(name, mode, size, kind, "")
}

fn linked_header(name: &str, mode: u32, size: u64, kind: u8, link: &str) -> io::Result<Vec<u8>> {
    let (prefix, name) = if name.len() <= TAR_NAME {
        ("", name)
    } else {
        name.rsplit_once('/')
            .filter(|(prefix, name)| prefix.len() <= TAR_PREFIX && name.len() <= TAR_NAME)
            .ok_or_else(|| io::Error::other("application archive path exceeds USTAR limits"))?
    };
    let mut output = Vec::with_capacity(TAR_BLOCK);
    field(&mut output, name.as_bytes(), TAR_NAME)?;
    octal(&mut output, u64::from(mode), 8)?;
    octal(&mut output, 0, 8)?;
    octal(&mut output, 0, 8)?;
    octal(&mut output, size, 12)?;
    octal(&mut output, 0, 12)?;
    output.extend_from_slice(b"        ");
    output.push(kind);
    field(&mut output, link.as_bytes(), TAR_NAME)?;
    output.extend_from_slice(b"ustar\0");
    output.extend_from_slice(b"00");
    field(&mut output, b"root", 32)?;
    field(&mut output, b"root", 32)?;
    octal(&mut output, 0, 8)?;
    octal(&mut output, 0, 8)?;
    field(&mut output, prefix.as_bytes(), TAR_PREFIX)?;
    field(&mut output, b"", 12)?;
    let checksum: u64 = output.iter().map(|byte| u64::from(*byte)).sum();
    let checksum = format!("{checksum:06o}\0 ");
    output
        .get_mut(TAR_CHECKSUM)
        .ok_or_else(|| io::Error::other("invalid USTAR header"))?
        .copy_from_slice(checksum.as_bytes());
    Ok(output)
}
const ZIP_LOCAL: u32 = 0x0403_4b50;
const ZIP_CENTRAL: u32 = 0x0201_4b50;
const ZIP_END: u32 = 0x0605_4b50;
const ZIP_UTF8: u16 = 1 << 11;
const ZIP_VERSION: u16 = 20;
const ZIP_UNIX: u16 = (3 << 8) | ZIP_VERSION;
const ZIP_EPOCH_DATE: u16 = 33;

pub(super) fn zip(source: &Path, mut file: File) -> io::Result<()> {
    let mut data = Vec::new();
    let mut central = Vec::new();
    let mut count = 0_u16;
    zip_append(&mut data, &mut central, &mut count, source, "tallyline")?;
    let central_offset = u32::try_from(data.len()).map_err(io::Error::other)?;
    let central_size = u32::try_from(central.len()).map_err(io::Error::other)?;
    data.extend(central);
    dword(&mut data, ZIP_END);
    for value in [0, 0, count, count] {
        word(&mut data, value);
    }
    dword(&mut data, central_size);
    dword(&mut data, central_offset);
    word(&mut data, 0);
    file.write_all(&data)?;
    file.sync_all()
}

fn zip_append(
    data: &mut Vec<u8>,
    central: &mut Vec<u8>,
    count: &mut u16,
    source: &Path,
    name: &str,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(
            "application archives reject symbolic links",
        ));
    }
    let (name, contents, mode) = if metadata.is_dir() {
        (format!("{name}/"), Vec::new(), 0o040_000 | DIRECTORY_MODE)
    } else if metadata.is_file() {
        (
            name.to_owned(),
            fs::read(source)?,
            0o100_000 | file_mode(source, &metadata),
        )
    } else {
        return Err(io::Error::other(
            "application archive input is not a regular file",
        ));
    };
    let size = u32::try_from(contents.len()).map_err(io::Error::other)?;
    let name_size = u16::try_from(name.len()).map_err(io::Error::other)?;
    let offset = u32::try_from(data.len()).map_err(io::Error::other)?;
    let mut crc = flate2::Crc::new();
    crc.update(&contents);
    dword(data, ZIP_LOCAL);
    for value in [ZIP_VERSION, ZIP_UTF8, 0, 0, ZIP_EPOCH_DATE] {
        word(data, value);
    }
    for value in [crc.sum(), size, size] {
        dword(data, value);
    }
    word(data, name_size);
    word(data, 0);
    data.extend_from_slice(name.as_bytes());
    data.extend_from_slice(&contents);
    dword(central, ZIP_CENTRAL);
    for value in [ZIP_UNIX, ZIP_VERSION, ZIP_UTF8, 0, 0, ZIP_EPOCH_DATE] {
        word(central, value);
    }
    for value in [crc.sum(), size, size] {
        dword(central, value);
    }
    for value in [name_size, 0, 0, 0, 0] {
        word(central, value);
    }
    dword(central, mode << 16);
    dword(central, offset);
    central.extend_from_slice(name.as_bytes());
    *count = count
        .checked_add(1)
        .ok_or_else(|| io::Error::other("ZIP entry limit exceeded"))?;
    if metadata.is_dir() {
        for (child, filename) in children(source)? {
            zip_append(
                data,
                central,
                count,
                &child.path(),
                &format!("{name}{filename}"),
            )?;
        }
    }
    Ok(())
}

fn word(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn dword(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn children(source: &Path) -> io::Result<Vec<(fs::DirEntry, String)>> {
    let mut children = fs::read_dir(source)?.collect::<io::Result<Vec<_>>>()?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    let mut names = std::collections::BTreeSet::new();
    children
        .into_iter()
        .map(|child| {
            let name = child
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("application archive path is not UTF-8"))?;
            if !portable_name(&name) || !names.insert(name.to_uppercase()) {
                return Err(io::Error::other(
                    "application archive path is not portable or has a case alias",
                ));
            }
            Ok((child, name))
        })
        .collect()
}

fn reserved_device(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().to_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
}

pub(super) fn portable_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && !name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|'])
        && !name.ends_with(['.', ' '])
        && !name.chars().any(char::is_control)
        && !reserved_device(name)
}

pub(super) fn tree_gzip(
    source: &Path,
    file: File,
    link: Option<(&str, &str)>,
    mode: fn(&Path, &fs::Metadata) -> u32,
) -> io::Result<()> {
    if link.is_some_and(|pair| pair != ("usr/bin/tallyline", "../lib/tallyline/tallyline")) {
        return Err(io::Error::other("unapproved Debian archive link"));
    }
    let mut gzip = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(3)
        .write(file, flate2::Compression::default());
    for (child, name) in children(source)? {
        append(&mut gzip, &child.path(), &name, mode)?;
    }
    if let Some((name, target)) = link {
        if source.join(name).try_exists()? {
            return Err(io::Error::other("Debian launcher path already exists"));
        }
        gzip.write_all(&linked_header(name, 0o777, 0, b'2', target)?)?;
    }
    gzip.write_all(&[0; TAR_BLOCK * 2])?;
    gzip.finish()?.sync_all()
}
