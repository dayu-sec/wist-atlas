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

### wist-agentd 发现快照接入

边缘侧（`wist-agentd` 发、网关数据面的 `DSNAP:` 通道收）的发现快照，用 `atlas-wist-adapter`
转成 `atlas.in.edge.v1`，再走上面的 `replay-jsonl` / `file` 入口：

```bash
# 从 agentd 的 state 目录（离线，不需要网关）
cargo run -p atlas-wist-adapter -- \
  --state-dir /var/lib/wist-agentd/state --agent-id mac-local-01 \
  --tenant tenant-demo --env dev --out /tmp/edge.jsonl --jsonl

# 从一帧 DSNAP: 的正文（网关 agent-discovery sink 落的那一行里的 body 字段）
cargo run -p atlas-wist-adapter -- \
  --report /tmp/dsnap-body.json --tenant tenant-demo --env dev \
  --out /tmp/edge.jsonl --jsonl

cargo run -p atlas-app -- replay-jsonl /tmp/edge.jsonl
```

映射规则（发现面 → 资源 kind → atlas 模型）见
[doc/external-integration/warp-insight-adapter-spec.md](./doc/external-integration/warp-insight-adapter-spec.md)。
映射的 golden fixture 在 [crates/atlas-wist-adapter/fixtures](./crates/atlas-wist-adapter/fixtures)，
用 `cargo test -p atlas-wist-adapter` 守。

### 自动消费网关的 sink 文件

上面的适配器是**一次性**的（读一个快照、写一个信封）。要让它持续干活，用 `atlas-wist-ingest`：
它增量读网关 `agent-discovery` sink 落下的文件，转成 `atlas.in.edge.v1` 直接入库。

```bash
DATABASE_URL='postgres://atlas:atlas@127.0.0.1:55432/wist_atlas' \
  atlas-wist-ingest \
    --input /var/lib/wist-gateway/data/out_dat/agent-discovery.jsonl \
    --checkpoint /var/lib/atlas/discovery.offset.json \
    --tenant <tenant-uuid> \
    --store postgres
```

为什么默认是「读文件」而不是「让网关 POST 过来」：文件由 wparse 追加写，**atlas 停机期间帧还在长**，恢复后自动补；也不需要为写入口设计鉴权（写 atlas 的只有 atlas 自己）。
HTTP 通道现在也支持（见下一节），两者可以并存。

几个必须知道的行为：

- `--checkpoint` **必须显式给**：worker 的状态不该跟着 CWD 走。
- 只消费**完整行**（以 `\n` 结尾）；网关正在写的半帧不会进解析器。
- 文件轮转 / 被截断 → 自动从 0 重读。重读是安全的：`ingest_id` 含 `snap_id`，重放幂等。
- 坏行（非 JSON / 没有 `body` / 快照缺 host）跳过并推进；写库失败**不推进**，下一轮重试。
- `--once` 只跑一轮，方便排查和 cron。

### HTTP 接收（数据面直接投递）

`serve` 模式下另开了一个写入端点，**两字段**：

```text
POST /internal/atlas[?env=<env>]
Content-Type: application/json

{
  "cust_id": "<atlas 租户 UUID>",
  "content": { ... }          // DSNAP 记录：单个对象，或 batch_size>1 时的对象数组
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `cust_id` | ✅ | atlas 的租户 UUID（不是外部 tenant ref）。索引分区维度就用它 |
| `content` | ✅ | 网关的 DSNAP 记录；与读文件路径是**同一个转换函数**（`convert_sink_line`） |

> 为什么比证据服务少一个 `cmd_id`：证据写的是「一次命令的产物」，`cmd_id` 是它的幂等键；
> atlas 写的是**状态快照**，幂等键在快照自己身上（`snap_id` → `ingest_id`）。
> 为一个不参与判重的字段强制数据面编值，只会让契约变钝。

- `content` 里的 `env` 不传就用 sink 行上的值；query 上的 `?env=` 优先级更高。
- body 上限 16 MB（axum 默认只有 2 MB），单请求最多 64 条记录。
- 不要求 `Content-Type`：数据面的 http 连接器默认不带它，用 415 拒掉一帧合法快照是最难排查的故障。

回执是给数据面看的 —— 非 2xx 会被重试、最终落 rescue（可见），2xx 意味着「已收到，不必再发」：

| 结果 | 状态码 | 含义 |
|---|---|---|
| 全部成功（含幂等重复） | `200` | `{"count","ingested","rejected","hosts","failures"}` |
| 有永久性失败 | `400` | 不是快照 / 没有 `body` / JSON 坏了；`failures[]` 带 `index` 与原因 |
| 有暂时性失败 | `503` | 写库失败，让数据面下一轮重试 |
| body 超限 | `413` | |

> 与 evidence 的一处刻意不同：atlas **不报 `duplicates`**。它的幂等由快照 `snap_id` 决定
> （`ingest_id` 含它），同一快照重复投递在库里是同一次 upsert，回执上区分不了也不该假装能区分。

数据面侧配参见 [`wist-console-stack/examples/warp-parse/`](https://github.com/dayu-sec/wist-console-stack/tree/main/examples/warp-parse)
（`atlas_uplink.toml`，复用现成的 `http_sink`，不新增 connector）：OML 输出必须落到这三个字段上。

边界与取舍：

- **端点还没有鉴权**（与网关内部接入端点同一档降级）：只绑环回或放在受信内网，别直接对外。
- `http` 连接器**能发 HTTPS 但没有 TLS 参数**（无 CA / 客户端证书 / skip-verify），自签证书验不过。
- 想要「不丢」就得同时留文件 sink：HTTP 走热路径，文件负责回放。只留 HTTP 时，接收端长时间
  不可用会把 `max_retries` 耗尽、帧进 rescue，需要有人捞。

## 只读接口与 API 文档

`serve` 模式下对外只有读接口（写入口 `POST /internal/atlas` 见上一节，**不进** spec）：

```text
GET /api/hosts                                主机列表（分页，剔除写探针主机）
GET /api/services                             服务列表（规模指标）
GET /api/topology/host/first                  第一台主机的焦点视图
GET /api/topology/host/{id}                   主机焦点视图
GET /api/topology/host/{id}/process-overview  进程概览（截断）
GET /api/topology/host/{id}/process-groups    进程组分页
GET /api/topology/host/first/processes        第一台主机的进程拓扑图
GET /api/topology/host/{id}/processes         主机进程拓扑图
GET /api/topology/network/{id}                网段视图
GET /api/topology/service/{id}                服务视图
```

文档**从代码生成**，不另维护一份手写 spec：

| 端点 | 用途 |
|---|---|
| `GET /api/openapi.json` | OpenAPI 3.1 spec（机器可读；前端 / 网关 / 测试都拿它） |
| `GET /api/docs` | 自包含 HTML 阅读页（不引 CDN，计算区可离线用） |

生成口径：路径来自 `utoipa-axum` 的 `routes!`（handler 上有 `#[utoipa::path]` 就会自动收录，
所以 Router 与文档共用一份路径），字段名来自 serde（`rename_all = "camelCase"` 直接反映到
schema）。**10 个读接口全部已标注**；深层嵌套的领域类型用 `http_schema.rs` 里一组只给 schema
用的镜像 DTO 展开（不给 `atlas-domain` 加 `ToSchema`）。

文档不会自己跑偏，三道单测守着：标注了 = spec 里有、每个 `$ref` 能解析、**真实响应 vs spec**
（多一个字段 / 少一个必填字段 / 类型不对都变红）。

读接口要令牌：`Authorization: Bearer ${WIST_ATLAS_TOKEN}`（`serve` 启动时从环境变量读，
**拿不到就拒绝启动**）。不要求令牌的只有四个：`/healthz`（探活）、`/internal/atlas`（数据面
写入）、`/api/openapi.json` 与 `/api/docs`（接口形状，不含数据；浏览器也没法给导航请求加头）。

经 `wist-field` 访问时由 BFF 代理并校验终端用户的令牌，BFF 再带着 atlas 令牌去读：
`https://<数据场入口>/api/atlas/openapi.json`。

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
