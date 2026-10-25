//! 监听端口 → 服务 / 实例 / 绑定 / 端点 的物化。
//!
//! 这条链路是**推断**：agentd 只报「某 pid 在某地址上监听」，服务实体是中心侧
//! 按可执行文件名造出来的。所以这里既要测「能造出来」，也要测「造不出来时不要
//! 留下半个对象」。

use atlas_domain::{IngestEnvelope, IngestMode, RuntimeObjectType, SourceKind, TenantId};
use atlas_storage::{CatalogStore, Page, RuntimeStore};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::TopologyIngestService;
use crate::query::TopologyQueryService;

fn envelope(tenant_id: TenantId, ingest_id: &str, payload: serde_json::Value) -> IngestEnvelope {
    envelope_at(tenant_id, ingest_id, Utc::now(), payload)
}

fn envelope_at(
    tenant_id: TenantId,
    ingest_id: &str,
    received_at: DateTime<Utc>,
    payload: serde_json::Value,
) -> IngestEnvelope {
    IngestEnvelope {
        ingest_id: ingest_id.to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "wist:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at,
        payload_ref: None,
        payload_inline: Some(payload),
        metadata: Default::default(),
    }
}

#[test]
fn endpoint_creates_service_instance_binding_and_endpoint() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    service
        .submit_and_materialize(envelope(
            tenant_id,
            "ing-endpoint-1",
            json!({
                "hosts": [{
                    "host_name": "node-07",
                    "machine_id": "hostname:node-07",
                    "external_ref": "hostname:node-07"
                }],
                "processes": [{
                    "pid": 4242,
                    "executable": "/usr/sbin/sshd",
                    "host_ref": "node-07",
                    "command_line": "/usr/sbin/sshd -D"
                }],
                "service_endpoints": [{
                    "protocol": "tcp",
                    "address": "0.0.0.0",
                    "port": 22,
                    "pid": 4242,
                    "host_ref": "node-07",
                    "process_executable": "/usr/sbin/sshd"
                }]
            }),
        ))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    assert_eq!(services.len(), 1);
    let sshd = &services[0];
    assert_eq!(sshd.name, "sshd");
    // 服务身份按可执行名跨主机归并，所以要能被下一次快照重新算出来。
    assert_eq!(sshd.external_ref.as_deref(), Some("proc:sshd"));

    let hosts = CatalogStore::list_hosts(store, tenant_id, Page::default()).unwrap();
    let host = &hosts[0];
    let processes =
        RuntimeStore::list_process_runtime_states(store, host.host_id, Page::default()).unwrap();
    assert_eq!(processes.len(), 1);

    let bindings = RuntimeStore::list_runtime_bindings_for_object(
        store,
        RuntimeObjectType::Process,
        processes[0].process_id,
        Page::default(),
    )
    .unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].instance_id, bindings[0].instance_id);

    let instances =
        RuntimeStore::list_service_instances(store, sshd.service_id, Page::default()).unwrap();
    assert_eq!(instances.len(), 1);
    let endpoints =
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap();
    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].protocol, "tcp");
    assert_eq!(endpoints[0].address, "0.0.0.0");
    assert_eq!(endpoints[0].port, 22);
    assert!(endpoints[0].valid_to.is_none());

    // 主机视图上要能顺带看到端口，否则前端只能显示一个服务名。
    let view = TopologyQueryService::new(store.clone())
        .host_topology_view(host.host_id)
        .unwrap()
        .unwrap();
    assert_eq!(view.services.len(), 1);
    assert_eq!(view.services[0].instances[0].endpoints.len(), 1);
    assert_eq!(view.services[0].instances[0].endpoints[0].port, 22);
}

#[test]
fn endpoint_without_owning_process_is_skipped() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    service
        .submit_and_materialize(envelope(
            tenant_id,
            "ing-endpoint-orphan",
            json!({
                "hosts": [{
                    "host_name": "node-08",
                    "machine_id": "hostname:node-08",
                    "external_ref": "hostname:node-08"
                }],
                "processes": [{
                    "pid": 111,
                    "executable": "/usr/sbin/cron",
                    "host_ref": "node-08"
                }],
                "service_endpoints": [{
                    "protocol": "tcp",
                    "address": "0.0.0.0",
                    "port": 9999,
                    "pid": 9999,
                    "host_ref": "node-08",
                    "process_executable": "/opt/who-knows"
                }]
            }),
        ))
        .unwrap();

    let store = service.store();
    // pid 9999 在本快照里不存在 → 造不出「服务-实例-端点」这条链，
    // 那就一个对象都不要写，别在目录里留个没有宿主的端口。
    assert!(
        CatalogStore::list_services(store, tenant_id, Page::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn endpoint_closes_when_the_port_is_no_longer_reported() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let snapshot = |ingest_id: &str, endpoints: Option<serde_json::Value>| {
        let mut payload = json!({
            "hosts": [{
                "host_name": "node-09",
                "machine_id": "hostname:node-09",
                "external_ref": "hostname:node-09"
            }],
            "processes": [{
                "pid": 222,
                "executable": "/usr/sbin/sshd",
                "host_ref": "node-09"
            }]
        });
        if let Some(endpoints) = endpoints {
            payload["service_endpoints"] = endpoints;
        }
        envelope(tenant_id, ingest_id, payload)
    };

    let live_endpoints = |service: &TopologyIngestService<_>| {
        let store = service.store();
        let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
        let instances =
            RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
                .unwrap();
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap()
    };

    service
        .submit_and_materialize(snapshot(
            "ing-ep-on",
            Some(json!([{
                "protocol": "tcp",
                "address": "0.0.0.0",
                "port": 22,
                "pid": 222,
                "host_ref": "node-09"
            }])),
        ))
        .unwrap();
    assert_eq!(live_endpoints(&service).len(), 1);

    // 进程还在、端口全部关掉：采集端会报一个空数组。
    service
        .submit_and_materialize(snapshot("ing-ep-off", Some(json!([]))))
        .unwrap();
    assert!(
        live_endpoints(&service).is_empty(),
        "关掉的端口不该再出现在活端点里"
    );
}

#[test]
fn endpoint_is_kept_when_the_source_does_not_report_the_field_at_all() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let payload = |endpoints: bool| {
        let mut value = json!({
            "hosts": [{
                "host_name": "node-10",
                "machine_id": "hostname:node-10",
                "external_ref": "hostname:node-10"
            }],
            "processes": [{
                "pid": 333,
                "executable": "/usr/sbin/sshd",
                "host_ref": "node-10"
            }]
        });
        if endpoints {
            value["service_endpoints"] = json!([{
                "protocol": "tcp",
                "address": "0.0.0.0",
                "port": 22,
                "pid": 333,
                "host_ref": "node-10"
            }]);
        }
        value
    };

    service
        .submit_and_materialize(envelope(tenant_id, "ing-ep-1", payload(true)))
        .unwrap();
    // 第二份快照里连 `service_endpoints` 字段都没有（旧采集器 / 不支持端点的平台）。
    // 这种情况不能当成「一个端口都没开」，否则 macOS 上一上报就会被清空。
    service
        .submit_and_materialize(envelope(tenant_id, "ing-ep-2", payload(false)))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    let instances =
        RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
            .unwrap();
    assert_eq!(
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap()
            .len(),
        1,
        "没上报不等于没端口"
    );
}

// ── 窗口语义 ──────────────────────────────────────────

#[test]
fn endpoint_window_start_is_stable_across_snapshots() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let first = Utc::now();
    let later = first + Duration::hours(6);

    let snapshot = |ingest_id: &str, at: DateTime<Utc>| {
        envelope_at(
            tenant_id,
            ingest_id,
            at,
            json!({
                "hosts": [{
                    "host_name": "node-12",
                    "machine_id": "hostname:node-12",
                    "external_ref": "hostname:node-12"
                }],
                "processes": [{
                    "pid": 600,
                    "executable": "/usr/sbin/sshd",
                    "machine_id": "hostname:node-12",
                    "host_ref": "node-12"
                }],
                "service_endpoints": [{
                    "protocol": "tcp",
                    "address": "0.0.0.0",
                    "port": 22,
                    "pid": 600,
                    "host_ref": "node-12"
                }]
            }),
        )
    };

    service
        .submit_and_materialize(snapshot("win-1", first))
        .unwrap();
    service
        .submit_and_materialize(snapshot("win-2", later))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    let instances =
        RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
            .unwrap();
    let endpoints =
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap();
    assert_eq!(endpoints.len(), 1, "重复上报不该多出一个端点");
    // 端口一直开着：窗口起点不能被每份快照推后，否则 valid_from 就是「上次上报」。
    assert_eq!(endpoints[0].valid_from, first);
    assert!(endpoints[0].valid_to.is_none());
    // 而「最近一次看到」由实例的 last_seen_at 承担。
    assert_eq!(instances[0].last_seen_at, later);
}

#[test]
fn reopened_endpoint_starts_a_new_window() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let first = Utc::now();
    let closed_at = first + Duration::hours(1);
    let reopened_at = first + Duration::hours(2);

    let snapshot = |ingest_id: &str, at: DateTime<Utc>, endpoints: serde_json::Value| {
        envelope_at(
            tenant_id,
            ingest_id,
            at,
            json!({
                "hosts": [{
                    "host_name": "node-13",
                    "machine_id": "hostname:node-13",
                    "external_ref": "hostname:node-13"
                }],
                "processes": [{
                    "pid": 700,
                    "executable": "/usr/sbin/sshd",
                    "machine_id": "hostname:node-13",
                    "host_ref": "node-13"
                }],
                "service_endpoints": endpoints
            }),
        )
    };
    let open = json!([{
        "protocol": "tcp",
        "address": "0.0.0.0",
        "port": 22,
        "pid": 700,
        "host_ref": "node-13"
    }]);

    service
        .submit_and_materialize(snapshot("re-1", first, open.clone()))
        .unwrap();
    service
        .submit_and_materialize(snapshot("re-2", closed_at, json!([])))
        .unwrap();
    service
        .submit_and_materialize(snapshot("re-3", reopened_at, open))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    let instances =
        RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
            .unwrap();
    let endpoints =
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap();
    assert_eq!(endpoints.len(), 1);
    // 重新打开是一段新窗口，起点必须重置 —— 拿 LEAST 会把上次的开窗时间当成这次的。
    assert_eq!(endpoints[0].valid_from, reopened_at);
}

// ── 服务聚合 ──────────────────────────────────────────

#[test]
fn two_ports_on_one_executable_share_service_and_instance() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    service
        .submit_and_materialize(envelope(
            tenant_id,
            "multi-port",
            json!({
                "hosts": [{
                    "host_name": "node-14",
                    "machine_id": "hostname:node-14",
                    "external_ref": "hostname:node-14"
                }],
                "processes": [{
                    "pid": 800,
                    "executable": "/usr/sbin/nginx",
                    "machine_id": "hostname:node-14",
                    "host_ref": "node-14"
                }],
                "service_endpoints": [
                    {"protocol": "tcp", "address": "0.0.0.0", "port": 80, "pid": 800, "host_ref": "node-14"},
                    {"protocol": "tcp", "address": "0.0.0.0", "port": 443, "pid": 800, "host_ref": "node-14"}
                ]
            }),
        ))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    assert_eq!(services.len(), 1, "同一个可执行名只能有一个服务");
    assert_eq!(services[0].name, "nginx");

    let instances =
        RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
            .unwrap();
    assert_eq!(instances.len(), 1, "一台主机上一个服务只该有一个实例");

    let endpoints =
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap();
    let ports: Vec<i32> = endpoints.iter().map(|endpoint| endpoint.port).collect();
    assert_eq!(ports, vec![80, 443]);
}

#[test]
fn same_executable_on_two_hosts_shares_service_but_not_instance() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    service
        .submit_and_materialize(envelope(
            tenant_id,
            "cross-host",
            json!({
                "hosts": [
                    {"host_name": "node-15", "machine_id": "hostname:node-15", "external_ref": "hostname:node-15"},
                    {"host_name": "node-16", "machine_id": "hostname:node-16", "external_ref": "hostname:node-16"}
                ],
                "processes": [
                    {"pid": 900, "executable": "/usr/sbin/sshd", "machine_id": "hostname:node-15", "host_ref": "node-15"},
                    {"pid": 901, "executable": "/usr/sbin/sshd", "machine_id": "hostname:node-16", "host_ref": "node-16"}
                ],
                "service_endpoints": [
                    {"protocol": "tcp", "address": "0.0.0.0", "port": 22, "pid": 900, "host_ref": "node-15"},
                    {"protocol": "tcp", "address": "0.0.0.0", "port": 22, "pid": 901, "host_ref": "node-16"}
                ]
            }),
        ))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    // 「sshd 跑在 2 台机器上」是一个服务两个实例，不是一个叫 node-15:sshd 的服务。
    assert_eq!(services.len(), 1);
    let instances =
        RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
            .unwrap();
    assert_eq!(instances.len(), 2);
    for instance in &instances {
        let endpoints =
            RuntimeStore::list_instance_endpoints(store, instance.instance_id, Page::default())
                .unwrap();
        assert_eq!(endpoints.len(), 1, "每个实例各自只看到自己主机的端口");
    }
}

#[test]
fn restarted_process_takes_over_the_port_and_the_old_binding_is_closed() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let first = Utc::now();
    let second = first + Duration::minutes(10);

    let host = json!([{
        "host_name": "node-18",
        "machine_id": "hostname:node-18",
        "external_ref": "hostname:node-18"
    }]);
    let snapshot = |ingest_id: &str, at: DateTime<Utc>, identity: &str| {
        envelope_at(
            tenant_id,
            ingest_id,
            at,
            json!({
                "hosts": host,
                "processes": [{
                    "pid": 1100,
                    "executable": "/usr/sbin/nginx",
                    "machine_id": "hostname:node-18",
                    "host_ref": "node-18",
                    "identity": identity
                }],
                // 同一个端口：进程重启后由新进程接管。
                "service_endpoints": [{
                    "protocol": "tcp",
                    "address": "0.0.0.0",
                    "port": 80,
                    "pid": 1100,
                    "host_ref": "node-18",
                    "process_identity": identity
                }]
            }),
        )
    };

    service
        .submit_and_materialize(snapshot("takeover-1", first, "start:before"))
        .unwrap();
    service
        .submit_and_materialize(snapshot("takeover-2", second, "start:after"))
        .unwrap();

    let store = service.store();
    let services = CatalogStore::list_services(store, tenant_id, Page::default()).unwrap();
    assert_eq!(services.len(), 1, "重启后服务还是那个服务");
    let instances =
        RuntimeStore::list_service_instances(store, services[0].service_id, Page::default())
            .unwrap();
    assert_eq!(instances.len(), 1, "也不该多出一个实例");

    let bindings = RuntimeStore::list_runtime_bindings_for_instance(
        store,
        instances[0].instance_id,
        Page::default(),
    )
    .unwrap();
    assert_eq!(bindings.len(), 2, "两个进程各一条绑定");
    assert_eq!(
        bindings
            .iter()
            .filter(|binding| binding.validity.valid_to.is_none())
            .count(),
        1,
        "旧进程的绑定必须收尾，否则‘谁在跑’同时有两个答案"
    );

    let endpoints =
        RuntimeStore::list_instance_endpoints(store, instances[0].instance_id, Page::default())
            .unwrap();
    assert_eq!(endpoints.len(), 1);
    // 端口没断过，窗口起点保留第一次看到的时间。
    assert_eq!(endpoints[0].valid_from, first);
}

// ── pid 复用 ──────────────────────────────────────────

#[test]
fn endpoint_binds_the_live_process_when_a_pid_was_recycled() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let first = Utc::now();

    let host = json!([{
        "host_name": "node-17",
        "machine_id": "hostname:node-17",
        "external_ref": "hostname:node-17"
    }]);

    // 同 pid 两行（启动身份不同 = 不同进程），这在 pid 被复用时会真实出现。
    service
        .submit_and_materialize(envelope_at(
            tenant_id,
            "pid-1",
            first,
            json!({
                "hosts": host,
                "processes": [
                    {"pid": 1000, "executable": "/usr/bin/app", "machine_id": "hostname:node-17", "host_ref": "node-17", "identity": "start:old"},
                    {"pid": 1000, "executable": "/usr/bin/app", "machine_id": "hostname:node-17", "host_ref": "node-17", "identity": "start:live"}
                ]
            }),
        ))
        .unwrap();

    // 第二份快照里只剩 start:live —— start:old 被快照清扫标 stale。
    // 端点故意**不带** identity：这时只能靠「优先活着的」来选。
    service
        .submit_and_materialize(envelope_at(
            tenant_id,
            "pid-2",
            first + Duration::minutes(5),
            json!({
                "hosts": host,
                "processes": [
                    {"pid": 1000, "executable": "/usr/bin/app", "machine_id": "hostname:node-17", "host_ref": "node-17", "identity": "start:live"}
                ],
                "service_endpoints": [
                    {"protocol": "tcp", "address": "0.0.0.0", "port": 7000, "pid": 1000, "host_ref": "node-17"}
                ]
            }),
        ))
        .unwrap();

    let store = service.store();
    let hosts = CatalogStore::list_hosts(store, tenant_id, Page::default()).unwrap();
    let processes =
        RuntimeStore::list_process_runtime_states(store, hosts[0].host_id, Page::default())
            .unwrap();
    assert_eq!(processes.len(), 2, "两行都还在，只是其中一行 stale");

    let live = processes
        .iter()
        .find(|process| {
            process
                .external_ref
                .as_deref()
                .map(|reference| reference.ends_with("start:live"))
                .unwrap_or(false)
        })
        .expect("live process");
    let stale = processes
        .iter()
        .find(|process| {
            process
                .external_ref
                .as_deref()
                .map(|reference| reference.ends_with("start:old"))
                .unwrap_or(false)
        })
        .expect("stale process");
    assert!(stale.stale_at.is_some());

    let live_bindings = RuntimeStore::list_runtime_bindings_for_object(
        store,
        RuntimeObjectType::Process,
        live.process_id,
        Page::default(),
    )
    .unwrap();
    let stale_bindings = RuntimeStore::list_runtime_bindings_for_object(
        store,
        RuntimeObjectType::Process,
        stale.process_id,
        Page::default(),
    )
    .unwrap();
    assert_eq!(live_bindings.len(), 1, "端点要绑到活着的那个进程");
    assert!(stale_bindings.is_empty(), "不能绑到被复用的旧 pid 行上");
}
