#!/bin/bash
# SkillLite P2P Phase 3 快速测试脚本
# 用法：先启动 skilllite swarm --listen 0.0.0.0:7700，再运行本脚本

SWARM_URL="${SKILLLITE_SWARM_URL:-http://127.0.0.1:7700}"

echo "=========================================="
echo "SkillLite Swarm Phase 3 测试"
echo "SWARM_URL=$SWARM_URL"
echo "=========================================="

# 检查 swarm 是否可达
if ! curl -s -o /dev/null -w "%{http_code}" "$SWARM_URL/task" -X POST -H "Content-Type: application/json" -d '{}' 2>/dev/null | grep -qE '^(200|400|422|503)'; then
  echo "错误: 无法连接 $SWARM_URL，请先启动: skilllite swarm --listen 0.0.0.0:7700"
  exit 1
fi

echo ""
echo "--- 场景 1: NoMatch (required_capabilities 无匹配) ---"
RESP=$(curl -s -w "\n%{http_code}" -X POST "$SWARM_URL/task" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-no-match",
    "description": "量子计算",
    "context": {"workspace": ".", "session_key": "test", "required_capabilities": ["quantum-xyz"]}
  }')
HTTP=$(echo "$RESP" | tail -n1)
BODY=$(echo "$RESP" | sed '$d')
echo "HTTP: $HTTP"
echo "$BODY" | head -c 500
echo ""
if echo "$BODY" | grep -q '"error":"no_match"'; then
  echo "✓ NoMatch 行为正确"
else
  echo "? 预期 error=no_match，请检查"
fi

echo ""
echo "--- 场景 2: 空能力本地执行 (需 OPENAI_API_KEY) ---"
RESP=$(curl -s -w "\n%{http_code}" -X POST "$SWARM_URL/task" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-local",
    "description": "只回答一个数字：1+1等于几？",
    "context": {"workspace": ".", "session_key": "test", "required_capabilities": []}
  }')
HTTP=$(echo "$RESP" | tail -n1)
BODY=$(echo "$RESP" | sed '$d')
echo "HTTP: $HTTP"
echo "$BODY" | head -c 600
echo ""
if [ "$HTTP" = "200" ] && echo "$BODY" | grep -q '"response"'; then
  echo "✓ 本地执行成功"
elif [ "$HTTP" = "503" ] && echo "$BODY" | grep -q 'local_executor'; then
  echo "! 无 agent/executor，跳过本地执行测试"
elif [ "$HTTP" = "500" ]; then
  echo "! 执行失败（可能缺 OPENAI_API_KEY）"
else
  echo "? 请检查响应"
fi

echo ""
echo "=========================================="
echo "测试完成"
echo "=========================================="
