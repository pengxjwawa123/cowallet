# 简单构建 - 优先保证构建成功
FROM rust:1.85-slim AS builder

WORKDIR /app

# 安装依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# 复制所有源码
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY backend ./backend
COPY migrations ./migrations

# 构建二进制文件
RUN cargo build --release --bin api-server --bin mpc-relay --bin indexer --bin worker

# 运行层
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# 从构建层复制二进制文件
COPY --from=builder /app/target/release/api-server /usr/local/bin/
COPY --from=builder /app/target/release/mpc-relay /usr/local/bin/
COPY --from=builder /app/target/release/indexer /usr/local/bin/
COPY --from=builder /app/target/release/worker /usr/local/bin/
COPY --from=builder /app/migrations ./migrations

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

EXPOSE 3000 4222 5432

CMD ["api-server"]
