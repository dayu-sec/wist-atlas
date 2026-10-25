# wist-atlas Security 与 Access Control 架构设计

## 1. 文档目的

本文档定义 `wist-atlas` 第一版安全与访问控制架构。

目标是固定：

- 哪些数据属于敏感数据
- Query / Ingest / Sync 三条路径如何鉴权
- 多租户与环境级访问边界如何表达
- 审计与最小权限原则如何落地

相关文档：

- [`../glossary.md`](../glossary.md)
- [`project-charter.md`](./project-charter.md)
- [`system-architecture.md`](./system-architecture.md)
- [`query-and-read-model-architecture.md`](./query-and-read-model-architecture.md)
- [`external-sync-architecture.md`](./external-sync-architecture.md)

---

## 2. 核心结论

第一版建议把安全问题拆成三层：

- Authentication
- Authorization
- Audit

一句话说：

- 先确认“你是谁”
- 再确认“你能看/改什么”
- 最后记录“你做了什么”

---

## 3. 敏感数据分级

第一版建议至少分三类：

### 3.1 普通元数据

例如：

- service name
- workload kind
- cluster name

### 3.2 受限元数据

例如：

- host name
- internal IP
- dependency graph
- effective responsibility

### 3.3 高敏感元数据

例如：

- 外部账号映射
- 原始同步 payload
- explain 证据链中的敏感字段
- 可能暴露基础设施细节的网络观测

---

## 4. 认证边界

第一版建议至少支持三类调用方：

### 4.1 人类用户

例如：

- UI 用户
- 平台操作人员

### 4.2 系统调用方

例如：

- 控制中心
- 分析系统
- 可视化系统

### 4.3 内部后台任务

例如：

- sync worker
- derive worker

统一建议：

- 所有入口都应带 caller identity
- 内部服务调用也不能默认信任

---

## 5. 授权边界

第一版建议按以下维度做授权：

- `tenant`
- `environment`
- `object kind`
- `operation`

### 5.1 读权限

例如：

- 是否可读 host 拓扑
- 是否可读责任归属
- 是否可读漏洞结果

### 5.2 写权限

例如：

- 是否可导入业务目录
- 是否可修改手工责任关系
- 是否可触发重建任务

### 5.3 explain 权限

这类权限应单独考虑。

原因：

- explain 往往暴露更底层的证据和内部细节

---

## 6. 多租户与环境隔离

第一版建议固定：

- 任何主对象都必须带租户边界
- 大多数查询应同时带环境边界
- cross-tenant 查询默认禁止

对于共享读场景：

- 通过显式聚合视图开放
- 不直接暴露底层对象明细

---

## 7. 最小权限原则

第一版建议：

- API server 只拿读写主库所需权限
- Sync service 只拿其所需的 connector secret 和写权限
- Worker 不应拥有多余管理权限
- 对象存储访问按 bucket/path 前缀最小化授权

---

## 8. 审计要求

第一版至少要审计：

- 手工导入
- 手工责任修正
- 外部同步任务触发
- explain 查询访问
- 敏感对象访问

审计记录至少包含：

- actor
- action
- target
- tenant
- time
- result

---

## 9. 当前建议

当前建议固定为：

- 安全控制不是 API 外挂，而是系统架构本身的一部分
- 鉴权、授权、审计必须与 ingest / query / sync 三条路径一起设计
- explain 查询和敏感图视图应单独设权限边界
