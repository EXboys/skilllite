#!/bin/bash
#
# SkillBox Benchmark Runner Script
# 高并发性能对比测试脚本
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SkillBox High-Concurrency Benchmark${NC}"
echo -e "${BLUE}========================================${NC}"

# 默认参数
REQUESTS=${REQUESTS:-100}
CONCURRENCY=${CONCURRENCY:-10}
COLD_START=${COLD_START:-false}
SKIP_DOCKER=${SKIP_DOCKER:-false}
OUTPUT_FILE=""
SANDBOX_LEVEL=""
COMPARE_LEVELS=${COMPARE_LEVELS:-false}

# 解析命令行参数
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
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -n, --requests NUM     Number of requests (default: 100)"
            echo "  -c, --concurrency NUM  Concurrency level (default: 10)"
            echo "  --cold-start           Run cold start benchmark"
            echo "  --skip-docker          Skip Docker tests"
            echo "  -l, --sandbox-level N  SkillBox sandbox level (1, 2, or 3)"
            echo "                         1 = No sandbox (direct execution)"
            echo "                         2 = Sandbox isolation only"
            echo "                         3 = Sandbox + static code scan (default)"
            echo "                         Can also be set via SKILLBOX_SANDBOX_LEVEL env var"
            echo "  --compare-levels       Compare performance across all sandbox levels"
            echo "  -o, --output FILE      Output JSON file"
            echo "  -h, --help             Show this help"
            echo ""
            echo "Examples:"
            echo "  $0 -n 500 -c 50                    # 500 requests, 50 concurrent"
            echo "  $0 --cold-start -n 100             # With cold start test"
            echo "  $0 -l 2                            # Test with sandbox level 2"
            echo "  $0 --compare-levels                # Compare all sandbox levels"
            echo "  SKILLBOX_SANDBOX_LEVEL=1 $0        # Use env var to set level"
            echo "  $0 -o results.json                 # Save results to file"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# 检查 SkillBox 是否已编译
SKILLBOX_BIN=""
if command -v skillbox &> /dev/null; then
    SKILLBOX_BIN=$(which skillbox)
elif [ -f "$PROJECT_ROOT/skillbox/target/release/skillbox" ]; then
    SKILLBOX_BIN="$PROJECT_ROOT/skillbox/target/release/skillbox"
else
    echo -e "${YELLOW}SkillBox binary not found. Building...${NC}"
    cd "$PROJECT_ROOT/skillbox"
    cargo build --release
    SKILLBOX_BIN="$PROJECT_ROOT/skillbox/target/release/skillbox"
fi

echo -e "${GREEN}SkillBox binary: $SKILLBOX_BIN${NC}"

# 检查 Python
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Python3 is required but not found${NC}"
    exit 1
fi

# 构建命令参数
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

# 运行 benchmark
echo -e "${BLUE}Running benchmark with: $CMD_ARGS${NC}"
echo ""

cd "$SCRIPT_DIR"
python3 benchmark_runner.py $CMD_ARGS

echo ""
echo -e "${GREEN}Benchmark completed!${NC}"
