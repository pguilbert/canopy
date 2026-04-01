# Snapshot Harness

Each case in `tests/snapshot/cases/*.sh`:

- creates a temporary Git repository in a known state using `git`
- runs the `canopy` CLI
- prints the final branch snapshot to stdout

`tests/snapshot_cases.rs` runs those shell cases during `cargo test` and verifies their stdout with
`insta` snapshots stored in `tests/snapshot/snapshots/`.

Snapshot names follow `insta`'s convention, for example:

- `snapshot_cases__dedup_linear.snap`
- `snapshot_cases__conflict_skip.snap`

Examples:

```sh
cargo test snapshot
INSTA_UPDATE=always cargo test snapshot
```

The snapshot format is currently the final branch log:

```sh
git log --graph --decorate=short --format='%H %P %s' integration
```
