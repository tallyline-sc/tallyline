// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Explicit data-only packaging command dispatch without workspace discovery.

use super::{
    Target, build, inject_fixture, legal, manifest, prepare, prepare_with_identity,
    probe_msi_tools, release_version, repack_debian, scan_command, source_map, tools,
    verify_injected_stage, verify_prebuilt_identity, verify_source_bindings, verify_stage,
    write_archive, write_debian_copyright, write_prebuilt_identity,
};
use std::io::{self, Write};
use std::path::Path;

/// Dispatches packaging without discovering a source checkout or invoking Cargo.
///
/// # Errors
/// Returns an invalid command, rejected release identity or output failure.
pub fn run(arguments: &[String]) -> io::Result<()> {
    let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] | ["help"] => writeln!(
            io::stdout(),
            "usage: xtask package identity-receipt <windows|linux> <vMAJOR.MINOR.PATCH> <application> <compiled-report> <fresh-output> | prepare-admitted <windows|linux> <vMAJOR.MINOR.PATCH> <payload> <tools> <fresh-stage> <receipt> <receipt-seal> | identity-verify <windows|linux> <vMAJOR.MINOR.PATCH> <application> <receipt> <receipt-seal> | version <vMAJOR.MINOR.PATCH> | tools-verify <windows|linux> <tools> | source-bindings <source-root> | source-verify <payload> <archive> | source-map <payload> <unpacked-source> <archive> <fresh-output> | msi-probe <approved-tools> <fresh-output> | build <stage> <seal> <fixture> <fresh-output> | repack-deb <prepared-tree> <fresh-output> | copyright <payload> <fresh-output> | legal-verify [windows|linux] <payload> | prepare <windows|linux> <vMAJOR.MINOR.PATCH> <payload> <tools> <fresh-stage> | inject <stage> <seal> <fixture> | verify-injected <stage> <seal> <fixture> | verify <stage> <seal> | archive <zip|tar.gz|source.tar.gz> <input> <fresh-output> | scan <unpacked-tree> <fixture-file> [allowed-relative-file]"
        ),
        ["archive", format, input, output] => {
            write_archive(format, Path::new(input), Path::new(output))
        }
        ["prepare", target, tag, input, tools, stage] => {
            let target = match *target {
                "windows" => Target::Windows,
                "linux" => Target::Linux,
                _ => return Err(io::Error::other("unsupported packaging platform")),
            };
            let seal = prepare(
                Path::new(input),
                Path::new(tools),
                Path::new(stage),
                target,
                tag,
            )?;
            writeln!(io::stdout(), "{seal}")
        }
        [
            "prepare-admitted",
            target,
            tag,
            input,
            tools,
            stage,
            receipt,
            seal,
        ] => {
            let target = match *target {
                "windows" => Target::Windows,
                "linux" => Target::Linux,
                _ => return Err(io::Error::other("unsupported packaging platform")),
            };
            let seal = prepare_with_identity(
                Path::new(input),
                Path::new(tools),
                Path::new(stage),
                target,
                tag,
                Path::new(receipt),
                seal,
            )?;
            writeln!(io::stdout(), "{seal}")
        }
        ["identity-receipt", target, tag, application, report, output] => {
            let target = match *target {
                "windows" => Target::Windows,
                "linux" => Target::Linux,
                _ => return Err(io::Error::other("unsupported packaging platform")),
            };
            let seal = write_prebuilt_identity(
                Path::new(application),
                Path::new(report),
                Path::new(output),
                target,
                tag,
            )?;
            writeln!(io::stdout(), "{seal}")
        }
        ["identity-verify", target, tag, application, receipt, seal] => {
            let target = match *target {
                "windows" => Target::Windows,
                "linux" => Target::Linux,
                _ => return Err(io::Error::other("unsupported packaging platform")),
            };
            verify_prebuilt_identity(
                Path::new(application),
                Path::new(receipt),
                seal,
                target,
                tag,
            )
        }
        ["verify", stage, seal] => verify_stage(Path::new(stage), seal),
        ["inject", stage, seal, fixture] => {
            inject_fixture(Path::new(stage), seal, Path::new(fixture))
        }
        ["verify-injected", stage, seal, fixture] => {
            verify_injected_stage(Path::new(stage), seal, Path::new(fixture))
        }
        ["source-map", payload, source, archive, output] => source_map::write(
            Path::new(payload),
            Path::new(source),
            Path::new(archive),
            Path::new(output),
        ),
        ["msi-probe", tools, output] => probe_msi_tools(Path::new(tools), Path::new(output)),
        ["build", stage, seal, fixture, output] => build(
            Path::new(stage),
            seal,
            Path::new(fixture),
            Path::new(output),
        ),
        ["repack-deb", tree, output] => repack_debian(Path::new(tree), Path::new(output)),
        ["copyright", payload, output] => {
            write_debian_copyright(Path::new(payload), Path::new(output))
        }
        ["source-bindings", root] => verify_source_bindings(Path::new(root)),
        ["source-verify", payload, archive] => {
            source_map::validate(Path::new(payload), Path::new(archive)).map(|_| ())
        }
        ["legal-verify", target, root] => {
            let target = match *target {
                "windows" => Target::Windows,
                "linux" => Target::Linux,
                _ => return Err(io::Error::other("unsupported packaging platform")),
            };
            manifest::legal_catalogue(Path::new(root))?;
            legal::verify_profile(Path::new(root), target)
        }
        ["legal-verify", root] => manifest::legal_catalogue(Path::new(root)).map(|_| ()),
        ["tools-verify", target, root] => tools::verify(target, Path::new(root)),
        ["scan", root, fixture] => scan_command(Path::new(root), Path::new(fixture), None),
        ["scan", root, fixture, allowed] => scan_command(
            Path::new(root),
            Path::new(fixture),
            Some(Path::new(allowed)),
        ),
        ["version", tag] => writeln!(io::stdout(), "{}", release_version(tag)?),
        _ => Err(io::Error::other(
            "usage: xtask package identity-receipt <windows|linux> <vMAJOR.MINOR.PATCH> <application> <compiled-report> <fresh-output> | prepare-admitted <windows|linux> <vMAJOR.MINOR.PATCH> <payload> <tools> <fresh-stage> <receipt> <receipt-seal> | identity-verify <windows|linux> <vMAJOR.MINOR.PATCH> <application> <receipt> <receipt-seal> | version <vMAJOR.MINOR.PATCH> | tools-verify <windows|linux> <tools> | source-bindings <source-root> | source-verify <payload> <archive> | source-map <payload> <unpacked-source> <archive> <fresh-output> | msi-probe <approved-tools> <fresh-output> | build <stage> <seal> <fixture> <fresh-output> | repack-deb <prepared-tree> <fresh-output> | copyright <payload> <fresh-output> | legal-verify [windows|linux] <payload> | prepare <windows|linux> <vMAJOR.MINOR.PATCH> <payload> <tools> <fresh-stage> | inject <stage> <seal> <fixture> | verify-injected <stage> <seal> <fixture> | verify <stage> <seal> | archive <zip|tar.gz|source.tar.gz> <input> <fresh-output> | scan <unpacked-tree> <fixture-file> [allowed-relative-file]",
        )),
    }
}
