# wist-atlas Identity Resolution 架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版 identity resolution 架构。

目标是固定：

- 不同来源对象如何归一为内部稳定主键
- 哪些对象必须做 identity resolution
- identity resolution 在 ingest / sync / normalize 流程中的位置
- 多源冲突、别名和外部映射如何处理

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`external-sync-architecture.md`](./external-sync-architecture.md)
- [`../model/host-responsibility-sync-from-external-systems.md`](../model/host-responsibility-sync-from-external-systems.md)

---

## 2. 核心结论

第一版建议把 identity resolution 定义为中心语义层中的核心能力，而不是某个数据源私有逻辑。

它应负责：

- 外部标识到内部主键的映射
- 多个候选事实的同一性判断
- 别名、历史 ID、来源优先级的统一处理

一句话说：

- 输入先有“候选对象”
- identity resolution 决定“它到底是谁”

---

## 3. 哪些对象必须做 Identity Resolution

第一版建议至少覆盖以下对象：

- `host`
- `service`
- `workload`
- `pod`
- `subject`
- `software`

按风险排序：

### 3.1 高优先级

- `host`
- `service`
- `subject`
- `software`

这些对象一旦归错，会直接污染主图谱。

### 3.2 中优先级

- `workload`
- `pod`

这些对象更多依赖平台元数据和运行时事实，但也需要统一主键。

---

## 4. Resolution Pipeline

第一版建议固定四步：

```text
candidate
  -> identifier extraction
  -> candidate matching
  -> conflict resolution
  -> internal identity assignment
```

### 4.1 Identifier Extraction

从输入中提取可用标识。

例如：

- host name
- machine id
- cloud instance id
- namespace + workload kind + workload name
- email / ldap uid / team id
- purl / cpe / bundle id / signer

### 4.2 Candidate Matching

把提取出的标识与现有对象做候选匹配。

输出：

- `0` 个候选
- `1` 个高置信候选
- `N` 个冲突候选

### 4.3 Conflict Resolution

若存在多个候选，需要按规则裁决。

### 4.4 Internal Identity Assignment

最终生成：

- 复用已有内部主键
- 或创建新的内部主键

同时更新：

- `ExternalIdentityLink`
- alias / history / source metadata

---

## 5. Resolution 规则层次

第一版建议按三层规则处理：

### 5.1 强标识规则

例如：

- `machine_id`
- `cloud_instance_id`
- `pod_uid`
- `external_id`
- `purl`

特点：

- 高置信
- 可直接命中

### 5.2 组合标识规则

例如：

- `cluster + namespace + workload_kind + workload_name`
- `tenant + subject_type + external_ref`
- `publisher + product + version family`

特点：

- 需要多字段组合
- 稳定性中等

### 5.3 弱标识规则

例如：

- display name
- binary name
- container name
- email 前缀

特点：

- 只能作为辅助证据
- 不能单独决定最终 identity

---

## 6. 冲突收敛原则

第一版建议固定：

- 强标识优先于弱标识
- 显式外部映射优先于推断匹配
- 手工修正优先于自动归一
- 历史 identity 不物理删除，应保留映射历史

如果无法稳定决策：

- 进入待审或低置信状态
- 不静默绑定到错误对象

---

## 7. 与 External Sync 的关系

`External Sync` 提供事实来源，`Identity Resolution` 提供统一主键裁决。

两者边界应固定：

- sync 负责拉数据
- identity resolution 负责认人、认主机、认服务、认软件

不建议：

- 每个 connector 各自实现一套 identity 规则
- 用 `wp-reactor` 这类规则计算引擎替代中心 identity resolver

---

## 8. 与规则计算引擎的关系

`wp-reactor v0.1.4` 可用于支持 evidence / observation 到候选结论的计算，但不负责最终 identity resolution。

边界固定为：

- `wp-reactor` 可以计算候选匹配信号、窗口聚合、规则命中、风险因子候选。
- identity resolver 负责把候选信号裁决为内部稳定对象，或将冲突放入 unresolved / review queue。
- 强标识、组合标识、弱标识的优先级和冲突策略仍由 `wist-atlas` 管理。
- `wp-reactor` 输出必须保留 rule id、window、input evidence refs、score / confidence，作为 explain 输入。
- 规则命中不能直接创建或合并 `host`、`service`、`subject`、`software` 等中心主对象。

典型用法：

```text
candidate / evidence / observation
  -> wp-reactor rule/window evaluation
  -> scored match signal / rule hit
  -> identity resolver
  -> internal identity assignment or unresolved
```

错误处理要求：

- `wp-reactor` 错误进入 atlas 集成层时必须保持结构化 source chain。
- 纯 reason 转换使用 `conv_err()`；建立 atlas 语义边界时使用 `source_err(...)`。
- 不允许 `anyhow::Error`、裸 `io::Error` 或字符串错误穿透 identity resolution 热路径。

---

## 9. 数据结构建议

第一版建议围绕以下对象落地：

- `ExternalIdentityLink`
- `alias / alternate key`
- `resolution evidence`
- `resolution status`

如后续需要，可补：

- `identity_merge_log`
- `identity_conflict_queue`

---

## 10. Explain 能力要求

identity resolution 必须支持 explain。

至少应回答：

- 为什么命中这个内部对象
- 用了哪些标识
- 哪条规则生效
- 是否存在竞争候选

---

## 11. 当前建议

当前建议固定为：

- identity resolution 是 `wist-atlas` 的核心语义能力
- 不允许分散在各数据源私有实现里
- 所有关键对象都应先经过 resolution，再进入主对象模型
- `wp-reactor` 只作为规则计算和候选信号生成组件，不替代 resolver / materializer
