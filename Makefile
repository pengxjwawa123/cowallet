.PHONY: help docker-up docker-down docker-logs docker-clean docker-build

help:
	@echo "cowallet Docker 命令速览"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo "启动:       make docker-up       # 启动所有服务"
	@echo "停止:       make docker-down     # 停止所有服务"
	@echo "日志:       make docker-logs     # 查看实时日志"
	@echo "清理:       make docker-clean    # 停止并删除所有数据"
	@echo "重建:       make docker-rebuild  # 重新构建镜像"
	@echo "检查:       make docker-ps       # 显示服务状态"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

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

# 本地开发
dev-up:
	@echo "🚀 启动本地开发环境（不含 Docker）..."
	@echo "请手动启动："
	@echo "  brew services start postgresql@16"
	@echo "  nats-server -js"
	@echo "  redis-server"
	@echo "然后运行：make dev-run"

dev-run:
	@echo "▶️  启动开发服务..."
	cargo run -p api-server &
	cargo run -p mpc-relay &
	cargo run -p indexer &
	cargo run -p worker &
	wait
