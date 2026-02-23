#!/usr/bin/env bash
# Check that Docker is available and the daemon is running.
# Exit 0 if OK, 1 otherwise (message to stderr).
# Can be run standalone or sourced (call check_docker).

check_docker() {
  if ! command -v docker &>/dev/null; then
    echo "Docker command not found." >&2
    return 1
  fi
  if ! docker --version &>/dev/null; then
    echo "Docker command failed to execute." >&2
    return 1
  fi
  if ! docker info &>/dev/null; then
    echo "Docker daemon is not running or not reachable." >&2
    return 1
  fi
  return 0
}

# If run as script, execute and exit
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  if check_docker; then
    echo "Docker is available."
    exit 0
  else
    exit 1
  fi
fi
