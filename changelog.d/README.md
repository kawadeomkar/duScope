# Changelog fragments

[CHANGELOG.md](../CHANGELOG.md) is assembled from the files in this directory.
Each pull request with a user-visible change adds one fragment named after its
package id, `changelog.d/<ID>.md`, where `<ID>` is the pull request title before
the colon: `P2-IDX-04: Bulk writer and atomic rename` adds
`changelog.d/P2-IDX-04.md`. At each release, `scripts/changelog.py assemble`
merges every fragment into the version's dated section of `CHANGELOG.md` and
deletes the fragments. This README is not a fragment, and it stays.

## Format

A fragment holds keep-a-changelog sections and nothing else:

```markdown
### Added

- `duscope --version` prints the version.

### Fixed

- One line per change, said from the point of view of someone running duscope.
  A long item continues on lines indented by at least one space.
```

The rules, which the `changelog` CI check applies:

- Sections are `### <Section>` headings. A section is one of `Added`,
  `Changed`, `Deprecated`, `Removed`, `Fixed` and `Security`, at most once per
  fragment, in any order. The release lists them in that order and merges the
  items of every fragment under one heading per section.
- Each section has at least one item. An item starts with `- `: a dash, a
  space, then text. A line indented by at least one space continues the item
  above it.
- Blank lines can go anywhere.
- Nothing else: no text before the first section, no other heading (the release
  writes the `## [X.Y.Z] - YYYY-MM-DD` heading), no `*` or `+` bullets.
- The file is UTF-8.

Write each item for someone who runs duscope: say what changed for them, and
name the verb, flag or field. Check a fragment before you push it:

```sh
python3 scripts/changelog.py check changelog.d/P2-IDX-04.md
```

## The `changelog` check, and the `no-changelog` label

A pull request passes the `changelog` check when either:

- it adds or changes `changelog.d/<ID>.md`, for the `<ID>` its title starts
  with, or
- it carries the `no-changelog` label.

Either way, every fragment the pull request adds or changes must parse.

A pull request with nothing user-visible, such as tests, CI, refactoring or
documentation for contributors, gets the `no-changelog` label instead of a
fragment. The maintainer sets the label; a contributor or an agent asks for it
in the pull request body. Adding or removing the label runs the check again.

## At release

From the repository root:

```sh
python3 scripts/changelog.py assemble 0.2.0
```

This writes the fragments into a `## [0.2.0] - YYYY-MM-DD` section, dated today
in UTC unless `--date YYYY-MM-DD` says otherwise, and deletes them. It fails and
changes nothing when there is no fragment, when a fragment does not parse, or
when the version already has a dated section. When `CHANGELOG.md` already has an
undated `## [X.Y.Z] - Unreleased` section for the version, as 0.0.1 does, the
heading gets the date and the notes under it stay above the assembled sections.
