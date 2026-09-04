use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

const WORKSPACE_CRATES: [&str; 7] = [
    "quay-core",
    "quay-store",
    "quay-forge",
    "quay-sync",
    "quay-git",
    "quay-app",
    "xtask",
];

fn permitted_dependencies(crate_name: &str) -> BTreeSet<&'static str> {
    match crate_name {
        "quay-core" => BTreeSet::new(),
        "quay-store" => BTreeSet::from(["quay-core"]),
        "quay-forge" => BTreeSet::from(["quay-core"]),
        "quay-sync" => BTreeSet::from(["quay-core", "quay-store", "quay-forge"]),
        "quay-git" => BTreeSet::from(["quay-core"]),
        "quay-app" => BTreeSet::from([
            "quay-core",
            "quay-store",
            "quay-forge",
            "quay-sync",
            "quay-git",
        ]),
        "xtask" => BTreeSet::from(["quay-core", "quay-store", "quay-forge", "quay-sync"]),
        _ => BTreeSet::new(),
    }
}

fn internal_dependency_graph() -> BTreeMap<String, BTreeSet<String>> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata must run");
    assert!(output.status.success(), "cargo metadata failed");

    let metadata: Value = serde_json::from_slice(&output.stdout).expect("metadata must be JSON");
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .expect("metadata must list packages");

    let workspace: BTreeSet<&str> = WORKSPACE_CRATES.into_iter().collect();
    let mut graph = BTreeMap::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .expect("a package must have a name");
        if !workspace.contains(name) {
            continue;
        }
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|dependency| dependency.get("name").and_then(Value::as_str))
            .filter(|dependency| workspace.contains(dependency))
            .map(str::to_owned)
            .collect();
        graph.insert(name.to_owned(), dependencies);
    }
    graph
}

#[test]
fn every_crate_of_the_specification_is_present_in_the_workspace() {
    let graph = internal_dependency_graph();
    for expected in WORKSPACE_CRATES {
        assert!(graph.contains_key(expected), "{expected} is missing");
    }
}

#[test]
fn no_crate_depends_on_a_crate_it_is_not_allowed_to_know() {
    let graph = internal_dependency_graph();
    for (crate_name, dependencies) in &graph {
        let permitted = permitted_dependencies(crate_name);
        for dependency in dependencies {
            assert!(
                permitted.contains(dependency.as_str()),
                "{crate_name} must not depend on {dependency}"
            );
        }
    }
}

#[test]
fn the_domain_crate_depends_on_no_other_crate_of_the_workspace() {
    let graph = internal_dependency_graph();
    assert_eq!(graph.get("quay-core"), Some(&BTreeSet::new()));
}

#[test]
fn the_store_and_the_forge_do_not_know_each_other() {
    let graph = internal_dependency_graph();
    let store = graph.get("quay-store").cloned().unwrap_or_default();
    let forge = graph.get("quay-forge").cloned().unwrap_or_default();
    assert!(!store.contains("quay-forge"));
    assert!(!forge.contains("quay-store"));
}

#[test]
fn nothing_in_the_core_crates_depends_on_the_application_binary() {
    let graph = internal_dependency_graph();
    for crate_name in [
        "quay-core",
        "quay-store",
        "quay-forge",
        "quay-sync",
        "quay-git",
    ] {
        let dependencies = graph.get(crate_name).cloned().unwrap_or_default();
        assert!(!dependencies.contains("quay-app"), "{crate_name}");
    }
}
