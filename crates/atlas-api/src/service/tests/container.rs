//! 容器物化与快照替换清扫。
//!
//! 三件事必须成立，否则容器资产会「看起来有、其实是错的」：
//! 1. 同一份快照里两台机上的同名容器是两个资产（id 由容器自己的 id 派生，不是名字）；
//! 2. 容器从下一份快照里消失时只标 stale，行保留；
//! 3. 输入里**没有** `containers` 字段时不清扫 —— 老采集器不支持容器，不能当作全没了。

use atlas_domain::{IngestEnvelope, IngestMode, SourceKind, TenantId};
use atlas_storage::{CatalogStore, Page, RuntimeStore};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::TopologyIngestService;
use crate::query::TopologyQueryService;

fn envelope(
    ingest_id: &str,
    tenant_id: TenantId,
    observed_at: &str,
    payload: serde_json::Value,
) -> IngestEnvelope {
    IngestEnvelope {
        ingest_id: ingest_id.to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "wist:agent-c".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(payload),
        metadata: Default::default(),
    }
}

fn seed_host(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
) {
    service
        .submit_and_materialize(envelope(
            "ing-host",
            tenant_id,
            "2026-10-25T09:18:00Z",
            json!({
                "target_kind": "host",
                "host_name": "node-c",
                "machine_id": "hostname:node-c",
                "external_ref": "hostname:node-c"
            }),
        ))
        .unwrap();
}

fn host_id(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
) -> Uuid {
    CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-c")
        .unwrap()
        .host_id
}

#[test]
fn containers_land_on_the_host_with_runtime_and_k8s_hints() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    seed_host(&service, tenant_id);

    let (_record, summary) = service
        .submit_and_materialize(envelope(
            "ing-containers",
            tenant_id,
            "2026-10-25T09:18:00Z",
            json!({
                "containers": [
                    {
                        "id": "3f2c9f3c1b8e",
                        "name": "nginx",
                        "runtime": "containerd",
                        "runtime_namespace": "k8s.io",
                        "image": "nginx:1.27",
                        "host_ref": "node-c",
                        "k8s": {
                            "namespace": "prod",
                            "pod_uid": "pod-uid-nginx",
                            "pod_name": "nginx-abc",
                            "container_name": "nginx"
                        }
                    },
                    {
                        "id": "7a1b2c3d4e5f",
                        "name": "redis",
                        "runtime": "docker",
                        "image": "redis:7",
                        "host_ref": "node-c"
                    }
                ]
            }),
        ))
        .unwrap();

    assert_eq!(summary.container_count, 2);
    assert_eq!(summary.unresolved_container_count, 0);

    let view = TopologyQueryService::new(service.store().clone())
        .host_topology_view(host_id(&service, tenant_id))
        .unwrap()
        .unwrap();
    assert_eq!(view.containers.len(), 2);
    // 按容器名排序，前端不需要再排。
    assert_eq!(view.containers[0].container_name.as_deref(), Some("nginx"));
    assert_eq!(
        view.containers[0].runtime_type,
        atlas_domain::RuntimeType::Containerd
    );
    assert_eq!(view.containers[0].image_ref.as_deref(), Some("nginx:1.27"));
    assert_eq!(view.containers[0].pod_name.as_deref(), Some("nginx-abc"));
    assert_eq!(view.containers[0].k8s_namespace.as_deref(), Some("prod"));
    // 没 k8s 线索的容器不能被写成 K8s 容器。
    assert!(view.containers[1].pod_name.is_none());
    assert_eq!(
        view.containers[1].runtime_type,
        atlas_domain::RuntimeType::Docker
    );
}

#[test]
fn a_container_that_disappears_is_marked_stale_not_deleted() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    seed_host(&service, tenant_id);

    let containers = |ids: &[(&str, &str)]| {
        json!({
            "containers": ids
                .iter()
                .map(|(id, name)| json!({
                    "id": id,
                    "name": name,
                    "runtime": "containerd",
                    "host_ref": "node-c"
                }))
                .collect::<Vec<_>>()
        })
    };

    service
        .submit_and_materialize(envelope(
            "ing-c1",
            tenant_id,
            "2026-10-25T09:18:00Z",
            containers(&[("3f2c9f3c1b8e", "nginx"), ("7a1b2c3d4e5f", "redis")]),
        ))
        .unwrap();

    // 第二轮只剩 nginx：redis 该被标 stale，但行还在。
    let (_record, summary) = service
        .submit_and_materialize(envelope(
            "ing-c2",
            tenant_id,
            "2026-10-25T09:19:00Z",
            containers(&[("3f2c9f3c1b8e", "nginx")]),
        ))
        .unwrap();
    assert_eq!(summary.stale_container_count, 1);

    let view = TopologyQueryService::new(service.store().clone())
        .host_topology_view(host_id(&service, tenant_id))
        .unwrap()
        .unwrap();
    assert_eq!(view.containers.len(), 1);
    assert_eq!(view.containers[0].container_name.as_deref(), Some("nginx"));
}

#[test]
fn a_snapshot_without_the_containers_field_never_sweeps() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    seed_host(&service, tenant_id);

    service
        .submit_and_materialize(envelope(
            "ing-with-containers",
            tenant_id,
            "2026-10-25T09:18:00Z",
            json!({
                "containers": [{
                    "id": "3f2c9f3c1b8e",
                    "name": "nginx",
                    "runtime": "containerd",
                    "host_ref": "node-c"
                }]
            }),
        ))
        .unwrap();

    // 老采集器：只发进程，压根没有 containers 字段。
    let (_record, summary) = service
        .submit_and_materialize(envelope(
            "ing-without-containers",
            tenant_id,
            "2026-10-25T09:19:00Z",
            json!({ "processes": [] }),
        ))
        .unwrap();
    assert_eq!(summary.container_count, 0);
    assert_eq!(summary.stale_container_count, 0);

    let view = TopologyQueryService::new(service.store().clone())
        .host_topology_view(host_id(&service, tenant_id))
        .unwrap()
        .unwrap();
    assert_eq!(
        view.containers.len(),
        1,
        "field absent must not be treated as 'no containers'"
    );
}

#[test]
fn container_ids_are_stable_across_snapshots() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    seed_host(&service, tenant_id);

    let payload = |observed_at: &str| {
        json!({
            "containers": [{
                "id": "3f2c9f3c1b8e",
                "name": "nginx",
                "runtime": "containerd",
                "host_ref": "node-c",
                "observed_at": observed_at
            }]
        })
    };
    service
        .submit_and_materialize(envelope(
            "ing-a",
            tenant_id,
            "2026-10-25T09:18:00Z",
            payload("2026-10-25T09:18:00Z"),
        ))
        .unwrap();
    let first = RuntimeStore::list_container_runtimes_for_host(
        service.store(),
        host_id(&service, tenant_id),
        Page::default(),
    )
    .unwrap();

    service
        .submit_and_materialize(envelope(
            "ing-b",
            tenant_id,
            "2026-10-25T09:19:00Z",
            payload("2026-10-25T09:19:00Z"),
        ))
        .unwrap();
    let second = RuntimeStore::list_container_runtimes_for_host(
        service.store(),
        host_id(&service, tenant_id),
        Page::default(),
    )
    .unwrap();

    // 同一份采集 id → 同一个资产 id，否则每轮快照都会多出一个容器。
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].container_id, second[0].container_id);
    assert_eq!(
        second[0].last_seen_at.to_rfc3339(),
        "2026-10-25T09:19:00+00:00"
    );
}
