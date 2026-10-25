# wist-atlas 服务与部署架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版服务与部署架构。

目标是固定：

- 逻辑模块如何映射到进程/服务
- 单体起步时如何部署
- 后续如何从单体演进到分服务
- 哪些边界可以单独伸缩

相关文档：

- [`../glossary.md`](../glossary.md)
- [`project-charter.md`](./project-charter.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`storage-architecture.md`](./storage-architecture.md)

---

## 2. 核心结论

第一版建议：

- 逻辑上按多模块拆分
- 物理上单体优先
- 在 ingest、sync、query 三条路径上预留独立服务边界

一句话说：

- 先保证边界清楚
- 再根据压力决定是否拆服务

---

## 3. 部署角色

第一版建议定义三类部署角色：

- API Server
- Worker
- Sync Service

### 3.1 API Server

负责：

- ingest 接入
- 查询 API
- 轻量 normalize
- 轻量 derived view 生成

### 3.2 Worker

负责：

- 批量 normalize
- 重视图重建
- 导入任务执行
- explain 重算

### 3.3 Sync Service

负责：

- CMDB / LDAP / IAM / Oncall / 漏洞源同步
- 游标推进
- 全量校准与增量刷新

---

## 4. 第一版推荐部署形态

### 4.1 形态 A：单体起步

```text
wist-atlas-server
```

内含：

- API
- ingest
- normalize
- query
- 部分 worker 能力

配套：

- PostgreSQL
- Object Storage

适用条件：

- 团队小
- 规模尚早
- 先追求模型闭环和实现速度

> **与统一 ingest 逻辑模型的关系**：形态 A 的单体进程中，[dataflow doc](./dataflow-and-pipeline-architecture.md) 描述的 Intake Consumer、Resolver Worker、Materializer 等角色以进程内异步任务形式运行，而非独立服务。单体阶段的输入可以来自文件导入、数据库 job table、进程内 channel，或外部 message queue。protocol registry 和 partition_key 设计在第一版可暂不落地完整版本。当演进到形态 B/C 时，这些内部角色才逐步拆分为独立 worker/sync 进程。

### 4.2 形态 B：单体 + 异步 worker

```text
wist-atlas-server
wist-atlas-worker
```

适用条件：

- 批量导入重
- 派生视图重建较多
- 查询和后台任务已开始互相影响

### 4.3 形态 C：API / Worker / Sync 三分

```text
wist-atlas-api
wist-atlas-worker
wist-atlas-sync
```

适用条件：

- 外部同步频率高
- 需要独立伸缩 sync
- 需要控制对外 API 与内部同步故障隔离

---

## 5. 建议演进路线

第一版建议按以下路线演进：

### Phase 1

- 单体 server
- PostgreSQL
- Object Storage

### Phase 2

- 增加 worker
- 把重 normalize / derive 任务从 API 路径移出

### Phase 3

- 增加独立 sync service
- 把外部系统同步从主 server 路径移出

### Phase 4

- 视压力再决定是否拆 query / graph / explain 专项服务

---

## 6. 服务边界判断标准

建议按以下信号判断是否拆服务：

### 6.1 拆 Worker 的信号

- 派生视图构建拖慢在线请求
- 批量导入或重建任务耗时明显
- explain 计算影响 API 延迟

### 6.2 拆 Sync Service 的信号

- 外部同步调用不稳定
- 同步失败频繁阻塞主服务
- 不同同步源节奏差异明显

### 6.3 拆 Query 专项服务的信号

- 读流量远高于写流量
- 图视图和 explain 查询明显重
- 需要面向多个下游系统提供读接口

---

## 7. 部署拓扑建议

第一版建议如下最小部署：

```text
+---------------------+
| wist-atlas-server|
+----------+----------+
           |
   +-------+--------+
   |                |
   v                v
+--------+   +--------------+
| Postgres|   | Object Store |
+--------+   +--------------+
```

后续扩展：

```text
+------------------+     +------------------+     +------------------+
| atlas-api     |     | topology-worker  |     | atlas-sync    |
+--------+---------+     +---------+--------+     +---------+--------+
         \_____________________|___________________________/
                              |
                              v
                    +------------------+
                    |    PostgreSQL    |
                    +------------------+
                              |
                              v
                    +------------------+
                    |  Object Storage  |
                    +------------------+
```

---

## 8. 运行时要求

第一版建议固定：

- API 路径可水平扩展
- Worker 任务可异步重试
- Sync 路径支持源级隔离
- 派生视图失败不影响主写路径

---

## 9. 当前建议

当前建议固定为：

- 第一版先做单体优先
- 代码结构按未来多服务边界组织
- 部署是否拆分由压力和故障隔离需求驱动，不由预设架构偏好驱动
