# Canopy

Canopy is a small Rust CLI for creating temporary integration branches from multiple Git tips.
It starts from your repository's default branch, tries to merge each requested tip in order, skips tips
that conflict, and writes the result to a target branch.

This project is an experiment for now.

## Installation

Install the latest GitHub Release binary:

```sh
./install.sh
```

You can also install a specific release tag:

```sh
./install.sh v0.1.0
```

By default the binary is installed to `~/.local/bin`. Override that with `INSTALL_DIR` if needed:

```sh
INSTALL_DIR=/usr/local/bin ./install.sh
```

If you prefer to build from source, use Cargo directly:

```sh
cargo install --path .
```

If `canopy` is not found afterwards, add the install directory to your shell `PATH`:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

## Releases

GitHub Releases are published by tagging a version and pushing the tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

That workflow builds release archives for:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

## How It Works

- detects the repository default branch
- resolves the tips you pass as branch names, tags, or commit SHAs
- removes redundant tips when one tip is already an ancestor of another
- attempts merges one by one
- skips conflicting merges and continues with the remaining tips
- creates or updates the target branch with the final integrated result

## Usage

From inside a Git repository:

```sh
cargo run -- branch <target-branch> <tip> [<tip>...]
```

Example:

```sh
cargo run -- branch integration feature-a feature-b 1a2b3c4d
```

To build from an explicit base instead of the detected default branch:

```sh
cargo run -- branch --base release integration feature-a feature-b
```

If the target branch already exists, re-run with `--force` to replace it:

```sh
cargo run -- branch --force integration feature-a feature-b
```

## GitHub PR Labels

The repository includes a GitHub Actions workflow that keeps synthetic branches in sync with PR
labels named `canopy/XXXX`.

- adding `canopy/XXXX` to one or more same-repository PRs rebuilds `canopy-XXXX`
- removing the label rebuilds the branch from the remaining labeled PRs
- removing the label from the last open PR deletes `canopy-XXXX`
- changing a labeled PR's base branch rebuilds `canopy-XXXX` from the new base
- pushes to a base branch also rebuild matching `canopy-*` branches

All open PRs sharing a given `canopy/XXXX` label must target the same base branch. If they do
not, the workflow fails for that label instead of guessing which base to use.

## Output

Canopy prints a step-by-step summary showing:

- the detected default branch
- the resolved tips
- the deduplicated tips
- which merges succeeded
- which merges were skipped because of conflicts
- the final branch that was created or updated

## Development

```sh
cargo test
```
