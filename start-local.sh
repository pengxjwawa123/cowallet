#!/bin/bash

# cowallet 本地部署启动脚本

set -e  # 遇到错误立即退出

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 环境变量配置
export DATABASE_URL=postgres://postgres@localhost:5432/cowallet
export REDIS_URL=redis://localhost:6379
export NATS_URL=nats://localhost:4222
export RPC_URL=https://sepolia.base.org
export RPC_WS_URL=wss://sepolia.base.org
export RUST_LOG=info,api_server=debug

echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
echo -e "${GREEN}    cowallet 本地部署启动脚本${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════${NC}"

# 检查前置条件
check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}❌ 错误: 未找到 $1${NC}"
        echo "请运行: brew install $2"
        exit 1
    fi
}

echo -e "\n${YELLOW}📋 检查前置条件...${NC}"
check_command "postgres" "postgresql@16"
check_command "redis-server" "redis"
check_command "nats-server" "nats-server"
check_command "cargo" "rust"
check_command "sqlx" "sqlx-cli"

echo -e "${GREEN}✅ 所有前置条件已满足${NC}"

# 启动基础服务
echo -e "\n${YELLOW}🚀 启动基础服务...${NC}"

# 检查服务是否已运行
check_service() {
    if brew services list | grep -q "$1.*started"; then
        echo -e "${GREEN}✅ $1 已在运行${NC}"
    else
        echo -e "${YELLOW}⏳ 启动 $1...${NC}"
        brew services start "$1"
        sleep 2
        echo -e "${GREEN}✅ $1 启动完成${NC}"
    fi
}

check_service "postgresql@16"
check_service "redis"

# 启动 NATS（需要终端运行）
if ! pgrep -f "nats-server" > /dev/null; then
    echo -e "${YELLOW}⏳ 请在另一个终端运行: nats-server -js${NC}"
    echo -e "${YELLOW}⏳ 等待 5 秒...${NC}"
    sleep 5
else
    echo -e "${GREEN}✅ NATS 已在运行${NC}"
fi

# 验证数据库
echo -e "\n${YELLOW}📦 初始化数据库...${NC}"

if ! psql "$DATABASE_URL" -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${YELLOW}⏳ 创建数据库...${NC}"
    createdb -U postgres cowallet || echo -e "${YELLOW}⚠️  数据库已存在${NC}"
fi

# 运行迁移
echo -e "${YELLOW}⏳ 运行数据库迁移...${NC}"
sqlx migrate run --source backend/migrations

echo -e "${GREEN}✅ 数据库已初始化${NC}"

# 编译项目
echo -e "\n${YELLOW}🔨 编译 Rust 项目...${NC}"
cargo build --release

echo -e "${GREEN}✅ 编译完成${NC}"

# 启动 API 服务
echo -e "\n${YELLOW}🎯 启动 API 服务器...${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
echo -e "API 地址: http://localhost:3000"
echo -e "数据库: $DATABASE_URL"
echo -e "Redis: $REDIS_URL"
echo -e "NATS: $NATS_URL"
echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
echo ""

cargo run --release --bin api-server
