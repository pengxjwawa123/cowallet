# cowallet CentOS 部署快速参考

## 前置条件检查

```bash
# 确保已完成以下安装
gcc --version          # 应 >= 11.0
/usr/pgsql-16/bin/psql --version
redis-server --version
nats-server --version
rustc --version
```

## 一键初始化（首次运行）

```bash
make local-init
```

这会：
- ✅ 检查 PostgreSQL、Redis、NATS、Rust 已安装
- ✅ 检查 GCC 版本 >= 11（编译需要）
- ✅ 启动 PostgreSQL 和 Redis
- ✅ 配置 PostgreSQL 本地认证
- ✅ 创建 cowallet 数据库
- ✅ 运行数据库迁移
- ✅ 提示启动 NATS 服务器

## 启动服务

### 方式 1：使用 Makefile（推荐）

**终端 1 - 启动消息队列**
```bash
sudo systemctl start nats
# 或后台启动: nats-server -js &
```

**终端 2 - 启动 API 服务**
```bash
cd /path/to/cowallet
make local-start
```

### 方式 2：使用启动脚本

```bash
./start-local.sh
```

### 方式 3：手动启动各服务

**系统服务（持久化）**
```bash
sudo systemctl enable postgresql-16 redis nats
sudo systemctl start postgresql-16 redis nats
```

**验证系统服务状态**
```bash
sudo systemctl status postgresql-16
sudo systemctl status redis
sudo systemctl status nats
```

**终端 - API 服务**
```bash
cd /path/to/cowallet
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
sudo -u postgres /usr/pgsql-16/bin/psql -d cowallet -c "SELECT COUNT(*) FROM users;"

# 检查 Redis
redis-cli ping

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
sudo systemctl stop postgresql-16
sudo systemctl stop redis
sudo systemctl stop nats
```

## 常见问题

### Q: GCC 版本过旧（< 11）
A: 升级 GCC：
```bash
# CentOS 7 用户
sudo yum install -y centos-release-scl devtoolset-11-gcc devtoolset-11-gcc-c++
echo "source scl_enable devtoolset-11" >> ~/.bashrc
source ~/.bashrc

# CentOS 8/9 用户
sudo yum install -y gcc-11 gcc-c++-11
sudo update-alternatives --install /usr/bin/gcc gcc /usr/bin/gcc-11 100

# 验证
gcc --version
```

### Q: 提示"password authentication failed"
A: PostgreSQL 认证配置问题，已在 `make local-init` 中自动修复。手动修复：
```bash
sudo sed -i 's/^local.*all.*all.*peer/local   all             all                                     trust/' /var/lib/pgsql/16/data/pg_hba.conf
sudo systemctl restart postgresql-16
```

### Q: 端口被占用
A: 查找占用进程并杀死：
```bash
ss -tlnp | grep -E '3000|5432|6379|4222'  # 查看占用

# 或逐个检查
ss -tlnp | grep 3000    # API 端口
ss -tlnp | grep 5432    # PostgreSQL 端口
ss -tlnp | grep 6379    # Redis 端口
ss -tlnp | grep 4222    # NATS 端口

# 杀死进程
kill -9 <PID>
```

### Q: 数据库迁移失败
A: 重置数据库：
```bash
sudo -u postgres /usr/pgsql-16/bin/dropdb cowallet
sudo -u postgres /usr/pgsql-16/bin/createdb cowallet
make local-migrate
```

### Q: API 无法启动
A: 检查环境变量和连接：
```bash
echo $DATABASE_URL
echo $REDIS_URL
echo $NATS_URL

# 测试数据库连接
sudo -u postgres /usr/pgsql-16/bin/psql -d cowallet -c "SELECT 1"

# 测试 Redis
redis-cli ping

# 查看完整日志
make local-start  # 会显示详细错误信息
```

### Q: 编译失败 - "COMPILER BUG DETECTED"
A: 这是 aws-lc-sys 库检测到的 GCC bug，见上方"GCC 版本过旧"解决方案

## 相关文档

- 📖 详细部署指南：[DEPLOY.md](DEPLOY.md)
- 📋 项目计划：[PLAN.md](PLAN.md)
- 🐳 Docker 部署：[DOCKER.md](DOCKER.md)
