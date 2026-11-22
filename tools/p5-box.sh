#!/usr/bin/env bash
# П5's own server on the Mod API bench, in the horizon-linux-dev box (prototypes/p5-bench.ps1 -Version
# -Export calls it through hlx.ps1 after syncing this folder to /hz/src/soulscraft-tools):
#   bash p5-box.sh start <export>   refuse if another /hz/servers/* runs or a build runs in the box;
#                                   then a fresh /hz/servers/sc-p5 (made again every run: flat world
#                                   `p5` from seed `soulscraft-p5`, port 25591 bound to 0.0.0.0, the
#                                   only one the box publishes), the export's engine, no natural
#                                   spawning, clear midnight, never paused when empty, no backups
#   bash p5-box.sh stop <export>    stop it (nothing if it is stopped)
#   bash p5-box.sh builds           the build processes running in the box, one per line (none: empty)
# The rules of p5-world.ps1 (the joint bench's П5 server). `world.difficulty`, not
# `properties.difficulty`: the latter makes `config set` fail without a word and leaves engine.path
# empty (found by the RustCraft session).
set -u
ACTION="${1:?start|stop|builds}"
EXPORT="${2:-}"
DIR=/hz/servers/sc-p5
WORLD=p5
SEED=soulscraft-p5
PORT=25591
RULES="spawn_mobs spawn_monsters spawn_patrols spawn_phantoms spawn_wandering_traders spawn_wardens advance_time advance_weather"

builds() {
  # A native build or a compile in the box (another session's hlx.ps1 native-build).
  # From /proc: the box's image need not have procps.
  for p in /proc/[0-9]*; do
    [ "$p" = "/proc/$$" ] && continue
    a="$(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)"
    [ -n "$a" ] && echo "${p#/proc/} $a"
  done | grep -E 'horizon-launcher .*native-(build|lock)|(^| |/)(cargo|rustc|horizon-mixins)( |$)' | grep -v 'p5-box.sh' || true
}

need_export() {
  [ -n "$EXPORT" ] || { echo "p5-box: name the export (e.g. soulscraft-4)"; exit 2; }
  HS="/hz/exports/$EXPORT/horizon-server"
  ENGINE="/hz/exports/$EXPORT/horizon-engine"
  [ -x "$HS" ] && [ -e "$ENGINE" ] || { echo "p5-box: no export at /hz/exports/$EXPORT"; exit 2; }
}

running() { "$HS" --dir "$1" status 2>/dev/null | head -1 | grep -qv '^Stopped'; }

case "$ACTION" in
  builds)
    builds
    ;;
  stop)
    need_export
    if [ -f "$DIR/server.toml" ] && running "$DIR"; then "$HS" --dir "$DIR" stop; echo "p5-box: stopped $DIR"; else echo "p5-box: $DIR is not running"; fi
    ;;
  start)
    need_export
    for d in /hz/servers/*; do
      [ "$d" = "$DIR" ] && continue
      [ -f "$d/server.toml" ] || continue
      if running "$d"; then echo "p5-box: busy: $d is running"; exit 3; fi
    done
    b="$(builds)"
    if [ -n "$b" ]; then echo "p5-box: busy: a build runs in the box:"; echo "$b"; exit 3; fi
    if [ -f "$DIR/server.toml" ] && running "$DIR"; then "$HS" --dir "$DIR" stop; fi
    rm -rf "$DIR"
    mkdir -p "$DIR"
    "$HS" --dir "$DIR" init --eula --world "$WORLD" --seed "$SEED" --port "$PORT" --no-whitelist --offline --yes >/dev/null || { echo "p5-box: init failed"; exit 1; }
    # One key per call: a key the tool refuses can't take the others down with it.
    for kv in "engine.path=$ENGINE" server.bind=0.0.0.0 world.game_mode=creative world.difficulty=normal \
              server.online_mode=false server.accept_older_clients=false properties.level-type=minecraft:flat \
              properties.pause-when-empty-seconds=0 backups.every=0; do
      "$HS" --dir "$DIR" config set "$kv" >/dev/null || { echo "p5-box: config set $kv failed"; exit 1; }
    done
    # No web panel, where the tool has one (an unknown key is no reason to stop).
    "$HS" --dir "$DIR" config set panel.bind= >/dev/null 2>&1 || true
    grep -q "$ENGINE" "$DIR/server.toml" || { echo "p5-box: engine.path did not take"; exit 1; }
    grep -Eq 'pause-when-empty-seconds *= *0' "$DIR/server.toml" || { echo "p5-box: pause-when-empty-seconds did not take"; exit 1; }
    grep -q 'minecraft:flat' "$DIR/server.toml" || { echo "p5-box: level-type did not take"; exit 1; }
    "$HS" --dir "$DIR" start || { echo "p5-box: start failed"; exit 1; }
    up=0
    for i in $(seq 1 90); do
      if "$HS" --dir "$DIR" cmd list >/dev/null 2>&1; then up=1; break; fi
      sleep 2
    done
    [ "$up" = 1 ] || { echo "p5-box: no answer in 180 s"; "$HS" --dir "$DIR" logs -n 60; exit 1; }
    for r in $RULES; do "$HS" --dir "$DIR" cmd "gamerule $r false" >/dev/null; done
    for c in 'time set midnight' 'weather clear' 'difficulty normal' 'kill @e[type=!player]'; do
      "$HS" --dir "$DIR" cmd "$c" >/dev/null
    done
    echo "p5-box: up on 0.0.0.0:$PORT (127.0.0.1:$PORT outside the box), fresh world $WORLD (seed $SEED), engine $ENGINE"
    ;;
  *)
    echo "p5-box: start|stop|builds"; exit 2
    ;;
esac
