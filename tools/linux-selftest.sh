#!/usr/bin/env bash
# SoulsCraft's in-world self-tests on the Linux dedicated server in the horizon-linux-dev box.
# Run through hlx.ps1:  & $hlx -Sh "bash /hz/src/soulscraft-tools/linux-selftest.sh <export> [filter]"
# Starts /hz/servers/soulscraft on the export, runs `soulscraft_selftest run` from the console,
# waits for selftest/soulscraft.json to say done, prints it and stops the server.
set -u
EXPORT="${1:?export name, e.g. soulscraft-2}"
FILTER="${2:-}"
DIR=/hz/servers/soulscraft
HS="/hz/exports/$EXPORT/horizon-server"
for d in /hz/servers/*; do
  [ "$d" = "$DIR" ] && continue
  if "$HS" --dir "$d" status 2>/dev/null | grep -qv '^Stopped'; then echo "busy: $d is running"; exit 3; fi
done
"$HS" --dir "$DIR" config set engine.path="/hz/exports/$EXPORT/horizon-engine" >/dev/null
rm -f "$DIR/selftest/soulscraft.json"
"$HS" --dir "$DIR" start || exit 1
for i in $(seq 1 60); do "$HS" --dir "$DIR" cmd list >/dev/null 2>&1 && break; sleep 2; done
"$HS" --dir "$DIR" cmd "soulscraft_selftest run $FILTER"
for i in $(seq 1 180); do
  sleep 5
  if [ -f "$DIR/selftest/soulscraft.json" ] && grep -q '"done":true' "$DIR/selftest/soulscraft.json"; then break; fi
done
cat "$DIR/selftest/soulscraft.json" 2>/dev/null || { echo "no result"; "$HS" --dir "$DIR" logs -n 80; }
echo
"$HS" --dir "$DIR" logs -n 400 | grep -E "soulscraft: selftest (FAIL|done)|panicked|ERROR" | tail -20
"$HS" --dir "$DIR" stop
