# wist-atlas Runtime Binding 子模型设计

## 1. 文档目的

本文档定义 `wist-atlas` 中心侧 `process / container / pod / service instance` 之间的绑定子模型。

目标是固定：

- 运行态对象之间的归属链路如何表达
- 一个进程、容器、Pod 如何归属到某个 `ServiceInstance`
- 什么是稳定绑定，什么是推断绑定
- 这部分如何与已有 `service / workload / pod / host` 模型衔接

相关文档：

- [`glossary.md`](../glossary.md)
- [`business-system-service-topology-model.md`](./business-system-service-topology-model.md)
- [`cluster-namespace-workload-topology-model.md`](./cluster-namespace-workload-topology-model.md)
- [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md)
- [`host-process-software-vulnerability-graph.md`](./host-process-software-vulnerability-graph.md)

---

## 2. 核心结论

第一版固定以下结论：

- `process`、`container`、`pod`、`service instance` 不是同一个对象
- `ServiceInstance` 是逻辑服务与底层运行对象之间的统一桥接点
- `ServiceInstance` 不应由 PID 单独决定，PID 变化优先表达为绑定变化
- 绑定关系应独立建模，不应只靠对象字段塞一个 `service_id`
- 绑定应区分“声明性绑定”和“推断性绑定”
- 绑定必须有来源、时间段和置信度语义

一句话说：

- `ServiceInstance` 回答“哪个服务的哪次运行副本会话”
- `RuntimeBinding` 回答“某个 process/container/pod 为什么归到这个实例”

---

## 3. 为什么必须单独建模

如果直接写成：

```text
ProcessRuntimeState {
  service_id
}
```

或者：

```text
PodInventory {
  service_id
}
```

会出现明显问题：

- 同一个 service 下可以有多个运行实例
- 一个 Pod 可能承载多个 container
- 一个服务进程的 PID 会重用和变化，不能直接当服务实例身份
- sidecar、agent、helper 进程不一定属于业务主服务
- 某些绑定只能推断出来，不能当绝对事实
- 绑定关系会变化，需要历史与来源审计

因此应明确：

- `service` 是逻辑定义
- `service instance` 是一次运行副本会话
- `process / container / pod` 是运行对象
- 归属关系通过独立 binding 表达

---

## 4. 模型定位

这不是新的顶层模型，而是运行层中的绑定子模型。

它主要回答：

- 一个 `ServiceInstance` 对应哪些运行对象
- 一个 `process` / `container` / `pod` 当前属于哪个实例
- 绑定是从哪里来的、可靠程度如何

它是以下两组模型之间的桥：

- `business / system / service / workload`
- `host / pod / process / software`

---

## 5. 对象与关系模型

### 5.0 核心术语中英对照

<!-- GLOSSARY_SYNC:START terms=ServiceInstance,ContainerRuntime,RuntimeBinding,RuntimeBindingEvidence -->
| 术语 | 中文名 | English | 中文说明 |
| `ServiceInstance` | 服务运行实例 | Service runtime instance | 表示逻辑服务在运行时的短生命周期副本，是业务服务与运行对象之间的桥。 |
| `ContainerRuntime` | 容器运行对象 | Container runtime object | 表示容器运行时对象，是比 Pod 更细一级的运行对象。 |
| `RuntimeBinding` | 运行归属绑定 | Runtime binding relation | 表示进程、容器或 Pod 为什么归属于某个服务实例。 |
| `RuntimeBindingEvidence` | 运行归属证据 | Runtime binding evidence | 表示支撑运行归属绑定结论的证据。 |




<!-- GLOSSARY_SYNC:END -->

### 5.1 `ServiceInstance`

沿用已有定义，作为统一运行实例对象。

关键点：

- 它不直接等于某个 Pod 或进程
- 它是业务逻辑实例的中心锚点，但生命周期仍然短于 `ServiceEntity`
- PID 变化不必然创建新的 `ServiceInstance`
- 只有确认是新的服务副本会话，才创建新的 `ServiceInstance`
- 进程、容器、Pod 与实例之间的变化通过 `RuntimeBinding` 保留历史

生命周期口径：

- 创建：首次确认某个服务副本会话存在
- 续期：后续观测或绑定证据仍能证明会话连续
- 结束：明确停止、退出，或超过失联 TTL 后仍无观测刷新
- 重建：运行身份线索断裂，例如 pod_uid、container_id、启动指纹发生变化

与 `RuntimeBinding` 的关系：

- `ServiceInstance` 记录副本会话的生命周期
- `RuntimeBinding` 记录底层运行对象在什么时间段归属到该会话
- 一个 `ServiceInstance` 生命周期内可以有多段 `RuntimeBinding`
- PID 变化通常只关闭旧 process binding，再打开新 process binding

### 5.2 `ContainerRuntime`

表示容器运行对象。

建议结构：

```text
ContainerRuntime {
  container_id
  tenant_id
  pod_id?
  host_id
  runtime_type
  runtime_namespace?
  container_name?
  image_ref?
  started_at?
  last_seen_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `container_id` | 容器主键 | Container ID |
| `tenant_id` | 所属租户 | Tenant ID |
| `pod_id` | 所属 Pod ID | Pod ID |
| `host_id` | 所在主机 ID | Host ID |
| `runtime_type` | 运行时类型 | Runtime type |
| `runtime_namespace` | 运行时命名空间 | Runtime namespace |
| `container_name` | 容器名称 | Container name |
| `image_ref` | 镜像引用 | Image reference |
| `started_at` | 启动时间 | Started time |
| `last_seen_at` | 最近观测时间 | Last seen time |

说明：

- `container` 是独立运行对象，不应只作为 `pod` 的附属字段
- 在 Kubernetes 环境中，它通常归属于某个 pod

### 5.3 `RuntimeBinding`

表示某个运行对象绑定到某个 `ServiceInstance`。

建议结构：

```text
RuntimeBinding {
  binding_id
  inst_id
  obj_type
  obj_id
  scope
  confidence
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `binding_id` | 绑定主键 | Binding ID |
| `inst_id` | 归属服务实例 ID | Service instance ID |
| `obj_type` | 运行对象类型 | Runtime object type |
| `obj_id` | 运行对象主键 | Runtime object ID |
| `scope` | 绑定范围/来源类型 | Binding scope |
| `confidence` | 置信度 | Confidence |
| `source` | 来源 | Source |
| `valid_from` | 生效开始时间 | Valid from |
| `valid_to` | 生效结束时间 | Valid to |
| `created_at` | 创建时间 | Created time |
| `updated_at` | 更新时间 | Updated time |

`obj_type` 建议取值：

- `process`
- `container`
- `pod`

`scope` 建议取值：

- `declared`
- `observed`
- `inferred`

`confidence` 建议取值：

- `high`
- `medium`
- `low`

说明：

- 绑定对象统一成一个表，比拆成多张小表更利于统一查询
- 通过 `obj_type + obj_id` 区分对象类型
- `RuntimeBinding` 只证明运行对象归属于某个服务实例，不证明运行程序一定真实可信
- 运行程序是否为可信制品，应通过 `ArtifactVerification` 的 hash、签名、包源或证明结果判断

### 5.4 `RuntimeBindingEvidence`

表示绑定判定依据。

建议结构：

```text
RuntimeBindingEvidence {
  evidence_id
  binding_id
  evidence_type
  evidence_value
  score?
  observed_at?
  created_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `evidence_id` | 证据主键 | Evidence ID |
| `binding_id` | 归属绑定 ID | Binding ID |
| `evidence_type` | 证据类型 | Evidence type |
| `evidence_value` | 证据值 | Evidence value |
| `score` | 证据评分 | Score |
| `observed_at` | 证据观测时间 | Observed time |
| `created_at` | 创建时间 | Created time |

`evidence_type` 示例：

- `k8s_label_match`
- `pod_owner_match`
- `container_name_match`
- `port_signature_match`
- `binary_path_match`
- `env_var_match`
- `manual_override`

说明：

- 绑定不是黑盒结果，应尽量保留可解释依据
- 这样后续才能调试错误归属和误绑定问题

---

**图：运行绑定关系**

```mermaid
erDiagram
  ServiceEntity ||--o{ ServiceInstance : instantiates
  ServiceInstance ||--o{ RuntimeBinding : binds
  RuntimeBinding }o--|| PodInventory : references
  RuntimeBinding }o--|| ContainerRuntime : references
  RuntimeBinding }o--|| ProcessRuntimeState : references
  RuntimeBinding ||--o{ RuntimeBindingEvidence : supported-by
```

> `RuntimeBinding` 是运行对象（Pod/Container/Process）到 `ServiceInstance` 的归属桥。每条绑定携带 `scope`（declared/observed/inferred）、`confidence` 和 `evidence`。`ServiceInstance` 不直接等于 PID 或 Pod。

---

## 6. 主绑定链路

第一版建议固定如下主链路：

### 6.1 Kubernetes 主路径

```text
ServiceEntity
  -> ServiceInstance
  -> RuntimeBinding(type = pod)
  -> PodInventory
  -> PodPlacement
  -> HostInventory
```

然后可选扩展为：

```text
ServiceInstance
  -> RuntimeBinding(type = container)
  -> ContainerRuntime
```

### 6.2 主机进程主路径

```text
ServiceEntity
  -> ServiceInstance
  -> RuntimeBinding(type = process)
  -> ProcessRuntimeState
  -> HostInventory
```

### 6.3 完整细粒度路径

```text
ServiceEntity
  -> ServiceInstance
  -> RuntimeBinding(type = pod)
  -> PodInventory
  -> RuntimeBinding(type = container)
  -> ContainerRuntime
  -> RuntimeBinding(type = process)
  -> ProcessRuntimeState
```

第一版不要求所有路径同时完整实现，但模型上要允许这样展开。

---

## 7. 声明绑定与推断绑定

### 7.1 声明绑定

来源包括：

- deployment / workload 显式 metadata
- service-to-workload 配置
- 手工治理配置
- 平台注册信息

特点：

- 稳定性更高
- 优先级更高

### 7.2 推断绑定

来源包括：

- label / annotation
- container name
- image signature
- port / endpoint 特征
- 可执行路径 / binary name
- telemetry 侧观测特征

特点：

- 适合补全未知归属
- 必须带 `confidence` 和 `evidence`

结论：

- 查询结果可混合两类绑定
- 但系统内部必须能区分它们

---

## 8. 与现有模型的衔接

### 8.1 与 `Business / System / Service` 模型

- `ServiceInstance` 继续承载业务服务的运行实例身份
- 本模型只解决实例和运行对象之间的归属绑定

### 8.2 与 `Cluster / Namespace / Workload` 模型

- `service -> workload -> pod` 仍是主编排路径
- 本模型补 `pod -> service instance` 及细粒度 `container / process` 归属

### 8.3 与 `Host / Pod / Network` 模型

- `PodInventory`、`PodPlacement`、`HostInventory` 继续表达拓扑
- 本模型不替代拓扑关系，只补业务归属关系

### 8.4 与软件和漏洞模型

这层绑定完成后，可以形成：

```text
ProcessRuntimeState
  -> SoftwareEvidence
  -> SoftwareEntity
  -> SoftwareVulnerabilityFinding
  -> ServiceInstance
  -> ServiceEntity
```

这样可以回答：

- 某个漏洞影响了哪个服务实例
- 它是通过哪个进程/容器命中的

---

## 9. 第一版查询视图建议

### 9.1 Service Instance 视图

从 `ServiceInstance` 出发，展示：

- 它归属哪个 `ServiceEntity`
- 绑定了哪些 pod / container / process
- 每条绑定的来源和置信度

### 9.2 Runtime Object 视图

从 `process` / `container` / `pod` 出发，展示：

- 当前归属哪个 `ServiceInstance`
- 是声明绑定还是推断绑定
- 绑定证据是什么

### 9.3 Binding Explain 视图

单独展示：

- 为什么这个运行对象被归到该服务实例
- 证据链和评分是什么
- 是否存在冲突绑定

---

## 10. PostgreSQL 存储建议

第一版建议增加：

- `container_runtime`
- `runtime_binding`
- `runtime_binding_evidence`

关键约束建议：

- `(obj_type, obj_id, valid_to)` 应控制单时刻有效绑定数
- `(inst_id, obj_type, obj_id, valid_from)` 可作为幂等落库键候选

建议索引：

- `runtime_binding(inst_id, valid_to)`
- `runtime_binding(obj_type, obj_id, valid_to)`
- `runtime_binding(scope, confidence)`
- `runtime_binding_evidence(binding_id)`

---

## 11. 第一版最小落地范围

当前建议固定为：

- 先支持 `RuntimeBinding`
- 先支持 `RuntimeBindingEvidence`
- `container_runtime` 若 discovery 还不完整，可先保留 schema 与占位对象

第一版先不要一开始做得过重：

- 不必先做完美自动归属
- 不必先做所有 sidecar / helper 的精确分类
- 不必先做复杂多来源冲突仲裁引擎

先把：

- pod 能否归属到 service instance
- process 能否归属到 service instance
- 绑定是否可解释

三件事固定住。

---

## 12. 当前建议

当前建议固定为：

- `RuntimeBinding` 是统一运行归属模型中的关键桥梁
- `ServiceInstance` 不应直接等于某个 pod/process/container，也不应直接等于 PID
- PID 变化先进入 `RuntimeBinding` 历史，是否新建 `ServiceInstance` 取决于是否形成新的运行副本会话
- 绑定应保留 `scope / confidence / source / evidence`
- 后续 service 查询、漏洞关联、责任影响分析都应依赖这层绑定模型
