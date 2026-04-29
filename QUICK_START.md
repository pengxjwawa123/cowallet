# cowallet 本地部署快速参考

## 一键初始化（首次运行）

```bash
make local-init
```

这会：
- ✅ 检查 PostgreSQL、Redis、NATS、Rust 已安装
- ✅ 启动 PostgreSQL 和 Redis
- ✅ 创建 cowallet 数据库
- ✅ 运行数据库迁移
- ✅ 提示启动 NATS 服务器

## 启动服务

### 方式 1：使用 Makefile（推荐）

**终端 1 - 启动消息队列**
```bash
nats-server -js
```

**终端 2 - 启动 API 服务**
```bash
cd /Users/jingle/cat/cowallet
make local-start
```

### 方式 2：使用启动脚本

```bash
./start-local.sh
```

### 方式 3：手动启动各服务

**终端 1 - PostgreSQL（可选，已由 brew 启动）**
```bash
# 已通过 brew services 启动
brew services start postgresql@16
```

**终端 2 - Redis**
```bash
redis-server
```

**终端 3 - NATS**
```bash
nats-server -js
```

**终端 4 - API 服务**
```bash
cd /Users/jingle/cat/cowallet
export DATABASE_URL=postgres://postgres@localhost:5432/cowallet
export REDIS_URL=redis://localhost:6379
export NATS_URL=nats://localhost:4222
export CLAUDE_API_KEY=sk-ant-xxxxx

cargo run --release --bin api-server
```

## 验证部署

```bash
# 检查 API 健康状态
curl http://localhost:3000/health

# 检查数据库
psql -U postgres cowallet -c "SELECT COUNT(*) FROM users;"

# 检查 Redis
redis-cli ping

# 检查 NATS
nats account info

# 查看服务状态
make local-status
```

## 常用命令

| 命令 | 功能 |
|------|------|
| `make local-init` | 一次性初始化环境 |
| `make local-start` | 启动 API 服务 |
| `make local-build` | 编译项目 |
| `make local-migrate` | 运行数据库迁移 |
| `make local-stop` | 停止应用服务 |
| `make local-status` | 查看服务状态 |

## 停止所有服务

```bash
# 停止应用
make local-stop

# 停止系统服务
brew services stop postgresql@16
brew services stop redis
pkill -f nats-server
```

## 常见问题

### Q: 提示"连接被拒绝"
A: 检查 PostgreSQL 是否运行：
```bash
brew services list | grep postgres
brew services start postgresql@16
```

### Q: 端口被占用
A: 查找占用进程并杀死：
```bash
lsof -i :3000      # API 端口
lsof -i :5432      # PostgreSQL 端口
kill -9 <PID>
```

### Q: 数据库迁移失败
A: 重置数据库：
```bash
dropdb -U postgres cowallet
createdb -U postgres cowallet
make local-migrate
```

### Q: API 无法启动
A: 检查环境变量：
```bash
echo $DATABASE_URL
echo $REDIS_URL
echo $NATS_URL
```

## 相关文档

- 📖 详细部署指南：[DEPLOY.md](DEPLOY.md)
- 📋 项目计划：[PLAN.md](PLAN.md)
- 🐳 Docker 部署：[DOCKER.md](DOCKER.md)
