//! 写探针：定期往**自己的写路径**投一份最小 DSNAP 快照，验证「转换 → 落库 → 物化」真的能跑。
//!
//! ## 为什么不是真发 HTTP
//!
//! `wist-evidence` 的探针走回环 HTTP，因为它要证明「sink 能写进来」。atlas 这里刻意不这么做：
//!
//! - 探针要在**进程内**跑，就必须再引一个 HTTP 客户端（atlas 现在没有），
//!   而这条链路上最容易坏的是转换与数据库，不是 axum 的路由；
//! - HTTP 层的形状由契约测试固定（`crates/atlas-app/src/tests.rs` 里打的是真 router）。
//!
//! 所以探针调用的是**同一个** `convert_sink_line` + `submit_atlas_input_and_materialize_async`，
//! 与 HTTP / 文件两条入口共用一份逻辑。
//!
//! ## 为什么不会把图谱写脏
//!
//! 探针固定写进一个**保留租户**（[`PROBE_TENANT`]）与固定的 `snap_id`（[`PROBE_SNAP_ID`]）：
//! `ingest_id` 由租户 + snap_id 派生，重复投递是**同一次 upsert**，不会随时间增长。
//! 星象台的资产页从不查询这个租户，所以它对使用者不可见 —— 与 evidence 的 `_probe` 同一条纪律。

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::TopologyMonolith;

/// 探针的保留租户：任何真实业务都不该用这个 UUID。
pub const PROBE_TENANT: &str = "00000000-0000-4000-8000-00000000a7a5";

/// 探针固定的快照 id：固定才能让重复投递变成同一个 ingest（表不增长）。
const PROBE_SNAP_ID: &str = "atlas-probe";

/// 探针写入的主机名：图谱里搜 `wist-atlas-probe` 就知道这条记录的来历。
pub const PROBE_HOST: &str = "wist-atlas-probe";

/// 探针现状：给 `/healthz` 看，让星象台的「接口可用性」页能区分
/// 「进程活着」与「真的写得进去」。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProbeStatus {
    /// 上一次探针是否成功；服务刚起、还没探过时是 `null`。
    pub last_ok: Option<bool>,
    pub last_at: Option<DateTime<Utc>>,
    pub last_at_ms: Option<u64>,
    pub last_latency_ms: Option<u64>,
    /// 连续失败次数（成功即归零）。
    pub consecutive_failures: u32,
}

impl ProbeStatus {
    fn record(&mut self, ok: bool, latency_ms: u64) {
        let now = Utc::now();
        self.last_ok = Some(ok);
        self.last_at = Some(now);
        self.last_at_ms = Some(now.timestamp_millis().max(0) as u64);
        self.last_latency_ms = Some(latency_ms);
        if ok {
            self.consecutive_failures = 0;
        } else {
            self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        }
    }
}

/// 共享状态：HTTP 层读、探针循环写。
pub type SharedProbe = Arc<RwLock<ProbeStatus>>;

pub fn new_shared() -> SharedProbe {
    Arc::new(RwLock::new(ProbeStatus::default()))
}

/// 启动探针循环。第一拍立即执行（进程刚起来就能在页面上看到结果）。
pub fn spawn(app: TopologyMonolith, status: SharedProbe, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let started = Instant::now();
            let outcome = write_probe(&app).await;
            let latency_ms = started.elapsed().as_millis() as u64;
            match &outcome {
                Ok(()) => {
                    log_line("atlas_probe_ok");
                }
                Err(detail) => {
                    log_line(&format!("atlas_probe_failed: {detail}"));
                }
            }
            if let Ok(mut status) = status.write() {
                status.record(outcome.is_ok(), latency_ms);
            }
        }
    });
}

fn log_line(message: &str) {
    eprintln!("{message}");
}

async fn write_probe(app: &TopologyMonolith) -> Result<(), String> {
    let tenant_id = uuid::Uuid::parse_str(PROBE_TENANT)
        .map(atlas_domain::TenantId)
        .map_err(|err| format!("probe tenant is not a UUID: {err}"))?;
    crate::ingest_http::submit_line_for_monolith(
        app,
        tenant_id,
        PROBE_TENANT,
        &probe_sink_line(),
        Some("probe"),
    )
    .await
    .map(|_| ())
    .map_err(|err| match err {
        crate::ingest_http::LineError::Permanent(detail) => format!("permanent: {detail}"),
        crate::ingest_http::LineError::Transient(detail) => format!("transient: {detail}"),
    })
}

/// 一份最小但**真形状**的 DSNAP sink 行：1 台主机、无进程、无网络段。
///
/// 形状必须和网关 `agent-discovery.jsonl` 的一行一致，否则探针验的就不是真实路径了。
fn probe_sink_line() -> String {
    let observed_at = Utc::now().to_rfc3339();
    let report = serde_json::json!({
        "api_version": "v1",
        "kind": "report_discovery_snapshot",
        "report_id": format!("disc_{PROBE_HOST}"),
        "agent_id": format!("agent-{PROBE_HOST}"),
        "instance_id": format!("agent-{PROBE_HOST}"),
        "snapshot_id": PROBE_SNAP_ID,
        "revision": 1,
        "generated_at": observed_at,
        "report_attempt": 1,
        "report_mode": "full_snapshot",
        "reported_at": observed_at,
        "snapshot": {
            "schema_version": "v1",
            "snapshot_id": PROBE_SNAP_ID,
            "revision": 1,
            "generated_at": observed_at,
            "origins": [],
            "resources": [{
                "resource_id": format!("machine-{PROBE_HOST}"),
                "kind": "host",
                "origin_idx": 0,
                "attributes": {
                    "host.id": format!("machine-{PROBE_HOST}"),
                    "host.name": PROBE_HOST,
                    "os.name": "linux",
                    "os.type": "linux",
                    "os.version": "probe",
                    "host.arch": "unknown"
                },
                "discovered_at": observed_at,
                "last_seen_at": observed_at,
                "health": "healthy",
                "source": "probe"
            }],
            "targets": []
        }
    });

    serde_json::json!({
        "schema": "v1",
        "agent_id": format!("agent-{PROBE_HOST}"),
        "observed_at": observed_at,
        "seq": 1,
        "category": "agent.discovery",
        "log_desc": "atlas 写探针",
        "body": report.to_string(),
        "biz": "agent-discovery",
        "env": "probe",
        "sink": "json",
        "wp_oml_name": "agent_discovery_snapshot"
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_line_is_a_parseable_sink_record() {
        let line = probe_sink_line();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["biz"], "agent-discovery");
        assert!(value["body"].as_str().unwrap().contains(PROBE_SNAP_ID));
    }

    #[test]
    fn consecutive_failures_accumulate_and_reset() {
        let mut status = ProbeStatus::default();
        assert_eq!(status.last_ok, None);
        status.record(false, 1);
        status.record(false, 2);
        assert_eq!(status.consecutive_failures, 2);
        status.record(true, 3);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.last_latency_ms, Some(3));
    }
}
