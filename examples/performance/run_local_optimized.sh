#!/bin/bash
# ============================================================================
#  Rosy vs COSY Performance Comparison - Local (Non-MPI) Benchmarks [OPTIMIZED]
#  Same as run_local.sh but saves output to *_output_optimized.txt files.
# ============================================================================
#
#  Usage:
#    ./run_local.sh                              # Rosy only
#    ./run_local.sh --cosy ./cosy                # Compare with COSY
#    ./run_local.sh --rosy ~/.cargo/bin/rosy --cosy ./cosy
#    ./run_local.sh --benchmark 01               # Run a specific benchmark
#    ./run_local.sh --tier 2                     # Run only tier 2
#

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
ROSY_BIN="${ROSY_BIN:-rosy}"
COSY_BIN="${COSY_BIN:-}"
BENCHMARK_FILTER=""
TIER_FILTER=""
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
T4_TIMEOUT=30
# GNU timeout is missing on stock macOS; gtimeout (coreutils) or perl alarm.
run_timeout() {
    local secs="$1"
    shift
    if command -v timeout >/dev/null 2>&1; then
        timeout "$secs" "$@"
    elif command -v gtimeout >/dev/null 2>&1; then
        gtimeout "$secs" "$@"
    else
        perl -e 'alarm shift; exec @ARGV' "$secs" "$@"
    fi
}

# ── Parse Arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rosy)       ROSY_BIN="$2"; shift 2 ;;
        --cosy)       COSY_BIN="$2"; shift 2 ;;
        --benchmark)  BENCHMARK_FILTER="$2"; shift 2 ;;
        --tier)       TIER_FILTER="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--rosy PATH] [--cosy PATH] [--benchmark NUM] [--tier 1-4]"
            exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Verify Tools ──────────────────────────────────────────────────────────────
if ! command -v "$ROSY_BIN" &>/dev/null; then
    echo "ERROR: Rosy binary not found at '$ROSY_BIN'"
    echo "Install with: cargo install --path rosy"
    exit 1
fi

HAS_COSY=false
# Resolve to absolute path so it works inside subshells (cd)
if [[ -n "$COSY_BIN" ]]; then
    COSY_BIN="$(cd "$(dirname "$COSY_BIN")" && pwd)/$(basename "$COSY_BIN")"
fi
if [[ -n "$COSY_BIN" ]] && [[ -x "$COSY_BIN" ]]; then
    HAS_COSY=true
    if strings "$COSY_BIN" 2>/dev/null | grep -q rosy_lib; then
        echo "WARNING: $COSY_BIN looks like a Rosy binary (contains rosy_lib), not the COSY INFINITY interpreter."
        echo "Times from this binary are not COSY. Pass the Fortran COSY executable to --cosy."
        HAS_COSY=false
    fi
elif [[ -n "$COSY_BIN" ]]; then
    echo "WARNING: COSY binary not found/executable at '$COSY_BIN'"
    echo "Running Rosy-only benchmarks."
fi

# ── Header ────────────────────────────────────────────────────────────────────
echo ""
echo "================================================================"
echo "       Rosy vs COSY Performance Comparison (Non-MPI)"
echo "================================================================"
echo ""
echo "  Rosy:  $ROSY_BIN ($($ROSY_BIN --version 2>/dev/null || echo 'unknown'))"
if $HAS_COSY; then
    echo "  COSY:  $COSY_BIN"
else
    echo "  COSY:  (not configured - use --cosy PATH)"
fi
echo "  Date:  $(date)"
echo "  Host:  $(hostname)"
echo "  T4 timeout: ${T4_TIMEOUT}s"
echo ""

# ── Build Directory (outside workspace to avoid Cargo conflicts) ──────────────
BUILD_DIR=$(mktemp -d /tmp/rosy_bench_XXXXXX)
trap "rm -rf $BUILD_DIR" EXIT

echo "  Note: first build takes ~4s (cold cache), subsequent ~1s each"
echo ""

# ── Table Header ──────────────────────────────────────────────────────────────
if $HAS_COSY; then
    printf "%-28s %-4s %11s %11s %10s\n" "Benchmark" "Tier" "Rosy (ms)" "COSY (ms)" "Speedup"
    printf "%-28s %-4s %11s %11s %10s\n" \
           "----------------------------" "----" "-----------" "-----------" "----------"
else
    printf "%-28s %-4s %11s\n" "Benchmark" "Tier" "Rosy (ms)"
    printf "%-28s %-4s %11s\n" \
           "----------------------------" "----" "-----------"
fi

# ── Run Benchmarks ────────────────────────────────────────────────────────────
TOTAL_ROSY_MS=0
TOTAL_COSY_MS=0
NUM_TESTS=0
BENCH_COUNT=0

for dir in "$SCRIPT_DIR"/non_mpi/*/; do
    name=$(basename "$dir")

    # Filter by benchmark name
    if [[ -n "$BENCHMARK_FILTER" ]] && [[ ! "$name" == *"$BENCHMARK_FILTER"* ]]; then
        continue
    fi

    BENCH_COUNT=$((BENCH_COUNT + 1))

    for tier in 1 2 3 4; do
        # Filter by tier
        if [[ -n "$TIER_FILTER" ]] && [[ "$tier" != "$TIER_FILTER" ]]; then
            continue
        fi

        rosy_file="$dir/bench_t${tier}.rosy"
        fox_file="$dir/bench_t${tier}.fox"

        if [[ ! -f "$rosy_file" ]]; then
            continue
        fi

        NUM_TESTS=$((NUM_TESTS + 1))
        tier_label="T${tier}"

        # Progress
        printf "  [%d] %s %s...\r" "$NUM_TESTS" "$name" "$tier_label" >&2

        # ── Build Rosy ────────────────────────────────────────────────
        if ! "$ROSY_BIN" build "$rosy_file" --release --optimized -d "$BUILD_DIR" -o "$dir/bench_rosy_t${tier}" 2>/dev/null; then
            printf "\r%80s\r" "" >&2
            printf "%-28s %-4s %11s\n" "$name" "$tier_label" "BUILD FAIL"
            continue
        fi

        # ── Run Rosy (T4 gets timeout) ────────────────────────────────
        rosy_start=$(date +%s%N)
        if [[ "$tier" == "4" ]]; then
            run_timeout $T4_TIMEOUT "$dir/bench_rosy_t${tier}" > "$dir/rosy_t${tier}_output_optimized.txt" 2>&1 || true
        else
            "$dir/bench_rosy_t${tier}" > "$dir/rosy_t${tier}_output_optimized.txt" 2>&1 || true
        fi
        rosy_end=$(date +%s%N)
        rosy_ms=$(awk "BEGIN { printf \"%.2f\", ($rosy_end - $rosy_start) / 1000000 }")
        TOTAL_ROSY_MS=$(awk "BEGIN { printf \"%.2f\", $TOTAL_ROSY_MS + $rosy_ms }")

        # ── Run COSY (T4 gets timeout) ────────────────────────────────
        # COSY reads filename from stdin (no .fox extension).
        # It has a short path buffer, so cd into the directory and use bare name.
        if $HAS_COSY && [[ -f "$fox_file" ]]; then
            cosy_fox_base="bench_t${tier}"
            cosy_start=$(date +%s%N)
            if [[ "$tier" == "4" ]]; then
                (cd "$dir" && echo "$cosy_fox_base" | run_timeout $T4_TIMEOUT "$COSY_BIN") > "$dir/cosy_t${tier}_output.txt" 2>&1 || true
            else
                (cd "$dir" && echo "$cosy_fox_base" | "$COSY_BIN") > "$dir/cosy_t${tier}_output.txt" 2>&1 || true
            fi
            cosy_end=$(date +%s%N)
            cosy_ms=$(awk "BEGIN { printf \"%.2f\", ($cosy_end - $cosy_start) / 1000000 }")

            # Check if COSY reported errors (compilation or runtime)
            if [[ ! -s "$dir/cosy_t${tier}_output.txt" ]] || grep -qE "### ERROR|ERROR OCCURED|cannot execute|Exec format error|No such file" "$dir/cosy_t${tier}_output.txt" 2>/dev/null; then
                printf "\r%80s\r" "" >&2
                printf "%-28s %-4s %11.2f %11s %10s\n" "$name" "$tier_label" "$rosy_ms" "COSY ERR" "N/A"
            else
                TOTAL_COSY_MS=$(awk "BEGIN { printf \"%.2f\", $TOTAL_COSY_MS + $cosy_ms }")

                speedup=$(awk "BEGIN { if ($rosy_ms > 0.01) printf \"%.1f\", $cosy_ms / $rosy_ms; else print \"INF\" }")

                printf "\r%80s\r" "" >&2
                printf "%-28s %-4s %11.2f %11.2f %9sx\n" "$name" "$tier_label" "$rosy_ms" "$cosy_ms" "$speedup"
            fi
        else
            printf "\r%80s\r" "" >&2
            printf "%-28s %-4s %11.2f\n" "$name" "$tier_label" "$rosy_ms"
        fi

        # Clean up binary
        rm -f "$dir/bench_rosy_t${tier}"
    done
done

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
if $HAS_COSY; then
    printf "%-28s %-4s %11s %11s %10s\n" \
           "----------------------------" "----" "-----------" "-----------" "----------"
    total_speedup=$(awk "BEGIN { if ($TOTAL_ROSY_MS > 0.01) printf \"%.1f\", $TOTAL_COSY_MS / $TOTAL_ROSY_MS; else print \"INF\" }")
    printf "%-28s      %11.2f %11.2f %9sx\n" "TOTAL ($NUM_TESTS tests)" "$TOTAL_ROSY_MS" "$TOTAL_COSY_MS" "$total_speedup"
else
    printf "%-28s %-4s %11s\n" \
           "----------------------------" "----" "-----------"
    printf "%-28s      %11.2f\n" "TOTAL ($NUM_TESTS tests)" "$TOTAL_ROSY_MS"
fi
echo ""
echo "  $BENCH_COUNT benchmarks x 4 tiers = $NUM_TESTS tests"
echo "  T4 timeout: ${T4_TIMEOUT}s"
echo "================================================================"
