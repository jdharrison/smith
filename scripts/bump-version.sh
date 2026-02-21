#!/bin/bash
set -e

usage() {
    echo "Usage: $0 --major|--minor|--patch"
    exit 1
}

BUMP_TYPE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --major)
            BUMP_TYPE="major"
            shift
            ;;
        --minor)
            BUMP_TYPE="minor"
            shift
            ;;
        --patch)
            BUMP_TYPE="patch"
            shift
            ;;
        *)
            usage
            ;;
    esac
done

if [[ -z "$BUMP_TYPE" ]]; then
    usage
fi

CARGO_FILE="Cargo.toml"

if [[ ! -f "$CARGO_FILE" ]]; then
    echo "Error: $CARGO_FILE not found"
    exit 1
fi

CURRENT_VERSION=$(grep -m1 '^version = ' "$CARGO_FILE" | sed 's/version = "\(.*\)"/\1/')

IFS='.' read -r -a VERSION_PARTS <<< "$CURRENT_VERSION"
MAJOR="${VERSION_PARTS[0]}"
MINOR="${VERSION_PARTS[1]}"
PATCH="${VERSION_PARTS[2]}"

case $BUMP_TYPE in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"

sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$CARGO_FILE"

echo "Bumped version: $CURRENT_VERSION -> $NEW_VERSION"
