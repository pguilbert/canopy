#!/usr/bin/env bash
set -euo pipefail

repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
event_name="${GITHUB_EVENT_NAME:?GITHUB_EVENT_NAME is required}"
event_path="${GITHUB_EVENT_PATH:?GITHUB_EVENT_PATH is required}"
canopy_bin="${CANOPY_BIN:-./target/debug/canopy}"

delete_remote_branch() {
  local target_branch="$1"

  if git ls-remote --exit-code --heads origin "$target_branch" >/dev/null 2>&1; then
    echo "Deleting $target_branch because no open PRs still use its label"
    git push origin --delete "$target_branch"
  else
    echo "Skipping delete for $target_branch because it does not exist on origin"
  fi
}

open_pr_pages="$(gh api --paginate --slurp "repos/$repo/pulls?state=open&per_page=100")"
open_prs="$(jq -c 'map(.[])' <<<"$open_pr_pages")"

if [[ "$event_name" == "push" ]]; then
  pushed_ref="$(jq -r '.ref' "$event_path")"
  pushed_branch="${pushed_ref#refs/heads/}"
  impacted_labels="$(jq -r --arg branch "$pushed_branch" '
    .[]
    | select(.base.ref == $branch)
    | .labels[]?.name
    | select(test("^canopy/"))
  ' <<<"$open_prs" | sort -u)"
else
  action="$(jq -r '.action' "$event_path")"
  if [[ "$action" == "labeled" || "$action" == "unlabeled" ]]; then
    impacted_labels="$(jq -r '
      .label.name?
      | select(type == "string" and test("^canopy/"))
    ' "$event_path")"
  else
    impacted_labels="$(jq -r '
      .pull_request.labels[]?.name
      | select(test("^canopy/"))
    ' "$event_path" | sort -u)"
  fi
fi

if [[ -z "${impacted_labels//$'\n'/}" ]]; then
  echo "No canopy labels were affected"
  exit 0
fi

while IFS= read -r label; do
  [[ -n "$label" ]] || continue

  label_suffix="${label#canopy/}"
  target_branch="canopy-$label_suffix"
  prs_for_label="$(jq -c --arg label "$label" '
    [
      .[]
      | select(any(.labels[]?; .name == $label))
    ]
    | sort_by(.number)
  ' <<<"$open_prs")"
  pr_count="$(jq 'length' <<<"$prs_for_label")"

  if [[ "$pr_count" == "0" ]]; then
    delete_remote_branch "$target_branch"
    continue
  fi

  foreign_prs="$(jq -r --arg repo "$repo" '
    [
      .[]
      | select(.head.repo.full_name != $repo)
      | .number
    ]
    | join(", ")
  ' <<<"$prs_for_label")"
  if [[ -n "$foreign_prs" ]]; then
    echo "Label $label is applied to fork PRs ($foreign_prs); this workflow only supports same-repository PRs" >&2
    exit 1
  fi

  base_count="$(jq '[.[].base.ref] | unique | length' <<<"$prs_for_label")"
  if [[ "$base_count" != "1" ]]; then
    bases="$(jq -r '[.[].base.ref] | unique | join(", ")' <<<"$prs_for_label")"
    echo "Label $label spans multiple base branches: $bases" >&2
    exit 1
  fi

  base_ref="$(jq -r '.[0].base.ref' <<<"$prs_for_label")"
  mapfile -t head_refs < <(jq -r '[.[].head.ref] | unique[]' <<<"$prs_for_label")

  fetch_specs=("+refs/heads/$base_ref:refs/remotes/origin/$base_ref")
  for head_ref in "${head_refs[@]}"; do
    fetch_specs+=("+refs/heads/$head_ref:refs/remotes/origin/$head_ref")
  done

  echo "Rebuilding $target_branch from base $base_ref for label $label"
  git fetch --no-tags origin "${fetch_specs[@]}"

  tip_refs=()
  for head_ref in "${head_refs[@]}"; do
    tip_refs+=("refs/remotes/origin/$head_ref")
  done

  "$canopy_bin" branch --force --base "refs/remotes/origin/$base_ref" "$target_branch" "${tip_refs[@]}"
  git push origin "+refs/heads/$target_branch:refs/heads/$target_branch"
done <<<"$impacted_labels"
