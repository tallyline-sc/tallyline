// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagnostic differences between sampled public response structures.

use std::collections::BTreeSet;

use super::shape::FieldKind;
use super::{PublicContractSchema, status_codes};

impl PublicContractSchema {
    /// Lists field/type and code changes without scalar response values.
    /// Null samples and empty array descendants do not establish removed types or fields.
    #[must_use]
    pub fn changes_from(&self, reference: &Self) -> Vec<String> {
        let label = self.endpoint.resource().trim_end_matches('/');
        if self.endpoint != reference.endpoint {
            return vec!["public contract endpoint changed".to_owned()];
        }
        let mut changes = Vec::new();
        if self.http_status != reference.http_status {
            changes.push(format!(
                "HTTP code changed for {label}: {} -> {}",
                reference.http_status, self.http_status
            ));
        }
        for path in reference.fields.keys() {
            if !self.fields.contains_key(path) && !unobserved_ancestor(path, self) {
                changes.push(format!("field removed: {label}.{path}"));
            }
        }
        for (path, kinds) in &self.fields {
            match reference.fields.get(path) {
                Some(previous) => {
                    let previous = non_null(previous);
                    let current = non_null(kinds);
                    if !previous.is_empty() && !current.is_subset(&previous) {
                        changes.push(format!(
                            "field type changed: {label}.{path} ({} -> {})",
                            labels(&previous),
                            labels(&current),
                        ));
                    }
                }
                None => changes.push(format!("field added: {label}.{path}")),
            }
        }
        changes.extend(status_codes::changes(
            self.status_codes.as_ref(),
            reference.status_codes.as_ref(),
            label,
        ));
        changes
    }
}

fn unobserved_ancestor(path: &str, schema: &PublicContractSchema) -> bool {
    path.char_indices()
        .filter(|(_, character)| matches!(character, '.' | '['))
        .any(|(index, _)| {
            let Some(ancestor) = path.get(..index) else {
                return false;
            };
            schema.fields.get(ancestor).is_some_and(|kinds| {
                kinds.iter().all(|kind| *kind == FieldKind::Null)
                    || (kinds.contains(&FieldKind::Array)
                        && !schema.fields.contains_key(&format!("{ancestor}[]")))
            })
        })
}

fn non_null(kinds: &BTreeSet<FieldKind>) -> BTreeSet<FieldKind> {
    kinds
        .iter()
        .copied()
        .filter(|kind| *kind != FieldKind::Null)
        .collect()
}

fn labels(kinds: &BTreeSet<FieldKind>) -> String {
    kinds
        .iter()
        .map(|kind| match kind {
            FieldKind::Null => "null",
            FieldKind::Boolean => "boolean",
            FieldKind::Number => "number",
            FieldKind::String => "string",
            FieldKind::Object => "object",
            FieldKind::Array => "array",
        })
        .collect::<Vec<_>>()
        .join("|")
}
