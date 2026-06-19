#!/bin/bash

set -e

# Get the latest tag, default to v0.0.0 if none exists
latest_tag=$(git tag --sort=-v:refname | head -1)
if [ -z "$latest_tag" ]; then
  latest_tag="v0.0.0"
fi

echo "Latest tag: $latest_tag"

# Check for any changes (staged, unstaged, or untracked)
if [ -n "$(git status --porcelain)" ]; then
  echo ""
  echo "Pending changes detected:"
  git status --short
  echo ""

  # Prompt for commit message
  read -p "Commit message: " commit_msg
  if [ -z "$commit_msg" ]; then
    echo "Error: commit message cannot be empty"
    exit 1
  fi

  git add .
  git commit -m "$commit_msg"
  echo "Changes committed."
else
  echo "No pending changes."
fi

# Parse version numbers from latest tag
version="${latest_tag#v}"
major=$(echo "$version" | cut -d. -f1)
minor=$(echo "$version" | cut -d. -f2)
patch=$(echo "$version" | cut -d. -f3)

# Parse the optional 4th (build/hotfix) segment, e.g. v0.4.6.1 → build=1.
build=$(echo "$version" | cut -d. -f4)
if [[ "$build" =~ ^[0-9]+$ ]]; then
  hotfix_tag="v${major}.${minor}.${patch}.$((build + 1))"
else
  hotfix_tag="v${major}.${minor}.${patch}.1"
fi

# Prompt for version bump type
echo ""
echo "Current version: $latest_tag"
echo "  1) patch  → v${major}.${minor}.$((patch + 1))"
echo "  2) minor  → v${major}.$((minor + 1)).0"
echo "  3) major  → v$((major + 1)).0.0"
echo "  4) hotfix → ${hotfix_tag}"
echo "  5) custom"
echo ""
read -p "Bump type [1/2/3/4/5] (default: 1): " bump

input="${bump:-1}"

if [[ "$input" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
  # User typed a version number directly (e.g. 0.0.1, v0.0.1, or 0.4.6.1)
  new_tag="v${input#v}"
elif [ "$input" = "1" ]; then
  new_tag="v${major}.${minor}.$((patch + 1))"
elif [ "$input" = "2" ]; then
  new_tag="v${major}.$((minor + 1)).0"
elif [ "$input" = "3" ]; then
  new_tag="v$((major + 1)).0.0"
elif [ "$input" = "4" ]; then
  new_tag="${hotfix_tag}"
elif [ "$input" = "5" ]; then
  read -p "Enter version (e.g. v1.2.3 or v1.2.3.4): " new_tag
  if [[ ! "$new_tag" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
    echo "Error: invalid version format. Use vX.Y.Z or vX.Y.Z.N"
    exit 1
  fi
  new_tag="v${new_tag#v}"
else
  echo "Error: invalid option"
  exit 1
fi

echo ""
echo "Creating tag: $new_tag"
git tag "$new_tag"
git push origin "$(git branch --show-current)" --tags
echo ""
echo "Pushed with tag $new_tag"
