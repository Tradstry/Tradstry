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

# Prompt for version bump type
echo ""
echo "Current version: $latest_tag"
echo "  1) patch  → v${major}.${minor}.$((patch + 1))"
echo "  2) minor  → v${major}.$((minor + 1)).0"
echo "  3) major  → v$((major + 1)).0.0"
echo "  4) custom"
echo ""
read -p "Bump type [1/2/3/4] (default: 1): " bump

case "${bump:-1}" in
  1) new_tag="v${major}.${minor}.$((patch + 1))" ;;
  2) new_tag="v${major}.$((minor + 1)).0" ;;
  3) new_tag="v$((major + 1)).0.0" ;;
  4)
    read -p "Enter version (e.g. v1.2.3): " new_tag
    if [[ ! "$new_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      echo "Error: invalid version format. Use vX.Y.Z"
      exit 1
    fi
    ;;
  *)
    echo "Error: invalid option"
    exit 1
    ;;
esac

echo ""
echo "Creating tag: $new_tag"
git tag "$new_tag"
git push origin "$(git branch --show-current)" --tags
echo ""
echo "Pushed with tag $new_tag"
