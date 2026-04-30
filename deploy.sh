#!/bin/bash

# ════════════════════════════════════════════════════════════════════════════
# cowallet 服务器部署和重启脚本
# 用于在远程服务器上应用 JWT_SECRET 配置
# ════════════════════════════════════════════════════════════════════════════

set -e  # 错误时停止执行

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}🚀 cowallet 服务器部署脚本${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"

# ────────────────────────────────────────────────────────────────────────────
# Step 1: 检查必要的命令
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}📋 检查依赖...${NC}"

if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker 未安装${NC}"
    exit 1
fi

if ! command -v git &> /dev/null; then
    echo -e "${RED}❌ Git 未安装${NC}"
    exit 1
fi

echo -e "${GREEN}✅ 依赖检查完成${NC}"

# ────────────────────────────────────────────────────────────────────────────
# Step 2: 生成强 JWT_SECRET（如果没有）
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}🔐 设置 JWT_SECRET...${NC}"

if [ -f ".env" ]; then
    echo -e "${YELLOW}   .env 文件已存在，使用现有配置${NC}"
    # 检查是否有 JWT_SECRET
    if ! grep -q "JWT_SECRET=" .env; then
        echo -e "${YELLOW}   .env 中没有 JWT_SECRET，添加一个...${NC}"
        JWT_SECRET=$(openssl rand -base64 32)
        echo "JWT_SECRET=$JWT_SECRET" >> .env
        echo -e "${GREEN}✅ JWT_SECRET 已添加到 .env${NC}"
    fi
else
    echo -e "${YELLOW}   创建 .env 文件...${NC}"
    JWT_SECRET=$(openssl rand -base64 32)
    cat > .env << EOF
# JWT 认证密钥
JWT_SECRET=$JWT_SECRET

# Claude API 密钥（可选）
CLAUDE_API_KEY=sk-ant-placeholder

# PostgreSQL 初始密码（⚠️ 生产环境改为强密码）
POSTGRES_PASSWORD=postgres
POSTGRES_USER=postgres
POSTGRES_DB=cowallet
EOF
    echo -e "${GREEN}✅ .env 文件已创建${NC}"
fi

source .env
echo -e "${GREEN}✅ JWT_SECRET 设置完成: ${JWT_SECRET:0:20}...${NC}"

# ────────────────────────────────────────────────────────────────────────────
# Step 3: 停止旧容器
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}🛑 停止旧容器...${NC}"

if docker ps | grep -q cowallet-api-server; then
    docker compose down
    echo -e "${GREEN}✅ 旧容器已停止${NC}"
else
    echo -e "${YELLOW}   没有运行的容器${NC}"
fi

# ────────────────────────────────────────────────────────────────────────────
# Step 4: 拉取最新代码
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}📥 更新代码...${NC}"

git fetch origin
git reset --hard origin/main
echo -e "${GREEN}✅ 代码已更新${NC}"

# ────────────────────────────────────────────────────────────────────────────
# Step 5: 重新构建镜像
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}🔨 构建新镜像...${NC}"

docker compose build --no-cache api-server mpc-relay indexer worker
echo -e "${GREEN}✅ 镜像构建完成${NC}"

# ────────────────────────────────────────────────────────────────────────────
# Step 6: 启动服务
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}🚀 启动服务...${NC}"

docker compose up -d
sleep 5

echo -e "${GREEN}✅ 服务已启动${NC}"

# ────────────────────────────────────────────────────────────────────────────
# Step 7: 验证服务健康
# ────────────────────────────────────────────────────────────────────────────

echo -e "${YELLOW}🏥 验证服务健康...${NC}"

# 等待服务启动
for i in {1..30}; do
    if docker compose ps api-server | grep -q "healthy"; then
        echo -e "${GREEN}✅ api-server 服务健康${NC}"
        break
    fi
    echo -e "${YELLOW}   等待服务启动... ($i/30)${NC}"
    sleep 2
done

# 测试健康检查端点
echo ""
echo -e "${YELLOW}📡 测试 API 端点...${NC}"

HEALTH_CHECK=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/health)

if [ "$HEALTH_CHECK" == "200" ]; then
    echo -e "${GREEN}✅ 健康检查通过 (HTTP $HEALTH_CHECK)${NC}"
else
    echo -e "${RED}❌ 健康检查失败 (HTTP $HEALTH_CHECK)${NC}"
    echo -e "${YELLOW}   检查日志: docker compose logs api-server${NC}"
fi

# ────────────────────────────────────────────────────────────────────────────
# Step 8: 测试认证流程
# ────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${YELLOW}🔐 测试认证流程...${NC}"

DEVICE_ID="test-device-$(date +%s)"

# 注册
REGISTER=$(curl -s -X POST http://localhost:3000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d "{\"device_id\":\"$DEVICE_ID\"}")

TOKEN=$(echo "$REGISTER" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -z "$TOKEN" ]; then
    echo -e "${RED}❌ 注册失败，无法获取 token${NC}"
    echo "Response: $REGISTER"
    exit 1
fi

echo -e "${GREEN}✅ 注册成功，获取 token: ${TOKEN:0:30}...${NC}"

# 验证 token
SESSION=$(curl -s -w "\nHTTP:%{http_code}" -X GET http://localhost:3000/api/v1/auth/session \
  -H "Authorization: Bearer $TOKEN" \
  -H "Accept: application/json")

HTTP_CODE=$(echo "$SESSION" | grep "^HTTP:" | cut -d: -f2)

if [ "$HTTP_CODE" == "200" ]; then
    echo -e "${GREEN}✅ Token 验证成功 (HTTP $HTTP_CODE)${NC}"
else
    echo -e "${RED}❌ Token 验证失败 (HTTP $HTTP_CODE)${NC}"
    echo "Response: $SESSION"
    exit 1
fi

# ────────────────────────────────────────────────────────────────────────────
# 完成
# ────────────────────────────────────────────────────────────────────────────

echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✨ 部署完成！${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "📝 服务信息："
echo -e "   API 服务器: http://localhost:3000"
echo -e "   JWT_SECRET: 已配置 ✅"
echo -e "   数据库: PostgreSQL (port 5433)"
echo -e "   Redis: (port 6380)"
echo -e "   NATS: (port 4223)"
echo ""
echo -e "📚 常用命令："
echo -e "   查看日志:     docker compose logs -f api-server"
echo -e "   停止服务:     docker compose down"
echo -e "   重启服务:     docker compose restart"
echo ""
