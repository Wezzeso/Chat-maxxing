#!/bin/sh
set -eu
relay_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ ! -x "$relay_root/bin/relay" ]; then
  printf '%s\n' 'Build Relay first: cargo build --release, then copy target/release/relay to bin/relay.' >&2
  exit 1
fi
exec "$relay_root/bin/relay" "$@"
