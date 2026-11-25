#!/usr/bin/env bash
# godot_verify.sh: one-command headless verification for a Godot 4 project.
#
#   godot_verify.sh <dir containing project.godot> [options]
#     --no-import          skip the import pass
#     --no-tests           skip *_check.gd
#     --playtest           also run *_playtest.gd (slow soak checks)
#     --addons a,b         also parse-check res://addons/a and res://addons/b (your own)
#     --no-cargo           skip `cargo test` even when a rust/ crate exists
#     --no-assets          skip the asset audit
#
# 1. Imports headlessly. Fails on a bad exit code or any script error.
# 2. Parse-checks every .gd in parallel. Exports do NOT fail on script errors; this does.
# 3. Runs every *_check.gd in the project root, tests/ and scripts/tests/ (recursive).
# 4. Runs asset_audit.py and cargo test when they apply.
#
# Exit 0 = all green. Env: GODOT, VERIFY_JOBS, VERIFY_TEST_TIMEOUT (seconds, default 120).
set -uo pipefail

if [[ -z "${GODOT:-}" ]]; then
  GODOT=/Applications/Godot.app/Contents/MacOS/Godot
  if [[ ! -x "$GODOT" ]]; then
    GODOT="$(command -v godot || true)"
    for g in "${GDP_CACHE:-$HOME/.cache/gdp}"/godot-*/Godot*; do
      [[ -z "$GODOT" && -f "$g" && -x "$g" ]] && GODOT="$g"
    done
  fi
fi
BAD_PATTERN='SCRIPT ERROR|Parse Error|Failed to load script|Failed loading resource|GDExtension.*(failed|error)'

with_timeout() {  # with_timeout SECONDS cmd...   (macOS may lack coreutils timeout)
  if command -v timeout >/dev/null 2>&1; then timeout "$@"; return; fi
  python3 -c 'import subprocess,sys
try: sys.exit(subprocess.call(sys.argv[2:], timeout=float(sys.argv[1])))
except subprocess.TimeoutExpired: sys.exit(124)' "$@"
}

# Worker mode: parse-check one script. Called by xargs below.
if [[ "${1:-}" == "__parse_one" ]]; then
  proj="$2"; logdir="$3"; f="$4"
  rel="${f#"$proj"/}"
  log="$logdir/parse_$(printf '%s' "$rel" | tr '/ ' '__').log"
  with_timeout 60 "$GODOT" --headless --path "$proj" --check-only --script "res://$rel" >"$log" 2>&1
  rc=$?
  # --check-only does not instantiate autoloads, so a script that names one fails in the
  # final compile step ("Identifier not found: Telemetry") even though it is valid. Type and
  # parse errors are reported before that step, so dropping only that message hides nothing;
  # smoke_check.gd then loads the script for real, with autoloads present.
  real="$log"
  autoloads="$(awk '/^\[/{on=($0=="[autoload]")} on && /=/{split($0,a,"="); print a[1]}' "$proj/project.godot" | paste -sd'|' -)"
  if [[ -n "$autoloads" ]] && grep -Eq "Compile Error: Identifier not found: ($autoloads)\$" "$log"; then
    real="$log.filtered"
    grep -Ev "Compile Error: Identifier not found: ($autoloads)\$|Failed to load script .*Compilation failed" "$log" >"$real"
    rc=0
  fi
  if [[ $rc -eq 0 ]] && ! grep -Eq "$BAD_PATTERN" "$real"; then
    exit 0
  fi
  printf '%s\n' "$rel" >>"$logdir/parse_failed.txt"
  exit 0
fi

PROJ="${1:-}"; shift || true
RUN_TESTS=1 RUN_IMPORT=1 RUN_PLAYTEST=0 RUN_CARGO=1 RUN_ASSETS=1 OWNED_ADDONS=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-tests) RUN_TESTS=0 ;;
    --no-import) RUN_IMPORT=0 ;;
    --playtest) RUN_PLAYTEST=1 ;;
    --no-cargo) RUN_CARGO=0 ;;
    --no-assets) RUN_ASSETS=0 ;;
    --addons) shift; OWNED_ADDONS="${1:-}" ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

if [[ -z "$PROJ" || ! -f "$PROJ/project.godot" ]]; then
  echo "usage: $0 <dir containing project.godot> [--no-import] [--no-tests] [--playtest] [--addons a,b] [--no-cargo] [--no-assets]" >&2
  exit 2
fi
[[ -n "$GODOT" && -x "$GODOT" ]] || { echo "Godot not found (run install_godot.sh beside this script, or set GODOT=/path/to/godot)" >&2; exit 2; }
PROJ="$(cd "$PROJ" && pwd)"
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
HERE="$(dirname "$SELF")"
HASH="$(printf '%s' "$PROJ" | cksum | cut -d' ' -f1)"
LOGDIR="${TMPDIR:-/tmp}/godot_verify/$(basename "$PROJ")_$HASH"
rm -rf "$LOGDIR"; mkdir -p "$LOGDIR"
FAIL=0
JOBS="${VERIFY_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"

echo "Godot: $("$GODOT" --version 2>/dev/null | head -1)"
echo "Project: $PROJ"
if [[ -f "$PROJ/pipeline.json" ]]; then
  want="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("engine",""))' "$PROJ/pipeline.json" 2>/dev/null)"
  have="$("$GODOT" --version 2>/dev/null | head -1)"
  if [[ -n "$want" && "$have" != "$want"* ]]; then
    echo "FAIL engine pin: project wants $want, running $have"; FAIL=1
  fi
fi

# 1. Import
if [[ $RUN_IMPORT -eq 1 ]]; then
  # A project with no cache cannot resolve class_name on its first pass, so warm it up once.
  if [[ ! -f "$PROJ/.godot/global_script_class_cache.cfg" ]]; then
    with_timeout 600 "$GODOT" --headless --path "$PROJ" --import >"$LOGDIR/import_warmup.log" 2>&1 || true
  fi
  with_timeout 600 "$GODOT" --headless --path "$PROJ" --import >"$LOGDIR/import.log" 2>&1
  rc=$?
  if [[ $rc -ne 0 ]] || grep -Eq "$BAD_PATTERN" "$LOGDIR/import.log"; then
    echo "FAIL import (exit $rc, see $LOGDIR/import.log)"
    grep -E "$BAD_PATTERN|^ERROR" "$LOGDIR/import.log" | head -20
    FAIL=1
  else
    echo "ok   import"
  fi
fi

# 2. Parse-check
list_scripts() {
  find "$PROJ" -name '*.gd' -not -path '*/.godot/*' -not -path '*/addons/*' -print0
  local a
  for a in ${OWNED_ADDONS//,/ }; do
    [[ -d "$PROJ/addons/$a" ]] && find "$PROJ/addons/$a" -name '*.gd' -print0
  done
  return 0
}
: >"$LOGDIR/parse_failed.txt"
COUNT="$(list_scripts | tr -dc '\0' | wc -c | tr -d ' ')"
export GODOT
list_scripts | xargs -0 -P "$JOBS" -n 1 "$SELF" __parse_one "$PROJ" "$LOGDIR"
if [[ -s "$LOGDIR/parse_failed.txt" ]]; then
  sort -u -o "$LOGDIR/parse_failed.txt" "$LOGDIR/parse_failed.txt"
  while IFS= read -r rel; do
    echo "FAIL parse $rel"
    grep -E "$BAD_PATTERN|ERROR" "$LOGDIR/parse_$(printf '%s' "$rel" | tr '/ ' '__').log" | head -5
  done <"$LOGDIR/parse_failed.txt"
  FAIL=1
else
  echo "ok   parse ($COUNT scripts)"
fi

# 3. Headless checks
find_tests() {  # find_tests PATTERN
  find "$PROJ" -maxdepth 1 -name "$1" -print0
  local d
  for d in "$PROJ/tests" "$PROJ/scripts/tests"; do
    [[ -d "$d" ]] && find "$d" -name "$1" -print0
  done
  return 0
}
run_tests() {  # run_tests PATTERN TIMEOUT
  local t rel log ran=0
  while IFS= read -r -d '' t; do
    ran=1
    rel="${t#"$PROJ"/}"
    log="$LOGDIR/test_$(printf '%s' "$rel" | tr '/ ' '__').log"
    # A check that does not parse would hang headless Godot: no main loop ever quits.
    if grep -Fxq "$rel" "$LOGDIR/parse_failed.txt"; then
      echo "FAIL test $rel (did not parse)"; FAIL=1; continue
    fi
    with_timeout "$2" "$GODOT" --headless --path "$PROJ" --script "res://$rel" >"$log" 2>&1 </dev/null
    local rc=$?
    if [[ $rc -eq 0 ]] && ! grep -Eq "$BAD_PATTERN" "$log"; then
      echo "ok   test $rel"
    else
      [[ $rc -eq 124 ]] && echo "     timed out after $2 s"
      echo "FAIL test $rel (exit $rc, see $log)"; tail -15 "$log"; FAIL=1
    fi
  done < <(find_tests "$1" | sort -z)
  [[ $ran -eq 1 ]] || echo "note no $1 found"
}
if [[ $RUN_TESTS -eq 1 ]]; then run_tests '*_check.gd' "${VERIFY_TEST_TIMEOUT:-120}"; fi
if [[ $RUN_PLAYTEST -eq 1 ]]; then run_tests '*_playtest.gd' "${VERIFY_PLAYTEST_TIMEOUT:-900}"; fi

# 4. Assets and Rust
if [[ $RUN_ASSETS -eq 1 && -f "$PROJ/pipeline.json" && -f "$HERE/asset_audit.py" ]]; then
  if python3 "$HERE/asset_audit.py" "$PROJ"; then :; else FAIL=1; fi
fi
if [[ $RUN_CARGO -eq 1 ]]; then
  for c in "$PROJ/rust" "$PROJ/../rust"; do
    [[ -f "$c/Cargo.toml" ]] || continue
    if ! command -v cargo >/dev/null 2>&1; then echo "FAIL cargo test (cargo not on PATH)"; FAIL=1; break; fi
    if (cd "$c" && cargo test --locked) >"$LOGDIR/cargo.log" 2>&1; then
      echo "ok   cargo test"
    else
      echo "FAIL cargo test (see $LOGDIR/cargo.log)"; tail -15 "$LOGDIR/cargo.log"; FAIL=1
    fi
    break
  done
fi

if [[ $FAIL -eq 0 ]]; then echo "ALL GREEN"; else echo "VERIFY FAILED: logs in $LOGDIR"; fi
exit $FAIL
