# wist-atlas Host / Pod / Network Topology 子模型设计

## 1. 文档目的

本文档定义 `wist-atlas` 中心侧 `host / pod / network` 之间的拓扑子模型。

目标不是另起一套新体系，而是在现有中心对象模型上补齐：

- 多个 `host` 上的 `pod` 如何表达
- 多个 `pod` 处于同一网络如何表达
- `host` 与 `network`、`pod` 与 `network` 的关系如何统一建模
- 这套关系如何与已有的 `HostInventory`、责任归属、软件与漏洞图谱对接

相关文档：

- [`glossary.md`](../glossary.md)
- [`host-inventory-and-runtime-state.md`](host-inventory-and-runtime-state.md)
- [`host-process-software-vulnerability-graph.md`](host-process-software-vulnerability-graph.md)
- [`host-responsibility-and-maintainer-model.md`](host-responsibility-and-maintainer-model.md)
- [`../external/warp-insight-edge.md`](../external/warp-insight-edge.md)

---

## 2. 核心结论

第一版固定以下结论：

- `host`、`pod`、`network` 都是独立对象，不应互相内嵌
- “多个 host 上的 pod 在同一网络”本质上是多个 attachment 指向同一网络对象
- `network` 不应作为 `host` 或 `pod` 的单字段属性
- `pod` 的调度归属与网络归属必须分开表达
- 这部分属于现有资源目录模型和关系图谱模型的扩展子模型，不是新体系

一句话说：

- `HostInventory` 回答“节点是谁”
- `PodInventory` 回答“运行对象是谁”
- `NetworkSegment` 回答“它们连在哪个网络里”

---

## 3. 为什么不是一个字段

如果把模型写成：

```text
HostInventory {
  pods[]
  network = "10.10.0.0/16"
}
```

或者：

```text
PodInventory {
  host_id
  network = "net-a"
}
```

会出现明显问题：

- 一个 `host` 可连接多个网络段
- 一个 `pod` 可挂多个网络附件，不一定只有一个网络
- `pod` 会迁移，调度到不同 `host`
- 同一网络可被很多 `host` 和很多 `pod` 共享
- `network` 有自己的身份、CIDR、域边界、生命周期

因此应明确：

- `host` 是计算承载对象
- `pod` 是运行对象
- `network` 是独立资源对象
- `attachment` 才是连接关系

---

## 4. 模型定位

这不是新的顶层模型，而是现有中心模型中的一个拓扑子模型。

它同时属于：

### 4.1 资源目录模型的扩展

以下对象都属于资源目录：

- `HostInventory`
- `PodInventory`
- `NetworkDomain`
- `NetworkSegment`

### 4.2 关系图谱模型的扩展

以下对象都属于关系边：

- `PodPlacement`
- `PodNetAssoc`
- `HostNetAssoc`

也就是说：

- 资源对象单独建模
- 动态状态单独建模
- 关系通过 attachment / placement 表达

---

## 5. 对象模型

### 5.0 核心术语中英对照

说明：

- `Assoc` 是 `association` 的缩写
- 在本模型里表示对象与网络段之间的接入关系

<!-- GLOSSARY_SYNC:START terms=HostInventory,PodInventory,NetworkDomain,NetworkSegment,PodPlacement,PodNetAssoc,HostNetAssoc -->
| 术语 | 中文名 | English | 中文说明 |
| `HostInventory` | 主机目录对象 | Host inventory object | 表示稳定的主机资产目录对象，回答“这台主机是谁”。 |
| `PodInventory` | Pod 目录对象 | Pod inventory object | 表示稳定的 Pod 目录对象，是实际运行副本，不是服务定义。 |
| `NetworkDomain` | 网络域对象 | Network domain object | 表示较高层的网络边界，下挂多个网络段。 |
| `NetworkSegment` | 网络段对象 | Network segment object | 表示具体可挂接 host 或 pod 的网络段。 |
| `PodPlacement` | Pod 调度关系 | Pod placement relation | 表示 Pod 在某个时间段调度到哪台主机上。 |
| `PodNetAssoc` | Pod 网络接入关系 | Pod network association | 表示 Pod 接入哪个网络段，以及对应地址和接口信息。 |
| `HostNetAssoc` | 主机网络接入关系 | Host network association | 表示主机接入哪个网络段，以及对应地址和接口信息。 |




<!-- GLOSSARY_SYNC:END -->

### 5.1 `HostInventory`

沿用已有定义，表示主机/节点资产。

回答：

- 这台主机是谁

不直接表达：

- 完整 pod 列表
- 所有网络关系

### 5.2 `PodInventory`

表示稳定的 pod 目录对象。

建议结构：

```text
PodInventory {
  pod_id
  tenant_id
  cluster_id?
  namespace
  workload_id?
  pod_uid
  pod_name
  node_id?
  phase?
  first_seen_at
  last_seen_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `pod_id` | 中心侧为 Pod 分配的稳定主键 | Stable internal pod ID |
| `tenant_id` | Pod 所属租户 | Tenant ID |
| `cluster_id` | Pod 所在集群 ID | Cluster ID |
| `namespace` | Pod 所在命名空间名称 | Namespace name |
| `workload_id` | Pod 归属的工作负载 ID | Workload ID |
| `pod_uid` | Kubernetes 语义下的稳定 Pod UID | Kubernetes pod UID |
| `pod_name` | 当前 Pod 名称 | Pod name |
| `node_id` | 当前或最近一次已知节点 ID | Node/host ID |
| `phase` | Pod 当前阶段 | Pod phase |
| `first_seen_at` | 首次发现时间 | First seen time |
| `last_seen_at` | 最近一次观测时间 | Last seen time |

说明：

- `pod_id` 是中心内部稳定主键
- `pod_uid` 是外部 Kubernetes 语义下的稳定标识候选
- `node_id` 表示当前或最近一次已知调度节点，不应承担完整调度历史
- 应用环境归属不写入 `PodInventory` 主对象，建议通过独立关系表达

### 5.3 `NetworkDomain`

表示更高层的网络边界。

建议结构：

```text
NetworkDomain {
  net_domain_id
  tenant_id
  kind
  name
  external_ref?
  metadata?
  created_at
  updated_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `net_domain_id` | 网络域主键 | Network domain ID |
| `tenant_id` | 所属租户 | Tenant ID |
| `kind` | 网络域类型 | Domain kind |
| `name` | 网络域名称 | Domain name |
| `external_ref` | 外部系统引用 | External reference |
| `metadata` | 扩展元数据 | Metadata |
| `created_at` | 创建时间 | Created time |
| `updated_at` | 更新时间 | Updated time |

`kind` 示例：

- `vpc`
- `vlan_fabric`
- `k8s_cluster_network`
- `cni_fabric`

### 5.4 `NetworkSegment`

表示具体的可连接网络段。

建议结构：

```text
NetworkSegment {
  net_seg_id
  net_domain_id
  segment_type
  name
  cidr?
  gateway_ip?
  metadata?
  created_at
  updated_at
}
```

字段中英说明：

| 字段 | 中文说明 | English |
| --- | --- | --- |
| `net_seg_id` | 网络段主键 | Network segment ID |
| `net_domain_id` | 归属网络域 ID | Network domain ID |
| `segment_type` | 网络段类型 | Segment type |
| `name` | 网络段名称 | Segment name |
| `cidr` | 网络段 CIDR | CIDR |
| `gateway_ip` | 网关地址 | Gateway IP |
| `metadata` | 扩展元数据 | Metadata |
| `created_at` | 创建时间 | Created time |
| `updated_at` | 更新时间 | Updated time |

`segment_type` 示例：

- `subnet`
- `overlay`
- `pod_network`
- `service_network`
- `namespace_network`

### 5.4.1 `NetworkDomain` 与 `NetworkSegment` 的区别

两者的区别在于层级不同：

- `NetworkDomain` 是较高层的网络边界
- `NetworkSegment` 是这个边界下面的具体网络段

可以理解为：

```text
NetworkDomain
  -> NetworkSegment[]
```

例如：

```text
NetworkDomain: vpc-prod-sh
  kind = vpc

NetworkSegment: subnet-app-a
  net_domain_id = vpc-prod-sh
  cidr = 10.20.1.0/24

NetworkSegment: subnet-db-a
  net_domain_id = vpc-prod-sh
  cidr = 10.20.2.0/24
```

这里：

- `vpc-prod-sh` 是一个网络域
- `subnet-app-a`、`subnet-db-a` 是这个网络域下的两个具体网段

所以：

- `net_domain_id` 表示“这段网属于哪个网络边界”
- `net_seg_id` 表示“当前挂接到的是哪一段具体网络”

### 5.4.2 这些 ID 怎么获得

第一版建议统一由中心侧生成稳定内部 ID，外部系统只提供候选事实。

#### `net_domain_id`

通常来自更高层平台对象或归一结果，例如：

- 云平台里的 `VPC` / `VNet`
- Kubernetes / CNI 的 cluster network / fabric
- CMDB 或网络资产系统中的网络域定义
- 手工导入的网络边界配置

中心侧会基于：

- 外部对象 ID
- 名称
- 类型
- 所属租户

做 identity resolution，然后生成或命中内部 `net_domain_id`。

#### `net_seg_id`

通常来自具体可挂接网段信息，例如：

- 云子网 `subnet`
- overlay 网络
- pod network
- service network
- 某个明确 CIDR 网段

中心侧会基于：

- 上层 `network_domain`
- CIDR
- segment 名称
- 网关地址
- 外部引用

做归一后生成或命中内部 `net_seg_id`。

### 5.4.3 为什么 host / pod 不直接连到 `NetworkDomain`

因为 `host` 或 `pod` 真正接入的是具体网段，而不是抽象边界。

所以关系应表达为：

```text
HostNetAssoc
  -> NetworkSegment
  -> NetworkDomain

PodNetAssoc
  -> NetworkSegment
  -> NetworkDomain
```

不要直接写成：

```text
host -> network_domain
pod -> network_domain
```

否则会丢掉：

- 具体接入的是哪段 CIDR
- 是否主网卡 / 主附件
- 同一网络域下挂了哪几个不同 segment

### 5.5 `PodPlacement`

表示 pod 与 host/node 的调度关系。

建议结构：

```text
PodPlacement {
  placement_id
  pod_id
  host_id
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `placement_id` | 调度关系主键 |
| `pod_id` | Pod 主键 |
| `host_id` | 主机主键 |
| `source` | 数据来源 |
| `valid_from` | 生效开始时间 |
| `valid_to` | 生效结束时间 |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

说明：

- 一个 `pod` 在同一时刻应只有一个有效 placement
- 历史迁移通过多段 `valid_from / valid_to` 保存

### 5.5.1 未解析调度事实与已解析关系

`PodPlacement` 也建议采用和网络归属相同的分层：

- 未解析的调度事实
- 已解析的 `PodPlacement`

例如在运行观测里可能先看到：

- `pod_uid?`
- `pod_name?`
- `host_name?`
- `node_name?`
- `observed_at`

但这时未必已经稳定命中 `pod_id` 和 `host_id`。

更合理的表达是：

```text
PodPlacementEvidence {
  evidence_id
  ingest_id?
  pod_uid?
  pod_name?
  host_name?
  node_name?
  source
  observed_at
  metadata?
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `evidence_id` | 证据主键 |
| `ingest_id` | 关联的接入包 ID |
| `pod_uid` | 观测到的 Pod UID |
| `pod_name` | 观测到的 Pod 名称 |
| `host_name` | 观测到的主机名 |
| `node_name` | 观测到的节点名 |
| `source` | 数据来源 |
| `observed_at` | 观测时间 |
| `metadata` | 其他原始事实 |

然后经过：

```text
PodPlacementEvidence
  -> pod / host identity resolution
  -> PodPlacement
```

### 5.5.1.1 `PodPlacementCandidate`

如果 pod 或 host identity resolution 不是同步完成，还可以显式保留候选层：

```text
PodPlacementCandidate {
  candidate_id
  evidence_id
  candidate_pod_id?
  candidate_host_id?
  confidence?
  source
  observed_at
  status
  created_at
  updated_at
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `candidate_id` | 候选主键 |
| `evidence_id` | 来源证据 ID |
| `candidate_pod_id` | 候选 Pod ID |
| `candidate_host_id` | 候选主机 ID |
| `confidence` | 候选置信度 |
| `source` | 数据来源 |
| `observed_at` | 观测时间 |
| `status` | 解析状态，例如 pending / matched / conflict / rejected |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

说明：

- 这层是“候选关系”，不是最终事实
- 第一版可以不长期保留
- 如果需要异步解析、人工审查或 explain，再单独持久化

### 5.5.2 `PodPlacement` 的落库前提

第一版建议固定以下规则：

- `PodPlacement.pod_id` 必须非空
- `PodPlacement.host_id` 必须非空
- 没有完整解析出 `pod_id + host_id` 时，只能进入 candidate / evidence / staging 层

推荐主链路：

```text
scan / probe / runtime evidence
  -> PodPlacementEvidence
  -> PodPlacementCandidate
  -> resolved pod_id + host_id
  -> PodPlacement
```

### 5.6 `PodNetAssoc`

表示 pod 连到哪个网络段。

建议结构：

```text
PodNetAssoc {
  assoc_id
  pod_id
  net_seg_id
  interface_name?
  ip_addr?
  mac_addr?
  is_primary
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `assoc_id` | 接入关系主键 |
| `pod_id` | Pod 主键 |
| `net_seg_id` | 接入的网络段 ID |
| `interface_name` | 网络接口名 |
| `ip_addr` | IP 地址 |
| `mac_addr` | MAC 地址 |
| `is_primary` | 是否主接入关系 |
| `source` | 数据来源 |
| `valid_from` | 生效开始时间 |
| `valid_to` | 生效结束时间 |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

说明：

- 一个 `pod` 可有多个 attachment
- 一个 `network_segment` 可被很多 `pod` 共享
- `source` 可标记 `k8s_api`、`cni_sync`、`runtime_hint`

### 5.6.1 未解析网络事实与已解析关系

`PodNetAssoc` 也建议采用和主机侧相同的分层：

- 未解析的网络事实
- 已解析的 `PodNetAssoc`

例如在运行观测里可能先看到：

- `ip_addr`
- `mac_addr`
- `interface_name?`
- `net_seg_id?`
- `observed_at`

但这时还没有稳定命中 `pod_id`。

更合理的表达是：

```text
PodNetworkEvidence {
  evidence_id
  ingest_id?
  ip_addr
  mac_addr?
  interface_name?
  net_seg_id?
  source
  observed_at
  metadata?
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `evidence_id` | 证据主键 |
| `ingest_id` | 关联的接入包 ID |
| `ip_addr` | 观测到的 IP 地址 |
| `mac_addr` | 观测到的 MAC 地址 |
| `interface_name` | 观测到的接口名 |
| `net_seg_id` | 命中的网络段 ID |
| `source` | 数据来源 |
| `observed_at` | 观测时间 |
| `metadata` | 其他原始事实 |

然后经过：

```text
PodNetworkEvidence
  -> pod identity resolution
  -> PodNetAssoc
```

也就是说：

- `PodNetworkEvidence` 表达“系统看到了什么 Pod 网络事实”
- `PodNetAssoc` 表达“系统已经确认这条事实属于哪个 Pod”

### 5.6.1.1 `PodNetAssocCandidate`

如果 pod identity resolution 不是同步完成，还可以显式保留候选层：

```text
PodNetAssocCandidate {
  candidate_id
  evidence_id
  candidate_pod_id?
  candidate_pod_keys?
  confidence?
  source
  observed_at
  status
  created_at
  updated_at
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `candidate_id` | 候选主键 |
| `evidence_id` | 来源证据 ID |
| `candidate_pod_id` | 候选 Pod ID |
| `candidate_pod_keys` | 候选 Pod 标识线索 |
| `confidence` | 候选置信度 |
| `source` | 数据来源 |
| `observed_at` | 观测时间 |
| `status` | 解析状态，例如 pending / matched / conflict / rejected |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

说明：

- 这层是“候选关系”，不是最终事实
- 第一版可以不长期保留
- 如果需要异步解析、人工审查或 explain，再单独持久化

### 5.6.2 `PodNetAssoc` 的落库前提

第一版建议固定以下规则：

- `PodNetAssoc.pod_id` 必须非空
- 没有 `pod_id` 时，只能进入 candidate / evidence / staging 层
- 只有在 pod identity resolution 成功后，才能 materialize 成正式关系

推荐主链路：

```text
scan / probe / runtime evidence
  -> PodNetworkEvidence
  -> PodNetAssocCandidate
  -> resolved pod_id
  -> PodNetAssoc
```

### 5.7 `HostNetAssoc`

表示 host 连到哪个网络段。

建议结构：

```text
HostNetAssoc {
  assoc_id
  host_id
  net_seg_id
  interface_name?
  ip_addr?
  mac_addr?
  is_primary
  source
  valid_from
  valid_to?
  created_at
  updated_at
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `assoc_id` | 接入关系主键 |
| `host_id` | 主机主键 |
| `net_seg_id` | 接入的网络段 ID |
| `interface_name` | 网卡接口名 |
| `ip_addr` | IP 地址 |
| `mac_addr` | MAC 地址 |
| `is_primary` | 是否主接入关系 |
| `source` | 数据来源 |
| `valid_from` | 生效开始时间 |
| `valid_to` | 生效结束时间 |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

说明：

- 它表达的是 host 自身与网络的关系
- 不应用 `PodPlacement` 替代这个关系

### 5.7.1 未解析网络事实与已解析关系

这里要明确区分两层：

- 未解析的网络事实
- 已解析的 `HostNetAssoc`

如果扫描只拿到了：

- `ip_addr`
- `mac_addr`
- `net_seg_id?`
- `observed_at`

但还没有稳定命中 `host_id`，那么这时还不能直接写正式的
`HostNetAssoc`。

更合理的表达是：

```text
HostNetworkEvidence {
  evidence_id
  ingest_id?
  ip_addr
  mac_addr?
  interface_name?
  net_seg_id?
  source
  observed_at
  metadata?
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `evidence_id` | 证据主键 |
| `ingest_id` | 关联的接入包 ID |
| `ip_addr` | 观测到的 IP 地址 |
| `mac_addr` | 观测到的 MAC 地址 |
| `interface_name` | 观测到的接口名 |
| `net_seg_id` | 命中的网络段 ID |
| `source` | 数据来源 |
| `observed_at` | 观测时间 |
| `metadata` | 其他原始事实 |

然后经过：

```text
HostNetworkEvidence
  -> host identity resolution
  -> HostNetAssoc
```

也就是说：

- `HostNetworkEvidence` 表达“系统看到了什么网络事实”
- `HostNetAssoc` 表达“系统已经确认这条事实属于哪台主机”

### 5.7.1.1 `HostNetAssocCandidate`

如果 identity resolution 不是同步完成，还可以显式保留候选层：

```text
HostNetAssocCandidate {
  candidate_id
  evidence_id
  candidate_host_id?
  candidate_host_keys?
  confidence?
  source
  observed_at
  status
  created_at
  updated_at
}
```

字段中文说明：

| 字段 | 中文说明 |
| --- | --- |
| `candidate_id` | 候选主键 |
| `evidence_id` | 来源证据 ID |
| `candidate_host_id` | 候选主机 ID |
| `candidate_host_keys` | 候选主机标识线索 |
| `confidence` | 候选置信度 |
| `source` | 数据来源 |
| `observed_at` | 观测时间 |
| `status` | 解析状态，例如 pending / matched / conflict / rejected |
| `created_at` | 创建时间 |
| `updated_at` | 更新时间 |

说明：

- 这层是“候选关系”，不是最终事实
- 第一版可以不长期保留
- 如果需要异步解析、人工审查或 explain，再单独持久化

### 5.7.2 `HostNetAssoc` 的落库前提

第一版建议固定以下规则：

- `HostNetAssoc.host_id` 必须非空
- 没有 `host_id` 时，只能进入 candidate / evidence / staging 层
- 只有在 host identity resolution 成功后，才能 materialize 成正式关系

推荐主链路：

```text
scan / probe / runtime evidence
  -> HostNetworkEvidence
  -> HostNetAssocCandidate
  -> resolved host_id
  -> HostNetAssoc
```

这样可以避免：

- 扫描阶段就写入错误主机
- 后续再做困难的 `host_id` 回填修补
- 多来源 IP 事实污染正式关系表

### 5.7.3 三层如何落地

第一版建议这样理解：

- `staging` 层：复用通用 `IngestEnvelope`
- `evidence` 层：建议独立建 `HostNetworkEvidence`
- `candidate` 层：可选持久化，必要时建 `HostNetAssocCandidate`
- `resolved relation` 层：正式落 `HostNetAssoc`

这里第一版不建议再单独造一个新的 `NetworkFactEnvelope`，避免和通用 ingest 模型重复。

也就是：

```text
IngestEnvelope
  -> HostNetworkEvidence
  -> HostNetAssocCandidate   // optional persisted layer
  -> HostNetAssoc
```

---

**图：网络拓扑 ER 关系**

```mermaid
erDiagram
  NetworkDomain ||--o{ NetworkSegment : contains
  NetworkSegment ||--o{ HostNetAssoc : attached-to
  NetworkSegment ||--o{ PodNetAssoc : attached-to
  HostInventory ||--o{ HostNetAssoc : has
  PodInventory ||--o{ PodNetAssoc : has
  HostInventory ||--o{ PodPlacement : schedules
  PodInventory ||--o{ PodPlacement : placed-on
```

> 网络对象（`NetworkDomain` / `NetworkSegment`）是独立资源。`HostNetAssoc` 和 `PodNetAssoc` 是接入关系。`PodPlacement` 表达调度归属。

---

## 6. 关系图谱

第一版建议固定以下关系：

```text
HostInventory
  -> HostNetAssoc[]
  -> PodPlacement[]

PodInventory
  -> PodPlacement[]
  -> PodNetAssoc[]

PodNetAssoc
  -> NetworkSegment

HostNetAssoc
  -> NetworkSegment

NetworkSegment
  -> NetworkDomain
```

若处于 Kubernetes 环境，还可补充：

```text
K8sCluster
  -> K8sNamespace
  -> Workload
  -> PodInventory
```

这样可以同时回答：

- 这个 pod 跑在哪台 host 上
- 这个 pod 连接了哪些网络
- 这台 host 上有哪些 pod 与外部网络或 overlay 发生连接

---

## 7. 调度归属与网络归属必须分开

这是这个模型里最容易混淆的一点。

### 7.1 调度归属

调度归属回答：

- 这个 pod 当前落在哪个 host / node 上

它对应：

- `PodPlacement`

### 7.2 网络归属

网络归属回答：

- 这个 pod 接入了哪个网络段

它对应：

- `PodNetAssoc`

因此：

- `pod -> host` 是 placement 关系
- `pod -> network` 是 attachment 关系

两者不能合并。

---

## 8. 与现有模型的衔接

### 8.1 与 `HostInventory`

- `HostInventory` 继续作为节点/主机目录对象
- 网络和 pod 不直接内嵌进 `HostInventory`

### 8.2 与 `HostRuntimeState`

- `HostRuntimeState` 保存 host 当前资源水位和健康
- 不表达完整拓扑关系

### 8.3 与 `ProcessRuntimeState`

- 进程仍主要挂在 `host`
- 若后续容器/pod 内进程可观测，可增加 `process -> pod` 的可选关联

### 8.4 与软件和漏洞图谱

后续可形成：

```text
PodInventory
  -> SoftwareEvidence[]
  -> SoftwareEntity
  -> SoftwareVulnerabilityFinding[]
```

这样可以回答：

- 某个漏洞软件落在哪些 pod 上
- 这些 pod 分布在哪些 host 上
- 它们位于哪些网络段中

### 8.5 与责任归属模型

责任归属可继续独立建模：

- `host -> responsibility`
- 后续可增加 `cluster / namespace / workload -> responsibility`

不要把责任关系直接并进网络 attachment。

---

## 9. 第一版查询视图建议

### 9.1 主机拓扑视图

从 `HostInventory` 出发，展示：

- 主机基础信息
- 当前 pod 数量
- 连接的网络段
- 这些网络段中的 pod 分布摘要

### 9.2 Pod 拓扑视图

从 `PodInventory` 出发，展示：

- 所属 host / node
- 所属 namespace / workload
- 连接的网络段
- 相关软件与漏洞摘要

### 9.3 网络视图

从 `NetworkSegment` 出发，展示：

- 该网络段挂接的 host
- 该网络段挂接的 pod
- 关联的 cluster / namespace 分布

---

## 10. PostgreSQL 存储建议

第一版建议继续采用 PostgreSQL。

原因：

- 这部分是结构化 inventory + relation 数据
- 需要事务、一致性和 join 查询
- 与现有 `host inventory`、责任关系、软件图谱天然同库更简单

建议至少区分三类表：

- 目录对象表
- 证据 / 候选表
- 最终关系表

第一版建议主表包括：

- `pod_inventory`
- `network_domain`
- `network_segment`
- `pod_placement`
- `pod_net_assoc`
- `host_net_assoc`

若需要保留未解析网络事实，再增加：

- `pod_placement_evidence`
- `pod_network_evidence`
- `host_network_evidence`

若需要异步解析、人工审查或 explain，再增加候选层：

- `pod_placement_candidate`
- `pod_net_assoc_candidate`
- `host_net_assoc_candidate`

建议索引重点：

- `pod_placement(pod_id, valid_to)`
- `pod_placement(host_id, valid_to)`
- `pod_placement_evidence(pod_uid)`
- `pod_placement_evidence(host_name)`
- `pod_placement_candidate(status, observed_at)`
- `pod_net_assoc(pod_id, valid_to)`
- `pod_net_assoc(net_seg_id, valid_to)`
- `host_net_assoc(host_id, valid_to)`
- `host_net_assoc(net_seg_id, valid_to)`
- `pod_network_evidence(ip_addr)`
- `host_network_evidence(ip_addr)`
- `pod_net_assoc_candidate(status, observed_at)`
- `host_net_assoc_candidate(status, observed_at)`

不建议第一版直接采用：

- 图数据库作为主存储
- 仅文档库存完整拓扑
- 把网络接入关系塞进 JSON 字段

---

## 11. V1 推荐最小字段集与落库草案

本节只收敛第一版建议优先落地的四个网络核心对象：

- `NetworkDomain`
- `NetworkSegment`
- `HostNetAssoc`
- `PodNetAssoc`

目标是：

- 先固定最小字段集
- 给实现提供可落库草案
- 不把候选层、证据层和所有扩展属性一次写死

### 11.1 `NetworkDomain`

#### 11.1.1 推荐最小字段集

| 字段名 | 中文名 | 是否必需 | 说明 |
| --- | --- | --- | --- |
| `net_domain_id` | 网络域 ID | 必需 | 内部稳定主键 |
| `tenant_id` | 租户 ID | 必需 | 多租户边界 |
| `kind` | 网络域类型 | 必需 | 例如 `vpc`、`k8s_cluster_network` |
| `name` | 网络域名称 | 必需 | 展示名或规范名 |
| `external_ref` | 外部引用 | 可选 | 云平台、CMDB 或平台侧外部 ID |
| `metadata` | 扩展元数据 | 可选 | 尽量克制使用 |
| `created_at` | 创建时间 | 必需 | 创建时间 |
| `updated_at` | 更新时间 | 必需 | 更新时间 |

建议第一版先固定：

- `net_domain_id`
- `tenant_id`
- `kind`
- `name`
- `created_at`
- `updated_at`

#### 11.1.2 SQL 草案

```sql
create table network_domain (
  net_domain_id uuid primary key,
  tenant_id uuid not null,
  kind text not null,
  name text not null,
  external_ref text,
  metadata jsonb,
  created_at timestamptz not null,
  updated_at timestamptz not null
);

create unique index uq_network_domain_tenant_kind_name
  on network_domain (tenant_id, kind, name);

create index idx_network_domain_tenant_kind
  on network_domain (tenant_id, kind);
```

#### 11.1.3 Rust Struct 草案

```rust
#[derive(Debug, Clone)]
pub struct NetworkDomain {
    pub net_domain_id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub kind: String,
    pub name: String,
    pub external_ref: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

### 11.2 `NetworkSegment`

#### 11.2.1 推荐最小字段集

| 字段名 | 中文名 | 是否必需 | 说明 |
| --- | --- | --- | --- |
| `net_seg_id` | 网络段 ID | 必需 | 内部稳定主键 |
| `net_domain_id` | 网络域 ID | 必需 | 所属网络边界 |
| `segment_type` | 网络段类型 | 必需 | 例如 `subnet`、`pod_network` |
| `name` | 网络段名称 | 必需 | 展示名 |
| `cidr` | 网段 CIDR | 建议必需 | 大多数网络查询依赖该字段 |
| `gateway_ip` | 网关 IP | 可选 | 不是所有来源都有 |
| `external_ref` | 外部引用 | 可选 | 子网/网络段外部 ID |
| `metadata` | 扩展元数据 | 可选 | 先少用 |
| `created_at` | 创建时间 | 必需 | 创建时间 |
| `updated_at` | 更新时间 | 必需 | 更新时间 |

建议第一版先固定：

- `net_seg_id`
- `net_domain_id`
- `segment_type`
- `name`
- `cidr`
- `created_at`
- `updated_at`

#### 11.2.2 SQL 草案

```sql
create table network_segment (
  net_seg_id uuid primary key,
  net_domain_id uuid not null references network_domain(net_domain_id),
  segment_type text not null,
  name text not null,
  cidr cidr not null,
  gateway_ip inet,
  external_ref text,
  metadata jsonb,
  created_at timestamptz not null,
  updated_at timestamptz not null
);

create unique index uq_network_segment_domain_type_name
  on network_segment (net_domain_id, segment_type, name);

create index idx_network_segment_domain
  on network_segment (net_domain_id);

create index idx_network_segment_cidr
  on network_segment using gist (cidr inet_ops);
```

#### 11.2.3 Rust Struct 草案

```rust
#[derive(Debug, Clone)]
pub struct NetworkSegment {
    pub net_seg_id: uuid::Uuid,
    pub net_domain_id: uuid::Uuid,
    pub segment_type: String,
    pub name: String,
    pub cidr: ipnet::IpNet,
    pub gateway_ip: Option<std::net::IpAddr>,
    pub external_ref: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

### 11.3 `HostNetAssoc`

#### 11.3.1 推荐最小字段集

| 字段名 | 中文名 | 是否必需 | 说明 |
| --- | --- | --- | --- |
| `assoc_id` | 接入关系 ID | 必需 | 主键 |
| `host_id` | 主机 ID | 必需 | 接入主体 |
| `net_seg_id` | 网络段 ID | 必需 | 接入目标 |
| `ip_addr` | IP 地址 | 建议必需 | 主机侧查询和地址归因常用 |
| `interface_name` | 接口名 | 可选 | 例如 `eth0` |
| `mac_addr` | MAC 地址 | 可选 | 来源不稳定时可为空 |
| `is_primary` | 是否主接入 | 建议必需 | 默认值可为 `false` |
| `source` | 数据来源 | 必需 | 例如 `cloud_api`、`host_discovery` |
| `valid_from` | 生效开始时间 | 必需 | 时态关系必需 |
| `valid_to` | 生效结束时间 | 可选 | 当前有效时为空 |
| `created_at` | 创建时间 | 必需 | 创建时间 |
| `updated_at` | 更新时间 | 必需 | 更新时间 |

建议第一版先固定：

- `assoc_id`
- `host_id`
- `net_seg_id`
- `ip_addr`
- `source`
- `valid_from`
- `created_at`
- `updated_at`

#### 11.3.2 SQL 草案

```sql
create table host_net_assoc (
  assoc_id uuid primary key,
  host_id uuid not null,
  net_seg_id uuid not null references network_segment(net_seg_id),
  interface_name text,
  ip_addr inet not null,
  mac_addr macaddr,
  is_primary boolean not null default false,
  source text not null,
  valid_from timestamptz not null,
  valid_to timestamptz,
  created_at timestamptz not null,
  updated_at timestamptz not null
);

create index idx_host_net_assoc_host_active
  on host_net_assoc (host_id, valid_to);

create index idx_host_net_assoc_seg_active
  on host_net_assoc (net_seg_id, valid_to);

create index idx_host_net_assoc_ip_active
  on host_net_assoc (ip_addr, valid_to);
```

#### 11.3.3 Rust Struct 草案

```rust
#[derive(Debug, Clone)]
pub struct HostNetAssoc {
    pub assoc_id: uuid::Uuid,
    pub host_id: uuid::Uuid,
    pub net_seg_id: uuid::Uuid,
    pub interface_name: Option<String>,
    pub ip_addr: std::net::IpAddr,
    pub mac_addr: Option<String>,
    pub is_primary: bool,
    pub source: String,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

### 11.4 `PodNetAssoc`

#### 11.4.1 推荐最小字段集

| 字段名 | 中文名 | 是否必需 | 说明 |
| --- | --- | --- | --- |
| `assoc_id` | 接入关系 ID | 必需 | 主键 |
| `pod_id` | Pod ID | 必需 | 接入主体 |
| `net_seg_id` | 网络段 ID | 必需 | 接入目标 |
| `ip_addr` | IP 地址 | 建议必需 | Pod 地址归因常用 |
| `interface_name` | 接口名 | 可选 | 多网卡场景才更重要 |
| `mac_addr` | MAC 地址 | 可选 | CNI 不一定稳定提供 |
| `is_primary` | 是否主接入 | 建议必需 | 多 attachment 时有用 |
| `source` | 数据来源 | 必需 | 例如 `k8s_api`、`cni_sync` |
| `valid_from` | 生效开始时间 | 必需 | 时态关系必需 |
| `valid_to` | 生效结束时间 | 可选 | 当前有效时为空 |
| `created_at` | 创建时间 | 必需 | 创建时间 |
| `updated_at` | 更新时间 | 必需 | 更新时间 |

建议第一版先固定：

- `assoc_id`
- `pod_id`
- `net_seg_id`
- `ip_addr`
- `source`
- `valid_from`
- `created_at`
- `updated_at`

#### 11.4.2 SQL 草案

```sql
create table pod_net_assoc (
  assoc_id uuid primary key,
  pod_id uuid not null,
  net_seg_id uuid not null references network_segment(net_seg_id),
  interface_name text,
  ip_addr inet not null,
  mac_addr macaddr,
  is_primary boolean not null default false,
  source text not null,
  valid_from timestamptz not null,
  valid_to timestamptz,
  created_at timestamptz not null,
  updated_at timestamptz not null
);

create index idx_pod_net_assoc_pod_active
  on pod_net_assoc (pod_id, valid_to);

create index idx_pod_net_assoc_seg_active
  on pod_net_assoc (net_seg_id, valid_to);

create index idx_pod_net_assoc_ip_active
  on pod_net_assoc (ip_addr, valid_to);
```

#### 11.4.3 Rust Struct 草案

```rust
#[derive(Debug, Clone)]
pub struct PodNetAssoc {
    pub assoc_id: uuid::Uuid,
    pub pod_id: uuid::Uuid,
    pub net_seg_id: uuid::Uuid,
    pub interface_name: Option<String>,
    pub ip_addr: std::net::IpAddr,
    pub mac_addr: Option<String>,
    pub is_primary: bool,
    pub source: String,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

### 11.5 本节约束

本节只作为第一版最小对象落地草案，先不在这里固定：

- `PodNetworkEvidence`
- `HostNetworkEvidence`
- `PodNetAssocCandidate`
- `HostNetAssocCandidate`
- 更复杂的唯一约束和排他约束

这些内容可以等写路径、identity resolution 和 explain 需求进一步收敛后再补。

---

## 12. 第一版最小落地范围

当前建议固定为：

- 先支持 `PodInventory`
- 先支持 `NetworkDomain` 与 `NetworkSegment`
- 先支持 `PodPlacement`
- 先支持 `PodNetAssoc`
- `HostNetAssoc` 可与主机网络 inventory 一起推进

如果网络事实来源较杂，第一版还建议：

- 先支持 `PodPlacementEvidence`
- 先支持 `PodNetworkEvidence`
- 先支持 `HostNetworkEvidence`

候选层则作为可选：

- `PodPlacementCandidate`
- `PodNetAssocCandidate`
- `HostNetAssocCandidate`

第一版不要一开始就做得过重：

- 不必先做全量 service mesh 拓扑
- 不必先做细粒度 east-west 实时流图
- 不必先做复杂网络策略推演
- 不必一开始就长期保留所有 candidate 数据

先把：

- pod 是谁
- pod 在哪台 host 上
- unresolved 调度事实如何进入 evidence 层
- resolved 调度关系如何进入 placement 层
- pod 连到哪个网络段
- 哪些 pod 共享同一网络段
- unresolved 网络事实如何进入 evidence 层
- resolved 网络关系如何进入 assoc 层

八件事固定住。

---

## 13. 当前建议

当前建议固定为：

- 这是现有中心模型的拓扑扩展子模型，不是新体系
- `network` 是独立对象，不是 `host` 或 `pod` 的单字段属性
- `placement` 与 `attachment` 必须分开建模
- 最终形成 `host / pod / network / software / responsibility` 的统一关系图谱
