//! The build record, `record.toml` beside the tree: which manifest hash the
//! tree was built from and, per case, whether it was built as specified or
//! skipped on this platform or filesystem and what stands in for it.

use std::fmt::Write as _;

use serde::Deserialize;

use crate::{Error, Result};

/// Whether a case was built as the manifest specifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Built as specified.
    Built,
    /// Not buildable here; `fallback` says what exists instead.
    Skipped,
}

/// The outcome of one case.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseRecord {
    /// The case's name.
    pub name: String,
    /// Built or skipped.
    pub status: Status,
    /// How a special case was built, or why it was skipped.
    #[serde(default)]
    pub detail: String,
    /// For a skipped case, what stands in for it (`entries absent`, `plain
    /// copies`, …).
    #[serde(default)]
    pub fallback: String,
    /// Paths (tree-relative, `%XX`-escaped) of entries that could not be
    /// created at all.
    #[serde(default)]
    pub absent: Vec<String>,
}

/// The record of one build.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// The manifest hash the tree was built from.
    pub hash: String,
    /// The manifest's instant (RFC 3339), exported as `DUSCOPE_TEST_NOW`.
    pub instant: String,
    /// The generator that built it.
    pub generator: String,
    /// Operating system and architecture of the build.
    pub platform: String,
    /// Filesystem type of the tree's volume, as the system names it.
    pub filesystem: String,
    /// Entries present in the tree, the root included.
    pub entries: u64,
    /// Wall time of the build in milliseconds.
    pub build_ms: u64,
    /// Whether mode 000 actually denies access (false when the build ran
    /// with privileges that bypass permission bits, such as root).
    pub unreadable_effective: bool,
    /// One outcome per case, in manifest order.
    #[serde(default, rename = "case")]
    pub cases: Vec<CaseRecord>,
}

impl Record {
    /// Parses a record file.
    pub fn parse(text: &str) -> Result<Record> {
        toml::from_str(text).map_err(|e| Error::new(format!("record: {e}")))
    }

    /// A case's outcome.
    pub fn case(&self, name: &str) -> Option<&CaseRecord> {
        self.cases.iter().find(|c| c.name == name)
    }

    /// The skipped cases.
    pub fn skipped(&self) -> impl Iterator<Item = &CaseRecord> {
        self.cases.iter().filter(|c| c.status == Status::Skipped)
    }

    /// Whether the entry at this escaped path was left out of the tree.
    pub fn is_absent(&self, escaped: &str) -> bool {
        self.cases
            .iter()
            .any(|c| c.absent.iter().any(|a| a == escaped))
    }

    /// The record as TOML.
    pub fn to_toml(&self) -> String {
        let mut out = String::from("# Written by mktree after a successful build; do not edit.\n");
        let _ = writeln!(out, "hash = {}", quote(&self.hash));
        let _ = writeln!(out, "instant = {}", quote(&self.instant));
        let _ = writeln!(out, "generator = {}", quote(&self.generator));
        let _ = writeln!(out, "platform = {}", quote(&self.platform));
        let _ = writeln!(out, "filesystem = {}", quote(&self.filesystem));
        let _ = writeln!(out, "entries = {}", self.entries);
        let _ = writeln!(out, "build_ms = {}", self.build_ms);
        let _ = writeln!(out, "unreadable_effective = {}", self.unreadable_effective);
        for case in &self.cases {
            let _ = writeln!(out, "\n[[case]]");
            let _ = writeln!(out, "name = {}", quote(&case.name));
            let status = match case.status {
                Status::Built => "built",
                Status::Skipped => "skipped",
            };
            let _ = writeln!(out, "status = {}", quote(status));
            if !case.detail.is_empty() {
                let _ = writeln!(out, "detail = {}", quote(&case.detail));
            }
            if !case.fallback.is_empty() {
                let _ = writeln!(out, "fallback = {}", quote(&case.fallback));
            }
            if !case.absent.is_empty() {
                let items: Vec<String> = case.absent.iter().map(|a| quote(a)).collect();
                let _ = writeln!(out, "absent = [{}]", items.join(", "));
            }
        }
        out
    }
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_test_01_record_round_trips() {
        let record = Record {
            hash: "ab".repeat(32),
            instant: "2030-01-01T00:00:00Z".into(),
            generator: "mktree 0.0.1".into(),
            platform: "macos aarch64".into(),
            filesystem: "apfs".into(),
            entries: 150_000,
            build_ms: 6_000,
            unreadable_effective: true,
            cases: vec![
                CaseRecord {
                    name: "clones".into(),
                    status: Status::Built,
                    detail: "cp -c".into(),
                    fallback: String::new(),
                    absent: vec![],
                },
                CaseRecord {
                    name: "non-utf8-names".into(),
                    status: Status::Skipped,
                    detail: "Illegal byte sequence \"x\"\n\u{1b}".into(),
                    fallback: "entries absent".into(),
                    absent: vec!["cases/bytes/caf%E9".into()],
                },
            ],
        };
        let parsed = Record::parse(&record.to_toml()).unwrap();
        assert_eq!(parsed, record);
        assert!(parsed.is_absent("cases/bytes/caf%E9"));
        assert_eq!(parsed.skipped().count(), 1);
    }
}
