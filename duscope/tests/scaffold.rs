//! P0-REPO-01: the workspace shape and the placeholder binary.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package};

/// The published crates and the workspace crates each may depend on, from the
/// spec's "Architecture and crate boundaries". Dev-dependencies are exempt, and
/// unpublished workspace members (the testkit, the fixture generator) may be
/// dev-dependencies only.
const ALLOWED: &[(&str, &[&str])] = &[
    ("duscope-walk", &[]),
    ("duscope-core", &["duscope-walk"]),
    ("duscope-treemap", &["duscope-core"]),
    ("duscope-tui", &["duscope-treemap", "duscope-core"]),
    ("duscope-mcp", &["duscope-core"]),
    (
        "duscope",
        &[
            "duscope-walk",
            "duscope-core",
            "duscope-treemap",
            "duscope-tui",
            "duscope-mcp",
        ],
    ),
];

fn metadata() -> Metadata {
    MetadataCommand::new()
        .manifest_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.toml"))
        .no_deps()
        .exec()
        .unwrap()
}

fn is_published(package: &Package) -> bool {
    package
        .publish
        .as_ref()
        .is_none_or(|registries| !registries.is_empty())
}

#[test]
fn p0_repo_01_crate_dependency_direction() {
    let metadata = metadata();
    let packages = metadata.workspace_packages();
    let members: BTreeSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let allowed: BTreeMap<&str, &[&str]> = ALLOWED.iter().copied().collect();

    let published: BTreeSet<&str> = packages
        .iter()
        .filter(|p| is_published(p))
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        published,
        allowed.keys().copied().collect(),
        "the published workspace crates are the six of the architecture"
    );

    let mut violations = Vec::new();
    for package in packages.iter().filter(|p| is_published(p)) {
        let ok = allowed[package.name.as_str()];
        for dep in &package.dependencies {
            let internal = members.contains(dep.name.as_str());
            if internal
                && dep.kind != DependencyKind::Development
                && !ok.contains(&dep.name.as_str())
            {
                violations.push(format!("{} -> {}", package.name, dep.name));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "dependency edges against the architecture: {violations:?}"
    );
}

#[test]
fn p0_repo_01_crates_share_the_workspace_version() {
    let metadata = metadata();
    let binary = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == "duscope")
        .unwrap();
    for package in metadata
        .workspace_packages()
        .iter()
        .filter(|p| is_published(p))
    {
        assert_eq!(
            package.version, binary.version,
            "{} is not at the workspace version",
            package.name
        );
    }
}

#[test]
fn p0_repo_01_version_prints_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_duscope"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success(), "exit status {}", output.status);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("duscope {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}
