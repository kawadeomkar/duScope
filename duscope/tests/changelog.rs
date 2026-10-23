//! P0-DOC-01: `scripts/changelog.py`, the one parser of the changelog fragments.
//!
//! `check` validates `changelog.d/<ID>.md` fragments with the rule of the
//! `changelog` CI job; `assemble` merges them into a dated section of
//! `CHANGELOG.md` at release and deletes them. These tests run the script on
//! the fragments under `tests/fixtures/changelog/`, laid out as a repository in
//! a scratch directory under `CARGO_TARGET_TMPDIR`. They read repository files
//! and run Python; they never run the duscope binary.
//!
//! The interpreter is `$PYTHON` when it is set, else `/usr/bin/python3` when it
//! exists (the system Python on macOS and Linux), else `python3` from `PATH`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The release date every assembly in these tests uses.
const DATE: &str = "2026-11-02";

/// Each fragment under `malformed/`, with the line and the message `check`
/// reports for it.
const MALFORMED: &[(&str, usize, &str)] = &[
    (
        "unknown-section.md",
        1,
        "unknown section '### Notes': use one of Added, Changed, Deprecated, Removed, Fixed, Security",
    ),
    (
        "version-heading.md",
        1,
        "only '### <Section>' headings belong in a fragment",
    ),
    (
        "deeper-heading.md",
        5,
        "only '### <Section>' headings belong in a fragment",
    ),
    (
        "text-before-heading.md",
        1,
        "text before the first '### <Section>' heading",
    ),
    (
        "duplicate-section.md",
        5,
        "section '### Added' appears twice",
    ),
    (
        "empty-section.md",
        1,
        "section '### Added' has no '- ' items",
    ),
    (
        "star-bullet.md",
        3,
        "expected a '- ' list item or an indented continuation line",
    ),
    (
        "orphan-continuation.md",
        3,
        "expected a '- ' list item or an indented continuation line",
    ),
    ("no-heading.md", 1, "no '### <Section>' heading"),
];

/// A fragment that is not valid UTF-8 (`café` in Latin-1), written by the tests
/// rather than committed.
const NOT_UTF8: &[u8] = b"### Added\n\n- caf\xe9\n";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn script() -> PathBuf {
    repo_root().join("scripts/changelog.py")
}

fn changelog_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/changelog")
        .join(name)
}

fn python() -> OsString {
    if let Some(python) = std::env::var_os("PYTHON").filter(|p| !p.is_empty()) {
        return python;
    }
    let system = Path::new("/usr/bin/python3");
    if system.is_file() {
        system.into()
    } else {
        "python3".into()
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The markdown files of a directory, by name: `*.md`, matched case-sensitively
/// as the script and the CI job match fragments.
fn markdown_files(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| Path::new(name).extension().is_some_and(|e| e == "md"))
        .collect()
}

/// A directory under `CARGO_TARGET_TMPDIR`, removed when dropped.
#[derive(Debug)]
struct Scratch(PathBuf);

impl Scratch {
    fn new(test: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p0_doc_01-{test}-{}-{n}", std::process::id()));
        // A directory left by a killed run of the same pid would hold stale files.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Lays a fixture out as a repository root: its `CHANGELOG.md`, and its
    /// `fragments/` as `changelog.d/`.
    fn lay_out(&self, fixture: &str) {
        let source = changelog_fixture(fixture);
        fs::copy(source.join("CHANGELOG.md"), self.0.join("CHANGELOG.md")).unwrap();
        let fragments = self.0.join("changelog.d");
        fs::create_dir(&fragments).unwrap();
        for name in markdown_files(&source.join("fragments")) {
            fs::copy(source.join("fragments").join(&name), fragments.join(&name)).unwrap();
        }
    }

    fn changelog(&self) -> String {
        read(&self.0.join("CHANGELOG.md"))
    }

    fn fragments(&self) -> BTreeSet<String> {
        markdown_files(&self.0.join("changelog.d"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `python <program> <args>` in `cwd`; with `github`, as inside a GitHub
/// Actions step, which switches the script's errors to workflow annotations.
fn python_run(program: &Path, cwd: &Path, args: &[&str], github: bool) -> Output {
    let python = python();
    let mut command = Command::new(&python);
    command.arg(program).args(args).current_dir(cwd);
    if github {
        command.env("GITHUB_ACTIONS", "true");
    } else {
        command.env_remove("GITHUB_ACTIONS");
    }
    command.output().unwrap_or_else(|e| {
        panic!(
            "cannot run {}: {e}; set PYTHON to a Python 3.9 or later interpreter",
            python.to_string_lossy()
        )
    })
}

/// `scripts/changelog.py <args>` in `cwd`, outside GitHub Actions.
fn changelog_py(cwd: &Path, args: &[&str]) -> Output {
    python_run(&script(), cwd, args, false)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn describe(out: &Output) -> String {
    format!(
        "exit status {:?}\n--- stdout\n{}--- stderr\n{}",
        out.status.code(),
        stdout(out),
        stderr(out)
    )
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

#[test]
fn p0_doc_01_assembly_merges_sections_in_keep_a_changelog_order() {
    let scratch = Scratch::new("merge");
    scratch.lay_out("merge");

    let out = changelog_py(scratch.path(), &["assemble", "0.1.0", "--date", DATE]);

    assert!(out.status.success(), "{}", describe(&out));
    assert_eq!(
        scratch.changelog(),
        read(&changelog_fixture("merge/expected.md")),
        "the assembled CHANGELOG.md differs from merge/expected.md"
    );
    assert_eq!(
        scratch.fragments(),
        set(&["README.md"]),
        "assembly deletes the fragments it consumed and keeps changelog.d/README.md"
    );
}

#[test]
fn p0_doc_01_assembly_dates_the_pending_section_and_keeps_its_notes() {
    let scratch = Scratch::new("pending");
    scratch.lay_out("pending");

    // The tag form of the version is accepted too.
    let out = changelog_py(scratch.path(), &["assemble", "v0.0.1", "--date", DATE]);

    assert!(out.status.success(), "{}", describe(&out));
    assert_eq!(
        scratch.changelog(),
        read(&changelog_fixture("pending/expected.md")),
        "the assembled CHANGELOG.md differs from pending/expected.md"
    );
    assert!(scratch.fragments().is_empty(), "the fragments are deleted");
}

#[test]
fn p0_doc_01_assembly_fails_without_fragments() {
    let scratch = Scratch::new("empty");
    scratch.lay_out("merge");
    for name in scratch.fragments() {
        if name != "README.md" {
            fs::remove_file(scratch.path().join("changelog.d").join(name)).unwrap();
        }
    }
    let before = scratch.changelog();

    let out = changelog_py(scratch.path(), &["assemble", "0.1.0", "--date", DATE]);
    assert!(
        !out.status.success(),
        "a release with only changelog.d/README.md must fail\n{}",
        describe(&out)
    );
    assert!(
        stderr(&out).contains("no fragments"),
        "the failure says why\n{}",
        describe(&out)
    );
    assert_eq!(scratch.changelog(), before, "CHANGELOG.md is unchanged");
    assert_eq!(scratch.fragments(), set(&["README.md"]));

    fs::remove_dir_all(scratch.path().join("changelog.d")).unwrap();
    let out = changelog_py(scratch.path(), &["assemble", "0.1.0", "--date", DATE]);
    assert!(
        !out.status.success(),
        "a release without changelog.d/ must fail\n{}",
        describe(&out)
    );
    assert_eq!(scratch.changelog(), before, "CHANGELOG.md is unchanged");
}

#[test]
fn p0_doc_01_assembly_changes_nothing_when_a_fragment_is_malformed() {
    let scratch = Scratch::new("malformed");
    scratch.lay_out("merge");
    fs::copy(
        changelog_fixture("malformed/unknown-section.md"),
        scratch.path().join("changelog.d/P1-DDD-04.md"),
    )
    .unwrap();
    let before = scratch.changelog();
    let fragments = scratch.fragments();

    let out = changelog_py(scratch.path(), &["assemble", "0.1.0", "--date", DATE]);

    assert!(!out.status.success(), "{}", describe(&out));
    assert!(
        stderr(&out).contains("changelog.d/P1-DDD-04.md:1: unknown section '### Notes'"),
        "the error names the fragment and the line\n{}",
        describe(&out)
    );
    assert_eq!(scratch.changelog(), before, "CHANGELOG.md is unchanged");
    assert_eq!(scratch.fragments(), fragments, "no fragment is deleted");
}

#[test]
fn p0_doc_01_assembly_refuses_a_released_version() {
    let scratch = Scratch::new("released");
    scratch.lay_out("merge");
    let before = scratch.changelog();
    let fragments = scratch.fragments();

    let out = changelog_py(scratch.path(), &["assemble", "0.0.1", "--date", DATE]);

    assert!(!out.status.success(), "{}", describe(&out));
    assert!(
        stderr(&out).contains("already has a section for 0.0.1"),
        "{}",
        describe(&out)
    );
    assert_eq!(scratch.changelog(), before, "CHANGELOG.md is unchanged");
    assert_eq!(scratch.fragments(), fragments, "no fragment is deleted");
}

/// Every fragment committed in `changelog.d/` (`P0-REPO-01.md` among them until
/// the v0.0.1 release consumes it) and every valid fixture fragment parses.
#[test]
fn p0_doc_01_check_accepts_the_committed_fragments() {
    let root = repo_root();
    let mut fragments: Vec<String> = markdown_files(&root.join("changelog.d"))
        .into_iter()
        .filter(|name| name != "README.md")
        .map(|name| format!("changelog.d/{name}"))
        .collect();
    for fixture in ["merge", "pending"] {
        for name in markdown_files(&changelog_fixture(fixture).join("fragments")) {
            if name != "README.md" {
                fragments.push(format!(
                    "duscope/tests/fixtures/changelog/{fixture}/fragments/{name}"
                ));
            }
        }
    }
    let args: Vec<&str> = ["check"]
        .into_iter()
        .chain(fragments.iter().map(String::as_str))
        .collect();

    let out = changelog_py(&root, &args);

    assert!(out.status.success(), "{}", describe(&out));
    let printed = stdout(&out);
    for fragment in &fragments {
        let parses = format!("{fragment}: parses");
        assert!(
            printed.lines().any(|line| line == parses),
            "check does not report that {fragment} parses\n{}",
            describe(&out)
        );
    }
}

#[test]
fn p0_doc_01_check_rejects_malformed_fragments() {
    let dir = changelog_fixture("malformed");
    let listed: BTreeSet<String> = MALFORMED
        .iter()
        .map(|&(name, _, _)| name.to_owned())
        .collect();
    assert_eq!(
        markdown_files(&dir),
        listed,
        "every fixture under malformed/ needs its expected error in MALFORMED"
    );

    for &(name, line, message) in MALFORMED {
        let out = changelog_py(&dir, &["check", name]);
        assert_eq!(out.status.code(), Some(1), "{name}\n{}", describe(&out));
        assert!(
            stderr(&out).contains(&format!("{name}:{line}: {message}")),
            "check must report `{name}:{line}: {message}`\n{}",
            describe(&out)
        );
        assert!(
            !stdout(&out).contains("parses"),
            "{name} must not parse\n{}",
            describe(&out)
        );
    }

    let scratch = Scratch::new("not-utf8");
    fs::write(scratch.path().join("P1-EEE-05.md"), NOT_UTF8).unwrap();
    let out = changelog_py(scratch.path(), &["check", "P1-EEE-05.md"]);
    assert_eq!(out.status.code(), Some(1), "{}", describe(&out));
    assert!(
        stderr(&out).contains("P1-EEE-05.md:1: cannot read the fragment as UTF-8"),
        "{}",
        describe(&out)
    );
}

/// The Python program the `changelog` job of ci.yml runs on the fragments, cut
/// out of its heredoc; `None` once the job calls `scripts/changelog.py check`
/// instead.
fn ci_fragment_rule() -> Option<String> {
    let workflow = read(&repo_root().join(".github/workflows/ci.yml"));
    let lines: Vec<&str> = workflow.lines().collect();
    let job = lines.iter().position(|line| *line == "  changelog:")?;
    let start = job + 1 + lines[job + 1..].iter().position(|l| l.contains("<<'PY'"))?;
    let end = start + 1 + lines[start + 1..].iter().position(|l| l.trim() == "PY")?;
    let body = &lines[start + 1..end];
    let indent = body
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()?;
    let program: Vec<&str> = body
        .iter()
        .map(|line| line.get(indent..).unwrap_or(""))
        .collect();
    Some(program.join("\n") + "\n")
}

/// `check` applies exactly the rule of ci.yml's `changelog` job: inside GitHub
/// Actions it prints the same annotations and exits with the same status on
/// every fixture fragment, so the job can call the script instead.
#[test]
fn p0_doc_01_check_matches_the_ci_changelog_rule() {
    let Some(rule) = ci_fragment_rule() else {
        let workflow = read(&repo_root().join(".github/workflows/ci.yml"));
        assert!(
            workflow.contains("scripts/changelog.py check"),
            "the changelog job of ci.yml neither embeds the fragment rule nor runs \
             `scripts/changelog.py check`"
        );
        return;
    };

    // Every fixture fragment, valid or not, the README the assembler skips, a
    // fragment that is not UTF-8, a missing file and the committed fragments,
    // under short relative names so both programs print the same paths.
    let scratch = Scratch::new("ci-rule");
    let mut corpus = Vec::new();
    let mut add = |name: String, source: &Path| {
        fs::copy(source, scratch.path().join(&name)).unwrap();
        corpus.push(name);
    };
    for name in markdown_files(&changelog_fixture("malformed")) {
        add(name.clone(), &changelog_fixture("malformed").join(&name));
    }
    for fixture in ["merge", "pending"] {
        let dir = changelog_fixture(fixture).join("fragments");
        for name in markdown_files(&dir) {
            add(format!("{fixture}-{name}"), &dir.join(&name));
        }
    }
    let committed = repo_root().join("changelog.d");
    for name in markdown_files(&committed) {
        add(format!("committed-{name}"), &committed.join(&name));
    }
    fs::write(scratch.path().join("not-utf8.md"), NOT_UTF8).unwrap();
    corpus.push("not-utf8.md".to_owned());
    corpus.push("missing.md".to_owned());
    let program = scratch.path().join("ci_rule.py");
    fs::write(&program, rule).unwrap();

    let compare = |args: &[&str]| {
        let ci = python_run(&program, scratch.path(), args, true);
        let ours = python_run(
            &script(),
            scratch.path(),
            &[&["check"][..], args].concat(),
            true,
        );
        assert_eq!(
            (stdout(&ours), ours.status.code()),
            (stdout(&ci), ci.status.code()),
            "scripts/changelog.py check and the ci.yml rule disagree on {args:?}\n\
             --- ci.yml rule\n{}\n--- changelog.py check\n{}",
            describe(&ci),
            describe(&ours)
        );
        assert!(
            ci.stderr.is_empty() && ours.stderr.is_empty(),
            "neither writes to stderr\n{}\n{}",
            describe(&ci),
            describe(&ours)
        );
        stdout(&ci)
    };

    let mut parsed = 0;
    let mut rejected = 0;
    for name in &corpus {
        let output = compare(&[name.as_str()]);
        parsed += usize::from(output.contains(": parses"));
        rejected += usize::from(output.contains("::error file="));
    }
    let all: Vec<&str> = corpus.iter().map(String::as_str).collect();
    compare(&all);

    assert!(
        parsed >= 5 && rejected >= MALFORMED.len() + 2,
        "the corpus must exercise both outcomes: {parsed} parsed, {rejected} rejected"
    );
}
