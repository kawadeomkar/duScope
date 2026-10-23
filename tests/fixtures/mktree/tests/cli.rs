//! P0-TEST-01: the mktree binary prints a manifest hash that does not depend
//! on where or how it runs, and that equals the hash the manifest records.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::process::Command;

use sha2::{Digest, Sha256};

fn mktree() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mktree"))
}

fn stdout(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{cmd:?}: {}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn p0_test_01_manifest_hash_is_stable() {
    let plan = mktree::Plan::embedded().unwrap();
    let hash = plan.hash();
    assert_eq!(
        hash, plan.recorded_hash,
        "the expanded manifest changed: if the change to the tree is intended, \
         record the new hash in tests/fixtures/mktree/manifest.toml (its own TEST package)"
    );
    assert_eq!(
        mktree::Plan::embedded().unwrap().hash(),
        hash,
        "expansion is deterministic"
    );

    // The printed hash ignores the working directory, the time zone, the
    // locale and the rest of the environment.
    let elsewhere = tempfile::tempdir().unwrap();
    let default_env = stdout(mktree().arg("--print-hash"));
    let odd = stdout(
        mktree()
            .arg("--print-hash")
            .env_clear()
            .env("TZ", "America/Los_Angeles")
            .env("LC_ALL", "fr_FR.UTF-8")
            .env("HOME", elsewhere.path())
            .current_dir(elsewhere.path()),
    );
    assert_eq!(default_env, format!("{hash}\n"));
    assert_eq!(odd, default_env);
}

#[test]
fn p0_test_01_hash_covers_the_printed_plan() {
    let plan = stdout(mktree().arg("--print-plan"));
    let hash = stdout(mktree().arg("--print-hash"));
    let digest = mktree::plan::hex(&Sha256::digest(plan.as_bytes()));
    assert_eq!(format!("{digest}\n"), hash);
    assert!(plan.starts_with("mktree expanded manifest, format 1\n"));
    assert!(plan.contains("\ninstant 2037-01-01T00:00:00Z\n"));
}

#[test]
fn p0_test_01_mktree_refuses_bad_arguments() {
    for args in [
        &["--bogus"][..],
        &["--print-hash", "--list"],
        &["--force", "--print-hash"],
        &["--out"],
    ] {
        let out = mktree().args(args).output().unwrap();
        assert!(!out.status.success(), "{args:?} succeeded");
        assert!(out.stdout.is_empty(), "{args:?} wrote to stdout");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("usage: mktree"),
            "{args:?}"
        );
    }
}
