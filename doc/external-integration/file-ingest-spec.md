# wist-atlas 文件导入规范

## 1. 目的

本文档固定 `wist-atlas` 第一版从文件载入 normalized batch import payload 的规范，并给出可直接运行的示例。

这里的 payload 是 source adapter 之后的中心侧候选输入，不是真实 edge agent、CMDB、IAM 或 K8s API 的原始输入协议。外部输入协议见 [`external-input-spec.md`](./external-input-spec.md)。

当前文件导入主要用于：

- 本地单体模式演示
- P0 smoke test
- 手工或批量导入 `host + network + responsibility` 最小闭环数据
- adapter 开发完成前，用 normalized payload 验证中心 pipeline

相关示例：

- [`../../fixtures/file-ingest/minimal-host-network.json`](../../fixtures/file-ingest/minimal-host-network.json)
- [`../../fixtures/file-ingest/small-office.json`](../../fixtures/file-ingest/small-office.json)

---

## 2. 当前运行方式

当前 `atlas-app` 的文件模式读取的是 payload JSON 本体，不读取完整 `IngestEnvelope`。

运行命令：

```bash
cargo run -p atlas-app -- file fixtures/file-ingest/minimal-host-network.json
```

单体入口会自动补齐 envelope 字段：

- `source_kind = BatchImport`
- `source_name = monolith`
- `ingest_mode = BatchUpsert`
- `tenant_id = 随机生成`
- `received_at = 当前时间`
- `payload_inline = 文件内容`

当前 CLI 文件模式不支持在文件内覆盖 `tenant_id`、`environment_id`、`ingest_mode` 或 `received_at`。

---

## 2.1 当前 JSONL replay / import 入口

除了 `file <json>` 的 normalized payload 导入外，当前还支持：

```bash
cargo run -q -p atlas-app -- replay-jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl

cargo run -q -p atlas-app -- postgres-live import-jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl

cargo run -q -p atlas-app -- postgres-live replace-jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
  ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl
```

这个入口与 `file <json>` 不同：

- `file <json>` 读取的是单个 JSON payload object
- `replay-jsonl` / `import-jsonl` 读取的是一个或多个 JSONL 文件
- `replace-jsonl` 会先执行 `reset-public`，再执行正式导入
- JSONL 的正式导入实现已下沉到 `atlas-sync::JsonlImportService`

当前职责分层如下：

```text
atlas-app
  └─ CLI 参数解析、store 初始化、结果打印

atlas-sync
  └─ 多文件 JSONL replay / import
  └─ JSON -> AtlasInputEnvelope -> IngestEnvelope

atlas-api
  └─ candidate 提取、identity 解析、materialize
```

---

## 2.2 JSONL import 当前支持的输入

当前 `replay-jsonl` / `import-jsonl` / `replace-jsonl` 已支持：

- `atlas.in.edge.v1`
  - host fact
  - network fact
  - process fact
- `atlas.in.telemetry.v1`
  - host metrics
  - process metrics

当前已接入的 telemetry materialize：

- host metrics -> `HostRuntimeState`
- `process.state` -> `ProcessRuntimeState.process_state`
- `process.memory.rss` -> `ProcessRuntimeState.memory_rss_kib`

---

## 2.3 JSONL import 当前落库范围

JSONL import 当前会把数据导入到 storage traits 对应的对象：

- `HostInventory`
- `NetworkSegment`
- `HostNetAssoc`
- `ProcessRuntimeState`
- `HostRuntimeState`
- `ServiceInstance`
- `RuntimeBinding`

其中：

- process 带 `service_ref` 时，才会额外生成 `ServiceInstance` / `RuntimeBinding`
- process telemetry 不会单独生成新进程，只会回写到已存在的 `ProcessRuntimeState`

---

## 2.4 当前是否落真实数据库

当前默认运行方式仍然不是落真实数据库：

- 默认模式使用 `InMemoryTopologyStore`
- `postgres-mock` 仍然不是正式 PostgreSQL，只是 mock executor

但 `atlas-app` 已支持正式 PostgreSQL 入口：

- `postgres-live reset-public`
- `postgres-live import-jsonl`
- `postgres-live replace-jsonl`

也就是说：

- 默认 `replay-jsonl` 结果仍只存在内存中
- `postgres-live import-jsonl` / `replace-jsonl` 已可直接把结果写入真实 PostgreSQL

当前 repo 已提供 PostgreSQL 开发环境管理：

- [`../../docker-compose.yml`](../../docker-compose.yml)

---

## 3. Payload 顶层结构

文件必须是一个 JSON object。

P0 推荐结构：

```json
{
  "hosts": [],
  "ips": [],
  "subjects": [],
  "responsibility_assignments": []
}
```

顶层字段说明：

| 字段 | 类型 | 必填 | 当前用途 |
| --- | --- | --- | --- |
| `hosts` | array | 是 | 主机目录候选 |
| `ips` | array | 是 | IP、CIDR 与 host 网络关联候选 |
| `network_segments` | array | 否 | 网段候选；可替代 `ips`，但 P0 示例优先使用 `ips` |
| `subjects` | array | 否 | 用户、团队、轮值、服务账号候选 |
| `responsibility_assignments` | array | 否 | 责任关系候选 |

当前实现会忽略未知顶层字段。规范上不建议在 P0 导入文件中混入未定义字段。

---

## 4. Host 行规范

`hosts[]` 每行表示一个主机候选。

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `host_name` | string | 是 | 主机名；当前也是责任关系指向 host 的 `target_external_ref` |
| `machine_id` | string | 否 | 强标识；优先用于 host identity resolution |
| `external_ref` | string | 否 | 外部资产 ID；当前提取为 candidate 字段，P0 materializer 尚不使用它做匹配 |
| `os_name` | string | 否 | 操作系统名称 |
| `os_version` | string | 否 | 操作系统版本 |

约束：

- `host_name` 不能为空字符串。
- 推荐始终填写 `machine_id`，避免只依赖主机名匹配。
- 当前 P0 文件闭环只会落库和 `ips[]` 或 `network_segments[]` 成功匹配的 host。仅出现在 `hosts[]` 中、没有网络行引用的 host 暂不保证落库。

---

## 5. IP / Network 行规范

P0 推荐使用 `ips[]` 表达主机地址与所属网段。

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `ip` | string | 是 | 主机 IP 地址；等价别名为 `ip_addr` |
| `cidr` | string | 是 | 所属网段，例如 `192.168.10.0/24` |
| `host_name` | string | 条件必填 | 用于匹配 `hosts[].host_name` |
| `machine_id` | string | 条件必填 | 用于匹配 `hosts[].machine_id`，优先级高于 `host_name` |
| `iface_name` | string | 否 | 网卡名 |
| `gateway_ip` | string | 否 | 网关地址 |
| `segment_name` | string | 否 | 网段显示名；不填时默认使用 `cidr` |

约束：

- `ip` 和 `cidr` 至少应同时提供，才能形成稳定 host-network association 和 network segment。
- `machine_id` 或 `host_name` 至少提供一个，并且必须能匹配到 `hosts[]` 中的行。
- 如果 `segment_name` 为空，落库后的 network segment 名称使用 `cidr`。

`network_segments[]` 也可用于输入网段候选，字段与上表基本一致。若希望形成主机网络关联，仍需提供 `ip_addr` 或 `ip`，并提供可匹配的 `host_name` 或 `machine_id`。

---

## 6. Subject 行规范

`subjects[]` 每行表示一个责任主体候选。

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `display_name` | string | 是 | 显示名；可用于责任关系匹配 |
| `email` | string | 否 | 邮箱；优先用于责任关系匹配 |
| `subject_type` | string | 否 | 默认 `user` |
| `external_ref` | string | 否 | 外部身份 ID |
| `is_active` | boolean | 否 | 默认 `true` |

`subject_type` 可选值：

- `user`
- `team`
- `rotation`
- `service_account`

约束：

- 责任关系目前只会匹配同一文件中成功导入的 subject。
- 推荐同时填写 `display_name` 和 `email`，并在责任关系中重复这两个字段。

---

## 7. Responsibility Assignment 行规范

`responsibility_assignments[]` 每行表示一个责任关系候选。

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `subject_email` | string | 条件必填 | 匹配 `subjects[].email` |
| `subject_display_name` | string | 条件必填 | 匹配 `subjects[].display_name` |
| `target_kind` | string | 否 | 默认 `host` |
| `target_external_ref` | string | 是 | 目标对象外部引用 |
| `role` | string | 否 | 默认 `owner` |

P0 支持的 `target_kind`：

- `host`
- `network_segment`

`role` 可选值：

- `owner`
- `maintainer`
- `oncall`
- `security`

目标匹配规则：

- `target_kind = host` 时，`target_external_ref` 必须等于已落库 host 的 `host_name`。
- `target_kind = network_segment` 时，`target_external_ref` 必须等于已落库 network segment 的名称。未填写 `segment_name` 时，该名称通常是 `cidr`。

当前责任关系有效期由导入时刻自动生成：

- `valid_from = received_at`
- `valid_to = null`

---

## 8. P0 文件导入约束

当前 P0 文件导入遵循以下约束：

- 文件格式只支持 JSON。
- 文件内容必须是 payload object，不是完整 envelope。
- `Delta` 模式尚未支持；CLI 固定使用 `BatchUpsert`。
- 字段名统一使用 `snake_case`。
- 枚举值统一使用小写字符串。
- 空字符串按缺失值处理。
- `unresolved` candidate 不应写入正式关系；当前实现会跳过无法匹配 host、subject 或 target 的关系。
- 同一文件重复导入应保持主对象幂等，但责任关系去重仍依赖后续存储约束继续收敛。

---

## 9. 示例

最小闭环：

```bash
cargo run -p atlas-app -- file fixtures/file-ingest/minimal-host-network.json
```

预期输出应包含：

```text
wist-atlas monolith started
ingest_id=demo-ingest-1
host=demo-node
network=10.42.0.0/24
ip=10.42.0.12
responsibilities=alice:Owner
```

小型办公网络：

```bash
cargo run -p atlas-app -- file fixtures/file-ingest/small-office.json
```

该示例包含：

- 3 台 host
- 2 个 CIDR 网段
- 3 个 subject
- host 与 network segment 两类责任关系

---

## 10. 后续演进

后续如果要支持生产级文件导入，应继续补齐：

- 完整 `IngestEnvelope` 文件格式
- `tenant_id` / `environment_id` 显式输入
- `snapshot` 与 `batch_upsert` 的差异化幂等语义
- `delta` 文件模式
- schema version 与 JSON Schema 校验
- 独立 dead-letter / rejected row 输出
