// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Public target-tree reconciliation contracts.

use serde_json::json;

#[test]
fn inactive_optional_feature_union_does_not_grant_windows_assembly_activation() {
    let metadata = json!({"packages":[
        {"id":"root","name":"app","version":"0.0.0"},
        {"id":"crypto","name":"digest","version":"0.11.3"},
        {"id":"assembly","name":"cmov","version":"0.5.4"}],
        "workspace_members":["root"],"resolve":{"nodes":[
            {"id":"root","features":[],"deps":[{"name":"renamed","pkg":"crypto"}]},
            {"id":"crypto","features":["default","mac"],"deps":[{"name":"renamed_asm","pkg":"assembly"}]},
            {"id":"assembly","features":[],"deps":[]}]}});
    let projected = crate::target_graph::resolved_metadata(
        &metadata.to_string(),
        "0app v0.0.0|\n1digest v0.11.3|default\n",
    )
    .unwrap();
    let document: serde_json::Value = serde_json::from_str(&projected).unwrap();
    let nodes = document["resolve"]["nodes"].as_array().unwrap();
    let crypto = nodes.iter().find(|node| node["id"] == "crypto").unwrap();
    assert_eq!(crypto["features"], json!(["default"]));
    assert_eq!(crypto["deps"], json!([]));
    assert_eq!(document["resolve"]["nodes"].as_array().unwrap().len(), 2);
}
#[test]
fn malformed_or_incomplete_target_evidence_cannot_hide_an_activation() {
    let metadata = json!({"packages":[
        {"id":"root","name":"app","version":"0.0.0"},
        {"id":"dep","name":"cmov","version":"0.5.4"}],
        "workspace_members":["root"],"resolve":{"nodes":[
            {"id":"root","features":[],"deps":[{"name":"renamed","pkg":"dep"}]},
            {"id":"dep","features":["std"],"deps":[]}]}});
    for tree in [
        "",
        "nothing to print",
        "0cmov v0.5.4|std\n",
        "0app v0.0.0|\n2cmov v0.5.4|std\n",
        "0app v0.0.0|\n1cmov v99.0.0|std\n",
        "0app v0.0.0|\n1cmov v0.5.4|unexpected\n",
        "0app v0.0.0|\n1cmov v0.5.4|std (*)\n",
        "0app v0.0.0|\n1cmov v0.5.4|std,,std\n",
        "0app v0.0.0|\n1cmov v0.5.4 ignored|std\n",
    ] {
        assert!(
            crate::target_graph::resolved_metadata(&metadata.to_string(), tree).is_err(),
            "{tree}"
        );
    }
}

#[test]
fn actual_target_activation_remains_visible_through_renamed_edges_and_every_root() {
    for version in ["0.5.4", "99.0.0"] {
        let metadata = json!({"packages":[
            {"id":"root","name":"app","version":"0.0.0"},
            {"id":"other","name":"tool","version":"0.0.0"},
            {"id":"dep","name":"cmov","version":version}],
            "workspace_members":["root","other"],"resolve":{"nodes":[
                {"id":"root","features":[],"deps":[]},
                {"id":"other","features":[],"deps":[{"name":"renamed","pkg":"dep"}]},
                {"id":"dep","features":[],"deps":[]}]}});
        let tree = format!("0app v0.0.0|\n0tool v0.0.0|\n1cmov v{version}|\n");
        let result = crate::target_graph::resolved_metadata(&metadata.to_string(), &tree).unwrap();
        let projected: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(
            projected["resolve"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["id"] == "dep")
        );
        assert!(
            crate::target_graph::resolved_metadata(&metadata.to_string(), "0app v0.0.0|\n")
                .is_err()
        );
    }
}
