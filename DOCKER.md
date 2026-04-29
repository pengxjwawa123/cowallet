# cowallet Docker 部署指南

## ✅ Docker 部署可行性评估

| 方面 | 状态 | 说明 |
|------|------|------|
| **后端服务** | ✅ 完全可行 | 4 个 Rust 服务（API、MPC、Indexer、Worker） |
| **数据库** | ✅ 完全可行 | PostgreSQL 容器化 |
| **消息队列** | ✅ 完全可行 | NATS JetStream 容器化 |
| **缓存** | ✅ 完全可行 | Redis 容器化 |
| **外部依赖** | ⚠️  部分依赖 | RPC 节点外部（Base Sepolia）、Claude API 外部 |
| **移动端** | ❌ 不适合 | Flutter 应用部署在设备上，不用 Docker |

## 🚀 快速开始

### 前置条件

```bash
# 确保已安装
docker --version       # 20.10+
docker-compose --version  # 2.0+
```

### 1️⃣  配置环境变量

```bash
# 复制模板
cp .env.example .env

# 编辑 .env，填入关键配置
nano .env
# 必需：CLAUDE_API_KEY
```

### 2️⃣  启动服务（一行命令）

```bash
# 方式 A: 使用启动脚本（推荐）
chmod +x docker-start.sh
./docker-start.sh up

# 方式 B: 直接使用 docker-compose
docker-compose up -d

# 方式 C: 查看实时日志
docker-compose up
```

### 3️⃣  验证服务

```bash
# 查看容器状态
docker-compose ps

# API 健康检查
curl http://localhost:3000/health
# 返回: "ok"

# 数据库连接
psql -h localhost -U postgres -d cowallet -c "SELECT version();"

# NATS 状态
docker-compose exec nats nats server info
```

## 📁 项目结构

```
cowallet/
├── Dockerfile                 # 多阶段构建（编译 + 运行）
├── docker-compose.yml         # 完整编排配置
├── .dockerignore              # 构建优化
├── .env.example               # 环境变量模板
├── docker-start.sh            # 快速启动脚本
├── migrations/
│   └── 001_initial_schema.sql # 数据库初始化
├── backend/
│   ├── api-server/
│   ├── mpc-relay/
│   ├── indexer/
│   └── worker/
└── crates/                    # Rust 库
```

## 🐳 服务详情

### api-server (API 服务器)

```yaml
容器: cowallet-api-server
端口: 3000
功能: HTTP REST API (认证、交易、策略、AI 代理)
依赖: PostgreSQL, Redis, NATS, Claude API
环境变量:
  - DATABASE_URL
  - RPC_URL
  - NATS_URL
  - REDIS_URL
  - CLAUDE_API_KEY
  - CORS_ALLOWED_ORIGINS
```

### mpc-relay (MPC 消息中继)

```yaml
容器: cowallet-mpc-relay
功能: NATS JetStream 消息路由 (MPC 协议轮次)
依赖: NATS
订阅:
  - cowallet.mpc.control (管理)
  - cowallet.mpc.> (协议消息)
```

### indexer (链上事务索引)

```yaml
容器: cowallet-indexer
功能: 轮询 Base Sepolia，存储交易历史
依赖: PostgreSQL, RPC_URL
间隔: 60 秒
表: tx_history
```

### worker (后台任务处理)

```yaml
容器: cowallet-worker
功能: 4 个并发任务
  - 价格更新 (30 秒)
  - 会话清理 (5 分钟)
  - 批准过期 (10 分钟)
  - Reshare 检查 (1 小时)
依赖: PostgreSQL, Redis
```

### PostgreSQL

```yaml
容器: cowallet-postgres
版本: 16-alpine
端口: 5432
初始化:
  - 数据库: cowallet
  - 用户: postgres
  - 密码: postgres (⚠️  改为强密码)
  - SQL: migrations/001_initial_schema.sql
存储: postgres_data (永久卷)
```

### Redis

```yaml
容器: cowallet-redis
版本: 7-alpine
端口: 6379
功能: 价格缓存、会话、限速
持久化: RDB (appendonly yes)
存储: redis_data (永久卷)
```

### NATS

```yaml
容器: cowallet-nats
版本: 2-alpine
端口: 4222 (客户端), 8222 (HTTP)
功能: JetStream 消息队列
存储: nats_data (永久卷)
```

## 📊 常用命令

```bash
# 启动
docker-compose up -d                    # 后台启动
docker-compose up                       # 前台启动（看日志）

# 查看日志
docker-compose logs                     # 所有服务
docker-compose logs -f api-server       # 跟随特定服务
docker-compose logs --tail=50 worker    # 最后 50 行

# 停止/清理
docker-compose down                     # 停止容器
docker-compose down -v                  # 停止并删除卷（数据丢失）

# 进入容器
docker-compose exec postgres psql -U postgres  # 进入 PostgreSQL
docker-compose exec redis redis-cli             # 进入 Redis
docker-compose exec api-server sh               # 进入 API 容器

# 重建
docker-compose build --no-cache          # 重新构建镜像
docker-compose up -d --force-recreate    # 强制重建容器

# 执行迁移
docker-compose exec postgres psql -U postgres cowallet < migrations/001_initial_schema.sql
```

## 🌐 访问方式

| 服务 | URL | 用途 |
|------|-----|------|
| API 服务器 | http://localhost:3000 | REST API |
| API 健康检查 | http://localhost:3000/health | 监控 |
| 数据库管理 | http://localhost:8081 | Adminer (Web GUI) |
| NATS 管理 | http://localhost:8222 | JetStream HTTP |
| PostgreSQL | localhost:5432 | 直接连接 |
| Redis | localhost:6379 | 直接连接 |
| NATS | localhost:4222 | 客户端连接 |

### 连接示例

```bash
# PostgreSQL
psql -h localhost -U postgres -d cowallet

# Redis
redis-cli -h localhost

# NATS
nats -s nats://localhost:4222 sub ">"
```

## 🔧 生产部署注意事项

### 1. 安全性

```bash
# ✅ 修改数据库密码
# docker-compose.yml 中：
  POSTGRES_PASSWORD: your-strong-password-here

# ✅ 设置 CLAUDE_API_KEY
export CLAUDE_API_KEY=sk-ant-xxxxxxxx

# ✅ 配置 CORS
CORS_ALLOWED_ORIGINS=your-domain.com

# ✅ 使用 HTTPS
# 在反向代理（nginx）中配置 SSL
```

### 2. 性能优化

```yaml
# docker-compose.yml 中增加资源限制：
services:
  api-server:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '1'
          memory: 1G
```

### 3. 监控和日志

```bash
# 使用 ELK Stack（可选）
# - Elasticsearch
# - Logstash
# - Kibana

# 或集成到 docker-compose.yml
```

### 4. 备份

```bash
# PostgreSQL 备份
docker-compose exec postgres pg_dump -U postgres cowallet > backup.sql

# Redis 备份
docker cp cowallet-redis:/data/dump.rdb ./redis-backup.rdb

# 恢复
docker-compose exec postgres psql -U postgres cowallet < backup.sql
```

## 🐛 故障排查

### 容器无法启动

```bash
# 查看错误日志
docker-compose logs api-server

# 检查网络
docker-compose ps

# 重启所有服务
docker-compose restart
```

### 数据库连接失败

```bash
# 检查 PostgreSQL 健康状态
docker-compose exec postgres pg_isready

# 查看 DATABASE_URL 是否正确
grep DATABASE_URL .env
```

### Redis 连接失败

```bash
# 检查 Redis 健康状态
docker-compose exec redis redis-cli ping
# 返回: PONG

# 查看 REDIS_URL 是否正确
grep REDIS_URL .env
```

### NATS 消息未被路由

```bash
# 检查 NATS 连接
docker-compose exec nats nats server info

# 查看消息
docker-compose exec nats nats sub "cowallet.>"
```

## 📈 扩展和定制

### 添加新服务

```yaml
# 在 docker-compose.yml 中添加：
  my-service:
    build:
      context: .
      dockerfile: Dockerfile
    command: my-binary
    environment:
      DATABASE_URL: postgresql://postgres:postgres@postgres:5432/cowallet
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - cowallet
```

### 使用私有 Docker Registry

```bash
# 构建并推送到 registry
docker build -t your-registry.com/cowallet:1.0.0 .
docker push your-registry.com/cowallet:1.0.0

# 在 docker-compose.yml 中引用
image: your-registry.com/cowallet:1.0.0
```

### Kubernetes 部署

```bash
# 导出为 Kubernetes 配置（使用 Kompose）
kompose convert -f docker-compose.yml

# 部署到 Kubernetes
kubectl apply -f ./
```

## ✨ 总结

| 优点 | 缺点 |
|------|------|
| 📦 完全隔离的环境 | 🔗 外部 API 依赖（RPC、Claude） |
| 🚀 快速部署 | ⚠️  首次构建较慢（5-10 分钟） |
| 🔄 易于扩展 | 📊 需要监控和日志系统 |
| 💾 数据持久化 | 🔐 生产环境需加强安全 |
| 🌍 跨平台一致性 | 📝 需要定期备份数据库 |

**结论：完全可行！✅** 使用 Docker 部署可以轻松在任何环境中运行 cowallet 后端服务。
