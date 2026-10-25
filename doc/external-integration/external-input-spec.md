# wist-atlas 外部输入数据规范

## 1. 文档目的

本文档定义 `wist-atlas` 第一版接收外部系统数据时的原始输入格式。

这里的“外部输入”指进入 adapter / connector 之前或刚进入 staging 区的结构化事实，不是中心侧已经归一后的内部候选、证据或主事实对象。

相关文档：

- [`input-taxonomy-and-style.md`](./input-taxonomy-and-style.md)
- [`external-glossary.md`](./external-glossary.md)
- [`../architecture/dataflow-and-pipeline-architecture.md`](../architecture/dataflow-and-pipeline-architecture.md)
- [`../architecture/external-sync-architecture.md`](../architecture/external-sync-architecture.md)
- [`../architecture/identity-resolution-architecture.md`](../architecture/identity-resolution-architecture.md)
- [`warp-insight-adapter-spec.md`](./warp-insight-adapter-spec.md)
- [`file-ingest-spec.md`](./file-ingest-spec.md)

Target 示例数据集：

- [`../../fixtures/external-input/target`](../../fixtures/external-input/target)

---

## 2. 核心原则

外部输入应表达来源系统看到的事实，不应假装已经是中心模型。

固定原则：

- 外部输入必须带来源、采集时间和 schema。
- 外部 ID 必须保留，不能提前替换成内部 UUID。
- 外部输入可以包含来源系统自己的字段名和结构，但必须是结构化 JSON。
- adapter 负责把外部输入转换成 atlas 内部处理对象。
- resolver 负责把内部处理对象归并到内部稳定对象。
- 无法解析的外部事实应停留在 atlas 内部 staging / unresolved 层。

不允许：

- 让 edge agent、CMDB 或 IAM 直接指定中心对象 UUID。
- 让某个 connector 私有实现 identity resolution。
- 把原始 telemetry 明细直接塞进 topology 主库。
- 用中心侧 normalized payload 反向要求外部系统改造数据结构。

---

## 3. 通用 Envelope

### 3.0 命名规则

外部输入字段优先使用短名，但必须保留语义：

| 长名 | 规范短名 | 说明 |
| --- | --- | --- |
| `schema_version` | `schema` | 外部输入 schema 标识 |
| `atlas.external_input` | `atlas.in` | schema 前缀 |
| `source_family` | `family` | schema 中的来源族 |
| `producer_id` | `producer` | 产生 payload 的 agent / job / connector |
| `tenant_external_ref` | `tenant_ref` | 外部租户引用 |
| `environment_external_ref` | `env_ref` | 外部环境引用 |
| `collection` | `collect` | 采集 / 同步元信息 |
| `snapshot_id` | `snap_id` | 快照或批次 ID |
| `resource_version` | `res_ver` | 来源侧资源版本 |
| `window_start` | `win_start` | 时间窗口开始 |
| `window_end` | `win_end` | 时间窗口结束 |

payload 内推荐把冗长引用字段缩短为 `*_ref`，例如 `service_external_ref -> service_ref`、`owner_subject_external_id -> owner_ref`。外部系统原生字段名可以保留，但 atlas 示例和规范优先使用短名。

所有外部输入文件或 staged payload 都应遵循统一 envelope：

```json
{
  "schema": "atlas.in.<family>.v1",
  "source": {},
  "collect": {},
  "payload": {}
}
```

### 3.1 `schema`

格式：

```text
atlas.in.<family>.v<major>
```

第一版建议的 `<family>`：

- `edge`
- `cmdb`
- `iam`
- `k8s`
- `telemetry`
- `sw`
- `artifact`
- `vuln`
- `bug`
- `security`
- `oncall`
- `manual`
- `correction`

旧长名到规范短名的迁移关系：

| 旧 family | 规范 family |
| --- | --- |
| `edge_discovery` | `edge` |
| `cmdb_catalog` | `cmdb` |
| `iam_directory` | `iam` |
| `k8s_inventory` | `k8s` |
| `telemetry_summary` | `telemetry` |
| `software_evidence` | `sw` |
| `artifact_verification` | `artifact` |
| `vulnerability_advisory` | `vuln` |
| `bug_signal` | `bug` |
| `security_event` | `security` |
| `oncall_schedule` | `oncall` |
| `manual_batch` | `manual` |
| `manual_correction` | `correction` |

版本规则：

- 兼容新增字段不提升 major。
- 删除字段、改变字段语义或改变枚举语义必须提升 major。
- adapter 必须显式声明支持的 `schema`。

### 3.2 `source`

`source` 描述数据来自哪里。

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `kind` | string | 是 | 来源类型 |
| `system` | string | 是 | 来源系统名，例如 `warp-insight`、`example-cmdb`、`kube-api` |
| `producer` | string | 是 | 产生这份 payload 的 agent、connector 或 job |
| `tenant_ref` | string | 是 | 外部租户标识 |
| `env_ref` | string | 否 | 外部环境标识，例如 `prod`、`office`、`cn-shanghai` |

`kind` 第一版建议值：

- `edge`
- `cmdb`
- `iam`
- `hr`
- `oncall`
- `kubernetes`
- `telemetry`
- `vulnerability_feed`
- `security`
- `manual`

### 3.3 `collect`

`collect` 描述这份数据如何被采集或同步。

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `mode` | string | 是 | `snapshot`、`full`、`incremental`、`window` |
| `snap_id` | string | 条件必填 | 快照或批次 ID |
| `observed_at` | string | 条件必填 | 事实发生或被观察到的时间 |
| `collected_at` | string | 否 | agent 采集完成时间 |
| `fetched_at` | string | 否 | connector 拉取完成时间 |
| `cursor` | string | 否 | 外部同步游标 |
| `res_ver` | string | 否 | K8s 等平台资源版本 |
| `win_start` | string | 否 | telemetry 窗口开始 |
| `win_end` | string | 否 | telemetry 窗口结束 |

时间字段统一使用 RFC3339 UTC，例如 `2026-04-26T02:20:30Z`。

### 3.3.1 `snapshot` 的替换语义

`mode = snapshot` 声明的是一份**完整**快照，所以它同时表达了「什么不在了」。处理时按
**作用域** `(schema, source.system, source.producer, tenant, env)` 为界：

- 本次快照里没出现的 process 会被标 `stale_at`（保留行、带时间点），**不删行**；
- 查询与导入汇总默认**不返回**已标 stale 的行；
- 它重新出现时 `stale_at` 自动清掉（复活）；
- `stale_at` 保留**首次**消失时间，后续快照不会把它往后推。

三条自我限制（都不是可选项）：

1. **空列表不算「全没了」** —— 采集失败和「确实没有对象」在数据上不可区分，宁可晚一轮清理。
2. **快照不完整就不清扫** —— 本次有对象解析不到宿主时，整个清扫跳过。误标一批活对象的
   代价远大于晚一轮清理。
3. **只动本次快照覆盖到的主机** —— 整台主机从快照里消失时不清扫它的进程
   （主机退役需要单独策略：一轮探针失败不等于机器没了）。

`incremental` / `window` / `correction` 描述的都是**部分**观测，不做任何清扫 ——
拿部分观测当全集比对会把没投递的对象全标死。

当前实现只对 `payload.processes[]` 生效（`host_inventory` / `network_segment` 的退役待定）。

### 3.4 `payload`

`payload` 是来源系统事实本体。它允许按来源保持不同结构，但必须满足：

- 必须是 JSON object。
- 不能包含二进制大对象；大对象用 URI 或 object storage ref 表达。
- 数组行必须保留外部 ID 或可组合成外部 ID 的字段。
- 金额、容量、耗时等数值必须带单位或使用规范字段名说明单位。

---

## 4. Edge Discovery 输入

用途：

- 表达边缘 agent 在某台机器或邻接运行环境中发现的资源事实。
- 主要表达 host、network、process、container、runtime binding、software 等来源事实。

Target 示例：

- [`../../fixtures/external-input/target/edge-discovery-snapshot.json`](../../fixtures/external-input/target/edge-discovery-snapshot.json)

推荐 payload 结构：

```json
{
  "host": {},
  "net_ifaces": [],
  "processes": [],
  "containers": [],
  "service_endpoints": []
}
```

如果来源系统是 `warp-insight`，推荐保留 `ReportDiscoverySnapshot` 原始对象：

```json
{
  "warp_insight": {
    "api_version": "v1",
    "kind": "report_discovery_snapshot",
    "snapshot": {
      "resources": [],
      "targets": []
    }
  }
}
```

详细映射规则见 [`warp-insight-adapter-spec.md`](./warp-insight-adapter-spec.md)。

关键字段：

| 路径 | 说明 | 后续映射 |
| --- | --- | --- |
| `payload.host.hostname` | 主机名 | host fact |
| `payload.host.machine_id` | 主机强标识 | host identity fact |
| `payload.host.os.*` | OS 事实 | host inventory attributes |
| `payload.net_ifaces[].addresses[]` | IP 与 prefix | network fact |
| `payload.processes[]` | 进程事实 | process fact |
| `payload.containers[]` | 容器事实 | container / runtime binding fact |
| `payload.containers[].kubernetes.*` | K8s 线索 | pod/workload binding fact |
| `payload.service_endpoints[]` | 监听地址事实（协议/地址/端口/持有进程） | service instance endpoint fact |

约束：

- Edge Discovery 不直接表达长期 owner。
- `processes[]` 和 `containers[]` 只表达观察事实，不直接创建 service。
- **例外：`service_endpoints[]` 会间接产生服务。** 因为 `InstEp` 必须挂在服务实例上，
  中心侧会按可执行文件名推一个 `Platform / Internal` 的逻辑服务
  （`external_ref = proc:<basename>`，跨主机归并），供 `business_catalog` 这类
  声明式输入后续纠正。这是有意的推断，不是投影。
- `service_endpoints` **缺失**与**空数组**语义不同：缺失表示来源不支持端口采集，
  不触发清扫；空数组表示「确认没有端口在听」，会把该主机的活端点置 `valid_to`。
- 如果只有 IP 没有 CIDR/prefix，只能作为 network fact，不能稳定创建 network segment。

---

## 5. CMDB / Catalog 输入

用途：

- 表达业务、系统、服务、资产、声明依赖和长期归属。
- 主要表达 catalog、asset membership、responsibility 等声明事实。

示例：

- [`../../fixtures/external-input/target/cmdb-catalog-snapshot.json`](../../fixtures/external-input/target/cmdb-catalog-snapshot.json)

推荐 payload 结构：

```json
{
  "biz_units": [],
  "systems": [],
  "services": [],
  "assets": [],
  "declared_deps": []
}
```

关键字段：

| 路径 | 说明 | 后续映射 |
| --- | --- | --- |
| `biz_units[].external_id` | 业务域外部 ID | external identity fact |
| `systems[].biz_unit_ref` | 系统所属业务域 | business/system hierarchy fact |
| `services[].system_ref` | 服务所属系统 | service catalog fact |
| `services[].owner_ref` | 长期 owner 线索 | responsibility fact |
| `assets[].machine_id` | 主机强标识 | host identity fact |
| `assets[].service_ref` | 资产归属服务 | runtime/catalog binding fact |
| `declared_deps[]` | 声明依赖 | declared dependency fact |

约束：

- CMDB 的 `external_id` 只能作为外部标识，不是内部主键。
- CMDB 可以提供强声明关系，但仍需经过 resolver。
- CMDB owner 应和 IAM / Oncall subject resolution 合并后再形成最终责任视图。

---

## 6. IAM / HR 输入

用途：

- 表达人、团队、组织、账号状态和成员关系。
- 主要表达 subject、membership 和 status 事实。

示例：

- [`../../fixtures/external-input/target/iam-directory-snapshot.json`](../../fixtures/external-input/target/iam-directory-snapshot.json)

推荐 payload 结构：

```json
{
  "users": [],
  "groups": [],
  "memberships": []
}
```

关键字段：

| 路径 | 说明 | 后续映射 |
| --- | --- | --- |
| `users[].external_id` | 用户外部 ID | subject identity fact |
| `users[].email` | 用户强匹配线索 | subject identity fact |
| `groups[].external_id` | 团队或轮值外部 ID | subject identity fact |
| `groups[].group_type` | `team` 或 `rotation` | subject type fact |
| `memberships[]` | 成员关系和有效期 | subject membership fact |

约束：

- IAM / HR 负责“主体是谁”，不负责“资产归谁”。
- 禁用、离职或删除账号必须作为状态事实进入，不应物理删除历史 subject。

---

## 7. Kubernetes Inventory 输入

用途：

- 表达 cluster、namespace、workload、pod、service、endpoint 等平台声明事实。
- 主要表达 cluster/workload/pod/service endpoint 等平台声明事实。

示例：

- [`../../fixtures/external-input/target/k8s-inventory-snapshot.json`](../../fixtures/external-input/target/k8s-inventory-snapshot.json)

推荐 payload 结构：

```json
{
  "cluster": {},
  "namespaces": [],
  "workloads": [],
  "pods": [],
  "services": []
}
```

关键字段：

| 路径 | 说明 | 后续映射 |
| --- | --- | --- |
| `cluster.external_id` | 集群外部 ID | cluster identity |
| `namespaces[].uid` | namespace UID | namespace identity |
| `workloads[].uid` | workload UID | workload identity |
| `workloads[].labels` | 服务绑定线索 | service workload binding fact |
| `pods[].uid` | pod 强标识 | pod identity |
| `pods[].node_name` | pod 所在 host 线索 | pod placement fact |
| `services[].cluster_ip` | 服务入口 | endpoint fact |
| `services[].selector` | workload 匹配线索 | selector matching fact |

约束：

- K8s UID 是强标识，但只在集群作用域内唯一。
- label / annotation 是高价值线索，但不能无审计地覆盖 CMDB 声明关系。
- Pod IP 是运行态事实，不应当作长期 host IP。

---

## 8. 从外部输入到 atlas 内部处理

外部输入进入中心模型前必须经过 adapter。

推荐流程：

```text
external input
  -> schema validation
  -> source-specific adapter
  -> atlas internal processing object
  -> identity resolver
  -> materializer
```

适配示例：

| 外部输入 | atlas 内部处理方向 |
| --- | --- |
| `edge.payload.host` | host identity / host inventory |
| `edge.payload.net_ifaces[].addresses[]` | network fact / host network association |
| `iam.payload.users[]` | subject identity |
| `iam.payload.groups[]` | team / rotation subject |
| `cmdb.payload.assets[]` | host identity + ownership fact |
| `cmdb.payload.services[]` | business/system/service catalog fact |

当前 [`file-ingest-spec.md`](./file-ingest-spec.md) 描述的是 adapter 之后的 normalized batch import payload，不能代表真实外部输入协议。

---

## 9. 数据质量规则

所有外部输入至少应校验：

- `schema` 是否支持。
- `source.kind`、`source.system`、`source.producer` 是否存在。
- `tenant_ref` 是否能映射到内部 tenant。
- 时间字段是否为合法 RFC3339 UTC。
- 同一 payload 内外部 ID 是否重复。
- 关键引用是否能在同一批次或历史外部映射中找到。
- 枚举值是否属于 adapter 支持范围。

失败处理：

- envelope 缺失或 schema 不支持：整批 rejected。
- 单行字段非法：写 rejected row，整批是否失败由来源策略决定。
- 引用暂不可解析：进入 atlas 内部 unresolved 状态，不写正式关系。

---

## 10. P0 与后续范围

P0 建议先固定并实现：

- `edge` 到 host/network fact 的 adapter。
- `iam` 到 subject fact 的 adapter。
- `cmdb` 到 host ownership / service catalog fact 的 adapter 草案。
- staged payload 记录与重放所需 metadata。

P1/P2 再扩展：

- K8s inventory 到 cluster/workload/pod/service endpoint。
- telemetry 到 dependency observation。
- vuln feed 到 software vulnerability finding。
- security event 到 risk fact。
