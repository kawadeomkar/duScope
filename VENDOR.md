# Vendored code

Code copied into this repository from another project has one row here, giving where it lives, where it came from, the upstream commit it matches, its licence and where the upstream notice is kept.

Nothing is vendored yet. The first rows will be:

- the dua-core work-stealing scheduler (`crates/duscope-walk/src/scheduler.rs`, MIT, P1-WALK-01)
- the treemap palette tables (`crates/duscope-treemap/src/packs.rs`, P1-MAP-02)
- the Inter font subset (OFL-1.1, with its licence file beside it, P2-MAP-02)

| Path | Upstream | Commit | Licence | Notice | Synced |
| --- | --- | --- | --- | --- | --- |

- **Path**: the vendored file or directory in this repository.
- **Upstream**: the source repository and the path in it.
- **Commit**: the full 40-character SHA of the upstream commit the copy matches. It must resolve in the upstream repository.
- **Licence**: the SPDX identifier. It must be on the `[licenses]` list in `deny.toml`.
- **Notice**: where the upstream copyright notice is kept: a header in the vendored file, or a licence file beside it.
- **Synced**: the date of the last sync and the package that made it.

## Adding a piece

Copy it from a single upstream commit and keep the upstream notice. Add its row in the same pull request.

## Sync procedure

1. A monthly step in `nightly-bench.yml` diffs the vendored scheduler against upstream `main`. When upstream has changed, it opens an issue labelled `vendor-sync`.
2. Syncing is then a package of its own, `P<phase>-REPO-SYNC-<nn>`, which does the following:
   1. Diff the upstream file against the vendored copy.
   2. Apply the upstream changes that are wanted, keeping the notice.
   3. Run the walker property tests and the benchmark harness.
   4. Record the new upstream SHA and the sync date in this table.
