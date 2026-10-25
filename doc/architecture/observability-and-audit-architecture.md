# wist-atlas Observability 与 Audit 架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版可观测性与审计架构。

目标是固定：

- 系统自身应观测什么
- 哪些动作必须进入审计
- 如何区分运行观测和治理审计
- 如何支持后续排障与 explain

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`security-and-access-control-architecture.md`](./security-and-access-control-architecture.md)

---

## 2. 核心结论

第一版建议把系统自身信号分成三类：

- Runtime Observability
- Pipeline Health
- Governance Audit

一句话说：

- 系统运行得怎么样，要靠 observability
- 系统做过什么决策和变更，要靠 audit

---

## 3. Runtime Observability

主要关注：

- 服务是否健康
- 延迟是否异常
- 数据流是否阻塞
- 查询是否退化

建议信号：

- request latency
- error rate
- queue depth
- sync lag
- derive lag
- DB latency

---

## 4. Pipeline Health

主要关注：

- intake 是否持续进入
- normalize 是否成功
- persist 是否稳定
- derived view 是否落后

建议指标：

- ingest envelopes per minute
- normalize success / failure count
- identity conflict count
- sync cursor lag
- materialization retry count
- derived rebuild duration

---

## 5. Governance Audit

主要关注：

- 谁触发了什么动作
- 谁修改了什么治理对象
- 谁访问了哪些敏感视图

应审计的动作：

- 手工导入
- 手工修正责任关系
- 手工 identity override
- 同步任务重放
- explain 查询敏感对象

---

## 6. Observability 与 Audit 的边界

必须明确：

- observability 关注系统运行状态
- audit 关注系统行为与治理动作

不要混成一类日志。

例如：

- `sync job failed` 属于 observability
- `user X changed assignment Y` 属于 audit

---

## 7. 建议输出形态

第一版建议：

### 7.1 Metrics

用于：

- 健康监控
- 延迟与吞吐
- 队列和同步滞后

### 7.2 Structured Logs

用于：

- pipeline debug
- resolver explain
- 失败原因定位

### 7.3 Audit Events

用于：

- 治理动作归档
- 合规审计
- 访问追踪

---

## 8. Explain 关联要求

第一版建议：

- identity resolution 失败应有结构化日志
- binding 决策应可追到 evidence
- dependency observation 归因应有 explain 线索
- 审计事件与 explain 查询应可互相关联

---

## 9. 当前建议

当前建议固定为：

- observability、pipeline health、audit 必须分开设计
- 先把关键指标、结构化日志和审计事件三件事定住
- 后续系统排障、合规和 explain 才能真正闭环
