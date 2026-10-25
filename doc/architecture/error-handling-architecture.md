# wist-atlas 错误处理架构设计

> 状态：**Target** — 定义 `wist-atlas` Rust crate、ingest pipeline、外部 API 和治理观测的统一错误处理目标态。

## 1. 文档目的

本文档定义 `wist-atlas` 的错误处理体系，重点固定以下问题：

- Rust 代码中如何表达领域错误、上下文、source chain 和跨层转换
- ingest / normalize / resolve / persist / derive / query 各阶段的失败如何分类
- 外部输入错误、数据质量错误、身份解析冲突、存储错误和查询错误如何投影到 API
- 哪些错误可重试，哪些进入 dead letter / unresolved，哪些必须告警
- 错误如何进入 ingest job、audit event、structured log、metrics 和 explain 视图
- 迁移期间如何从 `thiserror` / `Result<T, String>` 收敛到统一结构化错误

相关文档：

- [`dataflow-and-pipeline-architecture.md`](./dataflow-and-pipeline-architecture.md)：五段 pipeline 与失败处理建议
- [`identity-resolution-architecture.md`](./identity-resolution-architecture.md)：身份解析、冲突和 explain
- [`storage-architecture.md`](./storage-architecture.md)：source-of-truth、对象存储和派生视图边界
- [`observability-and-audit-architecture.md`](./observability-and-audit-architecture.md)：运行观测、pipeline health 和审计
- [`security-and-access-control-architecture.md`](./security-and-access-control-architecture.md)：权限、租户隔离和敏感信息边界
- [`../external-integration/external-input-spec.md`](../external-integration/external-input-spec.md)：当前外部输入契约
- [`../internal/processing-glossary.md`](../internal/processing-glossary.md)：内部 adapter / resolver / materializer 术语

---

## 2. 核心结论

目标态固定以下结论：

1. Rust 内部错误统一使用 `orion-error` 的 `StructError<R>`。
2. 每个 crate 定义自己的领域 reason enum，并通过 `#[derive(OrionError)]` 暴露稳定 identity。
3. reason enum 默认使用 unit variant，动态诊断信息放入 `StructError` 的 detail、context、fields、metadata 或 source chain。
4. 热路径不得扩散 `anyhow::Error`、裸 `std::io::Error`、`Box<dyn Error>` 或字符串错误；这些错误必须在边界立即转换为 `StructError<R>`。
5. 跨 crate 的错误转换分两类：仅改变 reason 类型用 `conv_err()`；建立新的语义边界用 `source_err(...)` 或 `source_raw_err(...)`。
6. API、ingest job、dead letter 和 audit event 只携带稳定错误投影，不直接序列化 `StructError<R>`。
7. `ErrorIdentity.code` 是内部稳定错误身份来源，并映射到 atlas 的协议错误码。
8. pipeline 裁决必须由稳定 reason / code / stage / scope 决定，不能解析 detail 文本。
9. 现有 `thiserror` / `Result<T, String>` 属于迁移对象；新热路径不得继续新增字符串错误返回。

---

## 3. 分层模型

错误体系分四层：

```text
Rust Domain Error
  StructError<Reason> + OperationContext + source chain
  负责本进程内传播、诊断和跨 crate 转换

Error Identity / Code
  ErrorIdentity.code + atlas protocol code
  负责稳定分类、统计、幂等和 API 投影

Pipeline Decision
  stage + scope + action + retryable + severity
  负责 accepted/rejected/dead-letter/unresolved/alert/retry

External Projection
  API error / ingest job error / audit event / metric label
  负责对调用方、运维和治理侧暴露
```

边界约束：

- Rust 层可以保留完整 source chain。
- API 层只能暴露稳定 code、可脱敏 message、correlation id 和允许公开的字段。
- ingest job / dead letter 可以保存更多诊断摘要，但原始 payload 和敏感字段必须按安全模型处理。
- metrics 标签只能使用低基数稳定字段，例如 `code`、`stage`、`scope`、`component`。

---

## 4. Crate 级 reason 划分

每个 crate 拥有自己的 reason enum。跨 crate 不共享一个巨大错误枚举。

| crate | 建议 reason | 职责范围 |
|---|---|---|
| `atlas-domain` | `DomainReason` | schema 对象、领域约束、外部 input envelope 校验、DTO 转换 |
| `atlas-storage` | `StorageReason` | PostgreSQL、对象存储、migration、repository、transaction |
| `atlas-api` | `ApiReason` | ingest API、query API、API DTO、认证授权前置校验 |
| `atlas-sync` | `SyncReason` | connector、fetch、cursor、external sync job、回补同步 |
| `atlas-app` | `AppReason` | 单体编排、CLI、demo / file run、跨模块组合 |

每个 reason enum 必须包含一个透明 `General(UnifiedReason)` variant，用于复用配置、IO、权限、系统、数据和校验等通用类别。
调用通用类别时优先使用 `#[derive(OrionError)]` 生成的 delegate constructor，例如 `ApiReason::validation_error()` 或 `StorageReason::system_error()`。

示例：

```rust
use derive_more::From;
use orion_error::prelude::*;

#[derive(Debug, Clone, PartialEq, From, OrionError)]
pub enum ApiReason {
    #[orion_error(identity = "atlas.api.payload_missing")]
    PayloadMissing,
    #[orion_error(identity = "atlas.api.schema_unsupported")]
    SchemaUnsupported,
    #[orion_error(identity = "atlas.api.ingest_rejected")]
    IngestRejected,
    #[orion_error(identity = "atlas.api.query_invalid")]
    QueryInvalid,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

pub type ApiError = StructError<ApiReason>;
```

实现约束：

- 新代码优先 `use orion_error::prelude::*;`。
- 单个 reason 变成错误时使用 `to_err()`。
- IO、serde、toml 等源错误进入结构化体系时优先用 `source_err(reason, detail)`。
- 第三方 `StdError` 没有 `UnstructuredSource` bridge 时用 `source_raw_err(reason, detail)`。
- 低层 `StructError<R1>` 只改 reason 类型时用 `conv_err()`。

---

## 5. 热路径禁止项

热路径函数签名不得返回：

- `anyhow::Result<T>`
- `Result<T, std::io::Error>`
- `Result<T, Box<dyn std::error::Error>>`
- `Result<T, String>`

热路径中的以下写法属于违规：

- `map_err(|e| e.to_string())` 后直接作为错误返回
- `Err(format!(...))`
- `Err("...".to_string())`
- 把 `Display` 文本写入稳定错误码字段
- 通过字符串内容判断错误类型或 pipeline action

文本只允许进入：

- `with_detail(...)`
- `OperationContext` 的字段或 metadata
- protocol projection 的可暴露 `message`
- dead letter / audit 中经过脱敏的诊断摘要

热路径包括：

- external input envelope validation
- ingest gateway / ingest job recorder
- source-specific adapter
- candidate / evidence / observation extractor
- identity resolver
- materializer / repository / transaction
- derived view builder
- query API
- sync connector / cursor advance
- monolith app orchestration / CLI file ingest

---

## 6. Pipeline 失败分类

错误必须携带或可推导出 pipeline stage。

| stage | 典型错误 | 默认 action |
|---|---|---|
| `intake` | payload missing、schema unsupported、auth failed、idempotency conflict | rejected 或 accepted-with-duplicate |
| `validate` | envelope invalid、field missing、type mismatch、timestamp invalid | rejected 或 row rejected |
| `normalize` | source-specific parse failed、unknown section、bad enum | dead letter 或 row rejected |
| `resolve` | identity conflict、weak identifier、tenant unresolved、ref unresolved | unresolved / review queue |
| `persist` | transaction failed、constraint conflict、object store unavailable | retry + alert |
| `derive` | view rebuild failed、graph projection failed | async retry，不污染 source-of-truth |
| `query` | invalid filter、not found、permission denied、view stale | API error |
| `sync` | fetch failed、cursor conflict、external rate limit | retry / backoff / sync job degraded |

裁决规则：

- `intake` envelope 缺失或 schema 不支持时整批 rejected。
- 单行字段非法时优先 row rejected，不阻塞整批，除非字段影响 envelope 或幂等键。
- 引用暂不可解析时进入 unresolved，不写正式关系。
- source-of-truth 写失败必须显式告警。
- derived view 失败不得回滚主对象写入；读模型可异步重建。
- query error 不反向修改主模型。

---

## 7. Pipeline Decision

目标态统一使用结构化裁决对象：

```text
PipelineDecision {
  stage
  scope
  action
  code
  retryable
  severity
}
```

字段说明：

| 字段 | 说明 |
|---|---|
| `stage` | `intake` / `validate` / `normalize` / `resolve` / `persist` / `derive` / `query` / `sync` |
| `scope` | `request` / `batch` / `row` / `candidate` / `relation` / `job` / `view` / `system` |
| `action` | `reject` / `row_reject` / `dead_letter` / `unresolved` / `retry` / `alert` / `return_api_error` |
| `code` | 稳定协议码 |
| `retryable` | 调用方或 worker 是否可重试 |
| `severity` | `info` / `warning` / `error` / `fatal` |

要求：

- adapter、resolver、materializer、worker、API 都消费同一套 decision，不各自解析错误文本。
- `PipelineDecision` 可以从 `StructError<R>` 的 identity、context metadata 和 stage 映射得到。
- 同一个底层错误在不同 stage 可以有不同 action。例如 `std::io::Error` 在读取导入文件时是 `reject`，在对象存储临时不可用时是 `retry + alert`。

---

## 8. 协议错误投影

`StructError<R>` 不直接作为 API / ingest job / audit event 的序列化对象。

### 8.1 API Error

```text
ApiErrorBody {
  code
  message
  request_id?
  ingest_id?
  fields?
  retryable?
}
```

API 投影规则：

- `code` 来自 `ErrorIdentity.code` 到 atlas protocol code 的映射。
- `message` 是可暴露短文本，不包含 SQL、连接串、token、完整 payload、内部路径或 stack trace。
- `fields` 只包含可暴露字段，例如 `schema`、`source.system`、`collect.snap_id`、`field_path`。
- `request_id` / `ingest_id` 用于排障关联。

### 8.2 Ingest Job Error

```text
IngestJobError {
  code
  stage
  action
  message?
  row_ref?
  field_path?
  raw_ref?
}
```

ingest job 投影规则：

- rejected 批次必须记录 `code/stage/action`。
- row rejected 必须能定位到 row 或 section。
- dead letter 必须保存 raw payload ref 或 staged payload id。
- unresolved 不等于 failed；它是可审查、可补证据、可重放的中间状态。

### 8.3 Audit Event

审计事件至少包含：

- actor / system caller
- action
- target object or ingest job
- decision code
- before / after ref（如适用）
- request id / ingest id
- redaction status

---

## 9. 稳定错误码建议

第一版建议按 family 组织协议码：

| family | 示例 code |
|---|---|
| `input.*` | `input.payload_missing`、`input.schema_unsupported`、`input.envelope_invalid` |
| `validate.*` | `validate.field_missing`、`validate.type_mismatch`、`validate.timestamp_invalid` |
| `normalize.*` | `normalize.section_unknown`、`normalize.enum_invalid`、`normalize.raw_parse_failed` |
| `resolve.*` | `resolve.identity_conflict`、`resolve.weak_identifier`、`resolve.ref_unresolved`、`resolve.tenant_unresolved` |
| `persist.*` | `persist.transaction_failed`、`persist.constraint_conflict`、`persist.object_store_unavailable` |
| `derive.*` | `derive.view_rebuild_failed`、`derive.graph_projection_failed` |
| `query.*` | `query.invalid_filter`、`query.not_found`、`query.permission_denied`、`query.view_stale` |
| `sync.*` | `sync.fetch_failed`、`sync.cursor_conflict`、`sync.rate_limited` |
| `system.*` | `system.config_invalid`、`system.dependency_unavailable`、`system.internal` |

Rust identity 到协议码示例：

| `ErrorIdentity.code` | 协议码 |
|---|---|
| `atlas.api.payload_missing` | `input.payload_missing` |
| `atlas.domain.envelope_invalid` | `input.envelope_invalid` |
| `atlas.domain.field_missing` | `validate.field_missing` |
| `atlas.domain.ref_unresolved` | `resolve.ref_unresolved` |
| `atlas.storage.transaction_failed` | `persist.transaction_failed` |
| `atlas.storage.constraint_conflict` | `persist.constraint_conflict` |
| `atlas.sync.fetch_failed` | `sync.fetch_failed` |

协议码必须稳定、短小、可审计；不得使用 Rust enum variant 名、`Display` 文本或 Debug 文本作为协议码。

---

## 10. 跨层转换规则

### 10.1 只改变 reason 类型

低层错误已经表达了正确语义，上层只是换成自己的 reason 类型时使用 `conv_err()`。

```rust
use orion_error::{conversion::ConvErr, prelude::*};

fn submit_ingest() -> Result<(), ApiError> {
    validate_envelope().conv_err()
}
```

使用 `conv_err()` 的前提是上层 reason 已实现从低层 reason 的转换，例如 `impl From<DomainReason> for ApiReason`。

### 10.2 建立新的语义边界

上层要表达新的业务语义时使用 `source_err(...)`，把低层结构化错误作为 source 保留。

```rust
use orion_error::prelude::*;

fn persist_ingest_job() -> Result<(), ApiError> {
    repository_write()
        .source_err(ApiReason::IngestRejected, "record ingest job")
}
```

### 10.3 第三方错误

第三方错误没有 `UnstructuredSource` bridge 时使用 `source_raw_err(...)`。

```rust
use orion_error::prelude::*;

fn fetch_external_source() -> Result<(), SyncError> {
    connector_fetch()
        .source_raw_err(SyncReason::FetchFailed, "fetch external source")
}
```

### 10.4 保留 `map_err` 的场景

只有在需要业务分支、改写可暴露 message、补充多个结构化字段或执行特殊 pipeline decision 时保留 `map_err`。

---

## 11. 可观测性与 Explain

每个结构化错误进入观测链路时至少产生以下维度：

| 维度 | 示例 |
|---|---|
| `error.code` | `resolve.identity_conflict` |
| `error.identity` | `atlas.domain.identity_conflict` |
| `pipeline.stage` | `resolve` |
| `pipeline.action` | `unresolved` |
| `component.name` | `identity_resolver` |
| `tenant_id` | redacted or hashed |
| `schema` | `atlas.in.edge.v1` |
| `source.system` | `warp-insight` |
| `retryable` | `false` |

日志策略：

- structured log 记录稳定 code、stage、action、request id、ingest id。
- debug 日志可以记录 redacted report。
- 不记录 token、连接串、完整 payload、完整 SQL、完整 stack trace、未脱敏租户敏感字段。

指标策略：

- `atlas_pipeline_error_total{code,stage,action,component}`
- `atlas_ingest_rejected_total{code,schema,source_system}`
- `atlas_unresolved_total{code,object_kind}`
- `atlas_persist_retry_total{code}`
- `atlas_derived_view_failure_total{view,code}`

Explain 策略：

- identity conflict / unresolved 必须能解释使用了哪些 identifier、哪些规则和哪些候选对象。
- explain 输出不暴露内部 source chain；只暴露经过脱敏的证据摘要和规则命中信息。

---

## 12. 迁移计划

### P0：错误基线

- 在 workspace 增加 `orion-error` 依赖，启用 `derive`，按需要启用 `serde_json`。
- 为 `atlas-domain`、`atlas-storage`、`atlas-api` 定义首批 reason enum。
- 增加 `ProtocolError`、`PipelineDecision` 和 code mapping 基础类型。
- 增加热路径错误签名检查，禁止新增 `anyhow::Result`、裸 `io::Error`、`Box<dyn Error>` 和 `String` 错误返回。

### P1：Current 行为兼容迁移

- 保留现有 API 行为和测试语义，先把 `Result<T, String>` 外层替换为 `StructError<R>`。
- `thiserror` 枚举迁移到 `#[derive(OrionError)]` reason enum。
- `input.validate()`、ingest extractor、storage repository、query service 进入结构化错误体系。
- API / ingest job 继续输出兼容字段，但错误来源改为 protocol projection。

### P2：Pipeline 裁决落地

- adapter / resolver / materializer 统一返回结构化错误和 `PipelineDecision`。
- row rejected、dead letter、unresolved、retry、alert 进入统一状态模型。
- metrics、audit、explain 消费同一套 `code/stage/action`。

### P3：Sync 与派生视图

- external sync connector、cursor advance、derived view builder 接入同一套错误体系。
- query API 增加 view stale / permission denied / explain denied 等稳定错误码。

---

## 13. 当前决定

当前阶段固定以下结论：

1. `orion-error` 是 Rust 内部错误处理的唯一主路径。
2. `StructError<R>` 不直接作为 API、ingest job、dead letter 或 audit event 序列化对象。
3. `ErrorIdentity.code` 是内部到协议错误码映射的稳定来源。
4. reason enum 不携带动态 payload；动态信息进入 detail、context、fields、metadata 或 source。
5. 热路径不得让 `anyhow::Error`、裸 `io::Error`、`Box<dyn Error>` 或字符串错误继续向上传播。
6. pipeline action 必须由稳定 reason / code / stage / scope 决定。
7. `thiserror` 和 `Result<T, String>` 属于迁移对象；新代码不得扩大其使用范围。
8. 先迁移 `domain`、`storage`、`api` 三条热路径，再扩展到 `sync`、`app`、derived view。
