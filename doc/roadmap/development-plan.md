# wist-atlas 开发计划

## 1. 文档目的

本文档基于当前 `wist-atlas/doc` 下的架构与模型文档，整理第一版可执行开发计划。

目标是明确：

- 第一版优先实现哪些对象与能力
- 为什么开发顺序应如此安排
- 每个阶段要产出什么
- 当前仓库结构下各模块应如何落地

相关文档：

- [`../README.md`](../README.md)
- [`../architecture/project-charter.md`](../architecture/project-charter.md)
- [`../architecture/system-architecture.md`](../architecture/system-architecture.md)
- [`../architecture/dataflow-and-pipeline-architecture.md`](../architecture/dataflow-and-pipeline-architecture.md)
- [`../architecture/unified-model-overview.md`](../architecture/unified-model-overview.md)
- [`../architecture/network-modeling-analysis.md`](../architecture/network-modeling-analysis.md)
- [`../architecture/scenario-and-scope-model.md`](../architecture/scenario-and-scope-model.md)
- [`execution-plan.md`](./execution-plan.md)
- [`todo-backlog.md`](./todo-backlog.md)

---

## 2. 当前判断

当前仓库处于“文档先行、代码骨架已建立、实现尚未展开”的阶段。

现状特点：

- 架构边界已经比较清楚
- 统一模型方向已经明确
- crate 拆分已经预留
- 代码中已有少量 struct、trait、migration 和 SQL 草案
- 这些代码目前仍主要体现 contract 和骨架，不应误判为闭环能力已完成

因此第一版开发重点不应放在：

- 过早拆分服务
- 过早接入过多外部源
- 过早把对象域一次铺到中大型企业复杂度
- 过早建设复杂图查询能力

而应优先放在：

- 固定稳定的底层统一模型
- 先打通文件输入的最小写路径闭环
- 先支持 `host + 基础网络 + 基础地址 + 最小责任关系`
- 提供最小稳定查询面
- 为后续 sync、derived view、explain 预留边界

---

## 3. 第一版范围建议

### 3.1 第一版优先对象

第一版建议优先实现以下对象域：

- `host`
- `network_domain / network_segment`
- `host_net_assoc`
- `subject / responsibility_assignment`
- `ingest_job`
- 最小 `evidence / candidate / resolution` 中间层 contract

说明：

- 第一版先不把 `cluster / namespace / workload / pod` 作为最小闭环前置对象
- 第一版先不把 `service_instance / runtime_binding / external_sync` 作为 P0 前置对象
- `business / system / service` 保留在模型边界中，但不要求在第一批交付时全部形成主线闭环

### 3.2 第一版优先能力

第一版建议优先实现以下能力：

- PostgreSQL 主库存储
- 文件导入优先的统一 ingest 闭环
- 最小 identity resolution
- 最小 Query API
- 单体模式启动与本地 smoke test

### 3.3 第一版延后能力

以下能力建议放在第一版后半段或下一阶段：

- `cluster / workload / pod` 运行编排域
- `service_instance / runtime_binding`
- 外部同步执行框架与 connector
- 复杂依赖图 explain
- 全量漏洞情报接入
- 风险传播视图
- 通用图查询语言
- 大规模缓存与多级存储优化

---

## 4. 开发总原则

第一版建议固定以下开发原则：

- 先统一模型，再扩展数据源
- 先做 source of truth，再做 derived view
- 先做单体闭环，再决定是否拆服务
- 先文件导入，再做外部同步
- 先保证幂等、一致性与 explain，再追求吞吐
- unresolved candidate 不进入正式关系
- 场景差异主要体现在启用能力和视图层，不体现在底层域模型分叉

一句话说：

- 第一版首先要做“可信、可解释、可收敛”的中心目录，不是做一个大而全的平台。

---

## 5. 分阶段开发计划

在技术阶段之外，第一版还建议补一条场景推进主线，避免默认按最高复杂度场景一次把所有模型铺开。

### 5.0 场景推进主线

建议按以下场景复杂度阶梯推进：

1. 家庭
2. 中小企业（云和办公电脑）
3. 中型企业（云、IDC、办公电脑）
4. 大型企业（多云、多 IDC、多办公区）

这条主线不替代后面的技术 phase，而是约束每个 phase 的范围选择：

- 家庭阶段优先固定 `host + 基础网络 + 基础地址`
- 中小企业阶段补 `service exposure + 基础 EpRes + 基础责任关系`
- 中型企业阶段补 `cluster / workload / pod + 文件批量导入 + 基础 external sync`
- 大型企业阶段再强化 `DepObs / DepEdge / explain / 多来源冲突收敛`

引用约束文档：

- [`../architecture/scenario-and-scope-model.md`](../architecture/scenario-and-scope-model.md)

---

## 5.1 Phase 0：固定实现基线

目标：

- 把文档约束收敛成实现 contract

主要工作：

- 固定第一版对象范围与非目标
- 固定主键、唯一键、有效期与快照时间语义
- 固定 `tenant` 与 `environment` 边界
- 固定 ingest / sync / query / derive 的职责边界
- 固定文件输入模式与最小网络对象范围
- 补充简短 ADR 或 design note

建议先固定的设计决议：

- 内部对象统一使用 `uuid`
- 关系对象统一使用 `valid_from / valid_to`
- 运行态统一使用 `observed_at`
- 文件导入统一支持 `snapshot / delta / batch_upsert`
- sync cursor 只在持久化成功后推进
- unresolved candidate 只保留在 candidate/evidence 层

完成标志：

- 团队对“第一版做什么、不做什么”达成一致
- 数据语义和边界不再频繁变动
- `P0` 范围收敛到家庭和中小企业早期场景

---

## 5.2 Phase 1：打通领域模型与主存储

目标：

- 建立第一版 source of truth 基座

主要工作：

- 扩充 `atlas-domain`
- 落 PostgreSQL schema 与 migration
- 实现 `atlas-storage` 的 repository 接口和 Postgres 版本
- 为主对象、关系对象和运行态对象建立存储测试

建议优先落地的领域对象：

- `HostInventory`
- `NetworkDomain`
- `NetworkSegment`
- `HostNetAssoc`
- `Subject`
- `ResponsibilityAssignment`
- `IngestEnvelope`
- `ResolutionResult`

建议优先固定的数据库能力：

- 主对象唯一约束
- 幂等 upsert
- 关系对象时间段关闭与续期
- 基础分页与过滤查询
- 文件导入作业记录

完成标志：

- 核心底层对象可稳定入库
- 约束、索引和时间语义基本可用
- 存储层可以支撑最小文件导入闭环

---

## 5.3 Phase 2：实现最小写路径闭环

目标：

- 从文件输入事实到主库对象形成最小闭环

主要工作：

- 实现 `IngestEnvelope`
- 实现 parser / validator
- 实现 candidate extractor
- 实现最小 identity resolver
- 实现 materializer

第一批建议支持的输入链路：

- 手工或批量导入主机目录
- 手工或批量导入 IP / 网段清单
- 手工或批量导入责任关系

第一批建议覆盖的 identity resolution：

- `host`
- `network_segment`
- `subject`

规则建议按三层实现：

- 强标识规则
- 组合标识规则
- 弱标识辅助规则

关键要求：

- resolution 失败可降级，但不能静默误归属
- materializer 不得把 unresolved candidate 写成正式关系
- 文件输入必须可重放、可解释、可重复导入
- 至少支持 `snapshot / batch_upsert` 两种文件模式

完成标志：

- 至少两条文件输入链路能从导入一路走到主库
- 核心 resolution 结果可 explain
- 仅凭 IP 清单时，也能形成最小网络模型占位层

---

## 5.4 Phase 3：实现最小 Query API

目标：

- 对外提供稳定读接口，而不是直接暴露底表

主要工作：

- 实现 catalog query
- 实现基础 topology view
- 实现 governance query
- 实现轻量 explain 返回

建议第一批对象查询：

- `host`
- `network_segment`
- `subject`

建议第一批视图：

- `host_topology_view`
- `network_topology_view`
- `effective_responsibility_view`

建议第一批 API 分组：

- `Catalog API`
- `Topology API`
- `Governance API`
- `Explain API`

建议第一版限制：

- 不直接开放底层 schema
- 不提供通用图查询语言
- explain 查询限深、分页

完成标志：

- 上层系统可以直接消费稳定读模型
- 不需要自行拼接底层表关系
- 家庭和中小企业早期场景可用同一底层模型返回不同视图

---

## 5.5 Phase 4：扩展到服务暴露与中型场景基础对象

目标：

- 在不破坏底层稳定模型的前提下，向中小企业和中型企业场景扩展

主要工作：

- 补 `business / system / service`
- 补 `EpRes`
- 补 `cluster / namespace / workload / pod`
- 扩展文件导入模式到 `delta`

第一批建议接入的对象域：

- 业务目录
- 服务暴露
- K8s 基础编排对象

必须固定的扩展原则：

- 仍然走统一 ingest / resolve / materialize 主路径
- 不引入场景专用底层模型
- 复杂对象只在前置底层模型稳定后进入主线

完成标志：

- 中型企业场景需要的核心对象具备可导入、可查询能力
- 家庭与中小企业原有闭环不被破坏

---

## 5.6 Phase 5：外部同步、派生视图与治理扩展

目标：

- 在最小闭环稳定后，补外部同步与复合读能力

主要工作：

- 实现 `atlas-sync` 基础执行框架
- 拆分 connector / fetch-stage / normalize-resolve / persist-cursor
- 落地 staged payload 存储与重放机制
- 构建 `business_overview_view`
- 接入 dependency observation
- 接入 software normalization 与 vulnerability finding
- 增加 metrics、structured logs 与 audit events

第一批建议接入的同步源：

- `CMDB`
- `LDAP / IAM`

必须固定的同步原则：

- 每个源独立 cursor
- cursor 只在持久化成功后推进
- staged payload 可重放
- 一个源失败不污染已有主对象

来源优先级建议固定为：

1. `manual`
2. `cmdb_sync`
3. `oncall_sync`
4. `rule_derived`

完成标志：

- 外部系统数据能幂等进入中心模型
- 同步失败时可隔离、可恢复、可重放
- 具备跨对象域联合查询能力

---

## 6. 按 crate 的落地建议

### 6.1 `atlas-domain`

负责：

- 领域对象定义
- 枚举和值对象
- ingest / candidate / resolver / read model contract

第一批应补齐：

- `host + network + responsibility` 相关 struct
- 查询 DTO
- resolver 输入输出类型
- 文件导入模式类型

### 6.2 `atlas-storage`

负责：

- repository trait
- Postgres 存储实现
- migration 与 schema 管理
- 幂等 upsert 与读查询

第一批应补齐：

- host/network/governance repository
- ingest job repository
- 最小读查询

### 6.3 `atlas-api`

负责：

- Query API
- ingest 接入层
- API DTO 与 handler
- 鉴权与审计接入点

第一批应补齐：

- 文件导入 submit handler
- host query handler
- network view handler
- governance query handler

### 6.4 `atlas-sync`

负责：

- connector 调度
- fetch/stage
- normalize / persist
- cursor 推进

第一批应补齐：

- 仅保留接口边界与最小占位
- 不作为 P0 前置交付

### 6.5 `atlas-app`

负责：

- 进程启动入口
- 配置装配
- server / worker / sync 角色编排

第一版建议：

- 先支持单体模式
- 代码结构上预留 API / Worker / Sync 三种运行角色
- P0 只要求单体模式真正可运行

---

## 7. 建议排期

如果按小团队、单体优先方式推进，建议排期如下：

### 第 1-2 周

- 完成 Phase 0
- 完成 Phase 1

### 第 3-4 周

- 完成 Phase 2

### 第 5 周

- 完成 Phase 3

### 第 6-7 周

- 完成 Phase 4

### 第 8 周以后

- 按优先级推进 Phase 5

说明：

- 该排期适合“先建立最小闭环”的推进方式
- 若第一版人力更少，可把 Phase 4 再向后推，先只做家庭和中小企业早期场景

---

## 8. 当前最优先的三件事

结合当前仓库现状，最应该先做的是：

1. 把 `host + network + responsibility` 的第一版领域模型和 PostgreSQL migration 落下来
2. 打通主机目录与 IP/网段清单两条最小文件 ingest 闭环
3. 提供 `host / network / responsibility` 的最小查询 API 与 smoke test

原因：

- 这三件事完成后，系统才真正拥有可验证的中心目录能力
- 后续服务、编排、同步、derive、explain 都能建立在稳定基座之上

---

## 9. 当前建议

当前建议固定为：

- 第一版以单体优先，不预设拆服务
- PostgreSQL 作为唯一 source of truth 主库
- 文件输入是第一等入口，不是过渡方案
- 开发顺序按“模型与存储 -> 文件写路径 -> 查询 -> 场景扩展 -> 同步与派生扩展”推进
- 任何新增能力都不应绕过统一模型、identity resolution 与 materialization 主路径
