#!/usr/bin/env bash
# Inference-profile sweep for issue #399: quality, latency and total memory for
# each context/KV-cache/Flash-Attention profile, on a real machine with a real
# GGUF.
#
# Each arm runs in a fresh process. That is required, not tidy:
# `LlamaBackend::init` may only be called once per process, and resident memory
# only falls back to a comparable baseline in a new one.
#
# Usage:
#   ./scripts/bench-inference-profiles.sh /abs/path/Qwen3-8B-Q4_K_M.gguf [out.jsonl]
#
# Environment:
#   RAMDOC_BENCH_FILL   percent of the usable context the probe prompt fills (default 75)
#   RAMDOC_BENCH_ARMS   space-separated arms to run (default: all five)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_DIR="$REPO_ROOT/dokassist/src-tauri"

MODEL="${1:-}"
OUT="${2:-$REPO_ROOT/inference-profile-sweep.jsonl}"
ARMS="${RAMDOC_BENCH_ARMS:-conservative governed f16-32k q8-32k q4-32k}"

BOLD='\033[1m'; RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RESET='\033[0m'

if [[ -z "$MODEL" ]]; then
  echo "usage: $0 /abs/path/model.gguf [out.jsonl]" >&2
  exit 2
fi
if [[ ! -f "$MODEL" ]]; then
  echo "no such model file: $MODEL" >&2
  exit 2
fi

# Resolve before the cd, so a relative output path means what the caller typed
# rather than landing inside src-tauri.
OUT="$(cd "$(dirname "$OUT")" && pwd)/$(basename "$OUT")"

cd "$TAURI_DIR"

# A stale sweep would silently be collated together with this one.
: >"$OUT"

# Run a cargo test that prints one JSON object and show only that object. The
# log is kept rather than discarded: dropping stderr would turn a compile error
# into a silent `set -e` exit with nothing to debug.
run_json_step() {
  local label="$1"; shift
  local log
  log="$(mktemp)"
  if "$@" >"$log" 2>&1; then
    sed -n '/^{/,/^}/p' "$log"
    rm -f "$log"
  else
    echo -e "  ${RED}✗${RESET} $label failed:" >&2
    cat "$log" >&2
    rm -f "$log"
    exit 1
  fi
}

echo -e "${BOLD}━━ Planning (no model needed) ━━${RESET}"
run_json_step "planning" \
  cargo test --release plan_inference_profiles -- --ignored --nocapture

echo -e "\n${BOLD}━━ Measuring ━━${RESET}"
for arm in $ARMS; do
  echo -e "  ${BOLD}$arm${RESET}"
  # An arm that the memory governor refuses is a result, not a failure: record
  # it and keep going, so one unsafe profile does not abort the whole sweep.
  if RAMDOC_BENCH_MODEL="$MODEL" \
     RAMDOC_BENCH_PROFILE="$arm" \
     RAMDOC_BENCH_OUT="$OUT" \
     cargo test --release benchmark_inference_profile -- --ignored --nocapture \
       >"/tmp/ramdoc-bench-$arm.log" 2>&1; then
    echo -e "    ${GREEN}✓${RESET} recorded"
  else
    echo -e "    ${YELLOW}~${RESET} refused or failed — see /tmp/ramdoc-bench-$arm.log"
    grep -m1 -E "panicked at|Refusing to load|error:" "/tmp/ramdoc-bench-$arm.log" |
      sed 's/^/      /' || true
  fi
done

echo -e "\n${BOLD}━━ Comparison ━━${RESET}"
run_json_step "collation" \
  env RAMDOC_BENCH_OUT="$OUT" \
  cargo test --release collate_profile_benchmark -- --ignored --nocapture

echo -e "\nRaw records: $OUT"
