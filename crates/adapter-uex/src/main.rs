// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UEX API spike: read-only GET calls and explicitly invoked `data_submit` dry runs.
//!
//! Usage: `tallyline-uex-spike [--confirm-expert-host] [--host uk|space] <command>` with the commands `game-versions`,
//! `status-levels`, `prices <id_terminal>`, `account`, `report-access` and
//! `dry-run <id_terminal> <id_commodity> <price_buy> <status_buy> <scu_buy> \
//! <game_version> <date_added> \
//! [container_sizes|-] [invalid_commodity_id]`.
//! `account` reads the secret key from `TALLYLINE_UEX_SECRET_KEY`; `report-access` and `dry-run`
//! also read the app token from
//! `TALLYLINE_UEX_APP_TOKEN`. Nothing is written to disk, and neither a credential, the personal
//! data of an answer nor the message text of a UEX error is printed.
//! `contract-snapshot` reads the fixed public routes and prints only field/type and code metadata.
//! `contract-check` compares those samples with the bundled metadata baseline.

#![forbid(unsafe_code)]

mod contract_cli;

#[cfg(test)]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
mod contract_cli_tests;

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use tallyline_adapter_uex::api::model::{PriceRow, StatusLevel, TerminalId};
use tallyline_adapter_uex::api::{
    AppToken, ClientIdentity, EXPERT_HOST_WARNING, ReadError, ReaderSettings, SecretKey, UexReader,
};

const UK_HOST: &str = "https://api.uexcorp.uk/2.0/";
const SPACE_HOST: &str = "https://api.uexcorp.space/2.0/";
const SECRET_KEY_VARIABLE: &str = "TALLYLINE_UEX_SECRET_KEY";
const APP_TOKEN_VARIABLE: &str = "TALLYLINE_UEX_APP_TOKEN";
const SCREENSHOT_PATH_VARIABLE: &str = "TALLYLINE_UEX_SCREENSHOT_PATH";
const USAGE: &str = "usage: tallyline-uex-spike [--confirm-expert-host] [--host uk|space] \
                     game-versions | status-levels | prices <id_terminal> | account | report-access | contract-snapshot | contract-check | \
                     dry-run <id_terminal> <id_commodity> <price_buy> <status_buy> <scu_buy> \
                     <game_version> <date_added> \
                     [container_sizes|-] [invalid_commodity_id]";

enum Command {
    GameVersions,
    StatusLevels,
    Prices(TerminalId),
    Account,
    ReportAccess,
    DryRun {
        terminal: u64,
        commodity: u64,
        price: u64,
        status: u8,
        scu: u32,
        game_version: String,
        date_added: i64,
        container_sizes: Option<String>,
        invalid_commodity: Option<u64>,
    },
}

fn main() -> ExitCode {
    let mut arguments: Vec<String> = env::args().skip(1).collect();
    let public_command = contract_cli::command(&arguments);
    let expert_confirmed = arguments
        .first()
        .is_some_and(|argument| argument == "--confirm-expert-host");
    if expert_confirmed {
        arguments.remove(0);
    }
    let mut stdout = io::stdout().lock();
    let outcome = match public_command {
        Some(Ok(mode)) => contract_cli::run(mode, &mut stdout),
        Some(Err(message)) => Err(message),
        None => match parse(&arguments) {
            Ok((host, command)) => run(host, &command, expert_confirmed, &mut stdout),
            Err(message) => Err(message),
        },
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            writeln!(io::stderr(), "tallyline-uex-spike: {message}").ok();
            ExitCode::FAILURE
        }
    }
}

fn parse(arguments: &[String]) -> Result<(&'static str, Command), String> {
    let (host, rest) = match arguments {
        [flag, host, rest @ ..] if flag == "--host" => match host.as_str() {
            "space" => (SPACE_HOST, rest),
            "uk" => (UK_HOST, rest),
            _ => return Err(USAGE.to_owned()),
        },
        rest => (UK_HOST, rest),
    };
    let command = match rest {
        [name] if name == "game-versions" => Command::GameVersions,
        [name] if name == "status-levels" => Command::StatusLevels,
        [name, terminal] if name == "prices" => {
            Command::Prices(TerminalId(terminal.parse().map_err(|_| USAGE.to_owned())?))
        }
        [name] if name == "account" => Command::Account,
        [name] if name == "report-access" => Command::ReportAccess,
        [
            name,
            terminal,
            commodity,
            price,
            status,
            scu,
            game_version,
            date_added,
        ] if name == "dry-run" => dry_run_command(
            (terminal, commodity),
            price,
            status,
            scu,
            game_version,
            date_added,
            (None, None),
        )?,
        [
            name,
            terminal,
            commodity,
            price,
            status,
            scu,
            game_version,
            date_added,
            sizes,
        ] if name == "dry-run" => dry_run_command(
            (terminal, commodity),
            price,
            status,
            scu,
            game_version,
            date_added,
            ((sizes != "-").then(|| sizes.to_owned()), None),
        )?,
        [
            name,
            terminal,
            commodity,
            price,
            status,
            scu,
            game_version,
            date_added,
            sizes,
            invalid_commodity,
        ] if name == "dry-run" => dry_run_command(
            (terminal, commodity),
            price,
            status,
            scu,
            game_version,
            date_added,
            (
                (sizes != "-").then(|| sizes.to_owned()),
                Some(parse_u64(invalid_commodity)?),
            ),
        )?,
        _ => return Err(USAGE.to_owned()),
    };
    Ok((host, command))
}

fn dry_run_command(
    (terminal, commodity): (&str, &str),
    price: &str,
    status: &str,
    scu: &str,
    game_version: &str,
    date_added: &str,
    (container_sizes, invalid_commodity): (Option<String>, Option<u64>),
) -> Result<Command, String> {
    Ok(Command::DryRun {
        terminal: parse_u64(terminal)?,
        commodity: parse_u64(commodity)?,
        price: parse_u64(price)?,
        status: status.parse().map_err(|_| USAGE.to_owned())?,
        scu: scu.parse().map_err(|_| USAGE.to_owned())?,
        game_version: game_version.to_owned(),
        date_added: date_added.parse().map_err(|_| USAGE.to_owned())?,
        container_sizes,
        invalid_commodity,
    })
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| USAGE.to_owned())
}

fn run(
    host: &str,
    command: &Command,
    expert_confirmed: bool,
    out: &mut impl Write,
) -> Result<(), String> {
    if matches!(command, Command::DryRun { .. }) && host != UK_HOST {
        return Err("data_submit dry runs require the primary .uk host".to_owned());
    }
    let credentialled = matches!(
        command,
        Command::Account | Command::ReportAccess | Command::DryRun { .. }
    );
    if host != UK_HOST && credentialled {
        if !expert_confirmed {
            return Err("credentialled expert host requires --confirm-expert-host; the selected host receives your secret key and app token".to_owned());
        }
        writeln!(out, "{EXPERT_HOST_WARNING}").map_err(|error| error.to_string())?;
        out.flush().map_err(|error| error.to_string())?;
    }
    let (reader, settings, runtime) = build_reader(host, expert_confirmed)?;
    let lines = runtime.block_on(lines(&reader, &settings, command))?;
    writeln!(out, "host: {host}").map_err(|error| error.to_string())?;
    for line in lines {
        writeln!(out, "{line}").map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn build_reader(
    host: &str,
    expert_confirmed: bool,
) -> Result<(UexReader, ReaderSettings, tokio::runtime::Runtime), String> {
    let identity = ClientIdentity::new("tallyline-uex-spike", env!("CARGO_PKG_VERSION"));
    let settings = ReaderSettings::new(host, identity).map_err(|error| format!("{error:?}"))?;
    let settings = if expert_confirmed {
        settings.with_expert_host_confirmation()
    } else {
        settings
    };
    let reader = UexReader::new(&settings).map_err(|error| format!("{error:?}"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    Ok((reader, settings, runtime))
}

async fn lines(
    reader: &UexReader,
    settings: &ReaderSettings,
    command: &Command,
) -> Result<Vec<String>, String> {
    match command {
        Command::GameVersions => {
            let versions = reader.game_versions().await.map_err(describe)?;
            Ok(vec![format!(
                "live: {}, ptu: {}",
                versions.live,
                versions.ptu.as_deref().unwrap_or("none")
            )])
        }
        Command::StatusLevels => {
            let levels = reader.commodity_status_levels().await.map_err(describe)?;
            let mut lines = vec![format!(
                "buy levels: {}, sell levels: {}",
                levels.buy.len(),
                levels.sell.len()
            )];
            lines.extend(levels.buy.iter().map(|level| status_line("buy", level)));
            lines.extend(levels.sell.iter().map(|level| status_line("sell", level)));
            Ok(lines)
        }
        Command::Prices(terminal) => {
            let rows = reader.price_rows(*terminal).await.map_err(describe)?;
            let mut lines = vec![format!("rows: {}", rows.len())];
            lines.extend(rows.iter().map(price_line));
            Ok(lines)
        }
        Command::Account => {
            let key = secret_key()?;
            let account = reader.account(&key).await.map_err(describe)?;
            Ok(vec![format!(
                "account: datarunner {}, banned {}, staff {} (username withheld)",
                account.is_datarunner, account.is_datarunner_banned, account.is_staff
            )])
        }
        Command::ReportAccess => {
            let key = secret_key()?;
            let token = AppToken::new(&variable(APP_TOKEN_VARIABLE)?);
            let account = reader.account(&key).await.map_err(describe)?;
            let access = reader
                .report_access(&token, &key, &account.username)
                .await
                .map_err(describe)?;
            Ok(vec![format!(
                "data_info with token and key: ok, {} row(s) of the own reports",
                access.rows
            )])
        }
        Command::DryRun {
            terminal,
            commodity,
            price,
            status,
            scu,
            game_version,
            date_added,
            container_sizes,
            invalid_commodity,
        } => {
            use tallyline_adapter_uex::api::UexSubmitter;
            use tallyline_adapter_uex::api::model::{DryRunReport, SubmitPrice};

            let key = secret_key()?;
            let token = AppToken::new(&variable(APP_TOKEN_VARIABLE)?);
            let row = SubmitPrice {
                id_commodity: *commodity,
                price_buy: Some(*price),
                price_sell: None,
                status_buy: Some(*status),
                status_sell: None,
                scu_buy: Some(*scu),
                scu_sell: None,
                container_sizes: container_sizes.clone(),
            };
            let mut prices = vec![row];
            if let Some(id_commodity) = invalid_commodity {
                prices.push(SubmitPrice {
                    id_commodity: *id_commodity,
                    price_buy: Some(1),
                    price_sell: Some(1),
                    status_buy: None,
                    status_sell: None,
                    scu_buy: None,
                    scu_sell: None,
                    container_sizes: None,
                });
            }
            let report = DryRunReport::new(
                *terminal,
                game_version.clone(),
                *date_added,
                screenshot()?,
                prices,
            );
            let submitter = UexSubmitter::new(settings).map_err(|error| format!("{error:?}"))?;
            let success = submitter
                .dry_run(&token, &key, &report)
                .await
                .map_err(describe)?;
            Ok(vec![format!(
                "dry run accepted: {} report id(s), date_added {}",
                success.ids_reports.len(),
                success.date_added
            )])
        }
    }
}

fn status_line(side: &str, level: &StatusLevel) -> String {
    format!(
        "{side} {}: {} / {} / {}",
        level.code, level.name, level.name_short, level.name_abbr
    )
}

fn price_line(row: &PriceRow) -> String {
    let sizes: Vec<String> = row
        .container_sizes
        .iter()
        .map(|size| size.scu().to_string())
        .collect();
    format!(
        "terminal {} commodity {}: side {:?}, status {}, container_sizes [{}]",
        row.terminal.0,
        row.commodity.0,
        row.side,
        row.status
            .map_or_else(|| "none".to_owned(), |status| status.to_string()),
        sizes.join(",")
    )
}

fn secret_key() -> Result<SecretKey, String> {
    Ok(SecretKey::new(&variable(SECRET_KEY_VARIABLE)?))
}

fn variable(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is not set"))
}

fn screenshot() -> Result<Option<String>, String> {
    match env::var(SCREENSHOT_PATH_VARIABLE) {
        Ok(path) => {
            let bytes =
                std::fs::read(path).map_err(|_| "could not read screenshot file".to_owned())?;
            let encoded = base64(&bytes);
            if encoded.len() > 10_000_000 {
                return Err("screenshot exceeds the 10 MB request limit".to_owned());
            }
            Ok(Some(encoded))
        }
        Err(_) => Ok(None),
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk.first().copied().unwrap_or(0);
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(base64_character(ALPHABET, usize::from(first >> 2)));
        encoded.push(base64_character(
            ALPHABET,
            usize::from(((first & 0b11) << 4) | (second >> 4)),
        ));
        encoded.push(if chunk.len() > 1 {
            base64_character(
                ALPHABET,
                usize::from(((second & 0b1111) << 2) | (third >> 6)),
            )
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            base64_character(ALPHABET, usize::from(third & 0b11_1111))
        } else {
            '='
        });
    }
    encoded
}

fn base64_character(alphabet: &[u8; 64], index: usize) -> char {
    alphabet.get(index).copied().map_or('=', char::from)
}

fn describe(error: ReadError) -> String {
    match error {
        ReadError::Rejected {
            http_status,
            code,
            data,
            ..
        } => {
            let detail = match data {
                Some(tallyline_adapter_uex::api::RejectedData::Row(row)) => {
                    format!(", failed row {row}")
                }
                Some(tallyline_adapter_uex::api::RejectedData::Report {
                    id_terminal,
                    id_commodity,
                }) => format!(", report terminal {id_terminal}, commodity {id_commodity}"),
                Some(
                    tallyline_adapter_uex::api::RejectedData::Other
                    | tallyline_adapter_uex::api::RejectedData::QuotaReset { .. },
                )
                | None => String::new(),
            };
            format!("UEX refused the request: HTTP {http_status}, code {code}{detail}")
        }
        ReadError::NotJson { .. }
        | ReadError::Redirect { .. }
        | ReadError::Malformed { .. }
        | ReadError::BodyTooLarge { .. }
        | ReadError::Transport(_) => format!("{error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, SPACE_HOST, TerminalId, UK_HOST, base64, parse};

    fn arguments(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn the_uk_host_is_the_default() {
        assert!(matches!(
            parse(&arguments("game-versions")),
            Ok((UK_HOST, Command::GameVersions))
        ));
    }

    #[test]
    fn the_uk_host_is_chosen_by_flag() {
        assert!(matches!(
            parse(&arguments("--host uk prices 89")),
            Ok((UK_HOST, Command::Prices(TerminalId(89))))
        ));
    }

    #[test]
    fn the_space_host_remains_available_for_comparison() {
        assert!(matches!(
            parse(&arguments("--host space game-versions")),
            Ok((SPACE_HOST, Command::GameVersions))
        ));
    }

    #[test]
    fn dry_run_parses_row_level_container_sizes_and_a_validation_row() {
        assert!(matches!(
            parse(&arguments(
                "dry-run 89 24 136 3 86 4.10.2 1790000000 1,2,4 31"
            )),
            Ok((UK_HOST, Command::DryRun {
                terminal: 89,
                commodity: 24,
                price: 136,
                status: 3,
                scu: 86,
                container_sizes: Some(sizes),
                invalid_commodity: Some(31),
                ..
            })) if sizes == "1,2,4"
        ));
    }

    #[test]
    fn screenshot_bytes_are_base64_encoded_with_standard_padding() {
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(&[0, 255]), "AP8=");
    }

    #[test]
    fn an_unknown_host_command_or_terminal_is_refused() {
        for text in [
            "--host example.com account",
            "data-submit",
            "prices eighty",
            "",
        ] {
            assert!(parse(&arguments(text)).is_err(), "{text}");
        }
    }
}
