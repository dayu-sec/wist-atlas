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

// ── 主机退役：只信「账号级完整快照」，不信「我们没看到」────────────
//
// 这里守的是一条产品判断：**采集不到 ≠ 不存在**。关机、agent 被卸、网络不通，
// 在 atlas 侧是同一件事 —— 没有新观测。三者都不说明机器没了，所以端侧快照
// 永远不能把主机退役。唯一能判死的是账号级完整快照：云上那张资产表就是「它存在」
// 的定义，但前提是这次真把全地域全类型采全了（`collect.coverage`）。

const HOST_A: &str = "node-a";
const HOST_B: &str = "node-b";

/// 端侧生产者与云侧生产者分开：退役判据靠 `ingest_scope` 区分，同名就测不出边界。
const EDGE_PRODUCER: &str = "agent-1";
const CLOUD_PRODUCER: &str = "harvest-1";

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
            EDGE_PRODUCER,
            IngestMode::Snapshot,
            tenant_id,
            payload_with_hosts(hosts),
        ))
        .expect("snapshot ingest");
    summary.stale_host_count
}

/// 云账号键。atlas 只做相等比较，不解析它的形状。
const CLOUD_ACCOUNT: &str = "tencent:1300000000";
const OTHER_ACCOUNT: &str = "tencent:1400000000";

/// 云侧资产行：`cloud_instance_id` + `cloud_account`（退役判定的两个前提）。
fn cloud_payload(hosts: &[(&str, &str)], account: &str) -> serde_json::Value {
    json!({
        "hosts": hosts
            .iter()
            .map(|(name, instance_id)| json!({
                "hostname": name,
                "cloud_instance_id": instance_id,
                "cloud_account": account,
            }))
            .collect::<Vec<_>>()
    })
}

/// 云账号级快照。`coverage = None` 表示没声明（等价于「什么也没说」）。
/// 参数多是因为每个维度都真的会被参数化（账号 / 声明 / 模式 / 生产者）。
#[allow(clippy::too_many_arguments)]
fn cloud_snapshot_hosts(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    ingest_id: &str,
    producer: &str,
    hosts: &[(&str, &str)],
    account: &str,
    coverage: Option<(&str, bool)>,
    mode: IngestMode,
    tenant_id: TenantId,
) -> u64 {
    let mut envelope = envelope(
        ingest_id,
        producer,
        mode,
        tenant_id,
        cloud_payload(hosts, account),
    );
    envelope
        .metadata
        .insert("schema".to_string(), "atlas.in.inventory.v1".to_string());
    if let Some((scope, complete)) = coverage {
        envelope
            .metadata
            .insert("collect.coverage.scope".to_string(), scope.to_string());
        envelope.metadata.insert(
            "collect.coverage.complete".to_string(),
            complete.to_string(),
        );
        // 账号键：退役目标靠它圈定，三处（coverage / 主机行 / 本函数）必须是同一个值。
        envelope
            .metadata
            .insert("collect.coverage.account".to_string(), account.to_string());
    }
    let (_record, summary) = service
        .submit_and_materialize(envelope)
        .expect("cloud ingest");
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

fn host_row(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
    host_name: &str,
) -> Option<atlas_domain::HostInventory> {
    CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == host_name)
}

/// 阈值只在 atlas-api 内部定义，测试里对称取用。
fn host_retire_threshold() -> i64 {
    crate::service::materialize::HOST_RETIRE_AFTER_COMPLETE_SNAPSHOTS
}

fn both_hosts() -> Vec<(&'static str, &'static str)> {
    vec![(HOST_A, "machine-a"), (HOST_B, "machine-b")]
}

fn only_a() -> Vec<(&'static str, &'static str)> {
    vec![(HOST_A, "machine-a")]
}

#[test]
fn hosts_missing_from_an_agent_snapshot_are_never_retired() {
    // 关机 / agent 被卸 / 网不通 —— 在 atlas 侧都是「这一轮没看到它」。
    // 什么都能缺，就是不能因此认定机器不存在。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);
    let before = host_row(&service, tenant_id, HOST_B).expect("host b");
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((0, false)));

    for round in 0..host_retire_threshold() + 2 {
        snapshot_hosts(&service, &format!("ing-miss-{round}"), &only_a(), tenant_id);
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "端侧缺席永不能累积成退役，连计数都不该动"
    );
    assert_eq!(
        host_row(&service, tenant_id, HOST_B)
            .expect("host b")
            .last_inventory_at,
        before.last_inventory_at,
        "没看到就不该刷新 last_inventory_at"
    );
}

#[test]
fn host_is_retired_only_after_complete_account_snapshots() {
    // 只有「账号级全量 + 采集完整」的快照能当「云上已不存在」的证据。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    cloud_snapshot_hosts(
        &service,
        "ing-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );

    // 差一轮时还不能退役。
    for round in 0..host_retire_threshold() - 1 {
        let newly = cloud_snapshot_hosts(
            &service,
            &format!("ing-miss-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
        assert_eq!(newly, 0, "没到阈值不退役");
    }

    let newly = cloud_snapshot_hosts(
        &service,
        "ing-threshold",
        CLOUD_PRODUCER,
        &only_a(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    assert_eq!(newly, 1, "达到阈值那一轮才报「新退役」1 台");
    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((host_retire_threshold(), true)),
        "计数停在达阈值的位置，且已标 stale"
    );
}

#[test]
fn a_snapshot_without_authoritative_coverage_never_retires_a_host() {
    // 「没声明」「声明了但没采全」「只盖了部分地域」—— 三者对存在性都什么也没说。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    cloud_snapshot_hosts(
        &service,
        "ing-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );

    let coverages = [Some(("partial", true)), Some(("account", false)), None];
    for (round, coverage) in coverages.into_iter().enumerate() {
        for repeat in 0..host_retire_threshold() {
            cloud_snapshot_hosts(
                &service,
                &format!("ing-{round}-{repeat}"),
                CLOUD_PRODUCER,
                &only_a(),
                CLOUD_ACCOUNT,
                coverage,
                IngestMode::Snapshot,
                tenant_id,
            );
        }
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "覆盖不扎实的快照不能当证据，连计数也不能推进"
    );
}

#[test]
fn a_cloud_snapshot_without_an_account_key_retires_nothing() {
    // 老采集器只发云实例 ID、没有账号键：该 upsert 就 upsert，什么也不能判死。
    // 这正是「宁可退不了役」的默认值。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let submit = |ingest_id: &str, payload: serde_json::Value| {
        let mut envelope = envelope(
            ingest_id,
            CLOUD_PRODUCER,
            IngestMode::Snapshot,
            tenant_id,
            payload,
        );
        envelope
            .metadata
            .insert("schema".to_string(), "atlas.in.inventory.v1".to_string());
        envelope
            .metadata
            .insert("collect.coverage.scope".to_string(), "account".to_string());
        envelope
            .metadata
            .insert("collect.coverage.complete".to_string(), "true".to_string());
        envelope.metadata.insert(
            "collect.coverage.account".to_string(),
            CLOUD_ACCOUNT.to_string(),
        );
        service
            .submit_and_materialize(envelope)
            .expect("cloud ingest")
            .1
            .stale_host_count
    };

    let rows = |hosts: &[(&str, &str)]| {
        json!({
            "hosts": hosts
                .iter()
                .map(|(name, instance_id)| json!({
                    "hostname": name,
                    "cloud_instance_id": instance_id,
                }))
                .collect::<Vec<_>>()
        })
    };

    submit("cloud-1", rows(&both_hosts()));
    for round in 0..host_retire_threshold() {
        assert_eq!(submit(&format!("cloud-miss-{round}"), rows(&only_a())), 0);
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "没有账号键就没有「本账号覆盖集」，不能判死"
    );
}

#[test]
fn an_empty_account_snapshot_retires_nothing() {
    // 空快照不等于「全没了」：采集器真的空账号也该先按 Aborted 处理，
    // atlas 这一层再兜一次 —— 空集合永不做退役比对。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    cloud_snapshot_hosts(
        &service,
        "cloud-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );

    for round in 0..host_retire_threshold() + 1 {
        let mut envelope = envelope(
            &format!("cloud-empty-{round}"),
            CLOUD_PRODUCER,
            IngestMode::Snapshot,
            tenant_id,
            json!({ "hosts": [] }),
        );
        envelope
            .metadata
            .insert("schema".to_string(), "atlas.in.inventory.v1".to_string());
        envelope
            .metadata
            .insert("collect.coverage.scope".to_string(), "account".to_string());
        envelope
            .metadata
            .insert("collect.coverage.complete".to_string(), "true".to_string());
        envelope.metadata.insert(
            "collect.coverage.account".to_string(),
            CLOUD_ACCOUNT.to_string(),
        );
        let summary = service
            .submit_and_materialize(envelope)
            .expect("empty cloud ingest")
            .1;
        assert_eq!(summary.stale_host_count, 0);
    }

    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((0, false)));
}

#[test]
fn the_account_key_self_corrects_when_the_collector_changes_it() {
    // 采集器把账号键从 source_code 换成真 AccountId 时，存量行必须能被改写；
    // 否则它们永远匹配不上退役判定（也就是永远退不了役，而且看不出来）。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    cloud_snapshot_hosts(
        &service,
        "cloud-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    assert_eq!(
        host_row(&service, tenant_id, HOST_B)
            .expect("host b")
            .cloud_account
            .as_deref(),
        Some(CLOUD_ACCOUNT)
    );

    // 同一个实例，新账号键。
    cloud_snapshot_hosts(
        &service,
        "cloud-2",
        CLOUD_PRODUCER,
        &both_hosts(),
        OTHER_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    assert_eq!(
        host_row(&service, tenant_id, HOST_B)
            .expect("host b")
            .cloud_account
            .as_deref(),
        Some(OTHER_ACCOUNT),
        "账号归属是云侧权威，旧值要被改写"
    );

    // 旧账号再也扫不到它；新账号能。
    for round in 0..host_retire_threshold() {
        assert_eq!(
            cloud_snapshot_hosts(
                &service,
                &format!("old-account-{round}"),
                CLOUD_PRODUCER,
                &only_a(),
                CLOUD_ACCOUNT,
                Some(("account", true)),
                IngestMode::Snapshot,
                tenant_id,
            ),
            0
        );
    }
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((0, false)));

    let mut newly = 0;
    for round in 0..host_retire_threshold() {
        newly = cloud_snapshot_hosts(
            &service,
            &format!("new-account-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            OTHER_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
    }
    assert_eq!(newly, 1);
}

#[test]
fn a_host_seen_by_agentd_is_still_retired_by_the_cloud() {
    // 这是 `cloud_account` 存在的全部理由：agentd 每 5 分钟覆盖一次 `ingest_scope`。
    // 如果退役目标按「最后谁写过」圈定，装了 agent 的云主机永远退不了役 ——
    // 而那批机器恰恰是成本治理最关心的。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    cloud_snapshot_hosts(
        &service,
        "cloud-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    // 端侧后到，把这两台机的 ingest_scope 改成自己的。
    snapshot_hosts(&service, "edge-1", &both_hosts(), tenant_id);
    let host_b = host_row(&service, tenant_id, HOST_B).expect("host b");
    assert!(
        host_b
            .ingest_scope
            .as_deref()
            .is_some_and(|scope| scope.contains(EDGE_PRODUCER)),
        "端侧后到就该拿到 scope（否则这条用例测不到真实场景）"
    );
    assert_eq!(
        host_b.cloud_account.as_deref(),
        Some(CLOUD_ACCOUNT),
        "账号归属是资产属性，不能被端侧报告抹掉"
    );

    // 云上这只实例没了。目标按账号圈定，不按 scope。
    let mut newly = 0;
    for round in 0..host_retire_threshold() {
        newly = cloud_snapshot_hosts(
            &service,
            &format!("cloud-miss-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
    }

    assert_eq!(newly, 1, "云侧完整快照连续缺失，应当退役");
    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((host_retire_threshold(), true))
    );
}

#[test]
fn a_retired_host_keeps_its_first_retirement_time() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    cloud_snapshot_hosts(
        &service,
        "ing-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    for round in 0..host_retire_threshold() {
        cloud_snapshot_hosts(
            &service,
            &format!("ing-miss-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
    }

    let before = host_row(&service, tenant_id, HOST_B).expect("host");
    let first = before.stale_at.expect("retired");

    for round in 0..3 {
        let newly = cloud_snapshot_hosts(
            &service,
            &format!("ing-extra-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
        assert_eq!(newly, 0, "已退役的不重复计入");
    }
    let after = host_row(&service, tenant_id, HOST_B).expect("host");

    assert_eq!(after.stale_at, Some(first), "首次退役时间必须冻结");
    assert_eq!(after.miss_count, before.miss_count, "已退役的不再累加");
}

#[test]
fn a_returning_host_is_revived_and_its_counter_cleared() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    cloud_snapshot_hosts(
        &service,
        "ing-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    // 缺到刚好差一轮就退役。
    for round in 0..host_retire_threshold() - 1 {
        cloud_snapshot_hosts(
            &service,
            &format!("ing-miss-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
    }
    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((host_retire_threshold() - 1, false))
    );

    // 又出现了：计数归零。
    cloud_snapshot_hosts(
        &service,
        "ing-back",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((0, false)));

    // 之后再缺一轮，也只是从 1 重新开始。
    cloud_snapshot_hosts(
        &service,
        "ing-miss-again",
        CLOUD_PRODUCER,
        &only_a(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );
    assert_eq!(host_state(&service, tenant_id, HOST_B), Some((1, false)));
}

#[test]
fn hosts_from_another_producer_scope_are_not_swept() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    // 另一个云采集器（另一个账号）报 node-b。
    cloud_snapshot_hosts(
        &service,
        "ing-other",
        "harvest-2",
        &[(HOST_B, "machine-b")],
        OTHER_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );

    // 我们的账号级快照里一直没有 node-b。
    for round in 0..host_retire_threshold() {
        cloud_snapshot_hosts(
            &service,
            &format!("ing-mine-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::Snapshot,
            tenant_id,
        );
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "别的作用域的资产不能拿我们的快照判死"
    );
}

#[test]
fn incremental_mode_never_accumulates_host_misses() {
    // 即使采集器声明了账号级完整，增量/批量模式描述的也是**部分**观测，不能当全集比对。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    cloud_snapshot_hosts(
        &service,
        "ing-1",
        CLOUD_PRODUCER,
        &both_hosts(),
        CLOUD_ACCOUNT,
        Some(("account", true)),
        IngestMode::Snapshot,
        tenant_id,
    );

    for round in 0..host_retire_threshold() + 2 {
        let newly = cloud_snapshot_hosts(
            &service,
            &format!("ing-batch-{round}"),
            CLOUD_PRODUCER,
            &only_a(),
            CLOUD_ACCOUNT,
            Some(("account", true)),
            IngestMode::BatchUpsert,
            tenant_id,
        );
        assert_eq!(newly, 0);
    }

    assert_eq!(
        host_state(&service, tenant_id, HOST_B),
        Some((0, false)),
        "部分观测不能当成「主机不在了」"
    );
}

#[test]
fn last_inventory_at_tracks_the_latest_observation() {
    // 可达性的唯一事实来源：它必须随每次上报推进，否则「最后采集于」永远是首次发现时间。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot_hosts(&service, "ing-1", &both_hosts(), tenant_id);
    let first = host_row(&service, tenant_id, HOST_A).expect("host a");

    snapshot_hosts(&service, "ing-2", &both_hosts(), tenant_id);
    let second = host_row(&service, tenant_id, HOST_A).expect("host a");

    assert!(
        second.last_inventory_at > first.last_inventory_at,
        "再次看到的机器要刷新最后观测时间"
    );
    assert_eq!(second.created_at, first.created_at, "首次发现时间不动");
}

// ── 描述字段刷新（主机匹配到已有行时）──────────────────────────
//
// 这条是 4 帧真实数据跑出来的：`resolve_host_candidate` 匹配到已有主机时是**克隆旧行**，
// 不显式刷新的话 `os_name` / `arch` 会永远停在第一次被看到时的值 —— 而那一次往往来自
// 还没采这些属性的旧 agentd。单帧 E2E 发现不了，必须「先没有、后有」。

#[test]
fn descriptive_fields_are_refreshed_when_an_existing_host_is_seen_again() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    // 第一帧：老 agentd，没有 os.* / arch。
    service
        .submit_and_materialize(envelope(
            "ing-old",
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            json!({
                "hosts": [{
                    "hostname": HOST_A,
                    "machine_id": "machine-a",
                    "external_ref": "machine-a"
                }]
            }),
        ))
        .expect("first ingest");

    let before = host_snapshot(&service, tenant_id, HOST_A);
    assert_eq!(before.0, None, "老 agentd 不报 os");

    // 第二帧：升级后的 agentd，带 os.* 与 arch。
    service
        .submit_and_materialize(envelope(
            "ing-new",
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            json!({
                "hosts": [{
                    "hostname": HOST_A,
                    "machine_id": "machine-a",
                    "external_ref": "machine-a",
                    "os_name": "macOS",
                    "os_version": "26.6.2",
                    "arch": "aarch64"
                }]
            }),
        ))
        .expect("second ingest");

    let after = host_snapshot(&service, tenant_id, HOST_A);
    assert_eq!(
        after,
        (
            Some("macOS".to_string()),
            Some("26.6.2".to_string()),
            Some("aarch64".to_string()),
            1
        ),
        "已有主机必须被新的观测刷新，且不能变成两台"
    );
}

#[test]
fn a_later_snapshot_without_os_does_not_erase_known_values() {
    // 升级后又被降级 / 换了老版本 agentd：不能用 None 把已知值擦掉。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    service
        .submit_and_materialize(envelope(
            "ing-new",
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            json!({
                "hosts": [{
                    "hostname": HOST_A,
                    "machine_id": "machine-a",
                    "external_ref": "machine-a",
                    "os_name": "macOS",
                    "os_version": "26.6.2",
                    "arch": "aarch64"
                }]
            }),
        ))
        .expect("first ingest");

    service
        .submit_and_materialize(envelope(
            "ing-old",
            "agent-1",
            IngestMode::Snapshot,
            tenant_id,
            json!({
                "hosts": [{
                    "hostname": HOST_A,
                    "machine_id": "machine-a",
                    "external_ref": "machine-a"
                }]
            }),
        ))
        .expect("second ingest");

    let after = host_snapshot(&service, tenant_id, HOST_A);
    assert_eq!(
        after.0.as_deref(),
        Some("macOS"),
        "缺字段的快照不能把已知值擦成空"
    );
    assert_eq!(after.2.as_deref(), Some("aarch64"));
}

/// `(os_name, os_version, arch, 该租户的主机总数)`
fn host_snapshot(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
    host_name: &str,
) -> (Option<String>, Option<String>, Option<String>, usize) {
    let hosts =
        CatalogStore::list_hosts(service.store(), tenant_id, Page::default()).expect("list hosts");
    let total = hosts.len();
    let host = hosts
        .into_iter()
        .find(|host| host.host_name == host_name)
        .expect("host");
    (host.os_name, host.os_version, host.arch, total)
}
