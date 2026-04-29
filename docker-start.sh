#!/bin/bash
# cowallet Docker 快速启动脚本

set -e

echo "🚀 cowallet Docker 部署启动"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 检查 Docker
if ! command -v docker &> /dev/null; then
    echo "❌ Docker 未安装，请先安装 Docker: https://www.docker.com/products/docker-desktop"
    exit 1
fi

# 检查 docker-compose
if ! command -v docker-compose &> /dev/null; then
    echo "⚠️  docker-compose 未找到，尝试使用 docker compose..."
    COMPOSE_CMD="docker compose"
else
    COMPOSE_CMD="docker-compose"
fi

# 检查 .env 文件
if [ ! -f .env ]; then
    echo "📋 创建 .env 文件..."
    cp .env.example .env
    echo "⚠️  请编辑 .env 文件，填入 CLAUDE_API_KEY 等必要配置"
    echo "   nano .env 或使用你的编辑器"
    exit 1
fi

# 获取命令
COMMAND=${1:-up}

case $COMMAND in
    up)
        echo "📦 构建镜像（首次运行可能需要 5-10 分钟）..."
        $COMPOSE_CMD build --no-cache
        
        echo "🔥 启动所有服务..."
        $COMPOSE_CMD up -d
        
        echo ""
        echo "✅ 服务启动完成！"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "API 服务器:     http://localhost:3000"
        echo "API 健康检查:   curl http://localhost:3000/health"
        echo "数据库管理:     http://localhost:8081 (Adminer)"
        echo "NATS 管理:      http://localhost:8222"
        echo ""
        echo "查看日志:       $COMPOSE_CMD logs -f"
        echo "停止服务:       $COMPOSE_CMD down"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        ;;
        
    down)
        echo "⛔ 停止所有服务..."
        $COMPOSE_CMD down
        echo "✅ 服务已停止"
        ;;
        
    logs)
        echo "📋 显示服务日志..."
        $COMPOSE_CMD logs -f
        ;;
        
    clean)
        echo "🧹 清理所有数据和容器..."
        $COMPOSE_CMD down -v
        echo "✅ 清理完成"
        ;;
        
    rebuild)
        echo "🔨 重新构建服务..."
        $COMPOSE_CMD down
        $COMPOSE_CMD build --no-cache
        $COMPOSE_CMD up -d
        echo "✅ 重建完成"
        ;;
        
    ps)
        echo "📊 服务状态..."
        $COMPOSE_CMD ps
        ;;
        
    *)
        echo "用法: $0 {up|down|logs|clean|rebuild|ps}"
        echo ""
        echo "命令:"
        echo "  up       - 启动所有服务 (默认)"
        echo "  down     - 停止所有服务"
        echo "  logs     - 显示实时日志"
        echo "  clean    - 停止并删除所有数据"
        echo "  rebuild  - 重新构建镜像"
        echo "  ps       - 显示服务状态"
        exit 1
        ;;
esac
