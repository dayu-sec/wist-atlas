# wist-atlas External Sync 架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版外部同步架构。

目标是固定：

- CMDB / LDAP / IAM / Oncall / 漏洞源如何接入
- 全量校准和增量同步如何分层
- 同步任务如何幂等、隔离和失败恢复
- 同步结果如何进入主对象模型

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`../external-integration/external-input-spec.md`](../external-integration/external-input-spec.md)
- [`../model/host-responsibility-sync-from-external-systems.md`](../model/host-responsibility-sync-from-external-systems.md)
- [`../model/public-vulnerability-source-ingestion.md`](../model/public-vulnerability-source-ingestion.md)

---

## 2. 核心结论

第一版建议把外部同步拆成四段：

- Connector
- Fetch & Stage
- Normalize & Resolve
- Persist & Advance Cursor

一句话说：

- 先把外部数据拉进来
- 再归一
- 再写主库
- 最后推进游标

---

## 3. 外部源分组

第一版建议按语义分成三类：

### 3.1 组织与责任源

- CMDB
- LDAP / IAM / HR
- Oncall

### 3.2 资源与平台源

- cluster metadata
- deployment metadata
- service registry

### 3.3 安全情报源

- OSV
- NVD
- GitHub Security Advisory
- vendor advisory

---

## 4. 同步阶段

### 4.1 Connector

职责：

- 认证
- API 访问
- 分页
- 速率限制

### 4.2 Fetch & Stage

职责：

- 拉取原始载荷
- 写入对象存储或 staging 区
- 记录 fetch metadata

### 4.3 Normalize & Resolve

职责：

- 映射外部 ID 到内部 ID
- 建立 `ExternalIdentityLink`
- 归一为 subject / assignment / software / vulnerability 等对象

### 4.4 Persist & Advance Cursor

职责：

- 幂等 upsert 主对象
- 写 `ExternalSyncCursor`
- 记录同步成功/失败状态

---

## 5. 同步策略

### 5.1 全量校准

适用于：

- 初次导入
- 周期性基线校准
- 结构性对象

例如：

- team / subject
- host group
- namespace / workload 元数据

### 5.2 增量同步

适用于：

- 高频责任变更
- oncall 轮值变更
- 漏洞源更新

### 5.3 回补同步

适用于：

- 游标损坏
- 历史数据回补
- 漏洞源大版本修复

---

## 6. 同步隔离原则

第一版建议：

- 每类源独立 cursor
- 每个 connector 独立失败隔离
- 一个源失败不阻塞其他源
- 同步失败不应污染已有主对象

---

## 7. 幂等与恢复

第一版建议固定：

- 外部对象幂等键基于 `system_type + object_type + external_id`
- 同步任务重试不应产生重复主对象
- cursor 只在持久化成功后推进
- 对象存储中的 staged payload 可用于重放

---

## 8. Sync Service 运行建议

建议 `wist-atlas-sync` 承担：

- connector 调度
- staged payload 管理
- cursor 推进
- 失败告警

不建议：

- 让 API server 直接承担高频外部同步

---

## 9. 当前建议

当前建议固定为：

- 外部同步是独立运行时能力
- 必须先 fetch/stage，再 normalize/persist
- cursor 推进必须晚于主写成功
- 代码层面应把 connector、normalizer、persistor 分模块实现
