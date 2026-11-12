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
//! ## 为什么每个客户各写一份
//!
//! 0.5.0 去掉了默认库：**没有“公共库”可以放一个保留租户的探针**了。写进一个不存在的库
//! （已删）或一个占位内存库（探针什么也没落）都是最坏的「绿」——`/healthz` 报 `last_ok: true`
//! 而现场一张表都没写。所以探针的目标就是**客户清单本身**：每个客户一个
//! [`Target`]（该客户的库 + 该客户的 `tenant_id`），每拍各投一次，任一客户失败整拍就算失败
//! 并点名是哪个客户 —— 与 `wist-evidence` 0.3.0 的「每客户一份探针」同一条纪律。
//!
//! ## 为什么不会把图谱写脏
//!
//! 探针用固定的 `snap_id`（[`PROBE_SNAP_ID`]）：`ingest_id` 由租户 + snap_id 派生，
//! 重复投递是**同一次 upsert**，不会随时间增长。写入的主机名固定是 [`PROBE_HOST`]，
//! 星象台的主机列表按这个名字把它过滤掉（见 `http.rs::host_list_response`），
//! 所以它对使用者不可见，但数据真的在库里 —— 这正是探针要证明的东西。

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use atlas_domain::TenantId;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::TopologyMonolith;

/// 探针固定的快照 id：固定才能让重复投递变成同一个 ingest（表不增长）。
const PROBE_SNAP_ID: &str = "atlas-probe";

/// 探针写入的主机名：图谱里搜 `wist-atlas-probe` 就知道这条记录的来历。
pub const PROBE_HOST: &str = "wist-atlas-probe";

/// 一个客户的探针目标：**写进这个客户自己的库**，租户也用这个客户自己的。
///
/// 两个字段都来自客户清单（`cells::CellRouter`），不从环境变量再编一份 ——
/// 探针写的库必须与 HTTP 写路径选中的是同一个，否则它证明不了数据面的写入能力。
pub struct Target {
    pub cust_id: String,
    pub tenant: TenantId,
    pub app: TopologyMonolith,
}

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
pub fn spawn(targets: Vec<Target>, status: SharedProbe, interval: Duration) {
    if targets.is_empty() {
        // 没有客户就没有「写进去」这件事。宁可让 `last_ok` 停在 `null`（“还没探过”），
        // 也不要把一个空循环记成绿 —— 那正是这次要修掉的误报。
        log_line("atlas_probe_disabled: no customer cells to write into");
        return;
    }
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let (failures, latency_ms) = probe_once(&targets).await;
            if failures.is_empty() {
                log_line("atlas_probe_ok");
            } else {
                log_line(&format!("atlas_probe_failed: {}", failures.join("; ")));
            }
            if let Ok(mut status) = status.write() {
                status.record(failures.is_empty(), latency_ms);
            }
        }
    });
}

fn log_line(message: &str) {
    eprintln!("{message}");
}

/// 探一拍：每个客户各写一次。返回失败清单（空 = 全部成功）与**最慢那个客户**的延迟。
///
/// 「最慢的客户」才是这拍的延迟：最快的那个不代表任何一个客户能按时写完。
async fn probe_once(targets: &[Target]) -> (Vec<String>, u64) {
    let mut failures = Vec::new();
    let mut max_latency_ms = 0u64;
    for target in targets {
        let started = Instant::now();
        let outcome = write_probe(target).await;
        max_latency_ms = max_latency_ms.max(started.elapsed().as_millis() as u64);
        if let Err(detail) = outcome {
            failures.push(format!("{}: {detail}", target.cust_id));
        }
    }
    (failures, max_latency_ms)
}

async fn write_probe(target: &Target) -> Result<(), String> {
    crate::ingest_http::submit_line_for_monolith(
        &target.app,
        target.tenant,
        &target.cust_id,
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
    use atlas_storage::{CatalogStore, Page};

    fn probe_target(cust_id: &str, tenant: uuid::Uuid) -> (Target, TopologyMonolith) {
        let app = TopologyMonolith::new_in_memory();
        (
            Target {
                cust_id: cust_id.to_string(),
                tenant: TenantId(tenant),
                app: app.clone(),
            },
            app,
        )
    }

    #[test]
    fn probe_line_is_a_parseable_sink_record() {
        let line = probe_sink_line();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["biz"], "agent-discovery");
        assert!(value["body"].as_str().unwrap().contains(PROBE_SNAP_ID));
    }

    #[tokio::test]
    async fn a_probe_tick_writes_into_every_customer_database_with_that_customers_tenant() {
        let tenant_a = uuid::Uuid::new_v4();
        let tenant_b = uuid::Uuid::new_v4();
        let (target_a, app_a) = probe_target("c-a", tenant_a);
        let (target_b, app_b) = probe_target("c-b", tenant_b);

        let (failures, _latency_ms) = probe_once(&[target_a, target_b]).await;
        assert!(failures.is_empty(), "{failures:?}");

        // 每个客户自己的库里都有一条探针主机，且挂在该客户自己的租户下。
        for (app, tenant) in [(app_a.clone(), tenant_a), (app_b.clone(), tenant_b)] {
            let hosts = app
                .store()
                .unwrap()
                .list_hosts(TenantId(tenant), Page::default())
                .unwrap();
            assert_eq!(hosts.len(), 1, "tenant {tenant}");
            assert_eq!(hosts[0].host_name, PROBE_HOST);
            assert_eq!(hosts[0].tenant_id, TenantId(tenant));
        }

        // 重复探同一拍还是同一行 upsert：表不随时间增长。
        let again = Target {
            cust_id: "c-a".to_string(),
            tenant: TenantId(tenant_a),
            app: app_a.clone(),
        };
        let (failures, _) = probe_once(&[again]).await;
        assert!(failures.is_empty(), "{failures:?}");
        let hosts = app_a
            .store()
            .unwrap()
            .list_hosts(TenantId(tenant_a), Page::default())
            .unwrap();
        assert_eq!(hosts.len(), 1);
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
