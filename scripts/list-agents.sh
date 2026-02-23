#!/usr/bin/env bash
# List active agent containers (names matching smith-agent-*).

set -e

PREFIX="smith-agent-"

echo "==> Active agent containers (name filter: ${PREFIX}*):"
if ! names=$(docker ps --filter "name=${PREFIX}" --format "{{.Names}}" 2>/dev/null); then
  echo "  (docker failed)"
  exit 1
fi

if [ -z "$names" ]; then
  echo "  (none)"
  exit 0
fi

while IFS= read -r full; do
  [ -z "$full" ] && continue
  agent="${full#$PREFIX}"
  echo "  $full  →  agent: $agent"
done <<< "$names"
