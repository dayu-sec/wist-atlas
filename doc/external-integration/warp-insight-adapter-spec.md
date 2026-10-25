# warp-insight Adapter 规范

## 1. 文档目的

本文档定义 `wist-atlas` 如何接入 `warp-insight` 输出的边缘 discovery 事实。

它重点固定：

- `warp-insight` 哪些对象可作为 `wist-atlas` 外部输入
- `ReportDiscoverySnapshot`（`WiDiscReport`）如何进入 `atlas.in.edge.v1`
- `DiscoveredResource / DiscoveredTarget` 如何映射到 atlas 的 cand / ev / obs
- 当前可落地范围与暂不接入范围

相关文档：

- [`README.md`](./README.md)
- [`external-glossary.md`](./external-glossary.md)
- [`../internal/processing-glossary.md`](../internal/processing-glossary.md)
- [`external-input-spec.md`](./external-input-spec.md)
- [`input-taxonomy-and-style.md`](./input-taxonomy-and-style.md)
- [`../../fixtures/external-input/target/edge-discovery-snapshot.json`](../../fixtures/external-input/target/edge-discovery-snapshot.json)
- [`warp-insight/doc/design/foundation/glossary.md`](../../../warp-insight/doc/design/foundation/glossary.md)
- `warp-insight/doc/design/center/report-discovery-snapshot-schema.md`
- `warp-insight/doc/design/center/discovery-sync-protocol.md`
- `warp-insight/doc/design/edge/resource-discovery-runtime.md`
- `warp-insight/doc/design/edge/discovery-output-examples-current.md`

---

## 2. 边界结论

第一版固定以下结论：

- `warp-insight` 是 `wist-atlas` 的外部事实来源之一。
- `wist-atlas` 不直接读取 `warp-insightd` 本地 `state/discovery/*.json` 作为线上协议。
- 线上接入对象应是 `ReportDiscoverySnapshot`（`WiDiscReport`），其中携带完整 `DiscoverySnapshot`（`DiscSnap`）。
- `DiscSnap.resources[] / targets[]` 是边缘本地事实，不是 atlas 中心资源目录。
- atlas adapter 负责把这些事实转换为 cand / ev。
- atlas resolver 负责跨来源 identity resolution 和主对象归并。

对外术语必须遵循 [`external-glossary.md`](./external-glossary.md)。内部 adapter 输出短名和字段名遵循 [`../internal/processing-glossary.md`](../internal/processing-glossary.md)：

- `DiscoveredResource` 在 atlas 侧只等价于 `ResFact`。
- `DiscoveredTarget` 在 atlas 侧只等价于 `TargetEv`。
- resource 之间的绑定、归属、监听等关系在 atlas 侧先进入 `RelFact` / ev，不直接写最终 topology relation。
- `resource_id / target_id` 是来源侧标识，不是 atlas 内部主键。

不允许：

- 把 `DiscoveredResource.resource_id` 直接当作 atlas 内部主键。
- 把 `DiscoveredTarget` 直接当作采集配置或服务实例。
- 把 discovery sync 与 telemetry record 混成同一种 payload。
- 让 `warp-insight` 边缘节点决定 atlas 的最终业务、服务、责任关系。

---

## 3. 输入对象

### 3.1 warp-insight 原始对象

`warp-insight` discovery 上送对象：

```text
ReportDiscoverySnapshot (WiDiscReport) {
  api_version
  kind
  report_id
  agent_id
  instance_id
  snapshot_id
  revision
  generated_at
  report_attempt
  report_mode
  reported_at
  snapshot
}
```

其中：

```text
snapshot: DiscoverySnapshot {
  schema_version
  snapshot_id
  revision
  generated_at
  resources[]
  targets[]
}
```

### 3.2 atlas 外部输入 envelope

进入 atlas staging 时使用：

```json
{
  "schema": "atlas.in.edge.v1",
  "source": {
    "kind": "edge",
    "system": "warp-insight",
    "producer": "<agent_id>",
    "tenant_ref": "<tenant>",
    "env_ref": "<environment>"
  },
  "collect": {
    "mode": "snapshot",
    "snap_id": "<snapshot_id>",
    "observed_at": "<generated_at>",
    "collected_at": "<reported_at>",
    "res_ver": "<revision>"
  },
  "payload": {
    "warp_insight": {}
  }
}
```

`payload.warp_insight` 保存完整 `ReportDiscoverySnapshot` 原始对象；atlas 文档可简称为 `WiDiscReport`。

---

## 4. Envelope 映射

| warp-insight 字段 | atlas 字段 | 说明 |
| --- | --- | --- |
| `agent_id` | `source.producer` | 产生 discovery 快照的 agent |
| `snapshot_id` | `collect.snap_id` | 快照幂等键之一 |
| `revision` | `collect.res_ver` | agent 侧快照推进版本 |
| `generated_at` | `collect.observed_at` | 快照事实形成时间 |
| `reported_at` | `collect.collected_at` | 上送对象形成时间 |
| full object | `payload.warp_insight` | 保留原始协议对象用于回放 |

atlas ingest 建议幂等键：

```text
source.system + source.producer + collect.snap_id
```

可辅助判定：

```text
source.system + source.producer + collect.res_ver
```

---

## 5. 支持范围

### 5.1 当前可接入

按 `warp-insight` 当前 discovery 实现，第一版 adapter 应优先支持：

- `DiscRes(kind=host)`，原名 `DiscoveredResource(kind=host)`
- `DiscRes(kind=process)`，原名 `DiscoveredResource(kind=process)`
- `DiscRes(kind=container)`，原名 `DiscoveredResource(kind=container)`
- `DiscRes(kind=service_endpoint)`，原名 `DiscoveredResource(kind=service_endpoint)`
- `DiscTarget(kind=host)`，原名 `DiscoveredTarget(kind=host)`
- `DiscTarget(kind=process)`，原名 `DiscoveredTarget(kind=process)`
- `DiscTarget(kind=container)`，原名 `DiscoveredTarget(kind=container)`

### 5.2 预留但不作为 P0

以下对象在 `warp-insight` 设计中存在，但不作为 atlas adapter P0 前置：

- `k8s_node`
- `k8s_pod`
- `service`
- `log_file`

原因：

- 当前 `warp-insightd` 主循环尚未稳定产出这些 discovery 类型。
- atlas 侧需要先固定 host/process/container 到 candidate/evidence 的基本链路。

> `service_endpoint` 已从这份名单里移出：agentd 的 endpoint probe 在 Linux 上已在采，
> 适配器也会投影成 `payload.service_endpoints[]`（见 §7.4）。macOS 侧探针尚未实现。

---

## 6. Attribute 处理规则

`warp-insight` 使用 OTel 风格 `attributes[]` 与 `runtime_facts[]`：

```json
[
  { "key": "host.name", "value": "office-build-01" }
]
```

adapter 处理规则：

- 先把数组规范化为 key-value map。
- `attributes` 优先级高于 `runtime_facts`。
- `runtime_facts` 可作为补充 evidence，不应直接覆盖稳定 attribute。
- 未识别字段保留到 raw evidence，不丢弃。

冲突处理：

- 同一 key 多值时，保留全部值进入 evidence。
- 只有一个高置信值时才提升为 candidate 字段。
- 冲突值不得静默任选其一写入主对象。

---

## 7. Resource 映射

### 7.1 `host` resource

输入：

```text
DiscoveredResource.kind = host
```

关键字段：

| warp-insight attribute | atlas candidate 字段 | 说明 |
| --- | --- | --- |
| `host.name` | `HostCand.host_name` | 主机名 |
| `host.id` | `HostCand.external_ref` | 边缘本地 host 标识 |
| `os.type` | `HostCand.os_name` | 如存在 |
| `os.version` | `HostCand.os_version` | 如存在 |

输出：

- `HostCand`（全称 `HostCandidate`）
- `ExtIdLinkCand(system_type=warp_insight, object_type=host)`，后续补

identity 规则：

1. 若未来出现 `host.machine_id`，优先作为强标识。
2. 其次使用 `host.id` 作为 `warp-insight` 来源外部标识。
3. 再退化到 `(tenant, agent_id, host.name)`。

注意：

- `host.name` 不应作为全局唯一主键。
- `host.id = hostname:<name>` 仍然是来源侧标识，不是 atlas 内部 ID。

### 7.2 `process` resource

输入：

```text
DiscoveredResource.kind = process
```

关键字段：

| warp-insight key | atlas 输出 | 说明 |
| --- | --- | --- |
| `host.id` | host 外部引用 | 用于绑定到 host candidate |
| `process.pid` | `ProcRtCand` | 进程 PID |
| `process.identity` | process identity ev | 优先使用启动身份 |
| `process.executable.name` | `SwEv` hint | 可执行名 |
| `process.executable.path` | `SwEv` hint | 可执行路径，如存在 |
| `discovery.identity_strength` | ev confidence hint | 弱身份需降置信 |
| `discovery.identity_status` | ev status hint | 标记不可用原因 |

输出：

- `ProcRtCand`（全称 `ProcessRuntimeCandidate`，规划输出；实现落地前可先保存为 process resource ev）
- `SwEv`
- `RtBindEv`，仅当存在可解释绑定线索时输出

identity 规则：

- 首选 `host.id + process.pid + process.identity`。
- 若无 `process.identity`，只输出低置信 process ev。
- 不用 PID 单独决定长期 identity。

### 7.3 `container` resource

输入：

```text
DiscoveredResource.kind = container
```

关键字段：

| warp-insight key | atlas 输出 | 说明 |
| --- | --- | --- |
| `container.id` | container runtime external id | 容器强标识 |
| `container.name` | container runtime attribute | 容器名 |
| `container.runtime` | container runtime type | containerd / docker |
| `container.runtime.namespace` | runtime namespace | 如存在 |
| `host.id` | host 外部引用 | 所在 host 线索 |
| `pid` | process/container link ev | 容器 init pid 线索 |
| `cgroup.path` | `RtBindEv` | cgroup 线索 |
| `k8s.namespace.name` | pod/workload ev | K8s 线索 |
| `k8s.pod.uid` | `PodCand` strong id | Pod UID |
| `k8s.pod.name` | `PodCand` name | Pod 名称 |
| `k8s.container.name` | container name | K8s container 名称 |

输出：

- `CtrRtCand`（全称 `ContainerRuntimeCandidate`，规划输出；实现落地前可先保存为 container resource ev）
- `PodCand`，仅当存在 `k8s.pod.uid`
- `RtBindEv`
- `SwEv`，仅当出现 image 或 artifact 线索

注意：

- 当前 `warp-insight` container discovery 可能来自 runtime task root，不依赖 runtime API。
- `k8s.*` 字段是线索，不等于完整 K8s inventory。
- 完整 cluster / namespace / workload 仍应优先来自 K8s API adapter。

### 7.4 `service_endpoint` resource

输入：

```text
DiscoveredResource.kind = service_endpoint
```

关键字段：

| warp-insight key | atlas 输出 | 说明 |
| --- | --- | --- |
| `endpoint.protocol` | `service_endpoints[].protocol` | `tcp` / `udp`，统一转小写 |
| `endpoint.bind.ip` | `service_endpoints[].address` | 监听地址，`0.0.0.0` 与 `127.0.0.1` 语义不同 |
| `endpoint.bind.port` | `service_endpoints[].port` | 监听端口 |
| `process.pid` | `service_endpoints[].pid` | socket 持有进程；缺失时只记地址 |
| `process.identity` | `service_endpoints[].process_identity` | 优先于 pid 匹配（pid 会复用） |
| `process.executable.name` | `service_endpoints[].process_executable` | 服务名的来源 |
| `container.id` | `service_endpoints[].container_id` | 容器内监听；中心侧据此给实例挂一条 Container 绑定 |
| `runtime.binding.confidence` | `service_endpoints[].binding_confidence` | socket owner 归属置信 |

输出：

- `payload.service_endpoints[]`，中心侧据此产生
  `ServiceEntity` → `ServiceInstance` → `RuntimeBinding` → `InstEp`。
- 快照里**没有任何** `service_endpoint` 资源时，`service_endpoints` 字段整个不出现（空列表不序列化）——
  下游把「字段缺失」和「确认没有端口」当成两件事。

不做「端口值不值得收」的过滤：`127.0.0.1` 上的监听只对本机开放，
而 `0.0.0.0` 上的监听是对外暴露，两者都是暴露面事实，不能按接口地址那套规则丢掉回环地址。

### 7.5 `process` resource

输入：`DiscoveredResource.kind = process`。

| warp-insight key | atlas 输出 | 说明 |
| --- | --- | --- |
| `process.pid` | `processes[].pid` | |
| `process.executable.name` | `processes[].executable_name` | 绝对路径（Linux 取 `/proc/<pid>/exe`） |
| `container.id` | `processes[].container_id` | 进程所属容器（探针从 `/proc/<pid>/cgroup` 推出）；与 `containers[].id` 同一个 id |

`container_id` 的意义：**不监听端口的容器化进程**（批处理、sidecar）拿不到端口证据，
只能靠这里挂到容器上。中心侧只在该容器本轮确实被采集到时才写引用，
避免出现指向不存在容器的边。字段缺失就整个不出现（宿主进程不写空串）。

---

## 8. Target 映射

`DiscoveredTarget` 表示边缘发现到的候选采集对象线索，在 atlas 侧接收为 `TargetEv`。

atlas 处理原则：

- 可保存为 `TargetEv` 或 raw ev。
- 不直接创建 `ServiceEntity`。
- 不直接创建 `SvcEp` 或 `InstEp`。
- 不直接创建 `CollectTarget`，即 `CandidateCollectionTarget`。
- 不直接生成采集配置。

映射建议：

| target kind | atlas 用途 |
| --- | --- |
| `host` | host metrics target ev |
| `process` | process metrics target ev |
| `container` | container metrics target ev |
| `service_endpoint` | endpoint cand ev，P1/P2；不能直接写成 `SvcEp` |
| `log_file` | log input ev，P1/P2 |

---

## 9. Candidate / Evidence 输出

第一版 adapter 输出建议：

```text
WarpInsightAdapterOutput {
  source_ref
  host_cands[]
  proc_rt_cands[] // planned, may be proc_res_ev[] before code model lands
  ctr_rt_cands[]  // planned, may be ctr_res_ev[] before code model lands
  pod_cands[]
  sw_ev[]
  rt_bind_ev[]
  target_ev[]
  unresolved[]
}
```

`source_ref` 至少包含：

- `system = warp-insight`
- `agent_id`
- `instance_id`
- `snapshot_id`
- `revision`
- `report_id`

---

## 10. 失败与降级

### 10.1 整批 rejected

以下情况整批拒绝：

- `payload.warp_insight.kind != report_discovery_snapshot`
- `snapshot_id` 与 `payload.warp_insight.snapshot.snapshot_id` 不一致
- `revision` 与 `payload.warp_insight.snapshot.revision` 不一致
- `snapshot.resources` 缺失或类型非法

### 10.2 单项 unresolved

以下情况单项进入 unresolved：

- resource `kind` 未支持。
- 必要 identity 字段缺失。
- 同一个 resource 内关键 identity 冲突。
- process 只有弱 PID，无法形成稳定 identity。

### 10.3 保留 raw evidence

adapter 不理解的 `attributes`、`runtime_facts`、`labels` 必须保留到 raw evidence。

---

## 11. 与 telemetry 的边界

`WiDiscReport` 不等于 telemetry uplink。

边界：

- discovery sync 进入 edge discovery adapter。
- metrics/logs/traces/security 进入 telemetry adapter。
- `DiscTarget` 可帮助 telemetry resource binding，但不承载 telemetry sample。
- telemetry congestion 不应阻塞 discovery sync 的控制事实同步。

---

## 12. 当前决定

当前阶段固定：

- `warp-insight` 对接以 `ReportDiscoverySnapshot`（`WiDiscReport`）为输入。
- atlas external input 中保留完整 `payload.warp_insight` 以支持回放。
- P0 只解析 `host / process / container` resource 和 target。
- `host` 转 `HostCand`。
- `process` 转 `ProcRtCand + SwEv`；在代码模型落地前，`ProcRtCand` 可降级为 process resource ev。
- `container` 转 `CtrRtCand + RtBindEv`，有 K8s 线索时补 `PodCand`；在代码模型落地前，`CtrRtCand` 可降级为 container resource ev。
- target 只作为 ev，不作为最终采集配置或服务对象。

---

## 13. 落地实现（截至 0.1.x）

| 项 | 实际 |
|---|---|
| 代码 | `crates/atlas-wist-adapter`（lib + bin），可执行文件 `atlas-wist-adapter` |
| 输入 | `--report`（一帧 `DSNAP:` 正文）或 `--state-dir`（agentd 的 `state/discovery/`） |
| 输出 | `atlas.in.edge.v1`，`payload = { hosts[], interfaces[], processes[] }` |
| 黄金样本 | `crates/atlas-wist-adapter/fixtures/macos-trimmed.{json,expected.json}` |
| 门禁 | `cargo test -p atlas-wist-adapter` |

两点与前面的规划**有意**不同，按实际实现：

1. **不保留原文**：§12 说的「在 `payload.warp_insight` 里保留完整对象以支持回放」没有落地。
   适配器做的是逐字段投影（`hosts / interfaces / processes`），因为那就是 `atlas-api` 的
   `extract_host_candidates` / `extract_network_segment_candidates` / `extract_process_runtime_candidates`
   实际读的形状。要回放，保留 `DSNAP:` 原帧就行（它是字节），不必在信封里再存一份。
2. **输入解析不用 `wist-contracts` 类型**：那些类型带 `deny_unknown_fields`，新版 agentd
   多一个字段就会把旧版适配器的**整帧**打回。适配器自行声明只读消费的子集，
   未知字段忽略（由单元测试钉住这个行为）。

尚未实现（仍在 §12 的范围内）：`container`，以及 `ProcRtCand` / `CtrRtCand` 这类结构化候选。
当前进程只作为 `processes[]` 行给出，由 `atlas-api` 侧落地成 runtime 候选；
`service_endpoint` 已实现（见 §7.4），但 **macOS 侧探针尚未做** ——
agentd 的 `discover_service_endpoints()` 在非 Linux 平台直接返回空，
所以本机验证只能靠构造快照。
