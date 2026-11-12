//! 弹性公网 IP 的端到端用例。
//!
//! 守的是「EIP 是独立资产」这件事：身份是地址 ID、绑定是快照态、
//! 退役与主机同一套判据（账号级完整快照），且「本帧没观测 EIP」不构成退役证据。

use atlas_domain::{
    ElasticIpStatus, ElasticIpTarget, IngestEnvelope, IngestMode, SourceKind, TenantId,
};
use atlas_storage::{CatalogStore, Page};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::TopologyIngestService;

const ACCOUNT: &str = "tencent:1300000000";

fn envelope(
    ingest_id: &str,
    payload: Value,
    coverage: bool,
    tenant_id: TenantId,
) -> IngestEnvelope {
    let mut envelope = IngestEnvelope {
        ingest_id: ingest_id.to_string(),
        source_kind: SourceKind::ExternalSync,
        source_name: "wist-harvest:tencent-cloud-prod".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(payload),
        metadata: Default::default(),
    };
    envelope
        .metadata
        .insert("schema".to_string(), "atlas.in.inventory.v1".to_string());
    if coverage {
        envelope
            .metadata
            .insert("collect.coverage.scope".to_string(), "account".to_string());
        envelope
            .metadata
            .insert("collect.coverage.complete".to_string(), "true".to_string());
        envelope
            .metadata
            .insert("collect.coverage.account".to_string(), ACCOUNT.to_string());
    }
    envelope
}

fn host(instance_id: &str, name: &str) -> Value {
    json!({
        "hostname": name,
        "cloud_instance_id": instance_id,
        "cloud_account": ACCOUNT,
    })
}

fn eip(id: &str, address: &str, status: &str, bound_to: Option<&str>) -> Value {
    json!({
        "external_ref": id,
        "address": address,
        "status": status,
        "charge_type": "TRAFFIC_POSTPAID_BY_HOUR",
        "bandwidth_mbps": 10,
        "cloud_account": ACCOUNT,
        "bound_instance_ref": bound_to,
        "bound_instance_type": "CVM",
    })
}

fn payload(hosts: Vec<Value>, eips: Vec<Value>) -> Value {
    json!({ "hosts": hosts, "elastic_ips": eips })
}

fn snapshot(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    ingest_id: &str,
    payload: Value,
    coverage: bool,
    tenant_id: TenantId,
) -> crate::PipelineRunSummary {
    service
        .submit_and_materialize(envelope(ingest_id, payload, coverage, tenant_id))
        .expect("ingest")
        .1
}

fn eips(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
) -> Vec<atlas_domain::ElasticIp> {
    CatalogStore::list_elastic_ips(service.store(), tenant_id, Page::default()).expect("list eips")
}

fn host_id(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
    name: &str,
) -> Uuid {
    CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|item| item.host_name == name)
        .expect("host")
        .host_id
}

#[test]
fn a_bound_eip_resolves_its_host_by_cloud_instance_id() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let summary = snapshot(
        &service,
        "ing-1",
        payload(
            vec![host("ins-1", "node-a")],
            vec![eip("eip-1", "203.0.113.10", "BIND", Some("ins-1"))],
        ),
        true,
        tenant_id,
    );
    assert_eq!(summary.elastic_ip_count, 1);

    let rows = eips(&service, tenant_id);
    assert_eq!(rows.len(), 1);
    let ip = &rows[0];
    assert_eq!(ip.external_ref, "eip-1");
    assert_eq!(ip.address, "203.0.113.10");
    assert_eq!(ip.status, ElasticIpStatus::Bound);
    assert_eq!(ip.cloud_account.as_deref(), Some(ACCOUNT));
    assert_eq!(ip.bandwidth_mbps, Some(10));

    let binding = ip.binding.as_ref().expect("bound");
    assert_eq!(binding.target_kind, ElasticIpTarget::Host);
    assert_eq!(binding.external_target_ref.as_deref(), Some("ins-1"));
    assert_eq!(
        binding.host_id,
        Some(host_id(&service, tenant_id, "node-a")),
        "绑定目标要落到主机对象上"
    );
    assert!(binding.bound_since.is_some());
}

#[test]
fn re_binding_to_another_host_moves_the_binding_but_keeps_the_object() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(
        &service,
        "ing-1",
        payload(
            vec![host("ins-1", "node-a"), host("ins-2", "node-b")],
            vec![eip("eip-1", "203.0.113.10", "BIND", Some("ins-1"))],
        ),
        true,
        tenant_id,
    );
    let first = eips(&service, tenant_id).remove(0);
    let node_a = host_id(&service, tenant_id, "node-a");
    assert_eq!(first.binding.as_ref().unwrap().host_id, Some(node_a));

    snapshot(
        &service,
        "ing-2",
        payload(
            vec![host("ins-1", "node-a"), host("ins-2", "node-b")],
            vec![eip("eip-1", "203.0.113.10", "BIND", Some("ins-2"))],
        ),
        true,
        tenant_id,
    );

    let rows = eips(&service, tenant_id);
    assert_eq!(rows.len(), 1, "换机器不换地址：仍然是同一个 EIP 对象");
    let second = &rows[0];
    assert_eq!(second.elastic_ip_id, first.elastic_ip_id);
    assert_eq!(
        second.binding.as_ref().unwrap().host_id,
        Some(host_id(&service, tenant_id, "node-b"))
    );
    assert_eq!(
        second.created_at, first.created_at,
        "首次出现时间不随重绑定变"
    );
    assert!(
        second.binding.as_ref().unwrap().bound_since != first.binding.as_ref().unwrap().bound_since
            || second.binding.as_ref().unwrap().host_id != first.binding.as_ref().unwrap().host_id,
        "换了目标就是新的一次绑定"
    );
}

#[test]
fn an_unbound_eip_is_kept_and_has_no_binding() {
    // 闲置 EIP 照样计费，也照样是资产 —— 解绑不等于消失。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(
        &service,
        "ing-1",
        payload(
            vec![host("ins-1", "node-a")],
            vec![
                eip("eip-1", "203.0.113.10", "BIND", Some("ins-1")),
                eip("eip-2", "203.0.113.11", "UNBIND", None),
            ],
        ),
        true,
        tenant_id,
    );

    let rows = eips(&service, tenant_id);
    let idle = rows
        .iter()
        .find(|ip| ip.external_ref == "eip-2")
        .expect("eip-2");
    assert_eq!(idle.status, ElasticIpStatus::Unbound);
    assert!(idle.binding.is_none(), "解绑后绑定额要清空");
    assert!(idle.stale_at.is_none(), "解绑不是退役");

    // 再跑一轮「还是没绑」，绑定额仍然是空的（不能被旧值粘住）。
    snapshot(
        &service,
        "ing-2",
        payload(
            vec![host("ins-1", "node-a")],
            vec![
                eip("eip-1", "203.0.113.10", "BIND", Some("ins-1")),
                eip("eip-2", "203.0.113.11", "UNBIND", None),
            ],
        ),
        true,
        tenant_id,
    );
    let idle = eips(&service, tenant_id)
        .into_iter()
        .find(|ip| ip.external_ref == "eip-2")
        .expect("eip-2");
    assert!(idle.binding.is_none());
}

#[test]
fn a_released_eip_is_retired_only_by_complete_account_snapshots() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(
        &service,
        "ing-1",
        payload(
            vec![host("ins-1", "node-a")],
            vec![
                eip("eip-1", "203.0.113.10", "BIND", Some("ins-1")),
                eip("eip-2", "203.0.113.11", "UNBIND", None),
            ],
        ),
        true,
        tenant_id,
    );

    // 云上释放了 eip-2：后续完整快照里没有它。
    let mut stale = 0;
    for round in 0..crate::service::materialize::ELASTIC_IP_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
        let summary = snapshot(
            &service,
            &format!("ing-miss-{round}"),
            payload(
                vec![host("ins-1", "node-a")],
                vec![eip("eip-1", "203.0.113.10", "BIND", Some("ins-1"))],
            ),
            true,
            tenant_id,
        );
        stale = summary.stale_elastic_ip_count;
    }
    assert_eq!(stale, 1, "达到阈值那一轮才报释放");

    let rows = eips(&service, tenant_id);
    assert_eq!(rows.len(), 2, "释放不删行：能回答「这个地址曾经属于谁」");
    let released = rows
        .iter()
        .find(|ip| ip.external_ref == "eip-2")
        .expect("eip-2");
    assert!(released.stale_at.is_some());
    let alive = rows
        .iter()
        .find(|ip| ip.external_ref == "eip-1")
        .expect("eip-1");
    assert!(alive.stale_at.is_none());
}

#[test]
fn a_frame_that_does_not_report_eips_never_retires_them() {
    // 采集器把 EIP 关掉 / 老版本没这个键：缺席不构成证据。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(
        &service,
        "ing-1",
        payload(
            vec![host("ins-1", "node-a")],
            vec![eip("eip-1", "203.0.113.10", "BIND", Some("ins-1"))],
        ),
        true,
        tenant_id,
    );

    for round in 0..crate::service::materialize::ELASTIC_IP_RETIRE_AFTER_COMPLETE_SNAPSHOTS + 1 {
        let summary = snapshot(
            &service,
            &format!("ing-nokeys-{round}"),
            json!({ "hosts": [host("ins-1", "node-a")] }),
            true,
            tenant_id,
        );
        assert_eq!(summary.stale_elastic_ip_count, 0);
    }

    let ip = eips(&service, tenant_id).remove(0);
    assert!(ip.stale_at.is_none(), "没观测 ≠ 释放");
    assert_eq!(ip.miss_count, 0, "连计数都不该推进");
}

#[test]
fn eip_retirement_still_requires_the_account_declaration() {
    // 手写地域子集（partial）的快照不能判死 EIP，与主机同一条纪律。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    snapshot(
        &service,
        "ing-1",
        payload(
            vec![host("ins-1", "node-a")],
            vec![eip("eip-1", "203.0.113.10", "BIND", Some("ins-1"))],
        ),
        true,
        tenant_id,
    );

    for round in 0..crate::service::materialize::ELASTIC_IP_RETIRE_AFTER_COMPLETE_SNAPSHOTS + 1 {
        snapshot(
            &service,
            &format!("ing-partial-{round}"),
            payload(vec![host("ins-1", "node-a")], Vec::new()),
            false,
            tenant_id,
        );
    }

    let ip = eips(&service, tenant_id).remove(0);
    assert!(ip.stale_at.is_none());
    assert_eq!(ip.miss_count, 0);
}
