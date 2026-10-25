# wist-atlas 执行计划

## 1. 文档目的

本文档在 [`development-plan.md`](./development-plan.md) 的基础上，进一步把第一版开发工作细化成可执行任务清单。

目标是明确：

- 每个阶段具体要做哪些任务
- 各任务之间的前置依赖是什么
- 各 crate 应承担什么实现责任
- 每个阶段如何验收
- 小团队如何按周推进

相关文档：

- [`development-plan.md`](./development-plan.md)
- [`../architecture/system-architecture.md`](../architecture/system-architecture.md)
- [`../architecture/dataflow-and-pipeline-architecture.md`](../architecture/dataflow-and-pipeline-architecture.md)
- [`../architecture/network-modeling-analysis.md`](../architecture/network-modeling-analysis.md)
- [`../architecture/scenario-and-scope-model.md`](../architecture/scenario-and-scope-model.md)

---

## 2. 执行策略

第一版建议采用以下执行策略：

- 以单体闭环为目标组织开发
- 以 PostgreSQL 主库和最小 API 为第一里程碑
- 以文件导入为第一等输入路径
- 每阶段只引入少量新对象域，避免同时铺开
- 每个阶段都要求“可运行、可测试、可解释”，而不只是“代码已写”

整体推进顺序固定为：

1. 领域模型与存储
2. 文件写路径闭环
3. 查询闭环
4. 场景扩展
5. 外部同步与派生扩展

---

## 3. 总任务分解

第一版任务建议拆成八个工作包：

- `WP-01` 底层领域模型收敛
- `WP-02` 数据库存储基座
- `WP-03` 文件 Ingest 与 Normalize 主写路径
- `WP-04` Query API 与读模型
- `WP-05` 场景扩展对象
- `WP-06` 应用装配与运行模式
- `WP-07` 可观测性、审计与测试
- `WP-08` 外部同步与扩展能力

各工作包之间的依赖关系：

```text
WP-01 -> WP-02 -> WP-03 -> WP-04
WP-04 -> WP-06 -> WP-07

WP-01/WP-02/WP-03 -> WP-05
WP-03/WP-04 -> WP-08
```

---

## 4. 工作包明细

## 4.1 WP-01：底层领域模型收敛

目标：

- 把第一版稳定底层模型收敛成代码级 contract

主要产出：

- `atlas-domain` 中的稳定领域对象
- 枚举和值对象定义
- ingest / candidate / resolver / read-model 类型定义

任务清单：

1. 建立基础共享类型
- `TenantId`
- `EnvironmentId`
- `ObservedAt`
- `ValidityWindow`
- `Confidence`
- `SourceKind`

2. 补齐底层稳定对象
- `HostInventory`
- `NetworkDomain`
- `NetworkSegment`
- `HostNetAssoc`
- `Subject`
- `ResponsibilityAssignment`

3. 补齐输入与中间层类型
- `IngestEnvelope`
- `HostCandidate`
- `IpEvidenceCandidate`
- `NetworkSegmentCandidate`
- `SubjectCandidate`
- `ResponsibilityAssignmentCandidate`

4. 补齐解释对象
- `ResolutionResult`
- `ResolutionExplain`
- `EvidenceRef`

5. 补齐查询 DTO
- `HostTopologyView`
- `NetworkTopologyView`
- `EffectiveResponsibilityView`

6. 固定文件输入模式
- `snapshot`
- `delta`
- `batch_upsert`

前置依赖：

- 无

验收标准：

- 第一版核心对象在代码中均有明确 struct/enum 定义
- 关键字段含义与文档一致
- 不再使用松散 `String` 表达核心枚举语义
- 文件导入模式已成为显式 contract

---

## 4.2 WP-02：数据库存储基座

目标：

- 建立可承载 source of truth 的 PostgreSQL 存储基座

主要产出：

- migration
- schema
- repository trait
- Postgres repository 实现

任务清单：

1. 建立 migration 管理方式
- 选定 migration 目录结构
- 固定建表、索引、回滚策略

2. 落第一批主表
- `host_inventory`
- `network_domain`
- `network_segment`
- `subject`

3. 落第一批关系与作业表
- `host_net_assoc`
- `responsibility_assignment`
- `ingest_job`

4. 为 evidence / unresolved 预留最小表或占位
- `resolution_result`
- `evidence_ref`

5. 固定关键约束
- 主对象唯一键
- `host_net_assoc` 的有效期约束
- `network_segment` 的地址范围唯一约束策略
- `responsibility_assignment` 的开放区间约束

6. 实现 repository trait
- `HostStore`
- `NetworkStore`
- `GovernanceStore`
- `IngestStore`

7. 实现 Postgres 版本
- upsert
- 按主键查询
- 简单列表
- 按时间窗口查询关系对象

8. 建立存储测试
- migration smoke test
- upsert 幂等测试
- 唯一键冲突测试
- 有效期续期/关闭测试

前置依赖：

- `WP-01`

验收标准：

- 空库可以一键初始化
- 第一批核心对象可稳定 upsert
- 关键唯一约束和时间语义由数据库保障
- 数据库存储可支撑最小家庭场景

---

## 4.3 WP-03：文件 Ingest 与 Normalize 主写路径

目标：

- 把文件输入事实转换为中心主对象与关系对象

主要产出：

- intake 接口
- parser / validator
- candidate extractor
- identity resolver
- materializer

任务清单：

1. 实现 ingest submit 入口
- 接收 `IngestEnvelope`
- 记录 ingest job 元数据
- 保存原始 payload 引用或 inline payload

2. 实现 parser / validator
- schema 校验
- 时间戳规范化
- tenant/environment 校验
- 文件模式校验

3. 实现第一批 candidate extractor
- 主机目录导入 candidate
- IP / 网段清单导入 candidate
- 责任关系导入 candidate

4. 实现第一批 identity resolver
- host resolver
- network segment resolver
- subject resolver

5. 实现 conflict handling
- 强标识命中
- 组合标识匹配
- unresolved / conflicting 状态输出

6. 实现 materializer
- 主机对象写入
- 网络对象写入
- 责任关系写入
- evidence / resolution metadata 写入

7. 实现 explain 基础链路
- 记录命中规则
- 记录关键标识
- 记录冲突候选摘要

8. 建立端到端测试
- 导入主机目录成功
- 导入 IP / 网段清单成功
- 导入责任关系成功
- 重复导入保持幂等
- 仅凭 IP 清单可形成最小网络模型

前置依赖：

- `WP-01`
- `WP-02`

验收标准：

- 至少两条文件输入链路可从提交一路写入主库
- 重放同一输入不产生重复主对象
- resolution 结果有最小 explain 信息
- `snapshot / batch_upsert` 已闭环

---

## 4.4 WP-04：Query API 与读模型

目标：

- 对外提供稳定读接口和最小视图能力

主要产出：

- catalog query
- topology query
- governance query
- explain query

任务清单：

1. 固定 API 分组
- `Catalog API`
- `Topology API`
- `Governance API`
- `Explain API`

2. 实现第一批对象查询
- `GET /hosts`
- `GET /hosts/{id}`
- `GET /networks/segments`
- `GET /networks/segments/{id}`
- `GET /subjects`

3. 实现第一批视图查询
- `GET /topology/hosts/{id}`
- `GET /topology/networks/{id}`
- `GET /governance/effective-responsibility`

4. 实现 explain 基础查询
- `GET /explain/resolution/{object_kind}/{id}`

5. 实现读模型组装层
- 对象查询直读主表
- 视图查询统一走组装器
- explain 查询统一走 evidence 组装器

6. 建立 API 测试
- handler 单测
- repository 集成测试
- API contract 测试
- 简单分页过滤测试

前置依赖：

- `WP-01`
- `WP-02`
- `WP-03`

验收标准：

- 上层调用方可不依赖底表直接查询核心对象
- 视图与 explain 返回结构稳定
- 基础查询具备分页和错误响应规范

---

## 4.5 WP-05：场景扩展对象

目标：

- 在不破坏第一版底层闭环的前提下，扩展到中小企业和中型企业对象

主要产出：

- `business / system / service`
- `EpRes`
- `cluster / namespace / workload / pod`

任务清单：

1. 补业务目录对象
- `BusinessDomain`
- `SystemBoundary`
- `ServiceEntity`

2. 补服务暴露对象
- `EpRes`
- service exposure 查询

3. 补编排对象
- `ClusterInventory`
- `NamespaceInventory`
- `WorkloadEntity`
- `PodInventory`

4. 扩展文件导入模式
- `delta`
- 批量覆盖与局部更新语义

5. 建立扩展场景测试
- 中小企业云 + 办公电脑
- 中型企业云 + IDC + 办公电脑

前置依赖：

- `WP-01`
- `WP-02`
- `WP-03`

验收标准：

- 中型场景所需对象可导入、可查询
- 家庭和中小企业原有闭环不被破坏

---

## 4.6 WP-06：应用装配与运行模式

目标：

- 让系统具备可启动、可配置、可切换角色的运行入口

主要产出：

- 统一配置结构
- 单体模式启动入口
- API / worker / sync 角色装配

任务清单：

1. 设计配置结构
- server 配置
- database 配置
- ingest 配置
- auth 配置

2. 实现 `atlas-app` 启动入口
- 单体模式
- API only 模式占位
- sync only 模式占位

3. 实现依赖装配
- repository 注入
- handler 注入
- query assembler 注入

4. 补基础健康检查
- liveness
- readiness

5. 增加开发环境启动方式
- 本地配置模板
- 最小启动说明

前置依赖：

- `WP-02`
- `WP-03`
- `WP-04`

验收标准：

- 本地可启动单体模式
- P0 不要求独立 worker/sync 角色真正可运行

---

## 4.7 WP-07：可观测性、审计与测试

目标：

- 让系统具备最小排障、审计和质量保障能力

主要产出：

- metrics
- structured logs
- audit events
- 自动化测试基线

任务清单：

1. 固定日志规范
- request log
- ingest pipeline log
- resolution log

2. 固定 metrics
- request latency
- ingest success/failure
- resolution conflict count

3. 固定 audit event
- 手工导入
- 手工责任修正
- 敏感 explain 查询

4. 建立测试分层
- domain 单测
- storage 集成测试
- api 测试
- e2e smoke test

5. 建立示例场景测试
- 家庭场景
- 中小企业早期场景

前置依赖：

- `WP-03`
- `WP-04`
- `WP-06`

验收标准：

- 关键路径具备日志和指标
- 高风险治理动作可审计
- 至少具备一套端到端 smoke test

---

## 4.8 WP-08：外部同步与扩展能力

目标：

- 在最小闭环稳定后，补外部同步与扩展能力

主要产出：

- connector contract
- staged payload 机制
- cursor 推进机制
- 第一批 connector

任务清单：

1. 定义 connector trait
- 认证
- 拉取分页
- 增量 cursor
- 全量校准

2. 定义 staged payload contract
- payload metadata
- payload ref
- fetch timestamp
- replay ability

3. 实现 sync job runner
- 调度
- fetch & stage
- normalize & resolve
- persist & cursor advance

4. 实现第一批 connector
- CMDB connector
- LDAP/IAM connector

5. 实现错误隔离与重试
- 源级失败隔离
- cursor 不前移
- staged payload 可重放

前置依赖：

- `WP-03`
- `WP-04`

验收标准：

- 至少一个组织类源和一个资源类源可完成同步
- cursor 推进和重试语义正确
- 同步失败不会破坏已有主对象

---

## 5. 按 crate 拆分任务

## 5.1 `atlas-domain`

第一批任务：

- 定义 `host + network + responsibility` 领域对象
- 定义枚举和值对象
- 定义 ingest / candidate / resolution / explain 类型
- 定义 read-model DTO

完成标准：

- 其他 crate 不再自行定义领域语义

## 5.2 `atlas-storage`

第一批任务：

- 落 migration
- 实现 repository trait
- 实现 Postgres repository
- 提供查询组装所需基础查询

完成标准：

- 所有主写路径与查询路径都能经由 storage 层访问数据库

## 5.3 `atlas-api`

第一批任务：

- 文件 ingest submit handler
- host query handler
- network view handler
- governance/explain handler

完成标准：

- 最小 API 可用于导入、查询、解释

## 5.4 `atlas-sync`

第一批任务：

- connector trait 占位
- sync runner 占位
- cursor repository 边界

完成标准：

- P0 不要求同步真正可运行

## 5.5 `atlas-app`

第一批任务：

- 配置加载
- 组件装配
- 单体模式启动
- smoke test 入口

完成标准：

- 本地和测试环境可通过统一入口启动

---

## 6. 里程碑与验收

### Milestone 1：存储基座可用

完成条件：

- `WP-01`
- `WP-02`

验收结果：

- 核心对象表已落地
- migration 可运行
- 基础 repository 可用

### Milestone 2：最小文件写路径闭环

完成条件：

- `WP-03`

验收结果：

- 主机目录导入可用
- IP / 网段清单导入可用
- 责任关系导入可用

### Milestone 3：最小查询闭环

完成条件：

- `WP-04`
- `WP-06`
- `WP-07` 部分能力

验收结果：

- 核心对象可查询
- 基础 topology view 可查询
- 服务可启动并本地演示

### Milestone 4：场景扩展可用

完成条件：

- `WP-05`

验收结果：

- 服务与编排对象可导入、可查询
- 中型场景可以建立基础视图

### Milestone 5：同步与治理扩展

完成条件：

- `WP-08`
- `WP-07`

验收结果：

- 第一批 connector 可同步
- cursor 与重试语义稳定

---

## 7. 周计划建议

以下计划按 2-4 人小团队估算。

### Week 1

- 完成 `WP-01`
- 确定 migration 方案
- 开始 `WP-02`

### Week 2

- 完成 `WP-02`
- 建立基础存储测试

### Week 3

- 开始 `WP-03`
- 打通主机目录导入

### Week 4

- 完成 `WP-03`
- 打通 IP / 网段清单与责任关系导入

### Week 5

- 完成 `WP-04`
- 打通最小查询闭环

### Week 6

- 完成 `WP-06`
- 补齐 smoke test 与基础可观测性

### Week 7+

- 按 `WP-05` 和 `WP-08` 顺序推进
