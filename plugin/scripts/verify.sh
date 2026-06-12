#!/usr/bin/env bash
# Verification Ladder — runs L1–L4 and emits Stage 6/7 DBA artifacts.
# Usage: ./plugin/scripts/verify.sh [--checkpoint]
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_DIR="$REPO_ROOT/logs/verify_$TIMESTAMP"
mkdir -p "$LOG_DIR"

# ── Git state ────────────────────────────────────────────────────────────────

DIRTY=$(git -C "$REPO_ROOT" diff --name-only | wc -l | tr -d ' ')
if [[ "$DIRTY" -gt 0 ]]; then
  echo "WARNING: working tree has $DIRTY modified file(s) — consider: git stash" >&2
fi

if [[ "${1:-}" == "--checkpoint" ]]; then
  git -C "$REPO_ROOT" tag "verify-$TIMESTAMP" -m "Pre-verify checkpoint" >&2
  echo "Tagged: verify-$TIMESTAMP" >&2
fi

# ── Layer runner ─────────────────────────────────────────────────────────────

PASS=0
FAIL=0

run_layer() {
  local name="$1"; shift
  local t0=$SECONDS
  if "$@" > "$LOG_DIR/$name.log" 2>&1; then
    local status="PASS"; ((PASS++)) || true
  else
    local status="FAIL"; ((FAIL++)) || true
  fi
  local dur=$(( (SECONDS - t0) ))
  printf "%-24s %s  (%ds)\n" "$name" "$status" "$dur"
  echo "$name $status $dur $LOG_DIR/$name.log"  >> "$LOG_DIR/summary.txt"
}

echo ""
echo "=== Verification Ladder  $(date -Iseconds) ==="
echo ""

run_layer "L1:cargo_test" \
  cargo test --manifest-path "$REPO_ROOT/modules/Cargo.toml" -q -- --test-threads=1

run_layer "L2a:behavioral" \
  bash -c "cd '$REPO_ROOT/plugin' && npm test -- --testPathPattern=behavioral --runInBand --silent"

run_layer "L2b:serialization" \
  bash -c "cd '$REPO_ROOT/plugin' && npm test -- --testPathPattern=serialization --runInBand --silent"

run_layer "L3:pytest_server" \
  bash -c "cd '$REPO_ROOT/plugin/server' && python3 -m pytest test_server.py -v --tb=short"

run_layer "L4:e2e_contract" \
  bash -c "cd '$REPO_ROOT/plugin' && npm test -- --testPathPattern=e2e --runInBand --silent"

run_layer "L5.5:electron_harness" \
  bash -c "cd '$REPO_ROOT/plugin' && npm run electron:test"

echo ""
echo "=== Results: ${PASS} passed, ${FAIL} failed ==="
echo ""

# ── Stage 6/7 artifact generation ────────────────────────────────────────────

STAGE6="$REPO_ROOT/events/logseq_plugin_stage6_observation.json"
STAGE7="$REPO_ROOT/events/logseq_plugin_stage7_reconciliation.md"

python3 "$REPO_ROOT/plugin/scripts/generate_stage6.py" \
  --log-dir "$LOG_DIR" \
  --output  "$STAGE6"

python3 "$REPO_ROOT/plugin/scripts/render_reconciliation.py" \
  "$STAGE6" > "$STAGE7"

echo "Stage 6: $STAGE6"
echo "Stage 7: $STAGE7"

[[ "$FAIL" -eq 0 ]] || exit 1
