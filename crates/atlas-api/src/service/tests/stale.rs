//! 快照替换语义（presence / stale）的端到端用例。
//!
//! 这一组守的是「资产目录会不会变成噪声」：`process_id` 是稳定 UUID + upsert 写入，
//! 没有替换语义的话退出的进程会永久停在现役视图里，状态还是退出前的那个值。

use atlas_domain::{IngestEnvelope, IngestMode, SourceKind, TenantId};
use atlas_storage::{CatalogStore, Page, RuntimeStore};
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::TopologyIngestService;
use crate::query::TopologyQueryService;

const HOST_NAME: &str = "node-stale-01";

fn host_payload() -> serde_json::Value {
    json!({
        "hosts": [{
            "hostname": HOST_NAME,
            "machine_id": "machine-stale-01",
            "external_ref": "machine-stale-01"
        }]
    })
}

fn payload_with_processes(pids: &[i32]) -> serde_json::Value {
    let mut payload = host_payload();
    payload["processes"] = serde_json::Value::Array(
        pids.iter()
            .map(|pid| {
                json!({
                    "pid": pid,
                    "host_ref": HOST_NAME,
                    "machine_id": "machine-stale-01",
                    "identity": format!("ps_lstart:{pid}"),
                    "executable_name": format!("/bin/proc-{pid}")
                })
            })
            .collect(),
    );
    payload
}

fn envelope(
    ingest_id: &str,
    producer: &str,
    mode: IngestMode,
    tenant_id: TenantId,
    payload: serde_json::Value,
) -> IngestEnvelope {
    IngestEnvelope {
        ingest_id: ingest_id.to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: format!("wist:{producer}"),
        ingest_mode: mode,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(payload),
        metadata: Default::default(),
    }
}

fn snapshot(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    ingest_id: &str,
    pids: &[i32],
    tenant_id: TenantId,
) -> u64 {
    let (_record, summary) = service
        .submit_and_materialize(envelope(
            ingest_id,
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            payload_with_processes(pids),
        ))
        .expect("snapshot ingest");
    summary.stale_process_count
}

/// 某台主机的全部进程（含 stale），按 pid 排序。
fn all_processes(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
) -> Vec<(i32, bool)> {
    let host = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_NAME)
        .expect("host materialized");
    let mut processes = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        Page {
            limit: 500,
            offset: 0,
        },
    )
    .expect("list processes");
    processes.sort_by_key(|process| process.pid);
    processes
        .into_iter()
        .map(|process| (process.pid, process.stale_at.is_some()))
        .collect()
}

#[test]
fn a_process_missing_from_the_next_snapshot_becomes_stale() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    assert_eq!(snapshot(&service, "ing-1", &[100, 200, 300], tenant_id), 0);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, false), (300, false)]
    );

    // 200 退出了。
    assert_eq!(snapshot(&service, "ing-2", &[100, 300], tenant_id), 1);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, true), (300, false)],
        "行必须保留（历史可追溯），但必须被标成 stale"
    );
}

#[test]
fn stale_at_keeps_the_first_disappearance_time() {
    // 「什么时候没的」要能回答。后续快照反复确认它不在，不能把时间往后推。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    snapshot(&service, "ing-1", &[100, 200], tenant_id);

    snapshot(&service, "ing-2", &[100], tenant_id);
    let first = process_stale_at(&service, tenant_id, 200);

    snapshot(&service, "ing-3", &[100], tenant_id);
    snapshot(&service, "ing-4", &[100], tenant_id);
    let later = process_stale_at(&service, tenant_id, 200);

    assert_eq!(first, later, "已是 stale 的行不应被再次改写");
}

fn process_stale_at(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
    pid: i32,
) -> Option<chrono::DateTime<Utc>> {
    let host = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_NAME)
        .expect("host");
    RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        Page {
            limit: 500,
            offset: 0,
        },
    )
    .expect("list processes")
    .into_iter()
    .find(|process| process.pid == pid)
    .expect("process row kept")
    .stale_at
}

#[test]
fn a_process_that_comes_back_is_revived() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(&service, "ing-1", &[100, 200], tenant_id);
    snapshot(&service, "ing-2", &[100], tenant_id);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, true)]
    );

    // 200 又出现了（同 pid + 同 identity ⇒ 同 process_id）。
    snapshot(&service, "ing-3", &[100, 200], tenant_id);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, false)],
        "重新出现必须清掉 stale 标记，而不是留下一个「已死但还在跑」的行"
    );
}

#[test]
fn an_empty_process_list_does_not_mark_everything_stale() {
    // 采集失败和「确实没有进程」在数据上不可区分。宁可晚一轮清理，也不能一次误标一批活进程。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(&service, "ing-1", &[100, 200], tenant_id);
    let marked = snapshot(&service, "ing-2", &[], tenant_id);

    assert_eq!(marked, 0);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, false)],
        "空列表不算「全没了」"
    );
}

#[test]
fn processes_from_another_producer_scope_are_not_marked_stale() {
    // 同一台主机可能被多个生产者看着（如容器采集器）。按 host 一刀切会清掉别人的进程。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    service
        .submit_and_materialize(envelope(
            "ing-other",
            "container-agent",
            IngestMode::Snapshot,
            tenant_id,
            payload_with_processes(&[900]),
        ))
        .expect("other producer ingest");

    let marked = snapshot(&service, "ing-mine", &[100], tenant_id);

    assert_eq!(marked, 0, "别的作用域的进程不该被本次清扫波及");
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (900, false)]
    );
}

#[test]
fn incremental_mode_never_sweeps() {
    // `incremental` / `window` / `correction` 描述的都是**部分**观测，不能当全集比对。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(&service, "ing-1", &[100, 200], tenant_id);

    let (_record, summary) = service
        .submit_and_materialize(envelope(
            "ing-2",
            "agent-1",
            IngestMode::BatchUpsert,
            tenant_id,
            payload_with_processes(&[100]),
        ))
        .expect("batch upsert");

    assert_eq!(summary.stale_process_count, 0);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, false)]
    );
}

#[test]
fn the_sweep_is_skipped_when_a_candidate_cannot_be_resolved() {
    // 宿主解析不到的条目既没物化、也不在 `seen` 里。此时清扫会把它的旧行误标成 stale ——
    // 「看不懂」和「不存在」在数据上不可区分，所以宁可不扫。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(&service, "ing-1", &[100, 200], tenant_id);

    let mut payload = payload_with_processes(&[100]);
    // 多塞一个指向不存在主机的进程（用**不同的** pid，否则它可能被当成 200 的重复观测，
    // 测试会因为错误的原因通过）。
    payload["processes"]
        .as_array_mut()
        .expect("array")
        .push(json!({
            "pid": 999,
            "host_ref": "some-unknown-host",
            "identity": "ps_lstart:999",
            "executable_name": "/bin/proc-999"
        }));

    let (_record, summary) = service
        .submit_and_materialize(envelope(
            "ing-2",
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            payload,
        ))
        .expect("ingest");

    assert_eq!(summary.stale_process_count, 0);
    assert_eq!(
        all_processes(&service, tenant_id),
        vec![(100, false), (200, false)],
        "快照不完整时宁可不清扫"
    );
}

#[test]
fn stale_processes_disappear_from_the_live_views() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(&service, "ing-1", &[100, 200], tenant_id);
    snapshot(&service, "ing-2", &[100], tenant_id);

    let host = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_NAME)
        .expect("host");

    let view = TopologyQueryService::new(service.store().clone())
        .host_process_topology_view(host.host_id)
        .expect("query")
        .expect("view present");

    let pids: Vec<i32> = view
        .processes
        .iter()
        .map(|process| process.pid)
        .collect::<Vec<_>>();
    assert!(
        pids.contains(&100),
        "活进程必须在现役视图里，实际: {pids:?}"
    );
    assert!(
        !pids.contains(&200),
        "stale 进程不该出现在现役视图里，实际: {pids:?}"
    );
}

#[test]
fn ingest_scope_is_recorded_on_the_process_row() {
    // 清扫靠 scope 划边界；写不进去就退化成「按 host 一刀切」。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let mut envelope = envelope(
        "ing-scope",
        "agent-1",
        IngestMode::Snapshot,
        tenant_id,
        payload_with_processes(&[100]),
    );
    envelope
        .metadata
        .insert("schema".to_string(), "atlas.in.edge.v1".to_string());

    service
        .submit_and_materialize(envelope)
        .expect("snapshot ingest");

    let host = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_NAME)
        .expect("host");
    let process = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        Page {
            limit: 500,
            offset: 0,
        },
    )
    .expect("list")
    .into_iter()
    .find(|process| process.pid == 100)
    .expect("process");

    assert_eq!(
        process.ingest_scope.as_deref(),
        Some(format!("atlas.in.edge.v1:wist:agent-1:{}:", tenant_id.0).as_str())
    );
}

#[test]
fn the_ingest_scope_is_stable_across_snapshots() {
    // 作用域一变，第二次快照就找不到第一次的行，清扫永远不生效（静默失效）。
    let tenant_id = TenantId(Uuid::new_v4());
    let first = envelope("a", "agent-1", IngestMode::Snapshot, tenant_id, json!({}));
    let second = envelope("b", "agent-1", IngestMode::Snapshot, tenant_id, json!({}));
    assert_eq!(first.ingest_scope(), second.ingest_scope());

    let other_producer = envelope("c", "agent-2", IngestMode::Snapshot, tenant_id, json!({}));
    assert_ne!(first.ingest_scope(), other_producer.ingest_scope());
}

#[test]
fn the_ingest_scope_uses_the_declared_env_not_the_unresolved_environment_id() {
    // CLI 导入路径不传 `environment_id`。只看它会得到空串，让两个环境共用一个作用域 ——
    // 那么 dev 的快照就会把 prod 的进程标死。
    let tenant_id = TenantId(Uuid::new_v4());
    let dev = {
        let mut envelope = envelope("a", "agent-1", IngestMode::Snapshot, tenant_id, json!({}));
        envelope
            .metadata
            .insert("source.env".to_string(), "dev".to_string());
        envelope
    };
    let prod = {
        let mut envelope = envelope("b", "agent-1", IngestMode::Snapshot, tenant_id, json!({}));
        envelope
            .metadata
            .insert("source.env".to_string(), "prod".to_string());
        envelope
    };

    assert!(
        dev.ingest_scope().ends_with(":dev"),
        "{}",
        dev.ingest_scope()
    );
    assert!(
        prod.ingest_scope().ends_with(":prod"),
        "{}",
        prod.ingest_scope()
    );
    assert_ne!(dev.ingest_scope(), prod.ingest_scope());
}

#[test]
fn a_stale_row_keeps_its_original_observation_time() {
    // 标 stale 只是加一个时间点，不能顺带把 observed_at 改成「现在」——
    // 否则「这个进程最后一次被看到是什么时候」就失真了。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    snapshot(&service, "ing-1", &[100, 200], tenant_id);

    let host = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_NAME)
        .expect("host");
    let before = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        Page {
            limit: 500,
            offset: 0,
        },
    )
    .expect("list")
    .into_iter()
    .find(|process| process.pid == 200)
    .expect("process")
    .observed_at;

    snapshot(&service, "ing-2", &[100], tenant_id);

    let after = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        Page {
            limit: 500,
            offset: 0,
        },
    )
    .expect("list")
    .into_iter()
    .find(|process| process.pid == 200)
    .expect("process");

    assert_eq!(after.observed_at, before);
    assert!(after.stale_at.is_some());
    assert!(
        after.stale_at.expect("stale") >= before.0 - Duration::seconds(1),
        "stale_at 不早于最后一次观测"
    );
}

// ── 主机退役：连续缺失计数，不是「一份没看到就退役」 ──────────────
//
// 注意这里必须用**多主机快照**才能测到清扫：一台主机一个信封时，信封里没它
// 就意味着这份数据根本不涉及它，`seen` 也不会是「少了某一个」。
// 当前适配器是一台机一个信封，所以主机退役实际上要等多主机快照（网关侧聚合 /
// CMDB 同步）才会真正触发 —— 机制先就位，并用下面这组用例守住。

const HOST_A: &str = "node-a";
const HOST_B: &str = "node-b";

fn payload_with_hosts(hosts: &[(&str, &str)]) -> serde_json::Value {
    json!({
        "hosts": hosts
            .iter()
            .map(|(name, machine_id)| json!({
                "hostname": name,
                "machine_id": machine_id,
                "external_ref": machine_id
            }))
            .collect::<Vec<_>>()
    })
}

fn snapshot_hosts(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    ingest_id: &str,
    hosts: &[(&str, &str)],
    tenant_id: TenantId,
) -> u64 {
    let (_record, summary) = service
        .submit_and_materialize(envelope(
            ingest_id,
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            payload_with_hosts(hosts),
        ))
        .expect("snapshot ingest");
    summary.stale_host_count
}

fn host_state(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
    host_name: &str,
) -> Option<(i64, bool)> {
    CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == host_name)
        .map(|host| (host.miss_count, host.stale_at.is_some()))
}

/// 阈值只在 atlas-api 内部定义，测试里对称取用。
fn atlas_api_host_threshold() -> i64 {
    crate::service::materialize::HOST_STALE_AFTER_MISSES
}

fn both_hosts() -> Vec<(&'static str, &'static str)> {
    vec![(HOST_A, "machine-a"), (HOST_B, "machine-b")]
}

fn only_a() -> Vec<(&'static str, &'static str)> {
    vec![(HOST_A, "machine-a")]
}

#[test]
fn a_host_is_not_retired_after_a_single_missing_snapshot() {
    // 主机消失可能只是一轮探针失败。误标一台活主机的代价远大于晚几轮。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((0, false)));

    snapshot_hosts(&service, "ing-2", &only_a(), tenant_id);
    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((1, false)),
        "缺一份只累计计数，不退役"
    );
    assert_eq!(
        host_state(&service, tenant_id, HOST_A),
        Some((0, false)),
        "出现过的主机计数归零"
    );
}

#[test]
fn a_host_is_retired_after_the_miss_threshold() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);

    let mut newly_stale = 0;
    for round in 0..atlas_api_host_threshold() {
        newly_stale = snapshot_hosts(&service, &format!("ing-miss-{round}"), &only_a(), tenant_id);
    }

    assert_eq!(newly_stale, 1, "达到阈值那一轮才报「新退役」1 台");
    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((atlas_api_host_threshold(), true)),
        "计数停在达阈值的位置，且已标 stale"
    );
}

#[test]
fn a_retired_host_keeps_its_first_retirement_time() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);
    for round in 0..atlas_api_host_threshold() {
        snapshot_hosts(&service, &format!("ing-miss-{round}"), &only_a(), tenant_id);
    }

    let before = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_B)
        .expect("host");
    let first = before.stale_at.expect("retired");

    for round in 0..3 {
        let newly = snapshot_hosts(
            &service,
            &format!("ing-extra-{round}"),
            &only_a(),
            tenant_id,
        );
        assert_eq!(newly, 0, "已退役的不重复计入");
    }
    let after = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_B)
        .expect("host");

    assert_eq!(after.stale_at, Some(first), "首次退役时间必须冻结");
    assert_eq!(after.miss_count, before.miss_count, "已退役的不再累加");
}

#[test]
fn a_returning_host_is_revived_and_its_counter_cleared() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);
    // 缺到刚好差一轮就退役。
    for round in 0..atlas_api_host_threshold() - 1 {
        snapshot_hosts(&service, &format!("ing-miss-{round}"), &only_a(), tenant_id);
    }
    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((atlas_api_host_threshold() - 1, false))
    );

    // 又出现了：计数归零。
    snapshot_hosts(&service, "ing-back", &both_hosts(), tenant_id);
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((0, false)));

    // 之后再缺一轮，也只是从 1 重新开始 —— 证明「一次抖动不会累积成退役」。
    snapshot_hosts(&service, "ing-miss-again", &only_a(), tenant_id);
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((1, false)));
}

#[test]
fn hosts_from_another_producer_scope_are_not_swept() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    // 另一个生产者报 node-b。
    service
        .submit_and_materialize(envelope(
            "ing-other",
            "container-agent",
            IngestMode::Snapshot,
            tenant_id,
            payload_with_hosts(&[(HOST_B, "machine-b")]),
        ))
        .expect("other producer");

    // 我们的快照里一直没有 node-b。
    for round in 0..atlas_api_host_threshold() {
        snapshot_hosts(&service, &format!("ing-mine-{round}"), &only_a(), tenant_id);
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "别的作用域报的主机不该被我们的缺失计数拖成退役"
    );
}

#[test]
fn incremental_mode_never_accumulates_host_misses() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);

    for round in 0..atlas_api_host_threshold() + 2 {
        let (_record, summary) = service
            .submit_and_materialize(envelope(
                &format!("ing-batch-{round}"),
                "agent-1",
                IngestMode::BatchUpsert,
                tenant_id,
                payload_with_hosts(&only_a()),
            ))
            .expect("batch upsert");
        assert_eq!(summary.stale_host_count, 0);
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "部分观测不能当成「主机不在了」"
    );
}
