// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reconciles target-resolved Cargo trees with conservative metadata.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

/// The source-reviewed Cargo build used to collect target evidence.
pub const REVIEWED_CARGO: &str = "cargo 1.99.0 (5f94df478 2026-08-27)";

/// Projects metadata onto a complete, depth-prefixed workspace Cargo target tree.
///
/// Features and edges are unioned across host and target occurrences.
///
/// # Errors
/// Returns an error for malformed trees, missing roots, unknown identities or edges.
pub fn resolved_metadata(metadata: &str, tree: &str) -> Result<String, String> {
    let mut document: Value = serde_json::from_str(metadata).map_err(|error| error.to_string())?;
    let packages = array(&document, "packages")?;
    let mut identities = BTreeMap::new();
    for package in packages {
        let key = format!("{} v{}", text(package, "name")?, text(package, "version")?);
        if identities
            .insert(key, text(package, "id")?.to_owned())
            .is_some()
        {
            return Err("ambiguous Cargo tree package identity".to_owned());
        }
    }
    let resolve = document
        .get("resolve")
        .ok_or("missing metadata resolve graph")?;
    let mut nodes = BTreeMap::new();
    for node in array(resolve, "nodes")? {
        if nodes
            .insert(text(node, "id")?.to_owned(), node.clone())
            .is_some()
        {
            return Err("duplicate metadata resolve node".to_owned());
        }
    }
    let roots = array(&document, "workspace_members")?
        .iter()
        .map(|root| {
            root.as_str()
                .map(str::to_owned)
                .ok_or("invalid workspace root")
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if roots.is_empty() {
        return Err("missing workspace roots".to_owned());
    }
    let mut seen_roots = BTreeSet::new();
    let mut active_features = BTreeMap::<String, BTreeSet<String>>::new();
    let mut active_edges = BTreeMap::<String, BTreeSet<String>>::new();
    let mut stack = Vec::<String>::new();
    let mut expanded = BTreeSet::new();
    let mut previous_duplicate = false;
    for line in tree.lines().filter(|line| !line.is_empty()) {
        let (label, features) = line.split_once('|').ok_or("malformed Cargo tree line")?;
        let digit_count = label.bytes().take_while(u8::is_ascii_digit).count();
        let depth: usize = label[..digit_count]
            .parse()
            .map_err(|_| "missing Cargo tree depth")?;
        let mut words = label[digit_count..].split_whitespace();
        let name = words.next().ok_or("missing Cargo tree name")?;
        let version = words.next().ok_or("missing Cargo tree version")?;
        let suffix = words.collect::<Vec<_>>().join(" ");
        if !suffix.is_empty()
            && suffix != "(proc-macro)"
            && !(suffix.starts_with('(') && suffix.ends_with(')'))
        {
            return Err("malformed Cargo tree package suffix".to_owned());
        }
        let id = identities
            .get(&format!("{name} {version}"))
            .ok_or("unknown Cargo tree package")?;
        if depth > stack.len() || (previous_duplicate && depth == stack.len()) {
            return Err("Cargo tree skips a parent depth".to_owned());
        }
        stack.truncate(depth);
        if let Some(parent) = stack.last() {
            let parent_node = nodes.get(parent).ok_or("missing parent node")?;
            if !array(parent_node, "deps")?
                .iter()
                .any(|dep| dep["pkg"] == id.as_str())
            {
                return Err(format!(
                    "Cargo tree edge absent from metadata: {parent} -> {id}"
                ));
            }
            active_edges
                .entry(parent.clone())
                .or_default()
                .insert(id.clone());
        } else if !roots.contains(id) {
            return Err("Cargo tree has a foreign root".to_owned());
        } else {
            seen_roots.insert(id.clone());
        }
        let duplicate = features.ends_with(" (*)");
        let features = features.strip_suffix(" (*)").unwrap_or(features);
        let feature_set: BTreeSet<_> = features.split(',').map(str::to_owned).collect();
        if !features.is_empty()
            && (feature_set.contains("") || feature_set.len() != features.split(',').count())
        {
            return Err("malformed Cargo tree features".to_owned());
        }
        if duplicate && !expanded.contains(&(id.clone(), feature_set.clone())) {
            return Err("Cargo tree duplicate has no expanded node".to_owned());
        }
        expanded.insert((id.clone(), feature_set));
        previous_duplicate = duplicate;
        let node = nodes
            .get(id)
            .ok_or("Cargo tree package has no resolve node")?;
        let full_features = array(node, "features")?;
        let active = active_features.entry(id.clone()).or_default();
        for feature in features.split(',').filter(|feature| !feature.is_empty()) {
            if !full_features
                .iter()
                .any(|full| full.as_str() == Some(feature))
            {
                return Err("Cargo tree feature absent from metadata".to_owned());
            }
            active.insert(feature.to_owned());
        }
        stack.push(id.clone());
    }
    if seen_roots != roots {
        return Err("Cargo tree does not contain every workspace root".to_owned());
    }
    let mut projected = Vec::new();
    for (id, features) in active_features {
        let mut node = nodes.remove(&id).ok_or("missing active node")?;
        *node.get_mut("features").ok_or("missing active features")? = json!(features);
        let edges = active_edges.remove(&id).unwrap_or_default();
        let deps = json!(
            array(&node, "deps")?
                .iter()
                .filter(|dep| { dep["pkg"].as_str().is_some_and(|id| edges.contains(id)) })
                .collect::<Vec<_>>()
        );
        *node.get_mut("deps").ok_or("missing active dependencies")? = deps;
        projected.push(node);
    }
    *document
        .get_mut("resolve")
        .and_then(|resolve| resolve.get_mut("nodes"))
        .ok_or("missing resolve nodes")? = json!(projected);
    serde_json::to_string(&document).map_err(|error| error.to_string())
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing metadata array {key}"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("missing metadata string {key}"))
}
