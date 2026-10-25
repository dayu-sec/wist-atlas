# wist-atlas

`wist-atlas` 是一个面向业务、服务、基础设施与安全治理的统一资产与运行拓扑中心。

它负责把以下对象和关系统一建模到中心侧：

- business / system / subsystem / service
- host / pod / network
- software / vulnerability
- responsibility / owner / maintainer / oncall
- dependency / attachment / placement / endpoint

## 命名

- 产品 / 仓库：`wist-atlas`
- Rust crates：`atlas-domain` / `atlas-storage` / `atlas-api` / `atlas-sync` / `atlas-app`
- 二进制：`wist-atlas`
- 外部输入 schema：`atlas.in.<family>.v1`（兼容旧前缀 `dayu.in.*` 一个版本）
- 环境变量 / 开发库：`WIST_ATLAS_*` / `wist_atlas`（旧 `DAYU_*` / `dayu_topology` 保留兜底）

## 项目定位

它首先是：

- 统一资产模型
- 统一运行拓扑模型
- 中心侧资源目录与关系图谱

它当前不是：

- 完整数字孪生系统
- 边缘 agent
- 控制平面执行器
- 通用 observability 平台

## 顶层边界

建议固定三条边界：

1. `wist-atlas` 负责中心侧对象模型、关系归并、查询与同步。
2. 边缘 agent 负责发现、本地采集和上报，不负责中心全局归并。
3. 控制中心负责审批、编译、下发、执行跟踪与审计，不直接承担统一拓扑目录职责。

## 目录结构

```text
doc/
  architecture/
  model/
  roadmap/
crates/
  atlas-domain/
  atlas-storage/
  atlas-api/
  atlas-sync/
  atlas-app/
```

## 第一阶段目标

- 固定 repo charter 与边界
- 固定核心中心对象模型
- 固定最小存储与查询接口边界
- 固定外部同步来源与幂等原则
- 为后续独立服务化保留清晰拆分面

## 当前文档

- [doc/README.md](./doc/README.md)
- [doc/glossary.md](./doc/glossary.md)
- [doc/architecture/project-charter.md](./doc/architecture/project-charter.md)
- [doc/architecture/system-architecture.md](./doc/architecture/system-architecture.md)
- [doc/architecture/storage-architecture.md](./doc/architecture/storage-architecture.md)
- [doc/architecture/service-and-deployment-architecture.md](./doc/architecture/service-and-deployment-architecture.md)
- [doc/architecture/external-sync-architecture.md](./doc/architecture/external-sync-architecture.md)
- [doc/architecture/query-and-read-model-architecture.md](./doc/architecture/query-and-read-model-architecture.md)
- [doc/architecture/identity-resolution-architecture.md](./doc/architecture/identity-resolution-architecture.md)
- [doc/architecture/security-and-access-control-architecture.md](./doc/architecture/security-and-access-control-architecture.md)
- [doc/architecture/observability-and-audit-architecture.md](./doc/architecture/observability-and-audit-architecture.md)
- [doc/architecture/dataflow-and-pipeline-architecture.md](./doc/architecture/dataflow-and-pipeline-architecture.md)
- [doc/external-integration/README.md](./doc/external-integration/README.md)
- [doc/external-integration/external-input-spec.md](./doc/external-integration/external-input-spec.md)
- [doc/external-integration/file-ingest-spec.md](./doc/external-integration/file-ingest-spec.md)
- [doc/architecture/unified-model-overview.md](./doc/architecture/unified-model-overview.md)
- [doc/model/README.md](./doc/model/README.md)
- [doc/model/unified-topology-schema.md](./doc/model/unified-topology-schema.md)
- [doc/external-integration/warp-insight-adapter-spec.md](./doc/external-integration/warp-insight-adapter-spec.md)
- [doc/roadmap/bootstrap-plan.md](./doc/roadmap/bootstrap-plan.md)

## 文件导入示例

当前单体模式支持从 normalized JSON payload 文件载入 P0 最小闭环数据：

```bash
cargo run -p atlas-app -- file fixtures/file-ingest/minimal-host-network.json
cargo run -p atlas-app -- file fixtures/file-ingest/small-office.json
```

文件格式规范见 [doc/external-integration/file-ingest-spec.md](./doc/external-integration/file-ingest-spec.md)。

目标态外部输入数据示例放在 [fixtures/external-input/target](./fixtures/external-input/target)，协议规范见 [doc/external-integration/external-input-spec.md](./doc/external-integration/external-input-spec.md)。

## PostgreSQL 开发环境

repo 根下提供了最小 PostgreSQL 开发环境：

```bash
cd wist-atlas
docker compose up -d
```

默认连接信息：

- host: `127.0.0.1`
- port: `55432`
- database: `wist_atlas`
- user: `atlas`
- password: `atlas`

当前用途：

- 管理 `wist-atlas` 所需的 PostgreSQL 实例生命周期
- 为后续真实 `PostgresExecutor` / `DATABASE_URL` backend 提供固定开发环境

当前状态要点：

- `atlas-storage` 已补齐 `PostgresTopologyStore` 的 runtime repository
- `postgres-mock` 已走完整 PostgreSQL repository 路径
- `atlas-app` 已支持 `postgres-live`，可直接把 replay 数据写入真实 PostgreSQL

一键清库 + 重放 + 查库可直接执行：

```bash
cd wist-atlas
python3 scripts/replay_pg_jsonl.py \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl
```
