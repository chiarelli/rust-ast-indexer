#!/usr/bin/env bash
# verify commit messages follow Conventional Commits: type(scope): subject
# This script checks the last 5 commits on the current branch.
set -euo pipefail
commits=$(git log -n 5 --pretty=format:%s)
regex='^(feat|fix|chore|docs|test|refactor|perf|build|ci|style)(\([a-zA-Z0-9_\-]+\))?: [A-Za-z0-9].{1,}$'
while read -r line; do
  if ! [[ $line =~ $regex ]]; then
    echo "Commit message does not match convention: '$line'" >&2
    exit 1
  fi
done <<< "$commits"
exit 0
