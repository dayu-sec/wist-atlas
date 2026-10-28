# wist-atlas — 中心侧资产与运行拓扑服务
#
# 构建阶段产出两个可执行文件：
#   wist-atlas           HTTP API / CLI 单体
#   atlas-wist-ingest    消费网关发现快照的增量 worker
FROM rust:1.99-bookworm AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p atlas-app -p atlas-wist-ingest

# 运行时只需要 wget 做容器健康检查；用 busybox 避免 apt 依赖。
FROM busybox:stable AS busybox

FROM debian:bookworm-slim

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=busybox /bin/busybox /bin/busybox

COPY --from=builder /src/target/release/wist-atlas /usr/local/bin/wist-atlas
COPY --from=builder /src/target/release/atlas-wist-ingest /usr/local/bin/atlas-wist-ingest

USER 10001:10001
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD ["/bin/busybox", "wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8080/healthz"]

ENTRYPOINT ["wist-atlas"]
CMD ["postgres-live", "serve", "--listen", "0.0.0.0:8080"]
