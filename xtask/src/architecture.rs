// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cargo metadata checks for the architecture context map.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// A violation of the documented Cargo context map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation(pub String);

/// Checks workspace dependency edges in `cargo metadata` JSON.
#[must_use]
pub fn violations(metadata: &str) -> Vec<Violation> {
    let Ok(document) = serde_json::from_str::<Value>(metadata) else {
        return vec![Violation("cargo metadata is not valid JSON".to_owned())];
    };
    let Some(packages) = document.get("packages").and_then(Value::as_array) else {
        return vec![Violation("cargo metadata has no packages array".to_owned())];
    };
    let workspace_ids: BTreeSet<&str> = document
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let members: BTreeMap<&str, &Value> = packages
        .iter()
        .filter(|package| {
            package
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| workspace_ids.contains(id))
        })
        .filter_map(|package| Some((package.get("name")?.as_str()?, package)))
        .collect();
    let mut result = Vec::new();
    let names: BTreeSet<&str> = members.keys().copied().collect();
    let mut graph = BTreeMap::<&str, BTreeSet<&str>>::new();

    for (name, package) in &members {
        let mut edges = BTreeSet::new();
        if let Some(dependencies) = package.get("dependencies").and_then(Value::as_array) {
            for dependency in dependencies {
                let Some(dep_name) = dependency.get("name").and_then(Value::as_str) else {
                    result.push(Violation(format!("{name} has a dependency without a name")));
                    continue;
                };
                let kind = dependency.get("kind").and_then(Value::as_str);
                if kind.is_some_and(|kind| kind == "dev") {
                    continue;
                }
                if let Some(workspace_dependency) =
                    names.iter().find(|candidate| **candidate == dep_name)
                {
                    edges.insert(*workspace_dependency);
                } else if is_core(name) && !PURE_CORE_DEPENDENCIES.contains(&dep_name) {
                    result.push(Violation(format!(
                        "core crate {name} has forbidden external dependency {dep_name}"
                    )));
                }
            }
        }
        graph.insert(name, edges);
    }

    for (name, edges) in &graph {
        if let Some(allowed) = allowed_edges(name) {
            for dependency in edges.difference(&allowed) {
                result.push(Violation(format!(
                    "{name} has dependency outside the context map: {dependency}"
                )));
            }
        } else {
            result.push(Violation(format!("unclassified workspace crate {name}")));
        }
        if is_adapter(name) {
            for dependency in edges.iter().filter(|dependency| is_adapter(dependency)) {
                result.push(Violation(format!(
                    "adapter {name} depends on adapter {dependency}"
                )));
            }
        }
        if *name == "tallyline-submission" && edges.contains("tallyline-reporting") {
            result.push(Violation(
                "submission must not depend on reporting; use workflows".to_owned(),
            ));
        }
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for name in graph.keys().copied() {
        visit(name, &graph, &mut visiting, &mut visited, &mut result);
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result.dedup();
    result
}

const PURE_CORE_DEPENDENCIES: &[&str] = &["tracing", "rust_decimal"];

pub(crate) fn is_core(name: &str) -> bool {
    matches!(
        name,
        "tallyline-shared-kernel"
            | "tallyline-game"
            | "tallyline-reference-data"
            | "tallyline-capture"
            | "tallyline-recognition"
            | "tallyline-reporting"
            | "tallyline-submission"
            | "tallyline-workflows"
    )
}

fn is_adapter(name: &str) -> bool {
    name.starts_with("tallyline-adapter-")
}

fn allowed_edges(name: &str) -> Option<BTreeSet<&'static str>> {
    let allowed: &[&str] = match name {
        "xtask" => &[],
        "tallyline-shared-kernel" => &[],
        "tallyline-game" | "tallyline-reference-data" => &["tallyline-shared-kernel"],
        "tallyline-capture" => &["tallyline-game", "tallyline-shared-kernel"],
        "tallyline-recognition" => &[
            "tallyline-reference-data",
            "tallyline-game",
            "tallyline-shared-kernel",
        ],
        "tallyline-reporting" => &[
            "tallyline-recognition",
            "tallyline-reference-data",
            "tallyline-game",
            "tallyline-shared-kernel",
        ],
        "tallyline-submission" => &[
            "tallyline-reference-data",
            "tallyline-game",
            "tallyline-shared-kernel",
        ],
        "tallyline-workflows" => &[
            "tallyline-capture",
            "tallyline-recognition",
            "tallyline-reporting",
            "tallyline-submission",
            "tallyline-reference-data",
            "tallyline-game",
            "tallyline-shared-kernel",
        ],
        "tallyline-ui" => &[
            "tallyline-egui",
            "tallyline-workflows",
            "tallyline-capture",
            "tallyline-recognition",
            "tallyline-reporting",
            "tallyline-submission",
            "tallyline-reference-data",
            "tallyline-game",
            "tallyline-shared-kernel",
        ],
        "tallyline-egui" => &[],
        "tallyline-app" => &[
            "tallyline-shared-kernel",
            "tallyline-game",
            "tallyline-reference-data",
            "tallyline-capture",
            "tallyline-recognition",
            "tallyline-reporting",
            "tallyline-submission",
            "tallyline-workflows",
            "tallyline-ui",
            "tallyline-egui",
            "tallyline-adapter-uex",
            "tallyline-adapter-storage",
            "tallyline-adapter-ocr",
            "tallyline-adapter-vlm",
            "tallyline-adapter-files",
            "tallyline-adapter-platform",
        ],
        "ocr-eval" => &[
            "tallyline-shared-kernel",
            "tallyline-recognition",
            "tallyline-reference-data",
            "tallyline-adapter-ocr",
            "tallyline-adapter-vlm",
            "tallyline-adapter-files",
            "tallyline-adapter-uex",
        ],
        "tallyline-adapter-uex" => &[
            "tallyline-shared-kernel",
            "tallyline-reference-data",
            "tallyline-submission",
        ],
        "tallyline-adapter-storage" => &[
            "tallyline-shared-kernel",
            "tallyline-game",
            "tallyline-reference-data",
            "tallyline-capture",
            "tallyline-recognition",
            "tallyline-reporting",
            "tallyline-submission",
            "tallyline-workflows",
        ],
        "tallyline-adapter-ocr" | "tallyline-adapter-vlm" => {
            &["tallyline-shared-kernel", "tallyline-recognition"]
        }
        "tallyline-adapter-files" => &[
            "tallyline-shared-kernel",
            "tallyline-capture",
            "tallyline-game",
            "tallyline-reporting",
            "tallyline-workflows",
        ],
        "tallyline-adapter-platform" => &[
            "tallyline-shared-kernel",
            "tallyline-game",
            "tallyline-submission",
            "tallyline-workflows",
        ],
        _ => return None,
    };
    Some(allowed.iter().copied().collect())
}

fn visit<'a>(
    name: &'a str,
    graph: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
    result: &mut Vec<Violation>,
) {
    if visited.contains(name) {
        return;
    }
    if !visiting.insert(name) {
        result.push(Violation(format!("dependency cycle includes {name}")));
        return;
    }
    if let Some(edges) = graph.get(name) {
        for dependency in edges {
            visit(dependency, graph, visiting, visited, result);
        }
    }
    visiting.remove(name);
    visited.insert(name);
}

#[cfg(test)]
mod tests {
    use super::violations;

    fn fixture(name: &str) -> String {
        match name {
            "valid" => include_str!("../fixtures/architecture/valid.json").to_owned(),
            "outside-map" => include_str!("../fixtures/architecture/outside-map.json").to_owned(),
            "cycle" => include_str!("../fixtures/architecture/cycle.json").to_owned(),
            "forbidden" => include_str!("../fixtures/architecture/forbidden.json").to_owned(),
            "adapter-leaf" => include_str!("../fixtures/architecture/adapter-leaf.json").to_owned(),
            "submission-reporting" => {
                include_str!("../fixtures/architecture/submission-reporting.json").to_owned()
            }
            _ => String::new(),
        }
    }

    #[test]
    fn eval_composes_shared_values_without_allowing_reverse_core_dependencies() {
        let mut metadata = serde_json::json!({
            "workspace_members": ["eval", "kernel"],
            "packages": [
                {"id": "eval", "name": "ocr-eval", "dependencies": [
                    {"name": "tallyline-shared-kernel", "kind": null}
                ]},
                {"id": "kernel", "name": "tallyline-shared-kernel", "dependencies": []}
            ]
        });
        assert!(violations(&metadata.to_string()).is_empty());
        metadata["packages"][1]["dependencies"] = serde_json::json!([
            {"name": "ocr-eval", "kind": null}
        ]);
        assert!(violations(&metadata.to_string()).iter().any(|item| {
            item.0
                .contains("tallyline-shared-kernel has dependency outside the context map")
        }));
    }

    #[test]
    fn accepts_context_map_fixture() {
        assert!(violations(&fixture("valid")).is_empty());
    }

    #[test]
    fn rejects_dependency_outside_context_map() {
        assert!(
            violations(&fixture("outside-map"))
                .iter()
                .any(|item| item.0.contains("outside the context map"))
        );
    }

    #[test]
    fn rejects_cycles() {
        assert!(
            violations(&fixture("cycle"))
                .iter()
                .any(|item| item.0.contains("cycle"))
        );
    }

    #[test]
    fn rejects_forbidden_external_dependencies() {
        assert!(
            violations(&fixture("forbidden"))
                .iter()
                .any(|item| item.0.contains("forbidden external dependency"))
        );
    }

    #[test]
    fn rejects_adapter_dependencies() {
        let findings = violations(&fixture("adapter-leaf"));
        assert!(
            findings
                .iter()
                .any(|item| item.0.contains("depends on adapter")),
            "{findings:?}"
        );
    }

    #[test]
    fn rejects_submission_reporting_dependency() {
        assert!(
            violations(&fixture("submission-reporting"))
                .iter()
                .any(|item| item.0.contains("submission must not depend on reporting"))
        );
    }
}
