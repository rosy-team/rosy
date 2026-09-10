#!/bin/bash
# ============================================================================
#  Rosy vs COSY Performance Comparison - Local (Non-MPI) Benchmarks
# ============================================================================
#
#  Usage:
#    ./run_local.sh                              # Rosy only, default TIER_SCALE
#    ./run_local.sh --cosy ./cosy                # Compare with COSY
#    ./run_local.sh --rosy ~/.cargo/bin/rosy --cosy ./cosy
#    ./run_local.sh --benchmark 01               # Run a specific benchmark
#    ./run_local.sh --scale 10000                # Uniform TIER_SCALE (expensive benches map it down)
#
#  Rosy binaries are always built with --optimized.
#
#  Each bench WRITEs a `Result:` checksum. With --cosy those values are
#  compared (rel 1e-5 / abs 1e-8) in the Rosy Result / COSY Result columns.

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
ROSY_BIN="${ROSY_BIN:-rosy}"
COSY_BIN="${COSY_BIN:-}"
BENCHMARK_FILTER=""
SCALE_OVERRIDE=""
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NON_MPI_DIR="$SCRIPT_DIR/non_mpi"
RUN_TIMEOUT=30

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

# Old T4 work units — used when --scale is omitted (one value per benchmark).
# Expensive benches interpret TIER_SCALE with a divisor / cube-root so a uniform
# --scale 10000 stays in the same time band as the cheap arithmetic tests; these
# defaults are the pre-divisor values (same T4 work as before).
default_scale() {
    case "$1" in
        01_arithmetic_loop)          echo 500000000 ;;
        02_da_multiply)              echo 500000 ;;
        03_da_trig)                  echo 500000 ;;
        04_matrix_inversion)         echo 20000 ;;
        05_matrix_determinant)       echo 20000 ;;
        06_optimization_simplex)     echo 50000 ;;
        07_optimization_lmdif)       echo 500000 ;;
        08_math_functions)           echo 500000000 ;;
        09_vector_operations)        echo 1000000 ;;
        10_nested_loops)             echo 125000000 ;;
        11_da_derivatives)           echo 500000 ;;
        12_string_operations)        echo 1000000 ;;
        13_da_transfer_map)          echo 200000 ;;
        14_da_high_order_multiply)   echo 5000 ;;
        15_da_bending_magnet)        echo 50000 ;;
        16_da_aberration)            echo 50000 ;;
        17_polval_map_eval)          echo 16000000 ;;
        18_any_operations)           echo 10000000 ;;
        *)                           echo 1 ;;
    esac
}

cleanup_artifacts() {
    local dir="$1"
    rm -f "$dir"/*.txt "$dir"/*.dat "$dir"/*.lis \
          "$dir"/bench_rosy "$dir"/bench_rosy_* 2>/dev/null || true
}

# WRITE prints each argument on its own line, so
#   WRITE 6 'Result: ' Z
# becomes:
#   Result:
#    0.1234567E+001
parse_result() {
    local file="$1"
    [[ -f "$file" ]] || { echo ""; return; }
    awk '
        {
            line = $0
            if (match(line, /Result:/)) {
                rest = substr(line, RSTART + RLENGTH)
                gsub(/^[ \t]+|[ \t]+$/, "", rest)
                if (rest != "") { val = rest; next }
                pending = 1
                next
            }
            if (pending) {
                gsub(/^[ \t]+|[ \t]+$/, "", line)
                if (line != "") { val = line; pending = 0 }
            }
        }
        END { print val }
    ' "$file"
}

# Compact a result for the table. Numbers become %.6e so COSY/Rosy
# formatting (G15.7 vs raw) still lines up.
format_result() {
    awk -v r="$1" 'BEGIN {
        gsub(/^[ \t]+|[ \t]+$/, "", r)
        if (r == "") { print "N/A"; exit }
        num = "^[+-]?(([0-9]+\\.?[0-9]*)|(\\.[0-9]+))([eE][+-]?[0-9]+)?$"
        if (r ~ num) { printf "%.6e", r + 0; exit }
        if (length(r) > 16) r = substr(r, 1, 15) "…"
        print r
    }'
}

# Numeric (rel 1e-5 / abs 1e-8) or exact string. Prints OK or DIFF.
compare_results() {
    awk -v a="$1" -v b="$2" 'BEGIN {
        gsub(/^[ \t]+|[ \t]+$/, "", a)
        gsub(/^[ \t]+|[ \t]+$/, "", b)
        if (a == "" || b == "") { print "N/A"; exit }
        if (a == b) { print "OK"; exit }
        num = "^[+-]?(([0-9]+\\.?[0-9]*)|(\\.[0-9]+))([eE][+-]?[0-9]+)?$"
        if (a ~ num && b ~ num) {
            fa = a + 0
            fb = b + 0
            d = fa - fb
            if (d < 0) d = -d
            ma = (fa < 0) ? -fa : fa
            mb = (fb < 0) ? -fb : fb
            m = (ma > mb) ? ma : mb
            if (d <= 1e-8 || (m > 0 && d / m <= 1e-5)) { print "OK"; exit }
        }
        print "DIFF"
    }'
}

# ── Parse Arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rosy)       ROSY_BIN="$2"; shift 2 ;;
        --cosy)       COSY_BIN="$2"; shift 2 ;;
        --benchmark)  BENCHMARK_FILTER="$2"; shift 2 ;;
        --scale)      SCALE_OVERRIDE="$2"; shift 2 ;;
        --timeout)    RUN_TIMEOUT="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--rosy PATH] [--cosy PATH] [--benchmark NUM] [--scale N] [--timeout SECS]"
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
echo "  Timeout: ${RUN_TIMEOUT}s"
if [[ -n "$SCALE_OVERRIDE" ]]; then
    echo "  TIER_SCALE: $SCALE_OVERRIDE (override)"
else
    echo "  TIER_SCALE: per-benchmark defaults (old T4 work units)"
fi
echo "  Mode:  optimized (nightly SIMD + fat LTO)"
echo ""

# ── Build Directory (outside workspace to avoid Cargo conflicts) ──────────────
BUILD_DIR=$(mktemp -d /tmp/rosy_bench_XXXXXX)
trap "rm -rf $BUILD_DIR; cleanup_artifacts \"$NON_MPI_DIR\"" EXIT

echo "  Note: first build takes ~4s (cold cache), subsequent ~1s each"
echo ""

# ── Table Header ──────────────────────────────────────────────────────────────
if $HAS_COSY; then
    printf "%-28s %12s %11s %11s %10s %16s %16s %6s\n" \
           "Benchmark" "Scale" "Rosy (ms)" "COSY (ms)" "Speedup" "Rosy Result" "COSY Result" "Match"
    printf "%-28s %12s %11s %11s %10s %16s %16s %6s\n" \
           "----------------------------" "------------" "-----------" "-----------" "----------" \
           "----------------" "----------------" "------"
else
    printf "%-28s %12s %11s %16s\n" "Benchmark" "Scale" "Rosy (ms)" "Rosy Result"
    printf "%-28s %12s %11s %16s\n" \
           "----------------------------" "------------" "-----------" "----------------"
fi

# ── Run Benchmarks ────────────────────────────────────────────────────────────
TOTAL_ROSY_MS=0
TOTAL_COSY_MS=0
NUM_TESTS=0
BENCH_COUNT=0
NUM_MATCH=0
NUM_DIFF=0
DIFF_NAMES=""

for fox_file in "$NON_MPI_DIR"/*.fox; do
    [[ -f "$fox_file" ]] || continue
    name=$(basename "$fox_file" .fox)

    if [[ -n "$BENCHMARK_FILTER" ]] && [[ ! "$name" == *"$BENCHMARK_FILTER"* ]]; then
        continue
    fi

    BENCH_COUNT=$((BENCH_COUNT + 1))
    NUM_TESTS=$((NUM_TESTS + 1))

    if [[ -n "$SCALE_OVERRIDE" ]]; then
        scale="$SCALE_OVERRIDE"
    else
        scale=$(default_scale "$name")
    fi

    printf "  [%d] %s scale=%s...\r" "$NUM_TESTS" "$name" "$scale" >&2

    BUILD_FLAGS="--optimized"
    rosy_bin_path="$NON_MPI_DIR/bench_rosy_${name}"
    if ! "$ROSY_BIN" build "$fox_file" $BUILD_FLAGS -d "$BUILD_DIR" -o "$rosy_bin_path" 2>/dev/null; then
        printf "\r%80s\r" "" >&2
        if $HAS_COSY; then
            printf "%-28s %12s %11s %11s %10s %16s %16s %6s\n" \
                   "$name" "$scale" "BUILD FAIL" "-" "-" "-" "-" "N/A"
        else
            printf "%-28s %12s %11s %16s\n" "$name" "$scale" "BUILD FAIL" "-"
        fi
        continue
    fi

    echo "$scale" > "$NON_MPI_DIR/tier_scale.dat"

    rosy_start=$(date +%s%N)
    run_timeout $RUN_TIMEOUT "$rosy_bin_path" "$scale" > "$NON_MPI_DIR/rosy_${name}_output.txt" 2>&1 || true
    rosy_end=$(date +%s%N)
    rosy_ms=$(awk "BEGIN { printf \"%.2f\", ($rosy_end - $rosy_start) / 1000000 }")
    TOTAL_ROSY_MS=$(awk "BEGIN { printf \"%.2f\", $TOTAL_ROSY_MS + $rosy_ms }")
    rosy_result=$(parse_result "$NON_MPI_DIR/rosy_${name}_output.txt")
    rosy_result_fmt=$(format_result "$rosy_result")

    if $HAS_COSY; then
        cosy_fox_base="$name"
        cosy_start=$(date +%s%N)
        # COSY reads the .fox basename from stdin. Do not pass TIER_SCALE as
        # argv — COSY treats extra args as the source file name.
        (cd "$NON_MPI_DIR" && echo "$cosy_fox_base" | run_timeout $RUN_TIMEOUT "$COSY_BIN") \
            > "$NON_MPI_DIR/cosy_${name}_output.txt" 2>&1 || true
        cosy_end=$(date +%s%N)
        cosy_ms=$(awk "BEGIN { printf \"%.2f\", ($cosy_end - $cosy_start) / 1000000 }")
        cosy_result=$(parse_result "$NON_MPI_DIR/cosy_${name}_output.txt")
        cosy_result_fmt=$(format_result "$cosy_result")
        match=$(compare_results "$rosy_result" "$cosy_result")

        if [[ ! -s "$NON_MPI_DIR/cosy_${name}_output.txt" ]] || grep -qE "### ERROR|ERROR OCCURED|cannot execute|Exec format error|No such file" "$NON_MPI_DIR/cosy_${name}_output.txt" 2>/dev/null; then
            printf "\r%80s\r" "" >&2
            printf "%-28s %12s %11.2f %11s %10s %16s %16s %6s\n" \
                   "$name" "$scale" "$rosy_ms" "COSY ERR" "N/A" "$rosy_result_fmt" "-" "N/A"
        else
            TOTAL_COSY_MS=$(awk -v a="$TOTAL_COSY_MS" -v b="$cosy_ms" 'BEGIN { printf "%.2f", a + b }')
            speedup=$(awk -v r="$rosy_ms" -v c="$cosy_ms" 'BEGIN { if (r > 0.01) printf "%.1f", c / r; else print "INF" }')
            if [[ "$match" == "OK" ]]; then
                NUM_MATCH=$((NUM_MATCH + 1))
            elif [[ "$match" == "DIFF" ]]; then
                NUM_DIFF=$((NUM_DIFF + 1))
                DIFF_NAMES="${DIFF_NAMES} ${name}"
            fi
            printf "\r%80s\r" "" >&2
            printf "%-28s %12s %11.2f %11.2f %9sx %16s %16s %6s\n" \
                   "$name" "$scale" "$rosy_ms" "$cosy_ms" "$speedup" \
                   "$rosy_result_fmt" "$cosy_result_fmt" "$match"
            if [[ "$match" == "DIFF" ]]; then
                printf "    rosy=%s\n    cosy=%s\n" "$rosy_result" "$cosy_result"
            fi
        fi
    else
        printf "\r%80s\r" "" >&2
        printf "%-28s %12s %11.2f %16s\n" "$name" "$scale" "$rosy_ms" "$rosy_result_fmt"
    fi

    rm -f "$rosy_bin_path"
    cleanup_artifacts "$NON_MPI_DIR"
done

echo ""
if [[ "$NUM_TESTS" -eq 0 ]]; then
    echo "  No benchmarks matched filter '${BENCHMARK_FILTER:-}'."
    echo "  Files are named like 18_any_operations.fox (fibonacci was removed)."
    echo "================================================================"
    exit 1
fi
if $HAS_COSY; then
    printf "%-28s %12s %11s %11s %10s %16s %16s %6s\n" \
           "----------------------------" "------------" "-----------" "-----------" "----------" \
           "----------------" "----------------" "------"
    total_speedup=$(awk -v r="$TOTAL_ROSY_MS" -v c="$TOTAL_COSY_MS" 'BEGIN { if (r > 0.01) printf "%.1f", c / r; else print "INF" }')
    printf "%-28s      %11.2f %11.2f %9sx %16s %16s %6s\n" \
           "TOTAL ($NUM_TESTS tests)" "$TOTAL_ROSY_MS" "$TOTAL_COSY_MS" "$total_speedup" \
           "" "" "${NUM_MATCH} OK"
    echo ""
    echo "  Results: ${NUM_MATCH} match, ${NUM_DIFF} differ (rel 1e-5 / abs 1e-8)"
    if [[ "$NUM_DIFF" -gt 0 ]]; then
        echo "  Mismatches:${DIFF_NAMES}"
    fi
else
    printf "%-28s %12s %11s %16s\n" \
           "----------------------------" "------------" "-----------" "----------------"
    printf "%-28s      %11.2f\n" "TOTAL ($NUM_TESTS tests)" "$TOTAL_ROSY_MS"
fi
echo ""
echo "  $BENCH_COUNT benchmarks (one TIER_SCALE each)"
echo "  Timeout: ${RUN_TIMEOUT}s"
echo "================================================================"
