# wist-atlas 网络建模分析

## 1. 文档目的

本文档从分析层回答 `wist-atlas` 为什么需要网络建模、网络应被分成哪些层、哪些对象属于中心模型范围、哪些对象暂不纳入第一版。

本文档不直接固定：

- 具体 PostgreSQL 表结构
- 具体索引与外键
- 具体匹配算法与缓存实现

相关文档：

- [`../glossary.md`](../glossary.md)
- [`system-architecture.md`](./system-architecture.md)
- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)
- [`../model/host-pod-network-topology-model.md`](../model/host-pod-network-topology-model.md)
- [`../model/endpoint-and-dependency-observation-model.md`](../model/endpoint-and-dependency-observation-model.md)
- [`../model/business-system-service-topology-model.md`](../model/business-system-service-topology-model.md)

---

## 2. 核心结论

第一版建议把 `wist-atlas` 的网络建模收敛成四层：

- 网络边界层
- 网络接入层
- 服务暴露层
- 连接观测层

一句话说：

- 网络对象回答“网络边界和网段是什么”
- 接入关系回答“谁接入了哪段网络”
- 服务入口回答“服务通过什么地址暴露”
- 观测层回答“流量或调用实际连到了哪里”

---

## 3. 为什么需要单独做网络建模分析

当前文档已经覆盖了两类内容：

- `host / pod / network` 之间的关系模型
- `endpoint / dependency` 之间的地址解析与依赖观测

但在分析层仍缺少一份统一说明，明确：

- `wist-atlas` 里的“网络”到底指什么
- 网络对象和服务对象的边界在哪里
- 哪些来源是 authoritative inventory，哪些只是 evidence
- 第一版到底做到哪一层，不做到哪一层

如果缺少这一层分析，后续实现很容易出现几类问题：

- 把 `IP`、`DNS`、`VIP`、`subnet`、`dependency` 混成一个概念
- 把控制面对象和运行时观测对象混成一套模型
- 把云网络、Kubernetes 网络、主机网络用不同口径各自建模
- 过早把路由表、安全组、NAT、LB backend 都纳入第一版，范围失控

---

## 4. wist-atlas 中“网络”到底是什么

在 `wist-atlas` 里，“网络”不应只理解为 `IP/CIDR`。

第一版更合理的定义是：

- 网络是承载连接关系的基础设施边界
- 它包含较高层的网络域，也包含可被对象实际挂接的具体网段
- 它既服务资源拓扑，也服务地址解析和依赖观测

因此应把网络相关问题分开看：

### 4.1 网络边界问题

回答：

- 存在哪些网络域
- 每个网络域下面有哪些具体网段
- 这些网段属于哪个租户、环境或平台边界

### 4.2 网络接入问题

回答：

- 某台主机接入了哪些网段
- 某个 Pod 接入了哪些网段
- 接入关系在什么时间段有效

### 4.3 服务暴露问题

回答：

- 某个服务通过哪些稳定入口被访问
- 某个实例当前通过哪些地址运行
- 这些地址面向集群内、VPC 内还是公网

### 4.4 连接观测问题

回答：

- 某次 trace / access log / flow 里出现的地址最终解析到哪里
- 某条连接是否能归因到服务或实例
- 多次观测是否足以形成稳定依赖边

---

## 5. 第一版网络建模范围

第一版建议纳入以下对象类型。

### 5.1 纳入中心对象模型的网络对象

- `NetworkDomain`
- `NetworkSegment`

它们回答：

- 网络边界是谁
- 具体可挂接网段是谁

### 5.2 纳入中心关系模型的网络关系

- `HostNetAssoc`
- `PodNetAssoc`

它们回答：

- 哪个对象接入了哪个网段
- 当前地址、接口和生效时间如何表达

### 5.3 纳入服务连接模型的地址对象

- `SvcEp`
- `InstEp`
- `EpRes`

它们回答：

- 服务从哪里暴露
- 实例当前在哪个地址上运行
- 观测到的地址最终解析到哪里

### 5.4 纳入观测与依赖模型的连接对象

- `DepObs`
- `DepEdge`

它们回答：

- 观测到了什么连接现象
- 哪些现象已收敛成稳定依赖关系

---

## 6. 第一版明确不作为核心主对象的内容

第一版不建议把下面这些网络控制面对象都纳入中心主模型：

- route table
- security group
- ACL
- NAT gateway
- LB backend pool
- 完整 DNS zone / record inventory
- 完整交换机 / 路由器 / 防火墙设备拓扑

原因：

- 这些对象的控制面语义重
- 来源系统差异大
- 如果没有明确查询场景，很容易先做成“大而全网络资产平台”
- 它们并不是 `wist-atlas` 建立服务、实例、依赖和责任图谱的第一版前置条件

这不表示它们永远不做，而是：

- 第一版先把“网络边界、网络接入、服务暴露、连接归因”做闭环
- 后续再按实际场景扩展安全控制面或网络控制面对象

---

## 7. 网络分层语义

第一版建议把网络语义固定成四层。

### 7.1 网络边界层

对象：

- `NetworkDomain`
- `NetworkSegment`

回答：

- 网络边界是什么
- 哪些 segment 属于同一个 domain

典型例子：

- 云环境里的 `VPC / VNet`
- `subnet`
- Kubernetes `pod network`
- Kubernetes `service network`
- overlay network

### 7.2 网络接入层

对象：

- `HostNetAssoc`
- `PodNetAssoc`

回答：

- 主机或 Pod 当前接入了哪个网段
- 使用了哪个地址、哪个接口、在哪个时间段有效

关键边界：

- 这层表达的是 attachment / association
- 不表达服务依赖
- 不表达业务归属

### 7.3 服务暴露层

对象：

- `SvcEp`
- `InstEp`

回答：

- 服务应该通过什么稳定入口访问
- 实例当前通过什么地址响应连接

关键边界：

- `SvcEp` 和 `InstEp` 都是地址对象
- 但它们不等同网络 inventory
- 它们回答的是“如何访问”，不是“接入了哪段网络”

### 7.4 连接观测层

对象：

- `EpRes`
- `DepObs`
- `DepEdge`

回答：

- 一次观测中的地址最终解析成了什么对象
- 多次观测是否说明某两个服务之间存在依赖

关键边界：

- 这层是 evidence / observation / derived relation
- 不应反向替代网络 inventory

---

## 8. authoritative source 与 evidence 的分层

网络建模最容易出错的地方，是把“平台声明对象”和“运行观测线索”混为一谈。

第一版建议固定以下分层。

### 8.1 authoritative inventory

适合作为 authoritative inventory 的来源通常包括：

- 云平台网络 inventory
- Kubernetes / CNI 的声明性网络信息
- CMDB 或网络资产系统中明确维护的网络边界
- 人工维护的网络边界配置

这类来源适合形成：

- `NetworkDomain`
- `NetworkSegment`
- 部分 `SvcEp`

### 8.2 runtime evidence

适合作为 runtime evidence 的来源通常包括：

- 主机网卡发现
- Pod 地址发现
- eBPF / flow / access log
- DNS 解析日志
- 网关访问日志
- trace 中的 endpoint 信息

这类来源适合形成：

- `HostNetAssoc` / `PodNetAssoc` 的证据或候选
- `InstEp`
- `EpRes`
- `DepObs`

### 8.3 分层原则

第一版建议固定：

- 声明性网络边界优先形成 inventory
- 运行态线索优先形成 evidence / association / observation
- 无法稳定确认网络边界时，不应硬写正式 `NetworkSegment`
- 无法稳定确认对象归属时，不应硬写正式 `HostNetAssoc / PodNetAssoc`
- 无法稳定解析地址时，不应直接生成正式 `DepEdge`

---

## 9. 三类网络来源如何统一

第一版至少会面对三类常见网络来源。

### 9.1 云网络来源

例如：

- `VPC / VNet`
- `subnet`
- private/public LB
- 云平台分配的主机地址

这类来源更适合作为：

- 网络边界定义
- 网段定义
- 稳定入口定义

### 9.2 Kubernetes / CNI 来源

例如：

- cluster network
- pod CIDR
- service CIDR
- Pod IP
- Service VIP
- Ingress / Gateway 入口

这类来源同时覆盖：

- 网络边界
- 服务暴露
- 运行实例地址

但要保持分层：

- `pod/service CIDR` 更偏 network inventory
- `Pod IP` 更偏实例地址或网络接入事实
- `Service VIP / Ingress` 更偏服务暴露对象

### 9.3 主机与运行观测来源

例如：

- 主机网卡与地址发现
- 容器 namespace / CNI 线索
- flow / log / trace / dns 观测

这类来源更适合作为：

- 接入事实
- 地址解析证据
- 依赖观测证据

不应直接替代：

- `NetworkDomain`
- `NetworkSegment`
- 平台声明性 `SvcEp`

---

## 10. 数据来源到网络对象的映射矩阵

为了避免“同一份数据既想建 inventory，又想直接生成依赖边”的混乱，第一版建议固定下面这张映射矩阵。

| 数据来源 | 典型数据 | 优先支持的对象 | 对象层次 | 备注 |
| --- | --- | --- | --- | --- |
| 云平台网络 inventory | `VPC / VNet` | `NetworkDomain` | inventory | 适合作为网络边界定义 |
| 云平台网络 inventory | `subnet` | `NetworkSegment` | inventory | 适合作为具体网段定义 |
| 云平台网络 inventory | private/public LB | `SvcEp` | service exposure | 更像稳定入口，不是网络段对象 |
| 云平台地址分配 | 主机私网/公网地址 | `HostNetAssoc` | association | 若来源稳定，也可辅助 `InstEp` |
| Kubernetes / CNI 声明信息 | `cluster network` | `NetworkDomain` | inventory | 集群级网络边界 |
| Kubernetes / CNI 声明信息 | `pod CIDR`、`service CIDR` | `NetworkSegment` | inventory | 更偏网段定义，不应当实例地址 |
| Kubernetes API | `Service VIP` | `SvcEp` | service exposure | 稳定服务入口 |
| Kubernetes API | `Ingress / Gateway` 入口 | `SvcEp` | service exposure | 稳定服务暴露地址 |
| Kubernetes / Runtime | `Pod IP` | `PodNetAssoc`、`InstEp` | association / instance exposure | 既可表达网络接入，也可表达实例地址 |
| 主机发现 | 网卡、接口、IP、MAC | `HostNetAssoc` | association | 主机接入事实来源 |
| 容器 / CNI 发现 | namespace、接口、Pod 地址线索 | `PodNetAssoc` | association | Pod 接入事实来源 |
| 服务配置 | DNS、VIP、LB、endpoint 配置 | `SvcEp` | service exposure | 平台声明性入口优先 |
| 运行监听事实 | `IP:Port`、监听协议 | `InstEp` | instance exposure | 当前实例地址 |
| DNS 日志 | 查询域名、解析结果 | `EpRes`、`DepObs` | resolution / observation | 只靠 DNS 不应直接生成稳定依赖 |
| 网关日志 | host、upstream、route | `EpRes`、`DepObs`、`DepEdge` | resolution / observation / derived relation | 需窗口聚合后再刷新依赖边 |
| access log / trace | caller、callee、endpoint、trace edge | `EpRes`、`DepObs`、`DepEdge` | resolution / observation / derived relation | 单条事件不直接生成最终依赖边 |
| eBPF / flow | 五元组、连接方向、端口 | `EpRes`、`DepObs` | resolution / observation | 先归一和去噪，再决定是否形成 `DepEdge` |
| CMDB / 网络资产系统 | 网络域定义、网络段定义 | `NetworkDomain`、`NetworkSegment` | inventory | 适合提供业务侧或人工治理过的边界 |
| 手工导入 | 网络边界配置、静态入口定义 | `NetworkDomain`、`NetworkSegment`、`SvcEp` | inventory / service exposure | 适合补齐平台侧缺失信息 |

### 10.1 使用这张矩阵的原则

- 一份数据可以支持多个对象，但必须先明确它处在哪一层。
- 同一个 `IP` 既可能进入 `PodNetAssoc`，也可能进入 `InstEp`，但两个对象职责不同。
- 只有声明性平台信息，才优先形成网络 inventory。
- 只有经过地址解析、去噪和窗口聚合的观测数据，才适合推进到 `DepEdge`。

---

## 11. 只有一列 IP 时如何建模

这是第一版实现里一个很常见的弱信号场景：

- 没有云平台网络 inventory
- 没有 Kubernetes / CNI 声明性网络信息
- 没有 host / pod / service / instance 的稳定身份
- 输入里只有一列 `IP` 地址

这种情况下，不应直接把这条数据 materialize 成正式网络对象。

### 11.1 只有 IP 时能回答什么，不能回答什么

单独一个 `IP` 通常只能回答：

- 系统看到了一个地址
- 这个地址在某个时间点出现过
- 它来自某个来源系统或某批数据

单独一个 `IP` 通常还不能稳定回答：

- 它属于哪台主机
- 它属于哪个 Pod
- 它属于哪个服务实例
- 它属于哪个租户或环境
- 它属于哪个正式 `NetworkSegment`
- 它是否足以支撑一条正式依赖边

### 11.2 第一版禁止直接做的事情

如果只有一列 `IP`，第一版不建议直接：

- 创建正式 `NetworkDomain`
- 创建正式 `NetworkSegment`
- 创建正式 `HostNetAssoc`
- 创建正式 `PodNetAssoc`
- 创建正式 `InstEp`
- 创建正式 `DepEdge`

原因：

- 这会把弱信号直接提升成 source-of-truth
- 后续补充 authoritative data 时很难修正
- 容易把噪声地址、短期地址或外部地址污染进正式模型

### 11.3 正确做法：先进入 evidence 层

第一版建议先把只有 `IP` 的输入建成未解析证据，而不是正式对象。

可抽象为：

```text
UnresolvedIpEvidence {
  evidence_id
  tenant_id?
  environment_id?
  ip_addr
  source
  observed_at
  ingest_id?
  net_seg_id?
  host_id?
  pod_id?
  svc_id?
  inst_id?
  confidence?
  status
  metadata?
  created_at
}
```

字段语义：

- `evidence_id`
  证据主键
- `tenant_id`
  若来源能提供租户边界则写入，否则为空
- `environment_id`
  若来源能提供环境边界则写入，否则为空
- `ip_addr`
  观测到的 IP 地址
- `source`
  数据来源，例如 `flow_log`、`dns_log`、`manual_import`
- `observed_at`
  该地址被观测到的时间
- `ingest_id`
  所属接入包或批次
- `net_seg_id`
  若后续通过 CIDR 或其他规则命中网段，可补入
- `host_id / pod_id / svc_id / inst_id`
  若后续解析成功，可补入候选或解析结果
- `confidence`
  当前解析置信度
- `status`
  例如 `unresolved / candidate / resolved / conflict`
- `metadata`
  原始附加信息
- `created_at`
  创建时间

### 11.4 第一版推荐处理链路

第一版建议按下面的顺序处理：

```text
raw ip
  -> UnresolvedIpEvidence
  -> try match known NetworkSegment by CIDR
  -> try match HostNetAssoc / PodNetAssoc / InstEp / SvcEp
  -> unresolved bucket or candidate
  -> only after stable identity resolution, materialize formal relation
```

这条链路表达的原则是：

- 先保留证据
- 再尝试做弱解析
- 解析不出来就停在 unresolved
- 只有命中稳定身份后，才写正式模型

### 11.5 在只有 IP 的情况下，最多能推进到哪里

#### 情况 A：只拿到 IP，除此之外没有任何上下文

最多推进到：

- `UnresolvedIpEvidence`

不能推进到：

- 正式 `NetworkSegment`
- 正式 `HostNetAssoc / PodNetAssoc`
- 正式 `InstEp`
- 正式 `DepEdge`

#### 情况 B：拿到 IP，并且已有已知 CIDR / 网络段定义

最多可推进到：

- `UnresolvedIpEvidence`
- `candidate net_seg_id`
- 在高置信前提下命中 `NetworkSegment`

但仍然不能自动推出：

- 这一定属于哪台主机
- 这一定属于哪个 Pod
- 这一定属于哪个服务实例

#### 情况 C：拿到 IP，并且后续补到了 host / pod / instance 身份线索

这时才可以视情况推进到：

- `HostNetAssoc`
- `PodNetAssoc`
- `InstEp`

若只是“能连通到某个 IP”，但还没有服务身份，则仍不应直接生成 `DepEdge`。

### 11.6 对依赖观测的特殊要求

只有 `IP` 地址时，依赖侧要更保守。

第一版建议固定：

- 只有 IP 连通但无法解析到服务身份时，不直接生成 `DepEdge`
- 无法解析的地址进入 unresolved bucket
- 即使存在多次访问，也应先停留在 `EpRes` 未决状态或 `DepObs`

也就是说：

- `IP` 可以形成连接证据
- 但不能单独形成稳定依赖关系

### 11.7 当前建议

当前建议固定为：

- 单列 `IP` 属于弱信号，不属于正式网络 inventory
- 第一版先建 `IP evidence`，不直接建正式 network object
- 最多先做 `CIDR` 命中和候选归属
- 等补齐 host / pod / service / instance 身份后，再 materialize 正式模型

---

## 12. Network 对象与 Service/Endpoint 的边界

这是网络建模里必须单独说清楚的一点。

### 12.1 `NetworkSegment` 不是 `SvcEp`

`NetworkSegment` 回答：

- 某对象连接到了哪段网络

`SvcEp` 回答：

- 调用方应通过哪个入口访问某个服务

即使某个 `SvcEp` 地址落在某个 `NetworkSegment` 内，它也不是同一个对象。

### 12.2 `InstEp` 不等同 `PodNetAssoc`

`InstEp` 回答：

- 某个实例当前在哪个地址和端口上提供服务

`PodNetAssoc` 回答：

- 某个 Pod 接入了哪段网络，并持有哪些地址

两者可能引用同一个 `IP`，但职责不同：

- `InstEp` 偏连接入口
- `PodNetAssoc` 偏基础接入关系

### 12.3 `EpRes` 不是网络资产对象

`EpRes` 的职责是把一次观测中的地址解析回服务或实例。

它依赖网络和地址对象，但本身不应被理解为：

- 网络边界 inventory
- 服务入口 inventory
- 实例 inventory

---

## 13. 第一版最小闭环

第一版网络建模建议先实现下面这条最小闭环：

```text
platform / inventory sources
  -> NetworkDomain / NetworkSegment

host / pod discovery
  -> HostNetAssoc / PodNetAssoc

service / platform exposure
  -> SvcEp / InstEp

trace / log / flow / dns
  -> EpRes
  -> DepObs
  -> DepEdge
```

这条闭环已经足以支持一批核心查询：

- 某台主机接入了哪些网络
- 某个 Pod 当前位于哪些网络
- 某个服务通过哪些入口暴露
- 某次连接最终落到了哪个服务或实例
- 某两个服务之间的依赖是否有观测支撑

---

## 14. 第一版暂不解决的问题

第一版不要求一次解决以下问题：

- 全网 L2/L3 设备级路径还原
- 完整网络策略模拟
- 安全组 / ACL / route 的全量治理
- NAT、代理、网关多跳路径的完全还原
- 通用网络数字孪生

这些问题如果提前纳入，会显著推高复杂度，并模糊 `wist-atlas` 的中心目标。

---

## 15. 对现有模型文档的约束

新增本分析文档后，后续相关模型文档应遵守以下边界：

- `host-pod-network-topology-model` 主要负责 network inventory 与 attachment 关系
- `business-system-service-topology-model` 主要负责 `SvcEp / InstEp` 的服务暴露语义
- `endpoint-and-dependency-observation-model` 主要负责 `EpRes / DepObs / DepEdge`
- `dataflow-and-pipeline-architecture` 主要回答这些对象分别由哪些来源进入、先进入 evidence 还是 inventory

不建议再在多个文档里重复展开“网络到底分哪几层”。

---

## 16. 当前建议

当前建议固定为：

- `wist-atlas` 的网络建模第一版只做网络边界、网络接入、服务暴露和连接归因四层
- 网络 inventory、地址对象、连接观测必须分层建模
- 云网络、Kubernetes 网络、主机观测网络都进入同一语义框架，但不混淆 authoritative inventory 与 runtime evidence
- 安全控制面和复杂网络控制面对象不作为第一版核心范围
