#!/bin/bash

# ════════════════════════════════════════════════════════════════════════════
# cowallet 认证与 MPC 会话流程测试脚本
# ════════════════════════════════════════════════════════════════════════════

API_URL="http://43.163.101.37:3000/api/v1"
DEVICE_ID="test-device-$(date +%s)"

echo "═══════════════════════════════════════════════════════════════════"
echo "🔐 Step 1: 注册设备 & 获取 Token"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "curl 命令："
echo "curl --location --request POST '$API_URL/auth/register' \\"
echo "  --header 'content-type: application/json' \\"
echo "  --data-raw '{\"device_id\":\"$DEVICE_ID\"}'"
echo ""

REGISTER_RESPONSE=$(curl -s --location --request POST "$API_URL/auth/register" \
  --header 'Content-Type: application/json' \
  --data-raw "{\"device_id\":\"$DEVICE_ID\"}")

echo "Response:"
echo "$REGISTER_RESPONSE" | jq '.'

# 提取 token
TOKEN=$(echo "$REGISTER_RESPONSE" | jq -r '.token')
USER_ID=$(echo "$REGISTER_RESPONSE" | jq -r '.user_id')

if [ "$TOKEN" == "null" ] || [ -z "$TOKEN" ]; then
  echo "❌ 获取 token 失败！"
  exit 1
fi

echo ""
echo "✅ Token 获取成功"
echo "   Device ID: $DEVICE_ID"
echo "   User ID: $USER_ID"
echo "   Token: ${TOKEN:0:50}..."
echo ""

echo "═══════════════════════════════════════════════════════════════════"
echo "📊 Step 2: 验证 Token (检查会话信息)"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "curl 命令："
echo "curl --location --request GET '$API_URL/auth/session' \\"
echo "  --header 'Authorization: Bearer $TOKEN' \\"
echo "  --header 'Accept: application/json'"
echo ""

SESSION=$(curl -s -w "\nHTTP:%{http_code}" --location --request GET "$API_URL/auth/session" \
  --header "Authorization: Bearer $TOKEN" \
  --header 'Accept: application/json')

HTTP_CODE=$(echo "$SESSION" | grep "^HTTP:" | cut -d: -f2)
BODY=$(echo "$SESSION" | sed '$d')

echo "HTTP Status: $HTTP_CODE"
echo "Response:"
echo "$BODY" | jq '.'

if [ "$HTTP_CODE" != "200" ]; then
  echo "⚠️  会话验证返回非 200 状态，但继续进行下一步..."
fi

echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "🔑 Step 3: 创建 MPC Keygen 会话（需要 Bearer Token）"
echo "═══════════════════════════════════════════════════════════════════"
echo ""
echo "curl 命令："
echo "curl --location --request POST '$API_URL/mpc/session' \\"
echo "  --header 'Authorization: Bearer $TOKEN' \\"
echo "  --header 'Content-Type: application/json' \\"
echo "  --data-raw '{\"session_type\":\"keygen\",\"parties\":[0,1,2],\"threshold\":2}'"
echo ""

MPC=$(curl -s -w "\nHTTP:%{http_code}" --location --request POST "$API_URL/mpc/session" \
  --header "Authorization: Bearer $TOKEN" \
  --header 'Content-Type: application/json' \
  --data-raw '{"session_type":"keygen","parties":[0,1,2],"threshold":2}')

HTTP_CODE=$(echo "$MPC" | grep "^HTTP:" | cut -d: -f2)
BODY=$(echo "$MPC" | sed '$d')

echo "HTTP Status: $HTTP_CODE"
echo "Response:"
echo "$BODY" | jq '.'

SESSION_ID=$(echo "$BODY" | jq -r '.session_id')

echo ""
if [ "$HTTP_CODE" == "200" ] || [ "$HTTP_CODE" == "201" ]; then
  echo "✅ MPC 会话创建成功"
  echo "   Session ID: $SESSION_ID"
else
  echo "❌ MPC 会话创建失败 (HTTP $HTTP_CODE)"
  echo "   检查："
  echo "   1. Token 是否正确："
  echo "      Token = $TOKEN"
  echo "   2. Authorization header 格式："
  echo "      Authorization: Bearer <token>"
  exit 1
fi

echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "✨ 全部测试完成！"
echo "═══════════════════════════════════════════════════════════════════"
