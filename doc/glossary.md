# wist-atlas 术语表

本文档用于统一 `wist-atlas` 中的核心术语，避免不同模型文档对同一对象使用不同叫法。

## 1. 总体原则

第一版统一以下用语规则：

- 一个术语只表达一类对象职责
- 目录对象、运行对象、关系对象、治理对象分开命名
- 优先使用稳定、可长期扩展的领域术语
- 不把实现细节词汇直接当领域术语
- 不把边缘上报对象和中心归一对象混为一谈

## 2. 通用术语

### 2.1 Inventory

表示慢变化、稳定目录对象。

例如：

- `HostInventory`
- `PodInventory`
- `ClusterInventory`
- `NamespaceInventory`

统一含义：

- 回答“这个对象是谁”
- 不直接承载高频运行态

### 2.2 Runtime State

表示高频变化的运行态快照。

例如：

- `HostRuntimeState`
- `ProcessRuntimeState`

统一含义：

- 回答“这个对象现在怎么样”
- 不作为目录主对象

### 2.3 Entity

表示逻辑上稳定、独立存在的中心对象。

例如：

- `ServiceEntity`
- `WorkloadEntity`
- `SoftwareEntity`

统一含义：

- 表达逻辑身份
- 不等同某个瞬时实例

### 2.4 Instance

表示逻辑对象的运行实例。

例如：

- `ServiceInstance`

统一含义：

- 回答“这个逻辑对象当前或近期有哪些运行副本”
- 生命周期通常短于逻辑对象，允许漂移、重建和消失
- 不作为稳定目录对象，不承载长期业务身份

### 2.5 Binding

表示逻辑对象与运行对象之间的归属绑定。

例如：

- `ServiceWorkloadBinding`
- `RuntimeBinding`

统一含义：

- 回答“为什么这个运行对象归属于这个逻辑对象”
- 必须保留来源和时间段语义

### 2.6 Assoc

表示对象与网络或基础设施资源的接入关系。

说明：

- `Assoc` 是 `association` 的缩写
- 在本文档里统一表示“接入关系/关联关系”这类关系对象
- 不表示逻辑依赖，也不等同 `Binding`

例如：

- `PodNetAssoc`
- `HostNetAssoc`

统一含义：

- 回答“对象接入了哪个网络段”
- 不表示逻辑依赖

### 2.7 Membership

表示某对象属于某集合或某上层边界。

例如：

- `HostGroupMembership`
- `WorkloadPodMembership`

统一含义：

- 回答“某对象属于哪个组/哪个 workload”
- 支持生效时间段

### 2.8 Endpoint

表示可连接的地址入口。

例如：

- `SvcEp`
- `InstEp`

统一含义：

- 回答“通过什么地址访问”
- 分稳定入口与实例运行地址两类

### 2.9 Dependency

表示逻辑依赖关系。

例如：

- `DepEdge`

统一含义：

- 回答“哪个服务依赖哪个服务”
- 不直接承载原始观测细节

### 2.10 Observation

表示从运行数据中归一出的观测记录。

例如：

- `DepObs`

统一含义：

- 回答“系统基于观测看到了什么关系或现象”
- 不等同原始日志、流量或 trace 明细

### 2.11 Evidence

表示支持某条绑定、依赖或归因结论的证据。

例如：

- `RuntimeBindingEvidence`
- `DepEv`
- `SoftwareEvidence`

统一含义：

- 回答“为什么系统得出这个判断”
- 强调可解释性

## 3. 业务与服务术语

### 3.1 BusinessDomain

表示业务域。

统一含义：

- 一个较高层的业务边界
- 下挂多个系统、服务和运行对象

### 3.2 SystemBoundary

表示系统边界。

统一含义：

- 业务中的一个系统
- 是服务编组和治理边界之一

### 3.3 Subsystem

表示系统内部的子系统。

统一含义：

- 比 system 更细一级的逻辑边界

### 3.4 ServiceEntity

表示逻辑服务。

统一含义：

- 是业务架构中的服务定义
- 不等同 Pod、进程或地址

### 3.5 ServiceInstance

表示服务的短生命周期运行实例。

统一含义：

- 是逻辑服务在运行时的副本，可能随 Pod、容器或进程重建而变化
- 可绑定到 Pod、container、process
- 不等同于稳定服务定义，也不应作为长期资产目录

## 4. 编排与运行术语

### 4.1 ClusterInventory

表示集群级目录对象。

统一含义：

- 一个运行环境边界
- 通常承载多个 namespace 与 workload

### 4.2 NamespaceInventory

表示命名空间边界。

统一含义：

- 编排隔离边界
- 常用于责任、配额、网络策略与权限边界

### 4.3 WorkloadEntity

表示部署工作负载对象。

统一含义：

- 是 `service` 与 `pod` 之间的桥接对象
- 例如 deployment、statefulset、job

### 4.4 PodInventory

表示 Pod 目录对象。

统一含义：

- 是实际运行副本对象
- 不是服务定义，也不是部署定义

### 4.5 ContainerRuntime

表示容器运行对象。

统一含义：

- 是比 Pod 更细一级的运行对象
- 常用于实例绑定和软件归属

### 4.6 ProcessRuntimeState

表示进程运行态对象。

统一含义：

- 是主机级最细粒度运行对象之一
- 常用于软件证据和实例归属

## 5. 网络与连接术语

### 5.1 NetworkDomain

表示较高层的网络边界。

统一含义：

- 一个网络域
- 下挂多个网络段

### 5.2 NetworkSegment

表示具体网络段。

统一含义：

- 一个可挂接 host / pod 的网络段
- 可带 CIDR、网关等属性

### 5.3 SvcEp

表示服务稳定入口地址。

统一含义：

- DNS、VIP、Ingress、LB 地址等

### 5.4 InstEp

表示实例运行地址。

统一含义：

- Pod IP:Port、Host IP:Port、Container IP:Port 等

### 5.5 EpRes

表示地址归一结果。

统一含义：

- 把地址解析回 service 或 instance 的桥接对象

## 6. 软件与安全术语

### 6.1 SoftwareEntity

表示归一后的软件中心对象。

统一含义：

- 是稳定的软件身份对象
- 不直接等于某个包名、进程名或路径
- 第一版落库建议拆成 `SoftwareProduct / SoftwareVersion / SoftwareArtifact`

### 6.1.1 SoftwareProduct

表示软件产品。

统一含义：

- 回答“这是什么软件”
- 不区分具体版本和文件制品

### 6.1.2 SoftwareVersion

表示软件版本。

统一含义：

- 回答“这是该软件的哪个版本”
- 漏洞匹配通常优先落到版本层

### 6.1.3 SoftwareArtifact

表示软件制品。

统一含义：

- 回答“这是哪个具体文件、脚本、包或镜像”
- 可执行文件和脚本应记录内容 `sha256`

### 6.2 SoftwareEvidence

表示指向某个软件对象的证据。

统一含义：

- 例如进程路径、容器镜像、包管理信息、签名信息

### 6.3 Vulnerability Finding

表示软件命中的漏洞结果。

统一含义：

- 是软件与漏洞源匹配后的结果对象
- 不等同原始漏洞情报源条目

### 6.4 SoftwareBug

表示软件缺陷。

统一含义：

- 是产品、版本或制品上的缺陷事实
- 不等同漏洞
- 有安全影响的 BUG 可关联到漏洞 finding

### 6.5 SoftwareBugFinding

表示某个资源、版本或制品命中了某个 BUG。

统一含义：

- 是 BUG 在具体资源或软件对象上的命中事实
- 可通过 evidence 解释为什么命中

### 6.6 Public Vulnerability Source

表示公开漏洞源。

统一含义：

- 例如 OSV、NVD、GitHub Security Advisory
- 是情报输入源，不是最终风险结论

## 7. 责任与治理术语

### 7.1 Subject

表示责任主体。

统一含义：

- user、team、service_account、vendor 等

### 7.2 ResponsibilityAssignment

表示责任分配关系。

统一含义：

- 回答“谁对哪个对象负什么责任”

### 7.3 HostGroup

表示主机组。

统一含义：

- 是批量归属和治理的中间层对象

### 7.4 ExternalIdentityLink

表示外部对象与内部对象的映射关系。

统一含义：

- 用于 CMDB、LDAP、Oncall 等系统同步

### 7.5 ExternalSyncCursor

表示外部同步游标。

统一含义：

- 用于跟踪增量同步进度

## 8. 必须避免的混用

第一版必须避免以下混用：

- 把 `Inventory` 和 `RuntimeState` 混成一个对象
- 把 `ServiceEntity` 和 `ServiceInstance` 混成一个对象
- 把 `WorkloadEntity` 和 `PodInventory` 混成一个对象
- 把 `Endpoint` 和 `Dependency` 混成一个对象
- 把 `Dependency` 和 `Observation` 混成一个对象
- 把 `Evidence` 和最终结论对象混成一个对象
- 把 `SoftwareEntity` 和 `ProcessRuntimeState` 混成一个对象
- 把 `ResponsibilityAssignment` 直接简化成资源对象上的单字段

## 9. 当前建议

当前建议固定为：

- 后续所有模型文档优先引用本术语表口径
- 新术语进入模型文档前，先判断是否已能被现有术语覆盖
- 如果必须新增术语，应在本术语表先补定义，再进入其他文档

## 10. Glossary Sync Source

本节是 `scripts/sync_glossary.py` 的同步源。

使用约束：

- 需要批量同步到其他文档的术语，先更新本表
- 更新后运行 `python3 scripts/sync_glossary.py`
- 其他文档中的 `GLOSSARY_SYNC` 块会按这里的内容自动回填

<!-- GLOSSARY_SYNC_SOURCE:START -->
| Term | 中文名 | English | 中文说明 |
| --- | --- | --- | --- |
| `HostInventory` | 主机目录对象 | Host inventory object | 表示稳定的主机资产目录对象，回答“这台主机是谁”。 |
| `HostRuntimeState` | 主机运行态快照 | Host runtime state snapshot | 表示主机当前运行状态快照，回答“这台主机现在怎么样”。 |
| `PodInventory` | Pod 目录对象 | Pod inventory object | 表示稳定的 Pod 目录对象，是实际运行副本，不是服务定义。 |
| `NetworkDomain` | 网络域对象 | Network domain object | 表示较高层的网络边界，下挂多个网络段。 |
| `NetworkSegment` | 网络段对象 | Network segment object | 表示具体可挂接 host 或 pod 的网络段。 |
| `PodPlacement` | Pod 调度关系 | Pod placement relation | 表示 Pod 在某个时间段调度到哪台主机上。 |
| `PodNetAssoc` | Pod 网络接入关系 | Pod network association | 表示 Pod 接入哪个网络段，以及对应地址和接口信息。 |
| `HostNetAssoc` | 主机网络接入关系 | Host network association | 表示主机接入哪个网络段，以及对应地址和接口信息。 |
| `BusinessDomain` | 业务域对象 | Business domain object | 表示较高层业务边界，下挂多个系统、服务和运行对象。 |
| `SystemBoundary` | 系统边界对象 | System boundary object | 表示业务内部的系统边界，是服务编组和治理边界之一。 |
| `Subsystem` | 子系统对象 | Subsystem object | 表示系统内部更细一级的逻辑边界。 |
| `ServiceEntity` | 逻辑服务对象 | Service entity object | 表示业务架构中的逻辑服务定义，不等同实例、Pod 或地址。 |
| `ServiceInstance` | 服务运行实例 | Service runtime instance | 表示逻辑服务在运行时的短生命周期副本，是业务服务与运行对象之间的桥。 |
| `SvcEp` | 服务稳定入口 | Service endpoint | 表示服务的稳定访问入口，例如 DNS、VIP、Ingress。 |
| `InstEp` | 实例运行地址 | Instance endpoint | 表示实例当前运行地址，例如 Pod IP:Port 或 Host IP:Port。 |
| `DepEdge` | 服务依赖边 | Dependency edge | 表示服务依赖图中的一条边，不直接承载原始观测明细。 |
| `ClusterInventory` | 集群目录对象 | Cluster inventory object | 表示集群级目录对象，是运行环境边界。 |
| `NamespaceInventory` | 命名空间目录对象 | Namespace inventory object | 表示集群内命名空间边界，是隔离和治理边界。 |
| `WorkloadEntity` | 工作负载对象 | Workload entity object | 表示部署工作负载对象，是 `service` 与 `pod` 之间的桥接层。 |
| `WorkloadPodMembership` | 工作负载成员关系 | Workload pod membership relation | 表示某个 Pod 属于哪个 Workload，并保留时间段语义。 |
| `ServiceWorkloadBinding` | 服务与工作负载绑定 | Service workload binding relation | 表示逻辑服务与部署工作负载之间的绑定关系。 |
| `ContainerRuntime` | 容器运行对象 | Container runtime object | 表示容器运行时对象，是比 Pod 更细一级的运行对象。 |
| `RuntimeBinding` | 运行归属绑定 | Runtime binding relation | 表示进程、容器或 Pod 为什么归属于某个服务实例。 |
| `RuntimeBindingEvidence` | 运行归属证据 | Runtime binding evidence | 表示支撑运行归属绑定结论的证据。 |
| `DepObs` | 依赖观测对象 | Dependency observation object | 表示从运行数据归一出的依赖观测摘要。 |
| `DepEv` | 依赖观测证据 | Dependency evidence | 表示支撑依赖观测结论的具体证据。 |
| `EpRes` | 地址归一结果 | Endpoint resolution object | 表示地址如何解析回服务或实例的桥接对象。 |
<!-- GLOSSARY_SYNC_SOURCE:END -->
