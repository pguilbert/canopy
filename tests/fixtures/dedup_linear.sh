#!/bin/zsh

set -euo pipefail

canopy_bin=$1

export GIT_AUTHOR_DATE="2000-01-01T00:00:00+00:00"
export GIT_COMMITTER_DATE="2000-01-01T00:00:00+00:00"

git init -b main . >/dev/null
git config user.name "Canopy Snapshot"
git config user.email "canopy-snapshot@example.com"

print "base" > story.txt
git add story.txt
git commit -m "base" >/dev/null

git checkout -b linear >/dev/null
print "feature one" > story.txt
git commit -am "feature one" >/dev/null
git branch linear-1
print "feature two" > story.txt
git commit -am "feature two" >/dev/null
git branch linear-2

git checkout main >/dev/null
git checkout -b other >/dev/null
print "other" > other.txt
git add other.txt
git commit -m "other" >/dev/null

git checkout main >/dev/null
linear_1=$(git rev-parse linear-1)
linear_2=$(git rev-parse linear-2)

"$canopy_bin" branch integration "$linear_1" "$linear_2" other >/dev/null

git log --graph --decorate=short --format='%H %P %s' integration
