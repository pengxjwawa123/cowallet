# cowallet ECS 服务器部署指南（CentOS/RHEL）

## 前置要求

### 1. 系统依赖（CentOS 7/8/9）

```bash
# 更新包列表
sudo yum update -y

# 安装开发工具（Rust 编译需要）
sudo yum install -y gcc g++ make pkg-config openssl-devel

# 安装 PostgreSQL 16
sudo yum install -y https://download.postgresql.org/pub/repos/yum/reporpms/EL-7-x86_64/pgdg-redhat-repo-latest.noarch.rpm
sudo yum update -y
sudo yum install -y postgresql16-server postgresql16-contrib

# 初始化 PostgreSQL（仅首次）
sudo /usr/pgsql-16/bin/postgresql-16-setup initdb
sudo systemctl enable postgresql-16
sudo systemctl start postgresql-16

# 安装 Redis
sudo yum install -y redis

# 启动 Redis
sudo systemctl enable redis
sudo systemctl start redis

# 安装 NATS Server
NATS_VERSION=2.10.14
wget https://github.com/nats-io/nats-server/releases/download/v${NATS_VERSION}/nats-server-v${NATS_VERSION}-linux-amd64.tar.gz
tar -xzf nats-server-v${NATS_VERSION}-linux-amd64.tar.gz
sudo mv nats-server-v${NATS_VERSION}-linux-amd64/nats-server /usr/local/bin/
rm -rf nats-server-v${NATS_VERSION}-linux-amd64*

# 验证安装
/usr/pgsql-16/bin/psql --version
redis-server --version
nats-server --version
```

> **Ubuntu/Debian** 用户请使用 `apt-get`：
> ```bash
> sudo apt-get update
> sudo apt-get install -y postgresql-16 redis-server build-essential pkg-config libssl-dev
> ```

### 2. 安装 Rust 工具链

```bash
# 安装 rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# 验证
rustc --version
cargo --version
```

### 3. 安装 sqlx-cli

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

---

## 部署步骤

### 第 1 步：启动基础服务

**PostgreSQL**
```bash
# 初始化数据目录（如已执行上述安装，可跳过）
sudo /usr/pgsql-16/bin/postgresql-16-setup initdb

# 启动服务
sudo systemctl enable postgresql-16
sudo systemctl start postgresql-16

# 设置 postgres 用户密码（可选，本地连接可不设）
sudo -u postgres /usr/pgsql-16/bin/psql -c "ALTER USER postgres PASSWORD 'your_password';"

# 验证
sudo -u postgres /usr/pgsql-16/bin/psql -c "SELECT version();"
```

**Redis**
```bash
sudo systemctl enable redis
sudo systemctl start redis

# 验证
redis-cli ping
```

**NATS**
```bash
# 后台启动（支持 JetStream）
nats-server -js &

# 或使用 systemd 管理（推荐生产环境）
sudo tee /etc/systemd/system/nats.service > /dev/null <<'EOF'
[Unit]
Description=NATS Server
After=network.target

[Service]
ExecStart=/usr/local/bin/nats-server -js
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable nats
sudo systemctl start nats
```

### 第 2 步：初始化数据库

```bash
# 配置 PostgreSQL 本地认证（允许无密码本地连接）
sudo sed -i 's/^local.*all.*all.*peer/local   all             all                                     trust/' /var/lib/pgsql/16/data/pg_hba.conf
sudo systemctl restart postgresql-16

# 切换到 postgres 用户创建数据库
sudo -u postgres /usr/pgsql-16/bin/createdb cowallet

# 运行迁移脚本（在项目目录执行）
cd /path/to/cowallet
export DATABASE_URL=postgres://postgres@localhost:5432/cowallet
sqlx migrate run --source backend/migrations

# 验证迁移
sudo -u postgres /usr/pgsql-16/bin/psql -d cowallet -c "\dt"
```

### 第 3 步：拉取代码并编译

```bash
# 克隆项目（如尚未克隆）
git clone https://github.com/your-org/cowallet.git
cd cowallet

# 构建 release 版本
cargo build --release

# 二进制文件位于 ./target/release/
ls -lh target/release/{api-server,mpc-relay,indexer,worker}
```

### 第 4 步：配置环境变量

```bash
# 创建环境变量文件（不要提交到 git）
cat > /etc/cowallet.env <<'EOF'
DATABASE_URL=postgres://postgres@localhost:5432/cowallet
REDIS_URL=redis://localhost:6379
NATS_URL=nats://localhost:4222
CLAUDE_API_KEY=sk-ant-xxxxx
RPC_URL=https://sepolia.base.org
RPC_WS_URL=wss://sepolia.base.org
RUST_LOG=info,api_server=debug
EOF

sudo chmod 600 /etc/cowallet.env
```

### 第 5 步：启动后端服务

**方式一：前台运行（调试用）**
```bash
source /etc/cowallet.env

# API 服务器
./target/release/api-server

# MPC 中继（另开终端）
./target/release/mpc-relay

# 索引器（另开终端）
./target/release/indexer

# 工作线程（另开终端）
./target/release/worker
```

**方式二：systemd 托管（推荐生产环境）**
```bash
# API 服务
sudo tee /etc/systemd/system/cowallet-api.service > /dev/null <<EOF
[Unit]
Description=CoWallet API Server
After=network.target postgresql.service redis-server.service nats.service

[Service]
EnvironmentFile=/etc/cowallet.env
ExecStart=/path/to/cowallet/target/release/api-server
Restart=always
RestartSec=5
User=www-data

[Install]
WantedBy=multi-user.target
EOF

# MPC 中继
sudo tee /etc/systemd/system/cowallet-mpc.service > /dev/null <<EOF
[Unit]
Description=CoWallet MPC Relay
After=network.target nats.service

[Service]
EnvironmentFile=/etc/cowallet.env
ExecStart=/path/to/cowallet/target/release/mpc-relay
Restart=always
RestartSec=5
User=www-data

[Install]
WantedBy=multi-user.target
EOF

# 启用并启动
sudo systemctl daemon-reload
sudo systemctl enable cowallet-api cowallet-mpc
sudo systemctl start cowallet-api cowallet-mpc
```

---

## 验证部署

```bash
# 检查 API 服务健康状态
curl http://localhost:3000/health

# 检查各服务状态
sudo systemctl status postgresql-16 redis nats cowallet-api

# 检查数据库
sudo -u postgres /usr/pgsql-16/bin/psql -d cowallet -c "SELECT COUNT(*) FROM users;"

# 检查 Redis
redis-cli ping

# 查看实时日志
sudo journalctl -u cowallet-api -f
```

---

## 快速启动脚本

`start-local.sh` 已在项目中，直接使用：

```bash
# 编辑脚本中的环境变量后执行
chmod +x start-local.sh
./start-local.sh
```

---

## 停止所有服务

```bash
# systemd 管理的服务
sudo systemctl stop cowallet-api cowallet-mpc

# 基础服务（如需停止）
sudo systemctl stop nats redis postgresql-16

# 手动杀死进程（应急）
pkill -f "api-server"
pkill -f "mpc-relay"
pkill -f "indexer"
pkill -f "worker"
```

---

## 故障排查

### PostgreSQL 连接失败
```bash
# 检查服务状态
sudo systemctl status postgresql-16

# 检查监听地址
sudo -u postgres /usr/pgsql-16/bin/psql -c "SHOW listen_addresses;"

# 查看日志
sudo journalctl -u postgresql-16 -n 50

# 重设数据库
sudo -u postgres /usr/pgsql-16/bin/dropdb cowallet
sudo -u postgres /usr/pgsql-16/bin/createdb cowallet
sqlx migrate run --source backend/migrations
```

### Redis 连接失败
```bash
sudo systemctl status redis
sudo journalctl -u redis -n 20
```

### 端口被占用
```bash
# 查找占用进程
netstat -tlnp | grep -E '3000|5432|6379|4222'

# 或使用 ss
ss -tlnp | grep -E '3000|5432|6379|4222'

# 杀死进程
kill -9 <PID>
```

### 迁移失败
```bash
# 检查迁移状态
sqlx migrate info --source backend/migrations

# 手动执行 SQL
sudo -u postgres /usr/pgsql-16/bin/psql -d cowallet -f backend/migrations/001_initial_schema.sql
```

### Rust 编译失败（找不到 openssl）
```bash
sudo yum install -y openssl-devel pkg-config
```

---

## 生产部署考虑

1. **密钥管理**：`/etc/cowallet.env` 权限设为 600，避免明文泄露
2. **防火墙**：仅对外暴露 3000（API），其余端口仅本机访问
   ```bash
   # CentOS/RHEL 使用 firewalld
   sudo firewall-cmd --permanent --add-port=3000/tcp
   sudo firewall-cmd --reload
   
   # 验证
   sudo firewall-cmd --list-ports
   ```
3. **反向代理**：使用 Nginx 添加 TLS 终止
   ```bash
   sudo yum install -y nginx certbot python3-certbot-nginx
   ```
4. **数据持久化**：配置 PostgreSQL 定时备份
   ```bash
   # crontab -e
   0 2 * * * /usr/pgsql-16/bin/pg_dump -U postgres cowallet > /backup/cowallet-$(date +\%F).sql
   ```
5. **监控**：`journalctl` 日志 + 可选接入 Prometheus / Grafana

---

## 相关命令

```bash
# 查看 API 文档
curl http://localhost:3000/docs

# 查看实时日志
sudo journalctl -u cowallet-api -f

# 数据库备份
/usr/pgsql-16/bin/pg_dump -U postgres cowallet > backup.sql

# 数据库恢复
sudo -u postgres /usr/pgsql-16/bin/psql cowallet < backup.sql
```
