#!/bin/zsh

set -euo pipefail

canopy_bin=$1

export HOME="$PWD"
export XDG_CONFIG_HOME="$PWD/.xdg-config"
export GIT_AUTHOR_DATE="2000-01-01T00:00:00+00:00"
export GIT_COMMITTER_DATE="2000-01-01T00:00:00+00:00"
unset GIT_AUTHOR_NAME
unset GIT_AUTHOR_EMAIL
unset GIT_COMMITTER_NAME
unset GIT_COMMITTER_EMAIL

git init -b main . >/dev/null
git config user.name "Canopy Snapshot"
git config user.email "canopy-snapshot@example.com"

print "base" > story.txt
git add story.txt
git commit -m "base" >/dev/null

git checkout -b linear >/dev/null
print "feature" > story.txt
git commit -am "feature" >/dev/null

git checkout main >/dev/null
git config --unset user.name
git config --unset user.email

"$canopy_bin" branch integration linear >/dev/null

git show -s --format='%an <%ae>|%cn <%ce>' integration
