# wist-atlas Host Inventory 与 Runtime State Schema 草案

## 1. 文档目的

本文档定义 `HostInventory` 和 `HostRuntimeState` 的字段级 schema 草案。

这里的目标是把：

- 主机静态资产事实
- 主机动态运行态快照

明确拆成两个对象，并给出第一版字段、约束和更新语义。

相关文档：

- [`glossary.md`](../glossary.md)
- [`host-inventory-and-runtime-state.md`](host-inventory-and-runtime-state.md)
- [`../external/warp-insight-edge.md`](../external/warp-insight-edge.md)
- [`../architecture/project-charter.md`](../architecture/project-charter.md)

---

## 2. 核心结论

第一版固定：

- `HostInventory` 是目录对象
- `HostRuntimeState` 是状态对象
- 两者通过 `host_id` 关联
- 两者不能合并成一个 schema

---

## 3. `HostInventory`

建议结构：

```text
HostInventory {
  api_version
  kind
  host_id
  tenant_id
  host_name
  machine_id?
  os_name?
  os_version?
  created_at
  last_inventory_at
}
```

### 3.1 固定值

- `api_version = "v1"`
- `kind = "host_inventory"`

### 3.2 必选字段

- `api_version`
- `kind`
- `host_id`
- `tenant_id`
- `host_name`
- `created_at`
- `last_inventory_at`

### 3.3 字段要求

- `host_id`
  - 中心内部稳定主键
  - 不允许为空
- `last_inventory_at`
  - 应不早于 `created_at`

### 3.4 第一版精简原则

`HostInventory` 第一版只保留“容易稳定获取”的最小字段集：

- 身份字段：`host_id`、`tenant_id`、`host_name`
- 身份字段：`host_id`、`tenant_id`、`host_name`
- 辅助身份字段：`machine_id`
- 最小展示字段：`os_name`、`os_version`
- 目录时间字段：`created_at`、`last_inventory_at`

第一版先不保留以下字段：

- `serial_number`
- `cloud_instance_id`
- `vendor`
- `model`
- `arch`
- `kernel_version`
- `cpu_model`
- `cpu_core_count`
- `memory_total_bytes`
- `disk_blob_ref`
- `nic_blob_ref`
- `inventory_revision`

### 3.5 字段说明

| 字段 | 含义 | 来源/口径 | 必填 | 备注 |
| --- | --- | --- | --- | --- |
| `host_id` | 中心侧稳定主机主键 | identity resolution 产物 | 是 | 不等于 `host_name` |
| `tenant_id` | 所属租户 | ingest / sync envelope | 是 | 所有主对象必须带租户边界 |
| `host_name` | 当前主机名 | 边缘发现或平台元数据 | 是 | 可变，不能作为唯一键 |
| `machine_id` | OS 级稳定机器标识 | agent / OS facts | 否 | 高置信 identity 线索 |
| `os_name` | 操作系统名称 | inventory facts | 否 | 例如 `linux`、`windows` |
| `os_version` | OS 版本 | inventory facts | 否 | 慢变化 |
| `created_at` | 中心目录对象创建时间 | 中心侧入库时间 | 是 | 与 domain 模型对齐 |
| `last_inventory_at` | 最近一次 inventory 刷新完成时间 | inventory pipeline | 是 | 应不早于 `created_at` |

说明：

- `environment_id` 若表示应用环境，不应作为稳定目录字段进入 `HostInventory`
- 应用环境归属建议改由独立关系模型维护

---

## 4. `HostRuntimeState`

建议结构：

```text
HostRuntimeState {
  api_version
  kind
  host_id
  observed_at
  boot_id?
  uptime_seconds?
  loadavg_1m?
  loadavg_5m?
  loadavg_15m?
  cpu_usage_pct?
  memory_used_bytes?
  memory_available_bytes?
  disk_used_bytes?
  disk_available_bytes?
  network_rx_bytes?
  network_tx_bytes?
  process_count?
  container_count?
  agent_health
  protection_state?
  degraded_reason?
  last_error?
}
```

### 4.1 固定值

- `api_version = "v1"`
- `kind = "host_runtime_state"`

### 4.2 必选字段

- `api_version`
- `kind`
- `host_id`
- `observed_at`
- `agent_health`

### 4.3 枚举字段

`agent_health` 建议取值：

- `healthy`
- `degraded`
- `protect`
- `unavailable`

`protection_state` 建议取值：

- `normal`
- `degraded`
- `protect`

### 4.4 字段说明

| 字段 | 含义 | 来源/口径 | 必填 | 备注 |
| --- | --- | --- | --- | --- |
| `host_id` | 关联主机主键 | 由 inventory/resolution 提供 | 是 | 指向 `HostInventory` |
| `observed_at` | 本次快照观测时间 | agent 上报或采样时间 | 是 | 快照时间语义 |
| `boot_id` | 本次启动周期标识 | OS runtime facts | 否 | 用于区分重启前后快照 |
| `uptime_seconds` | 主机已运行秒数 | OS runtime facts | 否 | 单位为 seconds |
| `loadavg_1m` | 1 分钟平均负载 | OS runtime facts | 否 | 浮点值 |
| `loadavg_5m` | 5 分钟平均负载 | OS runtime facts | 否 | 浮点值 |
| `loadavg_15m` | 15 分钟平均负载 | OS runtime facts | 否 | 浮点值 |
| `cpu_usage_pct` | 当前 CPU 使用率 | agent 采样值 | 否 | 百分比，`0-100` 或多核归一后口径需固定 |
| `memory_used_bytes` | 当前已用内存 | agent 采样值 | 否 | 单位为 bytes |
| `memory_available_bytes` | 当前可用内存 | agent 采样值 | 否 | 单位为 bytes |
| `disk_used_bytes` | 当前已用磁盘容量 | agent 采样聚合值 | 否 | 单位为 bytes；聚合口径需固定 |
| `disk_available_bytes` | 当前可用磁盘容量 | agent 采样聚合值 | 否 | 单位为 bytes；聚合口径需固定 |
| `network_rx_bytes` | 网络接收累计字节数 | OS/agent runtime counter | 否 | 建议固定为累计计数，不用增量语义 |
| `network_tx_bytes` | 网络发送累计字节数 | OS/agent runtime counter | 否 | 建议固定为累计计数，不用增量语义 |
| `process_count` | 当前进程数 | agent 采样值 | 否 | 整数 |
| `container_count` | 当前容器数 | agent 采样值 | 否 | 整数 |
| `agent_health` | agent 当前健康状态 | agent 自身状态机 | 是 | 枚举值 |
| `protection_state` | 当前保护/退化状态 | agent 或控制逻辑 | 否 | 枚举值 |
| `degraded_reason` | 当前退化原因摘要 | agent 或规则输出 | 否 | 建议短文本摘要，不保存大段日志 |
| `last_error` | 最近错误摘要 | agent 或 pipeline 输出 | 否 | 建议短文本；详细内容走日志或 blob |

### 4.5 时间语义

- `HostRuntimeState` 允许同一 `host_id` 存多条记录
- 主键不应只靠 `host_id`
- 建议唯一键：
  - `host_id + observed_at`

### 4.6 命名与边界 review 结论

第一版固定以下边界：

- 不保留 `current_ip_set`，主机当前地址统一通过 `host_net_assoc` 表达
- 不在 `HostRuntimeState` 内重复承载网络关系对象
- 自由文本字段仅保留摘要，详细错误内容进入结构化日志或 blob

字段命名规则：

- 指标字段保留单位后缀，例如 `*_bytes`、`*_pct`、`*_seconds`
- 状态字段使用领域词，不使用实现细节词
- 大型明细结构不用 `[]` 内嵌到主对象，优先 `*_blob_ref`

---

## 5. 与边缘对象的映射

### 5.1 `DiscoveredResource(kind=host)` 到 `HostInventory`

主要映射：

- `resource_id -> host_id` 候选
- `attributes -> inventory` 最小事实

### 5.2 `metrics_runtime_snapshot / samples` 到 `HostRuntimeState`

主要映射：

- `host metrics` 当前样本 -> runtime state 快照
- `agent health` / `protection state` -> runtime state 运行状态

---

## 6. 第一版限制

第一版不建议把以下内容直接并入 `HostInventory`：

- 当前 CPU 使用率
- 当前 load average
- 当前主机上运行的 process 细表
- 当前软件漏洞列表

这些内容应进入：

- `HostRuntimeState`
- `ProcessRuntimeState`
- `SoftwareVulnerabilityFinding`

---

## 7. 当前建议

当前建议固定为：

- `HostInventory` 只承载慢变化主机事实
- `HostRuntimeState` 只承载高频动态快照
- `host_id` 是两者的唯一关联键
