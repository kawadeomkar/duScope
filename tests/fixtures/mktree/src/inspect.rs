//! Reading a built tree back: the deterministic listing the determinism
//! check compares, and the verification of a tree against its plan.

use std::fmt::Write as _;
use std::fs::{self, Metadata};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use crate::build::abs;
use crate::manifest::encode;
use crate::plan::{Kind, Plan};
use crate::record::Record;
use crate::time::Stamp;
use crate::{Error, Result};

/// The type letter `find -printf %y` prints.
fn type_letter(meta: &Metadata) -> char {
    let t = meta.file_type();
    if t.is_dir() {
        'd'
    } else if t.is_symlink() {
        'l'
    } else if t.is_fifo() {
        'p'
    } else if t.is_socket() {
        's'
    } else if t.is_block_device() {
        'b'
    } else if t.is_char_device() {
        'c'
    } else {
        'f'
    }
}

fn mtime(meta: &Metadata) -> Stamp {
    Stamp {
        secs: meta.mtime(),
        nanos: u32::try_from(meta.mtime_nsec()).unwrap_or(0),
    }
}

fn atime(meta: &Metadata) -> Stamp {
    Stamp {
        secs: meta.atime(),
        nanos: u32::try_from(meta.atime_nsec()).unwrap_or(0),
    }
}

/// A listing of the tree under `root`, one line per entry sorted by path
/// bytes, with the fields of `find <root> -printf
/// '%P\t%y\t%s\t%m\t%T@\t%n\t%l\n'`: path (`.` for the root, `%XX`-escaped),
/// type, size, permission bits, mtime, link count and symlink target. Inode
/// numbers, ctimes and atimes are left out: no build can pin them.
pub fn listing(root: &Path) -> Result<String> {
    let mut out = String::new();
    walk(root, &mut Vec::new(), &mut out)?;
    Ok(out)
}

fn walk(root: &Path, rel: &mut Vec<u8>, out: &mut String) -> Result<()> {
    let path = abs(root, rel);
    let meta = fs::symlink_metadata(&path)
        .map_err(|e| Error::new(format!("lstat {}: {e}", path.display())))?;
    let target = if meta.file_type().is_symlink() {
        let t = fs::read_link(&path)
            .map_err(|e| Error::new(format!("readlink {}: {e}", path.display())))?;
        encode(t.as_os_str().as_bytes())
    } else {
        String::new()
    };
    let shown = if rel.is_empty() {
        ".".to_owned()
    } else {
        encode(rel)
    };
    let _ = writeln!(
        out,
        "{shown}\t{}\t{}\t{:o}\t{}\t{}\t{target}",
        type_letter(&meta),
        meta.size(),
        meta.mode() & 0o7777,
        mtime(&meta).text(),
        meta.nlink(),
    );
    if meta.is_dir() {
        let mut names: Vec<Vec<u8>> = fs::read_dir(&path)
            .and_then(|it| {
                it.map(|e| e.map(|e| e.file_name().as_bytes().to_vec()))
                    .collect()
            })
            .map_err(|e| Error::new(format!("read_dir {}: {e}", path.display())))?;
        names.sort();
        for name in names {
            let len = rel.len();
            if !rel.is_empty() {
                rel.push(b'/');
            }
            rel.extend_from_slice(&name);
            walk(root, rel, out)?;
            rel.truncate(len);
        }
    }
    Ok(())
}

/// The outcome of [`verify`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Every way the tree differs from its plan; empty when it is right.
    pub problems: Vec<String>,
    /// Entries whose atime something moved by reading them after the build
    /// (on macOS, Spotlight importing a `.png`, for example). They are not
    /// problems: a read sets atime to the moment of the read, which lies
    /// between the inode's ctime and now, while every planned atime lies
    /// elsewhere, so a wrong or unset atime is still caught.
    pub read_after_build: Vec<String>,
}

/// Whether an atime is one a read after the build left: later than the
/// inode's last change (the build's last `utimes` or `chmod`) and no later
/// than now.
fn read_since_build(meta: &Metadata) -> bool {
    let ctime = Stamp {
        secs: meta.ctime(),
        nanos: u32::try_from(meta.ctime_nsec()).unwrap_or(0),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let at = atime(meta);
    at > ctime && at.secs <= now
}

/// Compares the tree at `root` with `plan` and the build `record`: every
/// planned entry exists unless the record lists it absent, with the planned
/// type, size, mode, mtime, atime, link count and symlink target, and no
/// other entry exists. A directory that currently carries its test mode (a
/// test holds the tree) must have exactly that mode, and what lies beneath
/// it is not checked. An atime moved by a read after the build is reported
/// apart (see [`Report::read_after_build`]).
#[allow(clippy::too_many_lines)] // one check per property, read top to bottom
pub fn verify(root: &Path, plan: &Plan, record: &Record) -> Result<Report> {
    let mut problems = Vec::new();
    let mut read_after_build = Vec::new();
    let links = plan.link_counts();
    let mut expected = 0u64;
    // Directories found in their test mode; entries beneath are skipped.
    let mut armed: Vec<Vec<u8>> = Vec::new();
    for e in &plan.entries {
        let shown = e.display();
        if record.is_absent(&shown) {
            continue;
        }
        if armed
            .iter()
            .any(|a| e.path.starts_with(a) && e.path.get(a.len()) == Some(&b'/'))
        {
            continue;
        }
        expected += 1;
        let path = abs(root, &e.path);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(err) => {
                problems.push(format!("{shown}: missing ({err})"));
                continue;
            }
        };
        let mut want_mode = e.mode;
        if let Some(test_mode) = e.test_mode
            && meta.mode() & 0o7777 == test_mode
        {
            armed.push(e.path.clone());
            want_mode = test_mode;
        }
        let (want_type, want_size) = match &e.kind {
            Kind::Dir => ('d', None),
            Kind::File { size, .. } | Kind::Sparse { size, .. } | Kind::Compressed { size } => {
                ('f', Some(*size))
            }
            Kind::Clone { source: other } | Kind::Hardlink { target: other } => (
                'f',
                plan.get(other).and_then(|t| match t.kind {
                    Kind::File { size, .. }
                    | Kind::Sparse { size, .. }
                    | Kind::Compressed { size } => Some(size),
                    _ => None,
                }),
            ),
            Kind::Symlink { target } => ('l', Some(target.len() as u64)),
            Kind::Fifo => ('p', None),
            Kind::Socket => ('s', None),
        };
        let got_type = type_letter(&meta);
        if got_type != want_type {
            problems.push(format!("{shown}: type {got_type}, want {want_type}"));
            continue;
        }
        if let Some(size) = want_size
            && meta.size() != size
        {
            problems.push(format!("{shown}: size {}, want {size}", meta.size()));
        }
        if let Kind::Symlink { target } = &e.kind {
            match fs::read_link(&path) {
                Ok(t) if t.as_os_str().as_bytes() == target.as_slice() => {}
                Ok(t) => problems.push(format!(
                    "{shown}: target {}",
                    encode(t.as_os_str().as_bytes())
                )),
                Err(err) => problems.push(format!("{shown}: readlink: {err}")),
            }
        } else if meta.mode() & 0o7777 != want_mode {
            problems.push(format!(
                "{shown}: mode {:o}, want {want_mode:o}",
                meta.mode() & 0o7777
            ));
        }
        if mtime(&meta) != e.mtime {
            problems.push(format!(
                "{shown}: mtime {}, want {}",
                mtime(&meta).text(),
                e.mtime.text()
            ));
        }
        if atime(&meta) != e.atime && read_since_build(&meta) {
            read_after_build.push(shown.clone());
        } else if atime(&meta) != e.atime {
            problems.push(format!(
                "{shown}: atime {}, want {}",
                atime(&meta).text(),
                e.atime.text()
            ));
        }
        let want_links = match &e.kind {
            Kind::Dir | Kind::Symlink { .. } => None,
            Kind::Hardlink { target } => {
                Some(1 + links.get(target.as_slice()).copied().unwrap_or(0))
            }
            _ => Some(1 + links.get(e.path.as_slice()).copied().unwrap_or(0)),
        };
        if let Some(n) = want_links
            && meta.nlink() != n
        {
            problems.push(format!("{shown}: {} links, want {n}", meta.nlink()));
        }
    }
    let armed: Vec<std::path::PathBuf> = armed.iter().map(|a| abs(root, a)).collect();
    let found = count(root, &armed)?;
    if found != expected {
        problems.push(format!(
            "the tree has {found} entries, the plan and record expect {expected}"
        ));
    }
    if armed.is_empty() && record.entries != expected {
        problems.push(format!(
            "the record counts {} entries, expected {expected}",
            record.entries
        ));
    }
    Ok(Report {
        problems,
        read_after_build,
    })
}

/// Entries under `path`, itself included, not descending into `armed`.
fn count(path: &Path, armed: &[std::path::PathBuf]) -> Result<u64> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| Error::new(format!("lstat {}: {e}", path.display())))?;
    let mut n = 1;
    if meta.is_dir() && !armed.iter().any(|a| a == path) {
        for entry in fs::read_dir(path)
            .map_err(|e| Error::new(format!("read_dir {}: {e}", path.display())))?
        {
            let entry =
                entry.map_err(|e| Error::new(format!("read_dir {}: {e}", path.display())))?;
            n += count(&entry.path(), armed)?;
        }
    }
    Ok(n)
}
