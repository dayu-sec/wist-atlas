//! 数据面写入端点：**三字段**（`cust_id` / `cmd_id` / `content`），与 `wist-evidence` 同形。
//!
//! 与 [`atlas_wist_ingest`]（增量读网关 sink 文件）是**同一份数据、同一个转换函数**，
//! 区别只在传输：
//!
//! - **文件**：atlas 停机期间帧还在长，恢复后按偏移补投；代价是检查点与轮转。
//! - **HTTP**：一次性投递 + 回执，不需要与网关共享文件系统；代价是接收端不可用时
//!   由数据面重试（`max_retries`）兜底，耗尽后落 rescue。
//!
//! 两者可以并存：同一个 sink group 挂 `file_json_sink` + `http_sink`，HTTP 走热路径、
//! 文件留作回放。这个端点不假设网关只走其一。
//!
//! ## 为什么是三字段
//!
//! 数据面只需要说清三件事：谁的、哪次动作、产物是什么。`cust_id` 就是 atlas 的租户，
//! `content` 是网关的 DSNAP 记录（对象或数组），其余维度由服务端决定 —— 与
//! `wist-evidence` 的 `POST /internal/evidence` 同一个形状，数据面可以用同一套 OML 习惯。
//!
//! **一处刻意的差异**：atlas 的幂等由快照里的 `snap_id` 决定（`ingest_id` 含它），
//! 所以回执不报 `duplicates`；`cmd_id` 是链路追踪 id，不参与判重。
//!
//! ## 回执语义
//!
//! 回执是给**数据面**看的：非 2xx 会被重试、最终进 rescue（可见）；2xx 意味着
//! 「已收到，不必再发」。所以：
//!
//! - 全部成功（含幂等重复）→ `200`；
//! - 有**永久性**失败（不是快照、没有 `body`、JSON 坏了）→ `400`，让它在 rescue 里显形；
//! - 有**暂时性**失败（写库失败）→ `503`，让数据面下一轮重试。

use atlas_api::{PipelineRunSummary, TopologyIngestService};
use atlas_domain::TenantId;
use atlas_storage::{AsyncCatalogStore, AsyncGovernanceStore, AsyncIngestStore, AsyncRuntimeStore};
use atlas_wist_ingest::convert_sink_line;
use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::TopologyMonolith;
use crate::http::HttpAppState;

/// 单请求 body 上限。
///
/// 一帧原文快照约 0.5~0.9 MB；给 16 MB 是为了容纳进程特别多的机器，同时让「贴错了东西」
/// 在 413 上显形，而不是把内存吃光。注意 axum 默认只有 2 MB，必须显式放宽。
pub(crate) const MAX_INGEST_BODY_BYTES: usize = 16 * 1024 * 1024;

/// 单请求记录数上限。
///
/// `batch_size = 1` 时恒为 1；这个上限只防「把整个 sink 文件一次贴进来」。
const MAX_RECORDS_PER_REQUEST: usize = 64;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EdgeIngestQuery {
    /// 覆盖记录里的部署环境，与 `atlas-wist-ingest --env` 同义。
    /// 不给就用 sink 行上的 `env`（由 wparse 的 tags 带下来）。
    env: Option<String>,
}

/// **三字段**写入请求，与 `wist-evidence` 同形。
///
/// `deny_unknown_fields` 不要开：数据面可能带上别的信封字段，多一个键不该让一帧合法快照进 rescue。
#[derive(Debug, Deserialize)]
struct AtlasWrite {
    /// 客户 = atlas 租户（UUID）。**它同时是分区维度**，不接受外部租户别名。
    cust_id: String,
    /// 链路追踪 id。atlas 的幂等由快照 `snap_id` 决定，这个值不参与判重。
    cmd_id: String,
    /// 网关的 DSNAP 记录：单个对象，或 `batch_size > 1` 时的对象数组。
    content: Value,
}

/// 一条记录的失败原因。回给数据面，也会进它的日志 —— 所以要写清是哪一步。
#[derive(Debug, Clone, Serialize)]
struct IngestFailure {
    /// 记录在本次请求里的序号（`batch_size = 1` 时恒为 0）。
    index: usize,
    /// `permanent` = 改成合法数据才能过；`transient` = 下一轮可能就成。
    kind: &'static str,
    detail: String,
}

fn permanent_failure(index: usize, detail: impl Into<String>) -> IngestFailure {
    IngestFailure {
        index,
        kind: "permanent",
        detail: detail.into(),
    }
}

fn transient_failure(index: usize, detail: impl Into<String>) -> IngestFailure {
    IngestFailure {
        index,
        kind: "transient",
        detail: detail.into(),
    }
}

/// `POST /internal/atlas`：收 wparse `agent-discovery` sink 分组投来的记录。
///
/// 这里**不要求** `Content-Type`：数据面的 http 连接器默认不带它，用 415 拒掉一帧合法快照
/// 是最难排查的一类故障。
pub(crate) async fn ingest_atlas(
    State(state): State<HttpAppState>,
    Query(query): Query<EdgeIngestQuery>,
    body: Bytes,
) -> Response {
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(err) => {
            return ingest_error(StatusCode::BAD_REQUEST, format!("body is not JSON: {err}"));
        }
    };
    let write: AtlasWrite = match serde_json::from_value(payload) {
        Ok(write) => write,
        Err(err) => {
            return ingest_error(
                StatusCode::BAD_REQUEST,
                format!("body must be {{cust_id, cmd_id, content}}: {err}"),
            );
        }
    };

    let tenant_ref = write.cust_id.trim().to_string();
    let tenant_id = match Uuid::parse_str(&tenant_ref) {
        Ok(id) => TenantId(id),
        Err(_) => {
            return ingest_error(
                StatusCode::BAD_REQUEST,
                format!(
                    "cust_id `{tenant_ref}` is not a UUID; it is the atlas tenant id, \
                     not the external tenant ref"
                ),
            );
        }
    };
    if write.cmd_id.trim().is_empty() {
        return ingest_error(StatusCode::BAD_REQUEST, "cmd_id must not be empty");
    }

    // 数组是 `batch_size > 1` 的形态；单对象是默认形态。两种都收。
    let records = match write.content {
        Value::Array(items) => items,
        other => vec![other],
    };
    if records.is_empty() {
        // 空数组不是「这台机器什么都没有」——那是「请求发错了」，别让它被当成一次空快照。
        return ingest_error(
            StatusCode::BAD_REQUEST,
            "empty content: a discovery snapshot request must carry at least one record",
        );
    }
    if records.len() > MAX_RECORDS_PER_REQUEST {
        return ingest_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "{} records in one request exceeds the limit of {MAX_RECORDS_PER_REQUEST}",
                records.len()
            ),
        );
    }

    let env = query
        .env
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());

    ingest_records(
        state.app(),
        tenant_id,
        tenant_ref,
        env,
        records,
    )
    .await
}

/// 一行 sink 记录的失败分级：`permanent` 重试无用（进 rescue 让人看），
/// `transient` 下一轮可能就好（让数据面按 `max_retries` 重试）。
#[derive(Debug)]
pub(crate) enum LineError {
    Permanent(String),
    Transient(String),
}

/// 把一行 sink 记录转成 atlas 输入并物化 —— HTTP 端点与写探针共用的**唯一**入口。
pub(crate) async fn submit_line_for_monolith(
    app: &TopologyMonolith,
    tenant_id: TenantId,
    tenant_ref: &str,
    line: &str,
    env: Option<&str>,
) -> Result<PipelineRunSummary, LineError> {
    match app {
        TopologyMonolith::Memory(store) => {
            submit_line(store.clone(), tenant_id, tenant_ref, line, env).await
        }
        TopologyMonolith::PostgresMock(store) => {
            submit_line(store.clone(), tenant_id, tenant_ref, line, env).await
        }
        TopologyMonolith::PostgresLive(store) => {
            submit_line(store.clone(), tenant_id, tenant_ref, line, env).await
        }
    }
}

async fn submit_line<S>(
    store: S,
    tenant_id: TenantId,
    tenant_ref: &str,
    line: &str,
    env: Option<&str>,
) -> Result<PipelineRunSummary, LineError>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncIngestStore + AsyncGovernanceStore,
{
    let input = convert_sink_line(line, tenant_ref, env).map_err(LineError::Permanent)?;
    TopologyIngestService::new(store)
        .submit_atlas_input_and_materialize_async(input, tenant_id, None)
        .await
        .map(|(_record, summary)| summary)
        // 写库失败通常是数据库暂时不可用：回非 2xx 让数据面重试，而不是静默吞掉。
        .map_err(|err| LineError::Transient(err.to_string()))
}

async fn ingest_records(
    app: &TopologyMonolith,
    tenant_id: TenantId,
    tenant_ref: String,
    env: Option<String>,
    records: Vec<Value>,
) -> Response {
    let mut ingested = 0usize;
    let mut hosts = 0usize;
    let mut processes_marked_stale = 0u64;
    let mut failures: Vec<IngestFailure> = Vec::new();

    for (index, record) in records.iter().enumerate() {
        // 端点收的就是 sink 行；转回字符串再走与文件路径**同一条**转换，
        // 两条入口的形状差异（多套一层 / 少套一层）会在这里显形，而不是各写一份解析。
        let line = match serde_json::to_string(record) {
            Ok(line) => line,
            Err(err) => {
                failures.push(permanent_failure(index, format!("encode record: {err}")));
                continue;
            }
        };

        match submit_line_for_monolith(app, tenant_id, &tenant_ref, &line, env.as_deref()).await {
            Ok(summary) => {
                ingested += 1;
                hosts += summary.host_count;
                processes_marked_stale += summary.stale_process_count;
            }
            Err(LineError::Permanent(detail)) => failures.push(permanent_failure(index, detail)),
            Err(LineError::Transient(detail)) => failures.push(transient_failure(index, detail)),
        }
    }

    let status = if failures.is_empty() {
        StatusCode::OK
    } else if failures.iter().any(|failure| failure.kind == "transient") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::BAD_REQUEST
    };

    (
        status,
        Json(json!({
            "status": if failures.is_empty() { "accepted" } else { "rejected" },
            "count": ingested + failures.len(),
            "ingested": ingested,
            "rejected": failures.len(),
            "hosts": hosts,
            "processes_marked_stale": processes_marked_stale,
            "failures": failures,
        })),
    )
        .into_response()
}

fn ingest_error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({
            "status": "error",
            "code": status.as_u16().to_string(),
            "message": message.into(),
        })),
    )
        .into_response()
}
