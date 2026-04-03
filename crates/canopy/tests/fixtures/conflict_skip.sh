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

git checkout -b feature-a >/dev/null
print "feature a" > story.txt
git commit -am "feature a" >/dev/null

git checkout main >/dev/null
git checkout -b feature-b >/dev/null
print "feature b" > story.txt
git commit -am "feature b" >/dev/null

git checkout main >/dev/null
git checkout -b feature-c >/dev/null
print "feature c" > extra.txt
git add extra.txt
git commit -m "feature c" >/dev/null

git checkout main >/dev/null

"$canopy_bin" branch integration feature-a feature-b feature-c >/dev/null

git log --graph --decorate=short --format='%H %P %s' integration
