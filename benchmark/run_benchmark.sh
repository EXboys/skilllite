#!/bin/bash
#
# SkillLite Benchmark Runner Script
# High Concurrency Performance Comparison Test Script
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SkillLite High-Concurrency Benchmark${NC}"
echo -e "${BLUE}========================================${NC}"

# Default parameters
REQUESTS=${REQUESTS:-100}
CONCURRENCY=${CONCURRENCY:-10}
COLD_START=${COLD_START:-false}
SKIP_DOCKER=${SKIP_DOCKER:-false}
OUTPUT_FILE=""
SANDBOX_LEVEL=""
COMPARE_LEVELS=${COMPARE_LEVELS:-false}
COMPARE_IPC=${COMPARE_IPC:-false}
NATIVE_SANDBOX_CORE=${NATIVE_SANDBOX_CORE:-false}
CORE_ONLY=${CORE_ONLY:-false}
NATIVE_SANDBOX_ITERATIONS=${NATIVE_SANDBOX_ITERATIONS:-200}
NATIVE_SANDBOX_WARMUP=${NATIVE_SANDBOX_WARMUP:-20}
DOCKER_CORE_IMAGE=${DOCKER_CORE_IMAGE:-alpine:3.20}
DOCKER_CORE_TIMEOUT_SECS=${DOCKER_CORE_TIMEOUT_SECS:-5}
SRT_CORE_BIN=${SRT_CORE_BIN:-}

run_with_timeout() {
    local timeout_secs="$1"
    shift
    "$@" &
    local cmd_pid=$!
    (
        sleep "$timeout_secs"
        kill "$cmd_pid" 2>/dev/null || true
    ) &
    local watchdog_pid=$!

    wait "$cmd_pid"
    local status=$?
    kill "$watchdog_pid" 2>/dev/null || true
    wait "$watchdog_pid" 2>/dev/null || true
    return "$status"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--requests)
            REQUESTS="$2"
            shift 2
            ;;
        -c|--concurrency)
            CONCURRENCY="$2"
            shift 2
            ;;
        --cold-start)
            COLD_START=true
            shift
            ;;
        --skip-docker)
            SKIP_DOCKER=true
            shift
            ;;
        -o|--output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        -l|--sandbox-level)
            SANDBOX_LEVEL="$2"
            shift 2
            ;;
        --compare-levels)
            COMPARE_LEVELS=true
            shift
            ;;
        --compare-ipc)
            COMPARE_IPC=true
            shift
            ;;
        --native-sandbox-core)
            NATIVE_SANDBOX_CORE=true
            shift
            ;;
        --core-only)
            NATIVE_SANDBOX_CORE=true
            CORE_ONLY=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -n, --requests NUM     Number of requests (default: 100)"
            echo "  -c, --concurrency NUM  Concurrency level (default: 10)"
            echo "  --cold-start           Run cold start benchmark"
            echo "  --skip-docker          Skip Docker tests"
            echo "  -l, --sandbox-level N  SkillLite sandbox level (1, 2, or 3)"
            echo "                         1 = No sandbox (direct execution)"
            echo "                         2 = Sandbox isolation only"
            echo "                         3 = Sandbox + static code scan (default)"
            echo "                         Can also be set via SKILLLITE_SANDBOX_LEVEL env var"
            echo "  --compare-levels       Compare sandbox levels and enable memory stats"
            echo "                         for SkillLite, Docker, SRT, Pyodide when available"
            echo "  --compare-ipc          Include SkillLite IPC vs subprocess comparison"
            echo "  --native-sandbox-core  Also run native /usr/bin/true sandbox core microbenchmark"
            echo "                         Configure with NATIVE_SANDBOX_ITERATIONS and NATIVE_SANDBOX_WARMUP"
            echo "                         Docker core uses DOCKER_CORE_IMAGE (default: alpine:3.20)"
            echo "  --core-only            Run only no-Python native/sandbox/SRT/Docker core benchmarks"
            echo "  -o, --output FILE      Output JSON file"
            echo "  -h, --help             Show this help"
            echo ""
            echo "Examples:"
            echo "  $0 -n 500 -c 50                    # 500 requests, 50 concurrent"
            echo "  $0 --cold-start -n 100             # With cold start test"
            echo "  $0 -l 2                            # Test with sandbox level 2"
            echo "  $0 --compare-levels                # Compare all sandbox levels"
            echo "  $0 --native-sandbox-core           # Include no-Python sandbox core benchmark"
            echo "  $0 --core-only                     # Only run no-Python core benchmark"
            echo "  SKILLLITE_SANDBOX_LEVEL=1 $0       # Use env var to set level"
            echo "  $0 -o results.json                 # Save results to file"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Check if SkillLite is compiled
SKILLLITE_BIN=""
if command -v skilllite &> /dev/null; then
    SKILLLITE_BIN=$(which skilllite)
elif [ -f "$PROJECT_ROOT/target/release/skilllite" ]; then
    SKILLLITE_BIN="$PROJECT_ROOT/target/release/skilllite"
else
    echo -e "${YELLOW}SkillLite binary not found. Building...${NC}"
    cd "$PROJECT_ROOT/skilllite"
    cargo build --release
    SKILLLITE_BIN="$PROJECT_ROOT/target/release/skilllite"
fi

echo -e "${GREEN}SkillLite binary: $SKILLLITE_BIN${NC}"

# Run the native sandbox core benchmark before Python E2E so its no-Python
# measurement remains clearly separated from the skill benchmark below.
if [ "$NATIVE_SANDBOX_CORE" = true ]; then
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}cargo is required for --native-sandbox-core but was not found${NC}"
        exit 1
    fi

    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Native Sandbox Core Benchmark${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo "Program: /usr/bin/true"
    echo "Iterations: $NATIVE_SANDBOX_ITERATIONS"
    echo "Warmup: $NATIVE_SANDBOX_WARMUP"
    echo "Docker image: $DOCKER_CORE_IMAGE"
    echo ""

    cd "$PROJECT_ROOT"
    NATIVE_OUTPUT=$(cargo run --release -p skilllite-sandbox --example native_sandbox_microbench -- \
        --level native \
        --iterations "$NATIVE_SANDBOX_ITERATIONS" \
        --warmup "$NATIVE_SANDBOX_WARMUP")
    echo "$NATIVE_OUTPUT"
    echo ""

    SANDBOX_OUTPUT=$(cargo run --release -p skilllite-sandbox --example native_sandbox_microbench -- \
        --level sandbox \
        --iterations "$NATIVE_SANDBOX_ITERATIONS" \
        --warmup "$NATIVE_SANDBOX_WARMUP")
    echo "$SANDBOX_OUTPUT"
    echo ""

    SRT_OUTPUT=""
    if [ -z "$SRT_CORE_BIN" ]; then
        if command -v srt &> /dev/null; then
            SRT_CORE_BIN="srt"
        elif command -v sandbox-runtime &> /dev/null; then
            SRT_CORE_BIN="sandbox-runtime"
        fi
    fi
    if [ -n "$SRT_CORE_BIN" ]; then
        SRT_OUTPUT=$(cargo run --release -p skilllite-sandbox --example native_sandbox_microbench -- \
            --level srt \
            --iterations "$NATIVE_SANDBOX_ITERATIONS" \
            --warmup "$NATIVE_SANDBOX_WARMUP" \
            --srt-bin "$SRT_CORE_BIN")
        echo "$SRT_OUTPUT"
        echo ""
    else
        echo -e "${YELLOW}SRT core skipped: srt/sandbox-runtime is not available.${NC}"
        echo ""
    fi

    DOCKER_OUTPUT=""
    if command -v docker &> /dev/null && run_with_timeout "$DOCKER_CORE_TIMEOUT_SECS" docker version &> /dev/null; then
        if run_with_timeout "$DOCKER_CORE_TIMEOUT_SECS" docker image inspect "$DOCKER_CORE_IMAGE" &> /dev/null; then
            DOCKER_OUTPUT=$(cargo run --release -p skilllite-sandbox --example native_sandbox_microbench -- \
                --level docker \
                --iterations "$NATIVE_SANDBOX_ITERATIONS" \
                --warmup "$NATIVE_SANDBOX_WARMUP" \
                --docker-image "$DOCKER_CORE_IMAGE")
            echo "$DOCKER_OUTPUT"
            echo ""
        else
            echo -e "${YELLOW}Docker core skipped: image '$DOCKER_CORE_IMAGE' is not local.${NC}"
            echo "Pull it first with: docker pull $DOCKER_CORE_IMAGE"
            echo ""
        fi
    else
        echo -e "${YELLOW}Docker core skipped: Docker is not available.${NC}"
        echo ""
    fi

    NATIVE_AVG=$(echo "$NATIVE_OUTPUT" | awk -F': ' '/avg_ms/ {print $2}')
    SANDBOX_AVG=$(echo "$SANDBOX_OUTPUT" | awk -F': ' '/avg_ms/ {print $2}')
    NATIVE_P50=$(echo "$NATIVE_OUTPUT" | awk -F': ' '/p50_ms/ {print $2}')
    SANDBOX_P50=$(echo "$SANDBOX_OUTPUT" | awk -F': ' '/p50_ms/ {print $2}')
    NATIVE_P95=$(echo "$NATIVE_OUTPUT" | awk -F': ' '/p95_ms/ {print $2}')
    SANDBOX_P95=$(echo "$SANDBOX_OUTPUT" | awk -F': ' '/p95_ms/ {print $2}')
    NATIVE_RSS=$(echo "$NATIVE_OUTPUT" | awk -F': ' '/child_peak_rss_mb/ {print $2}')
    SANDBOX_RSS=$(echo "$SANDBOX_OUTPUT" | awk -F': ' '/child_peak_rss_mb/ {print $2}')
    DOCKER_AVG=$(echo "$DOCKER_OUTPUT" | awk -F': ' '/avg_ms/ {print $2}')
    DOCKER_P50=$(echo "$DOCKER_OUTPUT" | awk -F': ' '/p50_ms/ {print $2}')
    DOCKER_P95=$(echo "$DOCKER_OUTPUT" | awk -F': ' '/p95_ms/ {print $2}')
    DOCKER_RSS=$(echo "$DOCKER_OUTPUT" | awk -F': ' '/container_peak_rss_mb/ {print $2}')
    SRT_AVG=$(echo "$SRT_OUTPUT" | awk -F': ' '/avg_ms/ {print $2}')
    SRT_P50=$(echo "$SRT_OUTPUT" | awk -F': ' '/p50_ms/ {print $2}')
    SRT_P95=$(echo "$SRT_OUTPUT" | awk -F': ' '/p95_ms/ {print $2}')
    SRT_RSS=$(echo "$SRT_OUTPUT" | awk -F': ' '/child_peak_rss_mb/ {print $2}')

    if [ -n "$NATIVE_AVG" ] && [ -n "$SANDBOX_AVG" ] && \
       [ -n "$NATIVE_P50" ] && [ -n "$SANDBOX_P50" ] && \
       [ -n "$NATIVE_P95" ] && [ -n "$SANDBOX_P95" ] && \
       [ -n "$NATIVE_RSS" ] && [ -n "$SANDBOX_RSS" ]; then
        AVG_DELTA=$(awk "BEGIN { printf \"%.3f\", $SANDBOX_AVG - $NATIVE_AVG }")
        P50_DELTA=$(awk "BEGIN { printf \"%.3f\", $SANDBOX_P50 - $NATIVE_P50 }")
        P95_DELTA=$(awk "BEGIN { printf \"%.3f\", $SANDBOX_P95 - $NATIVE_P95 }")
        RSS_DELTA=$(awk "BEGIN { printf \"%.3f\", $SANDBOX_RSS - $NATIVE_RSS }")

        echo -e "${BLUE}Native Sandbox Core Summary${NC}"
        printf "%-24s | %12s | %12s | %12s\n" "Metric" "Native" "Sandbox" "Delta"
        printf "%-24s-+-%12s-+-%12s-+-%12s\n" "------------------------" "------------" "------------" "------------"
        printf "%-24s | %12s | %12s | %12s\n" "avg latency (ms)" "$NATIVE_AVG" "$SANDBOX_AVG" "$AVG_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "p50 latency (ms)" "$NATIVE_P50" "$SANDBOX_P50" "$P50_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "p95 latency (ms)" "$NATIVE_P95" "$SANDBOX_P95" "$P95_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "child peak RSS (MB)" "$NATIVE_RSS" "$SANDBOX_RSS" "$RSS_DELTA"
    fi
    if [ -n "$SRT_AVG" ] && [ -n "$SRT_P50" ] && \
       [ -n "$SRT_P95" ] && [ -n "$SRT_RSS" ]; then
        SRT_AVG_DELTA=$(awk "BEGIN { printf \"%.3f\", $SRT_AVG - $NATIVE_AVG }")
        SRT_P50_DELTA=$(awk "BEGIN { printf \"%.3f\", $SRT_P50 - $NATIVE_P50 }")
        SRT_P95_DELTA=$(awk "BEGIN { printf \"%.3f\", $SRT_P95 - $NATIVE_P95 }")
        SRT_RSS_DELTA=$(awk "BEGIN { printf \"%.3f\", $SRT_RSS - $NATIVE_RSS }")

        echo ""
        echo -e "${BLUE}SRT No-Python Core Summary${NC}"
        printf "%-24s | %12s | %12s | %12s\n" "Metric" "Native" "SRT" "Delta"
        printf "%-24s-+-%12s-+-%12s-+-%12s\n" "------------------------" "------------" "------------" "------------"
        printf "%-24s | %12s | %12s | %12s\n" "avg latency (ms)" "$NATIVE_AVG" "$SRT_AVG" "$SRT_AVG_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "p50 latency (ms)" "$NATIVE_P50" "$SRT_P50" "$SRT_P50_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "p95 latency (ms)" "$NATIVE_P95" "$SRT_P95" "$SRT_P95_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "child peak RSS (MB)" "$NATIVE_RSS" "$SRT_RSS" "$SRT_RSS_DELTA"
    fi
    if [ -n "$DOCKER_AVG" ] && [ -n "$DOCKER_P50" ] && \
       [ -n "$DOCKER_P95" ] && [ -n "$DOCKER_RSS" ]; then
        DOCKER_AVG_DELTA=$(awk "BEGIN { printf \"%.3f\", $DOCKER_AVG - $NATIVE_AVG }")
        DOCKER_P50_DELTA=$(awk "BEGIN { printf \"%.3f\", $DOCKER_P50 - $NATIVE_P50 }")
        DOCKER_P95_DELTA=$(awk "BEGIN { printf \"%.3f\", $DOCKER_P95 - $NATIVE_P95 }")

        echo ""
        echo -e "${BLUE}Docker No-Python Core Summary${NC}"
        printf "%-24s | %12s | %12s | %12s\n" "Metric" "Native" "Docker" "Delta"
        printf "%-24s-+-%12s-+-%12s-+-%12s\n" "------------------------" "------------" "------------" "------------"
        printf "%-24s | %12s | %12s | %12s\n" "avg latency (ms)" "$NATIVE_AVG" "$DOCKER_AVG" "$DOCKER_AVG_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "p50 latency (ms)" "$NATIVE_P50" "$DOCKER_P50" "$DOCKER_P50_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "p95 latency (ms)" "$NATIVE_P95" "$DOCKER_P95" "$DOCKER_P95_DELTA"
        printf "%-24s | %12s | %12s | %12s\n" "container peak RSS (MB)" "N/A" "$DOCKER_RSS" "N/A"
    fi
    echo ""

    if [ "$CORE_ONLY" = true ]; then
        echo -e "${GREEN}Core benchmark completed!${NC}"
        exit 0
    fi
fi

# Check Python
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Python3 is required but not found${NC}"
    exit 1
fi

# Build command arguments
CMD_ARGS="-n $REQUESTS -c $CONCURRENCY"

if [ "$COLD_START" = true ]; then
    CMD_ARGS="$CMD_ARGS --cold-start"
fi

if [ "$SKIP_DOCKER" = true ]; then
    CMD_ARGS="$CMD_ARGS --skip-docker"
fi

if [ -n "$OUTPUT_FILE" ]; then
    CMD_ARGS="$CMD_ARGS -o $OUTPUT_FILE"
fi

if [ -n "$SANDBOX_LEVEL" ]; then
    CMD_ARGS="$CMD_ARGS -l $SANDBOX_LEVEL"
fi

if [ "$COMPARE_LEVELS" = true ]; then
    CMD_ARGS="$CMD_ARGS --compare-levels"
fi

if [ "$COMPARE_IPC" = true ]; then
    CMD_ARGS="$CMD_ARGS --compare-ipc"
fi

# Suppress skilllite [INFO] logs when running benchmark (IPC daemon mode)
export SKILLLITE_QUIET=1

# Run benchmark
echo -e "${BLUE}Running benchmark with: $CMD_ARGS${NC}"
echo ""

cd "$SCRIPT_DIR"
python3 benchmark_runner.py $CMD_ARGS

echo ""
echo -e "${GREEN}Benchmark completed!${NC}"
