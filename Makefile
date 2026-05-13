.PHONY: help docker-up docker-down docker-logs docker-clean docker-build \
	local-init local-start local-migrate local-build local-stop local-status

help:
	@echo "cowallet 部署命令速览"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo "【本地直接部署（推荐）】"
	@echo "  初始化:    make local-init       # 一次性初始化环境"
	@echo "  启动:      make local-start      # 启动所有服务"
	@echo "  迁移:      make local-migrate    # 运行数据库迁移"
	@echo "  构建:      make local-build      # 编译 Rust 项目"
	@echo "  停止:      make local-stop       # 停止所有服务"
	@echo "  状态:      make local-status     # 显示服务状态"
	@echo ""
	@echo "【Docker 部署】"
	@echo "  启动:      make docker-up       # 启动所有服务"
	@echo "  停止:      make docker-down     # 停止所有服务"
	@echo "  日志:      make docker-logs     # 查看实时日志"
	@echo "  清理:      make docker-clean    # 停止并删除所有数据"
	@echo "  重建:      make docker-rebuild  # 重新构建镜像"
	@echo "  检查:      make docker-ps       # 显示服务状态"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo ""
	@echo "📖 详见: DEPLOY.md（本地部署详细指南）"

docker-up:
	@echo "🚀 启动 cowallet 服务..."
	docker-compose up -d
	@echo ""
	@echo "✅ 服务已启动！"
	@echo "API:     http://localhost:3000"
	@echo "健康检查: curl http://localhost:3000/health"
	@echo "Adminer: http://localhost:8081"
	@echo "NATS:    http://localhost:8222"

docker-down:
	@echo "⛔ 停止所有服务..."
	docker-compose down
	@echo "✅ 服务已停止"

docker-logs:
	docker-compose logs -f

docker-clean:
	@echo "🧹 清理所有数据和容器..."
	docker-compose down -v
	@echo "✅ 清理完成"

docker-build:
	@echo "🔨 构建镜像..."
	docker-compose build --no-cache

docker-rebuild: docker-down docker-build docker-up

docker-ps:
	docker-compose ps

docker-shell-api:
	docker-compose exec api-server sh

docker-shell-db:
	docker-compose exec postgres psql -U postgres

docker-shell-redis:
	docker-compose exec redis redis-cli

# Cargo 命令
build:
	cargo build --release

test:
	cargo test

format:
	cargo fmt

lint:
	cargo clippy -- -D warnings

# 完整流程
docker-fresh: docker-clean docker-build docker-up
	@echo "✨ 全新部署完成！"

# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
# 本地直接部署命令
# ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# 设置环境变量
export DATABASE_URL = postgres://postgres@localhost:5432/cowallet
export REDIS_URL = redis://localhost:6379
export NATS_URL = nats://localhost:4222
export RPC_URL = https://sepolia.base.org
export RPC_WS_URL = wss://sepolia.base.org
export RUST_LOG = info,api_server=debug

local-init:
	@echo "🔧 初始化 CentOS 部署环境..."
	@echo "📋 检查前置条件..."
	@command -v /usr/pgsql-16/bin/psql >/dev/null 2>&1 || (echo "❌ 需要安装 PostgreSQL: sudo yum install -y postgresql16-server" && exit 1)
	@command -v redis-server >/dev/null 2>&1 || (echo "❌ 需要安装 Redis: sudo yum install -y redis" && exit 1)
	@command -v nats-server >/dev/null 2>&1 || (echo "❌ 需要安装 NATS: 见 DEPLOY.md" && exit 1)
	@command -v cargo >/dev/null 2>&1 || (echo "❌ 需要安装 Rust: https://rustup.rs" && exit 1)
	@command -v sqlx >/dev/null 2>&1 || (echo "⏳ 安装 sqlx-cli..." && cargo install sqlx-cli --no-default-features --features postgres)
	@echo "✅ 所有前置条件已满足"
	@echo ""
	@echo "🚀 启动基础服务..."
	@sudo systemctl enable postgresql-16 > /dev/null 2>&1 || true
	@sudo systemctl start postgresql-16 > /dev/null 2>&1 || true
	@sudo systemctl enable redis > /dev/null 2>&1 || true
	@sudo systemctl start redis > /dev/null 2>&1 || true
	@sleep 2
	@echo "✅ PostgreSQL 已启动"
	@echo "✅ Redis 已启动"
	@echo ""
	@echo "🔍 检查 GCC 版本..."
	@GCC_VERSION=$$(gcc --version | head -1 | grep -oE '[0-9]+' | head -1); \
	if [ "$$GCC_VERSION" -lt 11 ]; then \
		echo "⚠️  GCC 版本过旧 ($$GCC_VERSION < 11)"; \
		echo "💡 CentOS 7: sudo yum install -y centos-release-scl devtoolset-11-gcc && source scl_enable devtoolset-11"; \
		echo "💡 CentOS 8/9: sudo yum install -y gcc-11 && sudo update-alternatives --install /usr/bin/gcc gcc /usr/bin/gcc-11 100"; \
		exit 1; \
	else \
		echo "✅ GCC 版本: $$GCC_VERSION (✓)"; \
	fi
	@echo ""
	@echo "🔐 配置 PostgreSQL 认证..."
	@sudo sed -i 's/^local.*all.*all.*peer/local   all             all                                     trust/' /var/lib/pgsql/16/data/pg_hba.conf
	@sudo systemctl restart postgresql-16 > /dev/null 2>&1
	@echo "✅ PostgreSQL 认证已配置"
	@echo ""
	@echo "📦 初始化数据库..."
	@sudo -u postgres /usr/pgsql-16/bin/createdb cowallet 2>/dev/null || echo "⚠️  数据库已存在"
	@echo ""
	@echo "📝 运行数据库迁移..."
	@sqlx migrate run --source backend/migrations
	@echo "✅ 数据库迁移完成"
	@echo ""
	@echo "📌 下一步:"
	@echo "   终端 1: sudo systemctl start nats"
	@echo "   终端 2: make local-start"

local-start:
	@echo "🎯 启动 cowallet 服务..."
	@echo "🔌 连接信息："
	@echo "   API:      http://localhost:3000"
	@echo "   Database: $(DATABASE_URL)"
	@echo "   Redis:    $(REDIS_URL)"
	@echo "   NATS:     $(NATS_URL)"
	@echo ""
	@cargo run --release --bin api-server

local-migrate:
	@echo "📝 运行数据库迁移..."
	@sqlx migrate run --source backend/migrations
	@echo "✅ 迁移完成"

local-build:
	@echo "🔨 编译 Rust 项目..."
	@cargo build --release
	@echo "✅ 编译完成"

local-stop:
	@echo "⛔ 停止所有服务..."
	@pkill -f "api-server" || true
	@pkill -f "mpc-relay" || true
	@pkill -f "worker" || true
	@echo "✅ 应用已停止"
	@echo ""
	@echo "💡 系统服务可使用以下命令停止:"
	@echo "   sudo systemctl stop postgresql-16"
	@echo "   sudo systemctl stop redis"
	@echo "   sudo systemctl stop nats"

local-status:
	@echo "📊 服务状态检查..."
	@echo ""
	@echo "🔍 PostgreSQL:"
	@sudo systemctl status postgresql-16 2>/dev/null | grep Active || echo "❌ 检查失败"
	@echo ""
	@echo "🔍 Redis:"
	@redis-cli ping 2>/dev/null || echo "❌ 未连接"
	@echo ""
	@echo "🔍 NATS:"
	@sudo systemctl status nats 2>/dev/null | grep Active || echo "❌ 检查失败"
	@echo ""
	@echo "🔍 API 服务:"
	@(curl -s http://localhost:3000/health | head -c 50 2>/dev/null && echo "" || echo "❌ 未连接")

dev-run:
	@echo "▶️  启动开发服务..."
	cargo run -p api-server &
	cargo run -p mpc-relay &
	cargo run -p worker &
	wait
