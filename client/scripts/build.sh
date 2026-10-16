#!/bin/sh
set -eu
if [ "$#" -ne 2 ] || [ ! -f "$1" ] || [ -L "$1" ] || [ ! -d "$2" ] || [ -L "$2" ]; then
  printf '%s\n' 'Usage: sh scripts/build.sh <regular compiled smith-service binary> <verified runtime directory>' >&2
  exit 2
fi
flutter build linux --release --no-pub
install -D -m 0755 "$1" build/linux/x64/release/bundle/smith_backend/smith-service
if [ ! -d "$2/cpu" ] || [ -L "$2/cpu" ] || [ ! -d "$2/vulkan" ] || [ -L "$2/vulkan" ]; then
  printf '%s\n' 'Runtime must contain regular directories named cpu and vulkan' >&2
  exit 1
fi
if find "$2" -mindepth 1 -maxdepth 1 ! -name cpu ! -name vulkan -print -quit | grep -q .; then
  printf '%s\n' 'Runtime root contains an unexpected entry' >&2
  exit 1
fi
if find "$2/cpu" "$2/vulkan" -mindepth 1 ! -type f -print -quit | grep -q .; then
  printf '%s\n' 'Runtime backend inputs must contain regular files only, not links or directories' >&2
  exit 1
fi
runtime=build/linux/x64/release/bundle/smith_backend/runtime
rm -rf "$runtime"
mkdir -p "$runtime"
cp -R --preserve=mode "$2/cpu" "$2/vulkan" "$runtime/"
