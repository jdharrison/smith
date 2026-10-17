#!/bin/sh
# CI builder only: install the reviewed CLI without curl-to-shell or sudo.
set -eu

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
    printf '%s\n' 'Dagger CI setup supports Linux x86_64 only.' >&2
    exit 1
fi
: "${RUNNER_TEMP:?GitHub runner temporary directory required}"
: "${GITHUB_PATH:?GitHub runner PATH file required}"

# Match dagger/dagger.json. Digest from the upstream v0.21.9 release asset:
# https://api.github.com/repos/dagger/dagger/releases/tags/v0.21.9
version=0.21.9
sha256=33eea0b08d6be444bada18e64b2216459100d6070abcb4b5345cee40c9fff982
tools="$RUNNER_TEMP/smith-dagger-$version"
mkdir -m 0700 "$tools"
archive="$tools/dagger.tar.gz"
curl --fail --silent --show-error --location \
    --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 180 \
    --output "$archive" \
    "https://github.com/dagger/dagger/releases/download/v$version/dagger_v${version}_linux_amd64.tar.gz"
printf '%s  %s\n' "$sha256" "$archive" | sha256sum --check --strict
tar -xzf "$archive" -C "$tools" dagger
chmod 0755 "$tools/dagger"
"$tools/dagger" version
printf '%s\n' "$tools" >> "$GITHUB_PATH"
