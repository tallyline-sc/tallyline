// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed public-route inspection outside credentialled command dispatch.

use std::io::Write;

use tallyline_adapter_uex::api::ReadError;
use tallyline_adapter_uex::api::model::{PublicContractEndpoint, PublicContractSnapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContractMode {
    Snapshot,
    Check,
}

pub(super) fn command(arguments: &[String]) -> Option<Result<ContractMode, String>> {
    if !arguments
        .iter()
        .any(|value| matches!(value.as_str(), "contract-snapshot" | "contract-check"))
    {
        return None;
    }
    let name = match arguments {
        [name] => name,
        [flag, host, name] if flag == "--host" && host == "uk" => name,
        _ => {
            return Some(Err(
                "public contract commands require the primary .uk host and no extra arguments"
                    .to_owned(),
            ));
        }
    };
    Some(match name.as_str() {
        "contract-snapshot" => Ok(ContractMode::Snapshot),
        "contract-check" => Ok(ContractMode::Check),
        _ => Err("unsupported public contract command".to_owned()),
    })
}

pub(super) fn run(mode: ContractMode, out: &mut impl Write) -> Result<(), String> {
    let (reader, _, runtime) = super::build_reader(super::UK_HOST, false)
        .map_err(|_| "public contract client could not initialise".to_owned())?;
    let mut schemas = Vec::new();
    for endpoint in PublicContractEndpoint::ALL {
        let payload = runtime
            .block_on(reader.public_contract_sample(endpoint))
            .map_err(|error| read_problem(endpoint, error))?;
        schemas.push(
            payload
                .into_schema()
                .map_err(|error| format!("public contract {endpoint:?}: {error:?}"))?,
        );
    }
    let snapshot = PublicContractSnapshot::new(schemas)
        .map_err(|_| "public contract snapshot is incomplete".to_owned())?;
    write_snapshot(mode, &snapshot, out)
}

pub(super) fn write_snapshot(
    mode: ContractMode,
    snapshot: &PublicContractSnapshot,
    out: &mut impl Write,
) -> Result<(), String> {
    let metadata = match mode {
        ContractMode::Snapshot => snapshot
            .to_json()
            .map_err(|_| "public contract metadata could not be encoded".to_owned())?,
        ContractMode::Check => {
            let reference =
                PublicContractSnapshot::from_json(include_str!("../contracts/public-v1.json"))
                    .map_err(|_| "the bundled public metadata baseline is invalid".to_owned())?;
            let changes = snapshot.changes_from(&reference);
            if !changes.is_empty() {
                for change in changes {
                    writeln!(out, "{change}")
                        .map_err(|_| "could not write public contract differences".to_owned())?;
                }
                return Err("observed public field/type or status-code changes".to_owned());
            }
            "No observed field/type/code changes in five public reference samples.".to_owned()
        }
    };
    writeln!(out, "{metadata}").map_err(|_| "could not write public contract metadata".to_owned())
}

fn read_problem(endpoint: PublicContractEndpoint, error: ReadError) -> String {
    let problem = match error {
        ReadError::Rejected { http_status, .. } => format!("rejected HTTP code {http_status}"),
        ReadError::NotJson { http_status, .. } => format!("non-JSON HTTP code {http_status}"),
        ReadError::Redirect { http_status } => format!("refused redirect HTTP code {http_status}"),
        ReadError::Malformed { .. } => "invalid transport metadata".to_owned(),
        ReadError::BodyTooLarge { .. } => "response size cap exceeded".to_owned(),
        ReadError::Transport(_) => "transport failed".to_owned(),
    };
    format!("public contract {endpoint:?}: {problem}")
}
