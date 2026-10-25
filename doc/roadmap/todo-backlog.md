# wist-atlas 待办清单

## 1. 文档目的

本文档把 [`execution-plan.md`](./execution-plan.md) 中的工作包进一步压缩成可跟踪的 backlog。

目标是明确：

- 当前应优先做哪些任务
- 每个任务的优先级、依赖和验收条件
- 任务如何分配到各 crate

相关文档：

- [`development-plan.md`](./development-plan.md)
- [`execution-plan.md`](./execution-plan.md)

---

## 2. 使用方式

建议每个任务至少维护以下字段：

- `Priority`
- `Status`
- `Owner`
- `Depends On`
- `Acceptance`

状态建议统一为：

- `todo`
- `in_progress`
- `blocked`
- `done`

状态口径约束：

- 只有“能力已闭环并可验证”才能标记为 `done`
- 只有“已有代码骨架或草案，但尚未形成闭环”时标记为 `in_progress`

负责人建议先填：

- `TBD`

---

## 3. P0：最小闭环必须完成

这些任务直接决定第一版是否能形成最小可运行闭环。

| ID | Priority | Task | Crate | Status | Owner | Depends On | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `P0-01` | `P0` | 定义 `host + network + responsibility` 核心领域对象与枚举 | `atlas-domain` | `in_progress` | `TBD` | - | `HostInventory/NetworkDomain/NetworkSegment/HostNetAssoc/Subject/ResponsibilityAssignment` 有稳定类型定义 |
| `P0-02` | `P0` | 定义共享值对象与时间语义类型 | `atlas-domain` | `in_progress` | `TBD` | `P0-01` | `tenant/environment/validity/observed_at/confidence` 不再散落为裸 `String` |
| `P0-03` | `P0` | 定义 ingest/candidate/resolution/read-model contract | `atlas-domain` | `in_progress` | `TBD` | `P0-01` | 文件输入、中间层和查询 DTO 有单独类型，API 和 storage 不自定义领域语义 |
| `P0-04` | `P0` | 建立 migration 目录与初始化方式 | `atlas-storage` | `in_progress` | `TBD` | `P0-01` | 空库初始化方式固定，migration 可被实际执行 |
| `P0-05` | `P0` | 落第一批 host/network/governance 主表 | `atlas-storage` | `done` | `TBD` | `P0-04` | `host/network/subject/responsibility/ingest_job` 主表落地 |
| `P0-06` | `P0` | 固定关键唯一键与时间约束 | `atlas-storage` | `in_progress` | `TBD` | `P0-05` | 数据库层保障幂等与有效期基础约束 |
| `P0-07` | `P0` | 实现基础 repository trait 与最小 Postgres 执行器 | `atlas-storage` | `in_progress` | `TBD` | `P0-05,P0-06` | `HostStore/NetworkStore/GovernanceStore/IngestStore` 可执行真实数据库读写，不只停留在 trait/SQL 常量 |
| `P0-08` | `P0` | 实现文件 ingest submit 入口 | `atlas-api` | `done` | `TBD` | `P0-03,P0-07` | 可接收 `IngestEnvelope` 并记录 ingest job |
| `P0-09` | `P0` | 实现主机目录 candidate extractor | `atlas-api` | `done` | `TBD` | `P0-08` | 可从目录导入 payload 提取 host candidate |
| `P0-10` | `P0` | 实现 IP / 网段清单 candidate extractor | `atlas-api` | `done` | `TBD` | `P0-08` | 可从 IP 或 CIDR 清单提取 network candidate |
| `P0-11` | `P0` | 实现 host/network/subject 最小 resolver | `atlas-api` | `in_progress` | `TBD` | `P0-03,P0-07,P0-09,P0-10` | 至少支持强标识和组合标识规则 |
| `P0-12` | `P0` | 实现 materializer | `atlas-api` | `done` | `TBD` | `P0-11` | 已解析对象可写入主库，未解析对象不写正式关系 |
| `P0-13` | `P0` | 打通主机目录导入链路 | `atlas-api` | `done` | `TBD` | `P0-09,P0-11,P0-12` | 导入主机目录后可查到 host 数据 |
| `P0-14` | `P0` | 打通 IP / 网段清单导入链路 | `atlas-api` | `done` | `TBD` | `P0-10,P0-11,P0-12` | 导入 IP / 网段清单后可查到网络对象与关联关系 |
| `P0-15` | `P0` | 实现责任关系导入链路 | `atlas-api` | `done` | `TBD` | `P0-11,P0-12` | 可导入 `ResponsibilityAssignment` 且不破坏已存在主对象 |
| `P0-16` | `P0` | 实现核心对象查询 API | `atlas-api` | `in_progress` | `TBD` | `P0-07` | 支持查询 `host/network/subject` |
| `P0-17` | `P0` | 实现基础 topology/governance view | `atlas-api` | `done` | `TBD` | `P0-16,P0-12` | 支持 `host_topology_view/network_topology_view/effective_responsibility_view` |
| `P0-18` | `P0` | 实现单体模式启动入口 | `atlas-app` | `done` | `TBD` | `P0-07,P0-16` | 本地可启动 API + storage 的最小单体 |
| `P0-19` | `P0` | 建立最小 e2e smoke test | `atlas-app` | `done` | `TBD` | `P0-13,P0-14,P0-15,P0-17,P0-18` | 本地或 CI 中可验证导入与查询最小闭环 |
| `P0-20` | `P0` | 固定文件导入模式 `snapshot / batch_upsert` | `atlas-domain`,`atlas-api` | `in_progress` | `TBD` | `P0-03,P0-08` | 导入模式成为显式 contract，并通过测试验证幂等语义 |

---

## 4. P1：第一版增强能力

这些任务不决定“能不能跑起来”，但决定第一版是否具备场景扩展能力。

| ID | Priority | Task | Crate | Status | Owner | Depends On | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `P1-01` | `P1` | 增加 `business / system / service` 基础模型 | `atlas-domain`,`atlas-storage` | `todo` | `TBD` | `P0-07` | 业务目录对象可导入、可查询 |
| `P1-02` | `P1` | 增加 `EpRes` 基础模型与查询 | `atlas-domain`,`atlas-storage`,`atlas-api` | `todo` | `TBD` | `P1-01` | 可查询服务暴露与基础端点资源 |
| `P1-03` | `P1` | 增加 `cluster / namespace / workload / pod` 基础模型 | `atlas-domain`,`atlas-storage` | `todo` | `TBD` | `P0-07` | 中型企业场景基础编排对象落地 |
| `P1-04` | `P1` | 扩展文件导入模式到 `delta` | `atlas-domain`,`atlas-api` | `todo` | `TBD` | `P0-20` | 支持局部更新且不破坏已存在对象 |
| `P1-05` | `P1` | 增加 resolution explain 最小版 | `atlas-api` | `todo` | `TBD` | `P0-11` | 可解释为何命中某内部对象 |
| `P1-06` | `P1` | 增加 request/pipeline 结构化日志 | `atlas-app` | `todo` | `TBD` | `P0-18` | 关键路径具备统一日志字段 |
| `P1-07` | `P1` | 增加基础 metrics | `atlas-app` | `todo` | `TBD` | `P0-18` | 至少暴露 request latency、ingest 成功率、冲突计数 |
| `P1-08` | `P1` | 增加 audit event 基础能力 | `atlas-api`,`atlas-app` | `todo` | `TBD` | `P0-15,P1-05` | 手工导入、敏感 explain、责任修正可审计 |
| `P1-09` | `P1` | 补本地启动与样例导入文档 | `doc` | `todo` | `TBD` | `P0-18,P0-19` | 新开发者可按文档跑起最小闭环 |

---

## 5. P2：扩展与优化任务

这些任务适合在第一版最小闭环稳定后推进。

| ID | Priority | Task | Crate | Status | Owner | Depends On | Acceptance |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `P2-01` | `P2` | 实现 connector trait 与 sync runner | `atlas-sync` | `todo` | `TBD` | `P0-20,P1-05` | sync job 可独立执行 fetch/resolve/persist 流程 |
| `P2-02` | `P2` | 实现 staged payload contract | `atlas-sync` | `todo` | `TBD` | `P2-01` | 外部 payload 可被记录、重放和追踪 |
| `P2-03` | `P2` | 实现 CMDB connector 最小版 | `atlas-sync` | `todo` | `TBD` | `P2-01,P2-02` | 可同步 host 与默认责任归属 |
| `P2-04` | `P2` | 实现 LDAP/IAM connector 最小版 | `atlas-sync` | `todo` | `TBD` | `P2-01,P2-02` | 可同步 user/team 到 `Subject` |
| `P2-05` | `P2` | 实现 cursor 推进、失败隔离与重试 | `atlas-sync` | `todo` | `TBD` | `P2-03,P2-04` | 同步失败不会推进 cursor，重试保持幂等 |
| `P2-06` | `P2` | 增加 dependency observation 模型 | `atlas-domain`,`atlas-storage` | `todo` | `TBD` | `P1-03` | `DepObs/DepEdge` 有明确存储与查询语义 |
| `P2-07` | `P2` | 增加 dependency explain 查询 | `atlas-api` | `todo` | `TBD` | `P2-06` | 可解释一条依赖为何成立 |
| `P2-08` | `P2` | 增加 software normalization 基础模型 | `atlas-domain`,`atlas-storage` | `todo` | `TBD` | `P0-07` | `software_product/version/artifact` 落地 |
| `P2-09` | `P2` | 增加 vulnerability finding 基础链路 | `atlas-sync`,`atlas-storage`,`atlas-api` | `todo` | `TBD` | `P2-08,P2-01` | 漏洞情报可同步并形成 finding |
| `P2-10` | `P2` | 增加 read-model 物化或缓存策略 | `atlas-storage`,`atlas-api` | `todo` | `TBD` | `P1-02,P2-09` | 热点视图有明确加速方案 |

---

## 6. 推荐分工

如果是 2-4 人小团队，建议按下面方式分工：

### 角色 A：领域与存储

负责：

- `P0-01` 到 `P0-07`
- `P1-01`
- `P1-03`
- `P2-08`

### 角色 B：API 与写路径

负责：

- `P0-08` 到 `P0-17`
- `P0-20`
- `P1-02`
- `P1-04`
- `P1-05`

### 角色 C：应用装配与测试

负责：

- `P0-18`
- `P0-19`
- `P1-06`
- `P1-07`
- `P1-08`
- `P1-09`

### 角色 D：同步与扩展

负责：

- `P2-01` 到 `P2-05`
- `P2-06`
- `P2-09`

---

## 7. 建议看板顺序

建议按以下顺序拉任务进入迭代：

### Sprint 1

- `P0-01`
- `P0-02`
- `P0-03`
- `P0-04`
- `P0-05`

### Sprint 2

- `P0-06`
- `P0-07`
- `P0-08`
- `P0-09`
- `P0-10`

### Sprint 3

- `P0-11`
- `P0-12`
- `P0-13`
- `P0-14`
- `P0-15`
- `P0-20`

### Sprint 4

- `P0-16`
- `P0-17`
- `P0-18`
- `P0-19`

### Sprint 5+

- 按 `P1` 顺序推进
- `P2` 只在 `P0` 和关键 `P1` 稳定后再进入

---

## 8. 当前建议

当前建议固定为：

- backlog 管理先以 `P0` 为主，不要同时拉入过多 `P1/P2`
- 先完成家庭和中小企业早期场景，再进入中型企业对象扩展
- 每个任务必须有明确验收条件，避免“代码写了但能力没闭环”
- 当前已有代码骨架应更多视为 `in_progress`，不要提前记成 `done`
