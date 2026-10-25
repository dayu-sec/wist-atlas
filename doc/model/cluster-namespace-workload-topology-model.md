# wist-atlas Cluster / Namespace / Workload Topology 子模型设计

## 1. 文档目的

本文档定义 `wist-atlas` 中心侧 `cluster / namespace / workload / pod` 之间的拓扑子模型。

目标是固定：

- Kubernetes 或类似编排环境中的稳定边界对象
- `service` 与 `pod` 之间缺失的编排层如何表达
- `cluster`、`namespace`、`workload` 如何与业务、服务、责任归属衔接
- 这部分如何接到已有 `host / pod / network` 与 `business / system / service` 模型

相关文档：

- [`glossary.md`](../glossary.md)
- [`business-system-service-topology-model.md`](./business-system-service-topology-model.md)
- [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md)
- [`host-responsibility-and-maintainer-model.md`](./host-responsibility-and-maintainer-model.md)
- [`host-inventory-and-runtime-state.md`](./host-inventory-and-runtime-state.md)

---

## 2. 核心结论

第一版固定以下结论：

- `cluster`、`namespace`、`workload`、`pod` 不是同一个对象
- `pod` 不应直接只通过 `service` 或 `host` 表达归属
- `workload` 是 `service` 与 `pod` 之间的关键桥接层
- `namespace` 是资源与责任、隔离、网络策略的重要边界，不应省略
- 这部分属于统一拓扑模型中的编排层子模型，不是新体系

一句话说：

- `Cluster` 回答“运行环境是谁”
- `Namespace` 回答“隔离边界是谁”
- `Workload` 回答“期望运行单元是谁”
- `PodInventory` 回答“实际运行副本是谁”

---

## 3. 为什么必须补这一层

如果把关系直接写成：

```text
ServiceEntity
  -> PodInventory[]
```

或者：

```text
PodInventory {
  service_id
  host_id
}
```

会出现明显问题：

- 一个服务可能由多个 workload 形态承载，例如 `api`、`worker`、`cron`
- 一个 workload 会产生多个 pod，pod 是短生命周期对象
- `namespace` 是天然的治理边界，但如果省略，责任和网络策略都很难落
- `cluster` 是环境边界，跨 cluster 的对象不能只靠名字区分

因此应明确：

- `service` 是逻辑服务定义
- `workload` 是部署与编排定义
- `pod` 是运行副本
- `cluster / namespace` 是编排边界对象

---

## 4. 模型定位

这不是新的顶层模型，而是 `business / system / service` 与 `host / pod / network` 之间的编排桥接子模型。

它主要回答：

- 服务是通过哪些 workload 部署的
- workload 位于哪个 cluster 和 namespace 中
- pod 属于哪个 workload
- workload 与 service、pod、责任关系如何关联

---

## 5. 对象模型

### 5.0 核心术语中英对照

<!-- GLOSSARY_SYNC:START terms=ClusterInventory,NamespaceInventory,WorkloadEntity,PodInventory,WorkloadPodMembership,ServiceWorkloadBinding -->
| 术语 | 中文名 | English | 中文说明 |
| `ClusterInventory` | 集群目录对象 | Cluster inventory object | 表示集群级目录对象，是运行环境边界。 |
| `NamespaceInventory` | 命名空间目录对象 | Namespace inventory object | 表示集群内命名空间边界，是隔离和治理边界。 |
| `WorkloadEntity` | 工作负载对象 | Workload entity object | 表示部署工作负载对象，是 `service` 与 `pod` 之间的桥接层。 |
| `PodInventory` | Pod 目录对象 | Pod inventory object | 表示稳定的 Pod 目录对象，是实际运行副本，不是服务定义。 |
| `WorkloadPodMembership` | 工作负载成员关系 | Workload pod membership relation | 表示某个 Pod 属于哪个 Workload，并保留时间段语义。 |
| `ServiceWorkloadBinding` | 服务与工作负载绑定 | Service workload binding relation | 表示逻辑服务与部署工作负载之间的绑定关系。 |




<!-- GLOSSARY_SYNC:END -->

### 5.1 `ClusterInventory`

表示集群级目录对象。

建议结构：

```text
ClusterInventory {
  cluster_id
  tenant_id
  name
  cluster_type
  region?
  provider?
  external_ref?
  created_at
  updated_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `cluster_id` | 集群主键 | Cluster ID |
| `tenant_id` | 所属租户 | Tenant ID |
| `name` | 集群名称 | Cluster name |
| `cluster_type` | 集群类型 | Cluster type |
| `region` | 区域 | Region |
| `provider` | 提供方 | Provider |
| `external_ref` | 外部系统引用 | External reference |
| `created_at` | 创建时间 | Created time |
| `updated_at` | 更新时间 | Updated time |

`cluster_type` 示例：

- `kubernetes`
- `nomad`
- `mesos`

第一版以 `kubernetes` 为主，但对象命名可保留泛化空间。
如果 `environment_id` 表示应用环境，不建议写入 `ClusterInventory` 主对象。

### 5.2 `NamespaceInventory`

表示集群内的命名空间边界。

建议结构：

```text
NamespaceInventory {
  namespace_id
  cluster_id
  tenant_id
  name
  purpose?
  state?
  created_at
  updated_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `namespace_id` | 命名空间主键 | Namespace ID |
| `cluster_id` | 所属集群 ID | Cluster ID |
| `tenant_id` | 所属租户 | Tenant ID |
| `name` | 命名空间名称 | Namespace name |
| `purpose` | 命名空间用途 | Purpose |
| `state` | 生命周期状态 | State |
| `created_at` | 创建时间 | Created time |
| `updated_at` | 更新时间 | Updated time |

说明：

- `namespace` 是编排隔离、责任归属、网络策略和配额的重要边界
- 不建议把它只做成字符串字段到处透传

### 5.3 `WorkloadEntity`

表示稳定的部署工作负载对象。

建议结构：

```text
WorkloadEntity {
  workload_id
  cluster_id
  namespace_id
  tenant_id
  service_id?
  workload_kind
  name
  desired_replicas?
  state?
  created_at
  updated_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `workload_id` | 工作负载主键 | Workload ID |
| `cluster_id` | 所属集群 ID | Cluster ID |
| `namespace_id` | 所属命名空间 ID | Namespace ID |
| `tenant_id` | 所属租户 | Tenant ID |
| `service_id` | 归属服务 ID | Service ID |
| `workload_kind` | 工作负载类型 | Workload kind |
| `name` | 工作负载名称 | Workload name |
| `desired_replicas` | 期望副本数 | Desired replicas |
| `state` | 生命周期状态 | State |
| `created_at` | 创建时间 | Created time |
| `updated_at` | 更新时间 | Updated time |

`workload_kind` 示例：

- `deployment`
- `statefulset`
- `daemonset`
- `job`
- `cronjob`

说明：

- `workload` 是服务部署形态的核心对象
- 一个 `service` 可以对应多个 workload
- `service_id` 可为空，允许先发现 workload，再做归属绑定

### 5.4 `PodInventory`

沿用已有定义，但在此模型里明确其归属关系。

最关键的是：

- `pod` 不是服务定义
- `pod` 也不是部署定义
- `pod` 是 workload 派生出的实际运行单元

### 5.5 `WorkloadPodMembership`

表示 pod 属于哪个 workload。

建议结构：

```text
WorkloadPodMembership {
  workload_id
  pod_id
  valid_from
  valid_to?
  source
  created_at
  updated_at
}
```

说明：

- 一般一个 pod 同时只属于一个 workload
- 采用独立关系表是为了保留时间段和来源审计

### 5.5.1 `ResourceScopeMembership`

表示某个资源当前属于哪个应用环境或资源集合。

建议结构：

```text
ResourceScopeMembership {
  membership_id
  tenant_id
  target_type
  target_id
  scope_type
  scope_id
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

说明：

- `target_type` 可取 `host`、`pod`、`workload`、`namespace`、`cluster`
- `scope_type` 可取 `app_environment`、`resource_set`
- 这层表达“资源当前实际归属到哪里”

### 5.6 `ServiceWorkloadBinding`

表示逻辑服务与 workload 的绑定关系。

建议结构：

```text
ServiceWorkloadBinding {
  binding_id
  service_id
  workload_id
  binding_role?
  valid_from
  valid_to?
  source
  created_at
  updated_at
}
```

`binding_role` 示例：

- `primary_api`
- `worker`
- `scheduler`
- `batch`

说明：

- 这层关系把业务服务定义和部署工作负载连接起来
- 比直接 `service -> pod` 更稳定，也更适合治理

### 5.6.1 `ServiceResourceSetBinding`

表示某个业务服务需要哪些资源集合。

建议结构：

```text
ServiceResourceSetBinding {
  binding_id
  service_id
  resource_set_id
  binding_role?
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

说明：

- 这层表达“业务需要什么资源”
- 它不等于”某个资源当前实际归属到哪里”

---

**图：集群编排 ER 关系**

```mermaid
erDiagram
  ClusterInventory ||--o{ NamespaceInventory : contains
  NamespaceInventory ||--o{ WorkloadEntity : contains
  WorkloadEntity ||--o{ WorkloadPodMembership : manages
  PodInventory ||--o{ WorkloadPodMembership : belongs-to
  WorkloadEntity ||--o{ ServiceWorkloadBinding : bound-to
  ServiceEntity ||--o{ ServiceWorkloadBinding : bound-to
```

> `ClusterInventory` → `NamespaceInventory` → `WorkloadEntity` 是编排边界链路。`WorkloadPodMembership` 表达 Pod 与 Workload 的动态归属。`ServiceWorkloadBinding` 是逻辑服务与部署工作负载之间的桥。

---

## 6. 关系图谱

第一版建议固定以下关系：

```text
ClusterInventory
  -> NamespaceInventory[]

NamespaceInventory
  -> WorkloadEntity[]
  -> PodInventory[]

ServiceEntity
  -> ServiceWorkloadBinding[]

ServiceWorkloadBinding
  -> WorkloadEntity

WorkloadEntity
  -> WorkloadPodMembership[]

WorkloadPodMembership
  -> PodInventory

PodInventory
  -> PodPlacement[]
  -> PodNetAssoc[]
  -> HostInventory
```

如果再接回业务层，则形成：

```text
BusinessDomain
  -> SystemBoundary
  -> ServiceEntity
  -> WorkloadEntity
  -> PodInventory
  -> HostInventory
```

---

## 7. 与现有模型的衔接

### 7.1 与 `Business / System / Service` 模型

- `ServiceEntity` 是逻辑服务定义
- `WorkloadEntity` 是部署承载定义
- `PodInventory` 是运行副本

所以：

- `service -> workload -> pod`

比：

- `service -> pod`

更合理。

### 7.2 与 `Host / Pod / Network` 模型

- `PodInventory` 继续作为运行对象
- `PodPlacement` 继续表达 pod 与 host 的调度关系
- `PodNetAssoc` 继续表达 pod 与 network 的接入关系

本模型只补：

- `cluster / namespace / workload`

这三层编排边界。

### 7.3 与责任归属模型

责任关系后续应支持：

- `cluster -> responsibility`
- `namespace -> responsibility`
- `workload -> responsibility`
- `service -> responsibility`

其中：

- `namespace` 往往是团队边界
- `workload` 往往是服务维护边界

不要把这类责任直接附在 `pod` 上作为长期归属。

---

## 8. 第一版查询视图建议

### 8.1 Cluster 视图

从 `ClusterInventory` 出发，展示：

- 集群基础信息
- namespace 数量
- workload 数量
- pod 数量
- 关键服务分布

### 8.2 Namespace 视图

从 `NamespaceInventory` 出发，展示：

- 所属 cluster
- namespace 下有哪些 workload
- workload 下有哪些 pod
- 责任团队是谁

### 8.3 Workload 视图

从 `WorkloadEntity` 出发，展示：

- 归属哪个 service
- 位于哪个 cluster / namespace
- 当前 pod 实例数
- pod 分布在哪些 host 上
- 依赖的软件与漏洞摘要

---

## 9. PostgreSQL 存储建议

第一版建议增加以下主表与关系表：

- `cluster_inventory`
- `namespace_inventory`
- `workload_entity`
- `workload_pod_membership`
- `service_workload_binding`

关键约束建议：

- `namespace_inventory(cluster_id, name)` 唯一
- `workload_entity(namespace_id, workload_kind, name)` 唯一
- `workload_pod_membership(pod_id, valid_to)` 应保证同一时刻只有一个有效 workload 归属

---

## 10. 第一版最小落地范围

当前建议固定为：

- 先支持 `ClusterInventory`
- 先支持 `NamespaceInventory`
- 先支持 `WorkloadEntity`
- 先支持 `ServiceWorkloadBinding`
- 先支持 `WorkloadPodMembership`

第一版先不要一开始做得过重：

- 不必先做所有 Kubernetes 资源对象
- 不必先做完整 CRD 生态建模
- 不必先做所有 namespace policy / quota / RBAC 细表

先把：

- 服务如何映射到 workload
- workload 如何映射到 pod
- pod 位于哪个 cluster / namespace

三件事固定住。

---

## 11. 当前建议

当前建议固定为：

- `cluster / namespace / workload` 是统一拓扑模型中的必需桥接层
- `workload` 是 `service` 与 `pod` 之间的核心桥梁
- `namespace` 是重要治理边界，不应省略
- 后续 schema、query 和 sync 都应围绕 `service -> workload -> pod -> host` 主线展开
