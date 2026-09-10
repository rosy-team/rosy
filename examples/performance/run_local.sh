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

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
ROSY_BIN="${ROSY_BIN:-rosy}"
COSY_BIN="${COSY_BIN:-}"
BENCHMARK_FILTER=""
SCALE_OVERRIDE=""
OPTIMIZED=false
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

# ── Parse Arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rosy)       ROSY_BIN="$2"; shift 2 ;;
        --cosy)       COSY_BIN="$2"; shift 2 ;;
        --benchmark)  BENCHMARK_FILTER="$2"; shift 2 ;;
        --scale)      SCALE_OVERRIDE="$2"; shift 2 ;;
        --timeout)    RUN_TIMEOUT="$2"; shift 2 ;;
        --optimized)  OPTIMIZED=true; shift ;;
        -h|--help)
            echo "Usage: $0 [--rosy PATH] [--cosy PATH] [--benchmark NUM] [--scale N] [--timeout SECS] [--optimized]"
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
if $OPTIMIZED; then
    echo "  Mode:  optimized (nightly SIMD + fat LTO)"
else
    echo "  Mode:  release"
fi
echo ""

# ── Build Directory (outside workspace to avoid Cargo conflicts) ──────────────
BUILD_DIR=$(mktemp -d /tmp/rosy_bench_XXXXXX)
trap "rm -rf $BUILD_DIR; cleanup_artifacts \"$NON_MPI_DIR\"" EXIT

echo "  Note: first build takes ~4s (cold cache), subsequent ~1s each"
echo ""

# ── Table Header ──────────────────────────────────────────────────────────────
if $HAS_COSY; then
    printf "%-28s %12s %11s %11s %10s\n" "Benchmark" "Scale" "Rosy (ms)" "COSY (ms)" "Speedup"
    printf "%-28s %12s %11s %11s %10s\n" \
           "----------------------------" "------------" "-----------" "-----------" "----------"
else
    printf "%-28s %12s %11s\n" "Benchmark" "Scale" "Rosy (ms)"
    printf "%-28s %12s %11s\n" \
           "----------------------------" "------------" "-----------"
fi

# ── Run Benchmarks ────────────────────────────────────────────────────────────
TOTAL_ROSY_MS=0
TOTAL_COSY_MS=0
NUM_TESTS=0
BENCH_COUNT=0

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

    BUILD_FLAGS="--release"
    if $OPTIMIZED; then BUILD_FLAGS="--optimized"; fi
    rosy_bin_path="$NON_MPI_DIR/bench_rosy_${name}"
    if ! "$ROSY_BIN" build "$fox_file" $BUILD_FLAGS -d "$BUILD_DIR" -o "$rosy_bin_path" 2>/dev/null; then
        printf "\r%80s\r" "" >&2
        printf "%-28s %12s %11s\n" "$name" "$scale" "BUILD FAIL"
        continue
    fi

    echo "$scale" > "$NON_MPI_DIR/tier_scale.dat"

    rosy_start=$(date +%s%N)
    run_timeout $RUN_TIMEOUT "$rosy_bin_path" "$scale" > "$NON_MPI_DIR/rosy_${name}_output.txt" 2>&1 || true
    rosy_end=$(date +%s%N)
    rosy_ms=$(awk "BEGIN { printf \"%.2f\", ($rosy_end - $rosy_start) / 1000000 }")
    TOTAL_ROSY_MS=$(awk "BEGIN { printf \"%.2f\", $TOTAL_ROSY_MS + $rosy_ms }")

    if $HAS_COSY; then
        cosy_fox_base="$name"
        cosy_start=$(date +%s%N)
        # COSY reads the .fox basename from stdin. Do not pass TIER_SCALE as
        # argv — COSY treats extra args as the source file name.
        (cd "$NON_MPI_DIR" && echo "$cosy_fox_base" | run_timeout $RUN_TIMEOUT "$COSY_BIN") \
            > "$NON_MPI_DIR/cosy_${name}_output.txt" 2>&1 || true
        cosy_end=$(date +%s%N)
        cosy_ms=$(awk "BEGIN { printf \"%.2f\", ($cosy_end - $cosy_start) / 1000000 }")

        if [[ ! -s "$NON_MPI_DIR/cosy_${name}_output.txt" ]] || grep -qE "### ERROR|ERROR OCCURED|cannot execute|Exec format error|No such file" "$NON_MPI_DIR/cosy_${name}_output.txt" 2>/dev/null; then
            printf "\r%80s\r" "" >&2
            printf "%-28s %12s %11.2f %11s %10s\n" "$name" "$scale" "$rosy_ms" "COSY ERR" "N/A"
        else
            TOTAL_COSY_MS=$(awk -v a="$TOTAL_COSY_MS" -v b="$cosy_ms" 'BEGIN { printf "%.2f", a + b }')
            speedup=$(awk -v r="$rosy_ms" -v c="$cosy_ms" 'BEGIN { if (r > 0.01) printf "%.1f", c / r; else print "INF" }')
            printf "\r%80s\r" "" >&2
            printf "%-28s %12s %11.2f %11.2f %9sx\n" "$name" "$scale" "$rosy_ms" "$cosy_ms" "$speedup"
        fi
    else
        printf "\r%80s\r" "" >&2
        printf "%-28s %12s %11.2f\n" "$name" "$scale" "$rosy_ms"
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
    printf "%-28s %12s %11s %11s %10s\n" \
           "----------------------------" "------------" "-----------" "-----------" "----------"
    total_speedup=$(awk -v r="$TOTAL_ROSY_MS" -v c="$TOTAL_COSY_MS" 'BEGIN { if (r > 0.01) printf "%.1f", c / r; else print "INF" }')
    printf "%-28s      %11.2f %11.2f %9sx\n" "TOTAL ($NUM_TESTS tests)" "$TOTAL_ROSY_MS" "$TOTAL_COSY_MS" "$total_speedup"
else
    printf "%-28s %12s %11s\n" \
           "----------------------------" "------------" "-----------"
    printf "%-28s      %11.2f\n" "TOTAL ($NUM_TESTS tests)" "$TOTAL_ROSY_MS"
fi
echo ""
echo "  $BENCH_COUNT benchmarks (one TIER_SCALE each)"
echo "  Timeout: ${RUN_TIMEOUT}s"
echo "================================================================"
