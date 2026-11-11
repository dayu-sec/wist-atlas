# ===========================================================================
# wist-atlas — 运行镜像（多架构）
#
# 与 `wist-field-api` / `wist-gateway` 同一套做法：二进制由 release 流水线按平台
# **原生**编译，build context（本目录）按 `amd64/` `arm64/` 提供，镜像只负责打包。
#
# 为什么不在镜像里 `cargo build`（2026-11-11 实测）：
#   `platforms: linux/amd64,linux/arm64` 一次构建时，arm64 那一段要在 amd64 runner 上
#   过 QEMU 编译整个 workspace，`Build and push` 一步就 30 分钟起；
#   而镜像里那套 rust toolchain 与编译缓存**没有任何复用价值**（每次都是新容器）。
#
# 运行期只需要三样：两个二进制、根证书、静态 busybox（compose / HEALTHCHECK 用）。
# 所以这里**不装 apt**：证书与 busybox 直接从现成镜像拷。
#
# 基底用 `ubuntu:24.04`：与原生构建的 runner 同代 glibc（2.39）。
# 换成 `debian:bookworm`（2.36）会直接跑不起来 —— 这一点和 field-api 同理。
# ===========================================================================
FROM busybox:stable-musl AS busybox
FROM alpine:3.20 AS certs

FROM ubuntu:24.04

ARG TARGETARCH

LABEL org.opencontainers.image.source="https://github.com/dayu-sec/wist-atlas"
LABEL org.opencontainers.image.description="Asset & runtime topology service (HTTP API + wist ingest worker)"
LABEL org.opencontainers.image.licenses="Apache-2.0"

# 根证书（外发同步/HTTPS 上游用）+ 静态 busybox（容器健康检查用）。
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=busybox /bin/busybox /bin/busybox

# 二进制由 CI 按平台原生编译后铺进 `docker/<arch>/`（见 .github/workflows/release.yml）。
COPY ${TARGETARCH}/wist-atlas /usr/local/bin/wist-atlas
COPY ${TARGETARCH}/atlas-wist-ingest /usr/local/bin/atlas-wist-ingest

# 配置与密钥由栈挂载进来（本仓不带默认配置）。
USER 10001:10001
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD ["/bin/busybox", "wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8080/healthz"]

ENTRYPOINT ["wist-atlas"]
CMD ["postgres-live", "serve", "--listen", "0.0.0.0:8080"]
