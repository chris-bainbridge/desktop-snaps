#!/bin/sh
set -eu

if [ "$#" -ne 0 ]; then
  echo "Usage: $0" >&2
  exit 2
fi

STARTUP_TIMEOUT=${STARTUP_TIMEOUT:-10}
OUTPUT=$(timeout --signal=TERM --kill-after=5s "${STARTUP_TIMEOUT}s" snap run squirrelsql 2>&1) || STATUS=$?
STATUS=${STATUS:-0}

if [ "$STATUS" -eq 124 ]; then
  printf '%s\n' "SQuirreL remained running for ${STARTUP_TIMEOUT}s."
elif [ "$STATUS" -eq 0 ]; then
  echo "SQuirreL exited before the startup timeout; expected a running GUI app." >&2
  printf '%s\n' "$OUTPUT" >&2
  exit 1
else
  printf '%s\n' "$OUTPUT" >&2
  exit "$STATUS"
fi
