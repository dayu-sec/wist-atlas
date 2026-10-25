# wist-atlas Query 与 Read Model 架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版查询架构与读模型架构。

目标是固定：

- 哪些查询直接读 source of truth
- 哪些查询应通过 derived/read model 提供
- explain、graph、summary 这几类读能力如何分层
- Query API 应暴露哪些稳定读模型，而不是直接暴露底表

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`storage-architecture.md`](./storage-architecture.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)

---

## 2. 核心结论

第一版建议把读路径分成三层：

- Object Query
- Derived Read Model
- Explain / Graph Query

一句话说：

- 简单对象查询可以直读主库
- 复杂视图、聚合和全景图应走读模型
- explain 和 graph 查询应明确作为单独读能力

---

## 3. 为什么不能直接暴露底表

如果让 Query API 直接暴露底层表结构，会出现明显问题：

- 底层表偏归一化和写入语义，不适合直接给 UI
- 多表 join 逻辑会泄漏到上层调用方
- explain、聚合和风险传播会变得不稳定
- 一旦 schema 调整，外部接口会被迫一起变

因此必须明确：

- `source of truth` 面向写入和一致性
- `read model` 面向查询和展示

---

## 4. 三层读路径

### 4.1 Object Query

面向：

- 单对象查询
- 简单列表查询
- 轻量过滤查询

例如：

- 查某个 host
- 查某个 service
- 查某个 workload

特点：

- 可直接读 PostgreSQL 主表
- 读放大可控

### 4.2 Derived Read Model

面向：

- 汇总视图
- 全景视图
- 影响范围视图
- effective responsibility 视图

例如：

- 业务全景
- 服务拓扑摘要
- 主机风险摘要

特点：

- 可以是视图、物化表或缓存
- 可异步重建

### 4.3 Explain / Graph Query

面向：

- 依赖解释
- 归属解释
- 风险传播路径
- 图查询

例如：

- 为什么这个进程属于这个 service instance
- 为什么系统判定 A 依赖 B
- 某个漏洞如何影响到业务

特点：

- 查询逻辑重
- 需要 evidence 链和关系遍历
- 不适合简单列表接口混用

---

## 5. 推荐读模型

第一版建议至少固定以下读模型：

### 5.1 `business_overview_view`

回答：

- 一个业务包含哪些系统、服务、实例和风险摘要

### 5.2 `service_topology_view`

回答：

- 一个服务有哪些 workload、instance、endpoint、dependency

### 5.3 `host_topology_view`

回答：

- 一个 host 上有哪些 pod、service、软件和责任归属

### 5.4 `effective_responsibility_view`

回答：

- 一个对象当前最终由谁负责

### 5.5 `software_risk_view`

回答：

- 某个软件当前命中了哪些 finding，影响了哪些对象

### 5.6 `dependency_explain_view`

回答：

- 某条依赖为何成立、来自哪些 evidence

---

## 6. Query API 分层

第一版建议对外把查询分成四类：

### 6.1 Catalog API

提供：

- host / service / workload / pod / software / subject 的对象查询

### 6.2 Topology API

提供：

- host 视图
- service 视图
- business 视图
- network 视图

### 6.3 Governance API

提供：

- responsibility 查询
- external mapping 查询
- ownership explain

### 6.4 Explain API

提供：

- runtime binding explain
- dependency explain
- risk impact explain

---

## 7. 查询一致性原则

第一版建议固定：

- source of truth 查询优先读已提交数据
- derived view 允许轻微延迟
- explain 查询应返回生成时间和证据时间
- graph 查询应返回版本或快照语义

---

## 8. 第一版性能策略

第一版建议：

- 简单查询优先直读主库
- 热摘要视图可物化
- explain 与 graph 查询可分页和限深
- 不在第一版追求通用图查询语言

---

## 9. 当前建议

当前建议固定为：

- Query API 不直接暴露底层 schema
- 读路径必须显式区分 object、derived、explain 三层
- 后续 UI、控制中心和可视化系统都应消费稳定 read model，而不是拼 SQL 风格查询
