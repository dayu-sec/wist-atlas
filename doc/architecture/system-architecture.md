# wist-atlas 系统架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版系统架构。

目标是固定：

- 逻辑模块如何拆分
- 哪些能力属于主写路径
- 哪些能力属于查询与派生层
- 哪些能力属于外部同步与治理层

相关文档：

- [`../glossary.md`](../glossary.md)
- [`project-charter.md`](./project-charter.md)
- [`unified-model-overview.md`](./unified-model-overview.md)
- [`error-handling-architecture.md`](./error-handling-architecture.md)
- [`../model/unified-topology-schema.md`](../model/unified-topology-schema.md)

---

## 2. 核心结论

第一版建议把 `wist-atlas` 定义为一个中心侧拓扑目录系统，逻辑上拆成六个模块：

- Ingest Gateway
- Normalization Engine
- Catalog Store
- Derived View Builder
- Query API
- External Sync Workers

一句话说：

- 写路径负责把外部事实归一为中心对象
- 读路径负责把中心对象投影成查询与视图

---

## 3. 总体逻辑架构

```text
                    +----------------------+
                    |   External Systems   |
                    | CMDB / LDAP / Oncall |
                    +----------+-----------+
                               |
                               v
                    +----------------------+
                    | External Sync Worker |
                    +----------+-----------+
                               |
                               v
+-------------+      +----------------------+      +----------------------+
| Edge / Other| ---> |    Ingest Gateway    | ---> | Normalization Engine |
| Producers   |      +----------------------+      +----------+-----------+
+-------------+                                            |
                                                           v
                                                +----------------------+
                                                |    Catalog Store     |
                                                | PostgreSQL + Objects |
                                                +----------+-----------+
                                                           |
                                 +-------------------------+-------------------------+
                                 |                                                   |
                                 v                                                   v
                      +----------------------+                           +----------------------+
                      | Derived View Builder |                           |      Query API       |
                      +----------------------+                           +----------------------+
```

---

## 4. 模块划分

### 4.1 `Ingest Gateway`

职责：

- 接收边缘发现、平台上报、批量导入等输入
- 做基础协议校验、鉴权、幂等检查
- 把输入转成内部统一 ingest job

不负责：

- 深度归一化
- 派生视图构建
- 复杂查询

### 4.2 `Normalization Engine`

职责：

- 把外部事实归一成中心对象
- 建立稳定主键和跨源映射
- 生成关系边、绑定关系、责任关系和软件归一结果

它是整套系统里最关键的语义层。

### 4.3 `Catalog Store`

职责：

- 保存 source of truth
- 保存主表、关系表、运行态表和同步游标
- 保存必要的原始载荷引用

第一版建议：

- PostgreSQL 为主
- 对象存储保存原始导入 payload / snapshot

### 4.4 `Derived View Builder`

职责：

- 生成面向查询和 UI 的派生视图
- 生成聚合、摘要和影响路径中间结果
- 负责把底层对象图谱投影成稳定读模型

不负责：

- 作为 source of truth

### 4.5 `Query API`

职责：

- 对外提供统一读接口
- 面向 UI、控制中心、分析系统和后续可视化系统提供查询
- 返回对象视图、关系图、摘要视图和 explain 视图

### 4.6 `External Sync Workers`

职责：

- 对接 CMDB / LDAP / IAM / Oncall / 漏洞源
- 维护 `ExternalIdentityLink` 和 `ExternalSyncCursor`
- 执行全量校准和增量刷新

---

## 5. 运行时分层

第一版建议把运行时拆成四层：

### 5.1 接入层

- HTTP / gRPC / batch import 入口
- 负责接入和幂等校验

### 5.2 归一层

- 负责 identity resolution
- 负责对象归一和关系归一

### 5.3 存储层

- 主库存储
- 对象存储
- 可选缓存

### 5.4 查询层

- 查询 API
- 派生视图
- explain 视图

---

## 6. 第一版进程/服务建议

逻辑上按模块拆，物理上第一版可先收敛成单体或少量进程。

建议两种部署形态：

### 6.1 单体起步

```text
wist-atlas-server
  = ingest gateway
  + normalization engine
  + query api
  + derived view builder
```

配套：

- PostgreSQL
- object storage

### 6.2 小规模拆分

```text
wist-atlas-api
wist-atlas-worker
wist-atlas-sync
```

适用于：

- 外部同步频繁
- 批量导入较重
- 查询和写入压力分离明显

---

## 7. 与外部系统边界

### 7.1 与边缘发现系统

`wist-atlas` 不直接承担边缘 discovery。

它只接收：

- 发现结果
- 运行态快照
- 资源线索
- 软件线索

### 7.2 与控制中心

`wist-atlas` 不负责：

- 审批
- 编译
- 下发
- 执行跟踪

它负责提供：

- 拓扑查询
- 责任查询
- 风险与影响范围查询

### 7.3 与可视化层

2D / 3D 可视化系统不应直接定义领域模型。

它们应消费：

- Query API
- graph view
- derived view

---

## 8. 核心非功能要求

第一版建议固定以下非功能要求：

- source of truth 与 derived view 分离
- 写路径幂等
- 跨源 identity 可追踪
- 关键归因链路可 explain
- 同步失败不污染主目录数据

---

## 9. 当前建议

当前建议固定为：

- 先按六模块完成逻辑拆分
- 第一版物理部署可以单体优先
- 后续是否服务化，取决于 ingest、sync、query 三条路径是否出现明显独立伸缩需求
