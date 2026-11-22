#!/bin/sh
# Prints the permission mode the agents' program starts in, from the first line of its stream, for the given arguments,
# with the engine's variable set and with it emptied.
# Usage: probe.sh <claude arguments...>
# The program is run from the current directory, once for each state of the variable, with a spending cap of one cent.
for state in 1 ""; do
  printf 'CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=%s: ' "$state"
  CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=$state claude -p --output-format stream-json --verbose "$@" \
    --max-budget-usd 0.01 "say ok" 2>&1 | head -n 1 | python3 -I -c '
import json, sys
line = sys.stdin.readline()
try:
    print(json.loads(line).get("permissionMode"))
except ValueError:
    print("not JSON:", line.strip()[:200])
'
done
