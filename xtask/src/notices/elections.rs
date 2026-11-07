// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact reviewed licence elections and bounded SPDX Boolean compatibility.

use std::collections::{BTreeMap, BTreeSet};
use std::{fs, io, path::Path};

const METADATA: &str = "supply-chain/licence-elections.toml";
const MAX_TERMS: usize = 128;
type Alternatives = Vec<BTreeSet<String>>;

struct Election {
    declared: String,
    elected: String,
}

pub(super) struct Elections {
    entries: BTreeMap<(String, String), Election>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl Elections {
    pub(super) fn read(root: &Path) -> io::Result<Self> {
        let document: toml::Table = fs::read_to_string(root.join(METADATA))?
            .parse()
            .map_err(io::Error::other)?;
        if document.keys().any(|key| key != "election") {
            return Err(invalid("unknown licence-election metadata key"));
        }
        let mut entries = BTreeMap::new();
        if let Some(records) = document.get("election") {
            let records = records
                .as_array()
                .ok_or_else(|| invalid("election must be an array"))?;
            for record in records {
                let record = record
                    .as_table()
                    .ok_or_else(|| invalid("election must be a table"))?;
                if record.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "crate" | "version" | "declared" | "elected" | "source" | "reason"
                    )
                }) {
                    return Err(invalid("unknown licence-election field"));
                }
                let text = |key: &str| {
                    record
                        .get(key)
                        .and_then(toml::Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .ok_or_else(|| invalid(format!("election needs {key}")))
                };
                let name = text("crate")?;
                let version = text("version")?;
                let declared = text("declared")?;
                let elected = text("elected")?;
                text("source")?;
                text("reason")?;
                if !compatible(declared, elected)? {
                    return Err(invalid(format!(
                        "{name} {version}: elected licence is not an option of the declaration"
                    )));
                }
                if entries
                    .insert(
                        (name.to_owned(), version.to_owned()),
                        Election {
                            declared: declared.to_owned(),
                            elected: elected.to_owned(),
                        },
                    )
                    .is_some()
                {
                    return Err(invalid("duplicate licence election"));
                }
            }
        }
        Ok(Self { entries })
    }

    pub(super) fn select(
        &self,
        name: &str,
        version: &str,
        declared: &str,
    ) -> io::Result<Option<&str>> {
        let Some(election) = self.entries.get(&(name.to_owned(), version.to_owned())) else {
            if self.entries.keys().any(|(candidate, _)| candidate == name) {
                return Err(invalid(format!(
                    "{name} {version}: election is for another version"
                )));
            }
            return Ok(None);
        };
        if election.declared != declared {
            return Err(invalid(format!(
                "{name} {version}: declaration differs from its reviewed election"
            )));
        }
        Ok(Some(&election.elected))
    }
}

fn compatible(declared: &str, elected: &str) -> io::Result<bool> {
    let declared = Parser::parse(declared)?;
    let elected = Parser::parse(elected)?;
    let available: BTreeSet<_> = declared.iter().flat_map(|clause| clause.iter()).collect();
    Ok(elected.iter().all(|choice| {
        choice.iter().all(|term| available.contains(term))
            && declared.iter().any(|required| required.is_subset(choice))
    }))
}

struct Parser {
    tokens: Vec<String>,
    position: usize,
}

impl Parser {
    fn parse(expression: &str) -> io::Result<Alternatives> {
        let expression = expression.replace('(', " ( ").replace(')', " ) ");
        let tokens: Vec<_> = expression.split_whitespace().map(str::to_owned).collect();
        if tokens.is_empty() || tokens.len() > MAX_TERMS {
            return Err(invalid("empty or overly complex SPDX election expression"));
        }
        let mut parser = Self {
            tokens,
            position: 0,
        };
        let result = parser.alternatives()?;
        if parser.position != parser.tokens.len() {
            return Err(invalid("unexpected SPDX election token"));
        }
        Ok(result)
    }

    fn take(&mut self, token: &str) -> bool {
        if self
            .tokens
            .get(self.position)
            .is_some_and(|value| value == token)
        {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn alternatives(&mut self) -> io::Result<Alternatives> {
        let mut choices = self.conjunction()?;
        while self.take("OR") {
            choices.extend(self.conjunction()?);
            if choices.len() > MAX_TERMS {
                return Err(invalid("SPDX election has too many alternatives"));
            }
        }
        Ok(choices)
    }

    fn conjunction(&mut self) -> io::Result<Alternatives> {
        let mut choices = self.primary()?;
        while self.take("AND") {
            let next = self.primary()?;
            if choices.len().saturating_mul(next.len()) > MAX_TERMS {
                return Err(invalid("SPDX election has too many conjunctions"));
            }
            choices = choices
                .iter()
                .flat_map(|left| {
                    next.iter()
                        .map(move |right| left.union(right).cloned().collect())
                })
                .collect();
        }
        Ok(choices)
    }

    fn primary(&mut self) -> io::Result<Alternatives> {
        if self.take("(") {
            let choices = self.alternatives()?;
            if !self.take(")") {
                return Err(invalid("unclosed SPDX election group"));
            }
            return Ok(choices);
        }
        let mut term = self.identifier()?;
        if self.take("WITH") {
            term = format!("{term} WITH {}", self.identifier()?);
        }
        Ok(vec![BTreeSet::from([term])])
    }

    fn identifier(&mut self) -> io::Result<String> {
        let token = self
            .tokens
            .get(self.position)
            .filter(|token| {
                !matches!(token.as_str(), "AND" | "OR" | "WITH")
                    && token.chars().all(|character| {
                        character.is_ascii_alphanumeric()
                            || matches!(character, '-' | '.' | '+' | ':')
                    })
            })
            .ok_or_else(|| invalid("expected SPDX licence or exception identifier"))?
            .clone();
        self.position += 1;
        Ok(token)
    }
}
