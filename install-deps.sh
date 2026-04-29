#!/bin/bash

# cowallet 本地部署依赖安装脚本（macOS）

set -e

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}╔═══════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║     cowallet 本地部署 - 依赖安装${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════╝${NC}"

# 检查 Homebrew
if ! command -v brew &> /dev/null; then
    echo -e "${RED}❌ 未找到 Homebrew${NC}"
    echo "请先安装 Homebrew: https://brew.sh"
    exit 1
fi

echo -e "${YELLOW}📦 安装依赖...${NC}"
echo ""

# PostgreSQL 16
if ! command -v postgres &> /dev/null; then
    echo -e "${YELLOW}⏳ 安装 PostgreSQL 16...${NC}"
    brew install postgresql@16
    brew services start postgresql@16
    echo -e "${GREEN}✅ PostgreSQL 已安装${NC}"
else
    echo -e "${GREEN}✅ PostgreSQL 已安装${NC}"
    brew services start postgresql@16 || true
fi

# Redis
if ! command -v redis-server &> /dev/null; then
    echo -e "${YELLOW}⏳ 安装 Redis...${NC}"
    brew install redis
    brew services start redis
    echo -e "${GREEN}✅ Redis 已安装${NC}"
else
    echo -e "${GREEN}✅ Redis 已安装${NC}"
    brew services start redis || true
fi

# NATS
if ! command -v nats-server &> /dev/null; then
    echo -e "${YELLOW}⏳ 安装 NATS...${NC}"
    brew install nats-server
    echo -e "${GREEN}✅ NATS 已安装${NC}"
else
    echo -e "${GREEN}✅ NATS 已安装${NC}"
fi

# sqlx-cli
if ! command -v sqlx &> /dev/null; then
    echo -e "${YELLOW}⏳ 安装 sqlx-cli...${NC}"
    cargo install sqlx-cli --no-default-features --features postgres
    echo -e "${GREEN}✅ sqlx-cli 已安装${NC}"
else
    echo -e "${GREEN}✅ sqlx-cli 已安装${NC}"
fi

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║          🎉 所有依赖已安装完成！${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════╝${NC}"
echo ""

# 验证
echo -e "${YELLOW}📋 验证已安装的工具...${NC}"
echo ""
echo -e "${GREEN}✓ PostgreSQL:${NC} $(postgres --version)"
echo -e "${GREEN}✓ Redis:${NC} $(redis-server --version)"
echo -e "${GREEN}✓ NATS:${NC} $(nats-server --version)"
echo -e "${GREEN}✓ Rust:${NC} $(rustc --version)"
echo -e "${GREEN}✓ sqlx-cli:${NC} $(sqlx --version)"

echo ""
echo -e "${GREEN}════════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}📝 下一步操作：${NC}"
echo ""
echo "1️⃣  初始化数据库和服务:"
echo "   ${GREEN}make local-init${NC}"
echo ""
echo "2️⃣  启动 NATS 服务器（新终端）:"
echo "   ${GREEN}nats-server -js${NC}"
echo ""
echo "3️⃣  启动 API 服务器:"
echo "   ${GREEN}make local-start${NC}"
echo ""
echo "💡 详细说明见: QUICK_START.md 或 DEPLOY.md"
echo ""
