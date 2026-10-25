# wist-atlas 存储架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版存储架构。

目标是固定：

- 哪些数据进入 PostgreSQL
- 哪些数据进入对象存储
- 哪些数据适合缓存
- source of truth 与 derived view 如何分层

相关文档：

- [`../glossary.md`](../glossary.md)
- [`project-charter.md`](./project-charter.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)

---

## 2. 核心结论

第一版建议采用三层存储：

- PostgreSQL 作为主存储
- Object Storage 作为原始载荷与快照存储
- Cache 作为可选加速层

一句话说：

- 主对象和关系对象进 PostgreSQL
- 原始输入和大快照进对象存储
- 缓存不作为 source of truth

---

## 3. 存储分层

### 3.1 PostgreSQL

负责保存：

- 目录主表
- 关系边表
- 运行态快照索引与明细
- 同步游标
- 外部映射
- 关键 explain/evidence 摘要

原因：

- 强事务
- 清晰外键
- 适合中心对象图谱的 join 查询
- 适合时间段语义和幂等 upsert

### 3.2 Object Storage

负责保存：

- 原始 ingest payload
- 原始外部同步载荷
- 批量导入快照
- 大型 explain 附件
- 后续可能的导出快照

原因：

- 便宜
- 适合大对象
- 不应让主库承担原始载荷归档责任

### 3.3 Cache

负责：

- 热查询结果缓存
- 常用 graph view 缓存
- explain 视图短期缓存

说明：

- 第一版可暂不引入
- 即使引入，也不作为主数据来源

---

## 4. 数据放置原则

### 4.1 主对象优先进入 PostgreSQL

例如：

- `host_inventory`
- `service_entity`
- `workload_entity`
- `software_entity`
- `subject`

### 4.2 关系对象优先进入 PostgreSQL

例如：

- `runtime_binding`
- `responsibility_assignment`
- `dep_edge`
- `pod_net_assoc`

### 4.3 运行态快照优先进入 PostgreSQL

例如：

- `host_runtime_state`
- `process_runtime_state`
- `container_runtime`

说明：

- 第一版规模可控时，用 PostgreSQL 足够
- 后续如量级明显扩大，再考虑冷热分层或独立时序扩展

### 4.4 原始输入永远不直接作为主查询对象

原始输入应：

- 先归档到对象存储
- 再由 normalize/persist 形成主对象

---

## 5. PostgreSQL 内部再分层

第一版建议在逻辑上分成四类 schema 区域：

- catalog
- runtime
- governance
- sync

例如：

### 5.1 `catalog`

保存：

- business / system / service
- cluster / namespace / workload
- host / pod / network
- software

### 5.2 `runtime`

保存：

- host runtime
- process runtime
- container runtime
- runtime binding
- dependency observation

### 5.3 `governance`

保存：

- responsibility
- host group
- explain / evidence 摘要

### 5.4 `sync`

保存：

- external identity link
- external sync cursor
- import job / sync job 元数据

第一版可以先不真的拆成多个 PostgreSQL schema，但文档和代码上应先有这个分层意识。

---

## 6. Derived View 存储策略

第一版建议把派生视图分两类：

### 6.1 可重算轻视图

例如：

- effective responsibility view
- service impact summary
- host risk summary

可选择：

- 即时查询生成
- 物化表/缓存加速

### 6.2 重查询重聚合视图

例如：

- 业务全景视图
- 依赖 explain 图
- 风险传播视图

建议：

- 允许单独物化
- 允许异步重建

关键原则：

- 派生视图必须可重建
- 派生视图不应成为唯一数据来源

---

## 7. 保留与清理策略

### 7.1 长期保留

- 目录对象
- 关系对象
- 责任对象
- 外部映射

### 7.2 窗口保留

- 高频运行态快照
- 高频观测证据

例如：

- `7-30` 天明细
- 长期只留聚合摘要

### 7.3 原始载荷归档

对象存储中的原始 payload 可按策略：

- 短期全保留
- 中期抽样保留
- 长期只保留关键快照

---

## 8. 第一版不建议的方案

第一版不建议：

- 图数据库作为主存储
- 文档库作为唯一存储
- 全部对象都做事件溯源
- 把缓存当主数据源

原因：

- 复杂度过高
- 不利于先把对象模型和关系模型定稳

---

## 9. 当前建议

当前建议固定为：

- PostgreSQL = source of truth
- Object Storage = raw payload / snapshot archive
- Cache = optional accelerator
- 先把主存储逻辑分层定住，再决定物理层是否进一步拆分
