//! 云侧网络结构：VPC → `NetworkDomain`、子网 → `NetworkSegment`、主机 → 子网。
//!
//! 三条规矩（都是踩过的）：
//!
//! 1. **域/段必须能独立落地**：云侧 VPC/子网没有 `machine_id`/`host_name`，
//!    以前「解析不到主机就整段跳过」，等于云上网络结构一行都没入库。
//! 2. **VPC 不是网段**：塞进 `network_segments[]` 只能靠 CIDR 猜归属；VPC 是地址空间。
//! 3. **主机找子网：先认 `SubnetId`，再按 CIDR 包含（最长前缀）兜底**。

use atlas_domain::{IngestEnvelope, IngestMode, NetworkDomainKind, SourceKind, TenantId};
use atlas_storage::{CatalogStore, Page, RuntimeStore};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::TopologyIngestService;

fn envelope(ingest_id: &str, payload: Value, tenant_id: TenantId) -> IngestEnvelope {
    IngestEnvelope {
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
    }
}

fn ingest(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    ingest_id: &str,
    payload: Value,
    tenant_id: TenantId,
) {
    service
        .submit_and_materialize(envelope(ingest_id, payload, tenant_id))
        .expect("ingest");
}

/// 账号级完整快照声明 —— 退役的唯一合法入场券。
fn declare_full_account_coverage(envelope: &mut IngestEnvelope, account: &str) {
    envelope
        .metadata
        .insert("collect.coverage.scope".to_string(), "account".to_string());
    envelope
        .metadata
        .insert("collect.coverage.complete".to_string(), "true".to_string());
    envelope
        .metadata
        .insert("collect.coverage.account".to_string(), account.to_string());
}

fn segments(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
) -> Vec<atlas_domain::NetworkSegment> {
    CatalogStore::list_network_segments(service.store(), tenant_id, Page::default())
        .expect("list segments")
}

fn assocs(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    host_id: Uuid,
) -> Vec<atlas_domain::HostNetAssoc> {
    RuntimeStore::list_host_net_assocs(service.store(), host_id, Page::default())
        .expect("list assocs")
}

fn host_id(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
    name: &str,
) -> Uuid {
    CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == name)
        .expect("host")
        .host_id
}

fn vpc(name: &str, subnet_cidr: &str) -> Value {
    json!({
        "network_domains": [{ "external_ref": "vpc-1", "name": "prod-vpc", "cidr": "10.0.0.0/16" }],
        "network_segments": [{
            "segment_name": name,
            "cidr": subnet_cidr,
            "external_ref": "subnet-1",
            "parent_external_ref": "vpc-1",
        }],
    })
}

#[test]
fn a_vpc_and_its_subnet_land_without_any_host() {
    // 云侧网络结构没有主机可解析 —— 也必须入库，否则拓扑里看不到网络。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    ingest(&service, "ing-1", vpc("prod-a", "10.0.1.0/24"), tenant_id);

    let segments = segments(&service, tenant_id);
    assert_eq!(segments.len(), 1, "子网要独立入库");
    let segment = &segments[0];
    assert_eq!(segment.name, "prod-a");
    assert_eq!(segment.cidr.as_deref(), Some("10.0.1.0/24"));

    let domain_id = segment.network_domain_id.expect("挂在某个域下");
    let domain = CatalogStore::get_network_domain(service.store(), domain_id)
        .expect("get domain")
        .expect("domain row");
    assert_eq!(domain.kind, NetworkDomainKind::Vpc, "VPC 是域，不是段");
    assert_eq!(domain.name, "prod-vpc");
    assert_eq!(
        domain.cidr.as_deref(),
        Some("10.0.0.0/16"),
        "VPC 的地址空间是域的核心属性，必须存下来"
    );
    assert_eq!(domain.external_ref.as_deref(), Some("vpc-1"));
}

#[test]
fn a_cloud_host_joins_its_subnet_by_explicit_ref() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let mut payload = vpc("prod-a", "10.0.1.0/24");
    payload["hosts"] = json!([{
        "hostname": "node-a",
        "cloud_instance_id": "ins-1",
        "ip_addr": "10.0.1.10",
        "subnet_external_ref": "subnet-1",
    }]);
    ingest(&service, "ing-1", payload, tenant_id);

    let host_id = host_id(&service, tenant_id, "node-a");
    let assocs = assocs(&service, host_id);
    assert_eq!(assocs.len(), 1, "主机要挂上子网");
    assert_eq!(assocs[0].ip_addr, "10.0.1.10");
    assert_eq!(
        Some(assocs[0].network_segment_id),
        segments(&service, tenant_id)
            .first()
            .map(|s| s.network_segment_id)
    );
}

#[test]
fn a_host_without_a_subnet_ref_is_matched_by_cidr_containment() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    // 两个子网都在同一条地址轴上：10.1.0.0/16 包住 10.1.2.0/24。
    let mut payload = json!({
        "network_domains": [{ "external_ref": "vpc-1", "name": "prod-vpc", "cidr": "10.0.0.0/16" }],
        "network_segments": [
            { "segment_name": "wide", "cidr": "10.1.0.0/16", "external_ref": "subnet-wide",
              "parent_external_ref": "vpc-1" },
            { "segment_name": "narrow", "cidr": "10.1.2.0/24", "external_ref": "subnet-narrow",
              "parent_external_ref": "vpc-1" },
        ],
    });
    payload["hosts"] = json!([{
        "hostname": "node-a",
        "cloud_instance_id": "ins-1",
        "ip_addr": "10.1.2.7",
    }]);
    ingest(&service, "ing-1", payload, tenant_id);

    let host_id = host_id(&service, tenant_id, "node-a");
    let assocs = assocs(&service, host_id);
    assert_eq!(assocs.len(), 1, "没给 SubnetId 也要能按 CIDR 对上");

    let narrow = segments(&service, tenant_id)
        .into_iter()
        .find(|segment| segment.name == "narrow")
        .expect("narrow");
    assert_eq!(
        assocs[0].network_segment_id, narrow.network_segment_id,
        "最长前缀胜出：10.1.2.0/24 比 10.1.0.0/16 更精确"
    );
}

#[test]
fn a_host_outside_every_subnet_gets_no_association() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let mut payload = vpc("prod-a", "10.0.1.0/24");
    payload["hosts"] = json!([{
        "hostname": "node-a",
        "cloud_instance_id": "ins-1",
        "ip_addr": "192.0.2.9",
    }]);
    ingest(&service, "ing-1", payload, tenant_id);

    let host_id = host_id(&service, tenant_id, "node-a");
    assert!(
        assocs(&service, host_id).is_empty(),
        "对不上子网就不要编一条关联出来"
    );
}

#[test]
fn changing_a_subnet_cidr_keeps_the_segment_identity() {
    // 身份是 SubnetId：改 CIDR 不该分裂成两个段。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    ingest(&service, "ing-1", vpc("prod-a", "10.0.1.0/24"), tenant_id);
    let before = segments(&service, tenant_id).remove(0);

    ingest(&service, "ing-2", vpc("prod-a", "10.0.9.0/24"), tenant_id);
    let after = segments(&service, tenant_id);

    assert_eq!(after.len(), 1, "同一个子网不能变成两行");
    assert_eq!(after[0].network_segment_id, before.network_segment_id);
    assert_eq!(after[0].cidr.as_deref(), Some("10.0.9.0/24"), "CIDR 要更新");
}

#[test]
fn moving_a_host_to_another_subnet_closes_the_old_association() {
    // 关联 ID 里含 IP，换子网就是一条新行。旧的那条必须封口（valid_to 置位），
    // 否则一台机在拓扑里同时挂在两个子网、两个 IP 上，而且看起来都「现在有效」。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let two_subnets = json!({
        "network_domains": [{ "external_ref": "vpc-1", "name": "prod-vpc", "cidr": "10.0.0.0/16" }],
        "network_segments": [
            { "segment_name": "a", "cidr": "10.0.1.0/24", "external_ref": "subnet-a",
              "parent_external_ref": "vpc-1" },
            { "segment_name": "b", "cidr": "10.0.2.0/24", "external_ref": "subnet-b",
              "parent_external_ref": "vpc-1" },
        ],
    });

    let mut first = two_subnets.clone();
    first["hosts"] = json!([{
        "hostname": "node-a",
        "cloud_instance_id": "ins-1",
        "ip_addr": "10.0.1.10",
        "subnet_external_ref": "subnet-a",
    }]);
    ingest(&service, "ing-1", first, tenant_id);

    let node_a = host_id(&service, tenant_id, "node-a");
    assert_eq!(
        assocs(&service, node_a)
            .iter()
            .filter(|assoc| assoc.validity.valid_to.is_none())
            .count(),
        1
    );

    // 换子网（连带换 IP）。
    let mut second = two_subnets;
    second["hosts"] = json!([{
        "hostname": "node-a",
        "cloud_instance_id": "ins-1",
        "ip_addr": "10.0.2.10",
        "subnet_external_ref": "subnet-b",
    }]);
    ingest(&service, "ing-2", second, tenant_id);

    let rows = assocs(&service, node_a);
    let open: Vec<_> = rows
        .iter()
        .filter(|assoc| assoc.validity.valid_to.is_none())
        .collect();
    assert_eq!(open.len(), 1, "同时只能有一条「现在有效」的关联");
    assert_eq!(open[0].ip_addr, "10.0.2.10");

    let closed: Vec<_> = rows
        .iter()
        .filter(|assoc| assoc.validity.valid_to.is_some())
        .collect();
    assert_eq!(closed.len(), 1, "旧的关联要封口，不能删（历史要留）");
    assert_eq!(closed[0].ip_addr, "10.0.1.10");
}

#[test]
fn a_deleted_subnet_is_retired_after_complete_snapshots() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let mut first = envelope("ing-1", vpc("prod-a", "10.0.1.0/24"), tenant_id);
    declare_full_account_coverage(&mut first, "tencent:a");
    // 候选要带账号，退役目标才圈得住。
    first.payload_inline = Some({
        let mut payload = vpc("prod-a", "10.0.1.0/24");
        payload["network_segments"][0]["cloud_account"] = json!("tencent:a");
        payload["network_domains"][0]["cloud_account"] = json!("tencent:a");
        payload["hosts"] = json!([{
            "hostname": "node-a",
            "cloud_instance_id": "ins-1",
            "cloud_account": "tencent:a",
            "ip_addr": "10.0.1.10",
            "subnet_external_ref": "subnet-1",
        }]);
        payload
    });
    service.submit_and_materialize(first).expect("ingest");

    assert_eq!(segments(&service, tenant_id).len(), 1);
    assert!(segments(&service, tenant_id)[0].stale_at.is_none());

    // 云上删了子网：后续完整快照里没有它（但主机还在，只是没有子网可挂）。
    let mut per_round = Vec::new();
    for round in 0..crate::service::materialize::NETWORK_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
        let mut missing = envelope(
            &format!("ing-miss-{round}"),
            json!({ "network_segments": [], "network_domains": [], "hosts": [] }),
            tenant_id,
        );
        declare_full_account_coverage(&mut missing, "tencent:a");
        let summary = service.submit_and_materialize(missing).expect("ingest").1;
        per_round.push(summary.stale_network_segment_count);
    }

    assert_eq!(
        per_round.iter().sum::<u64>(),
        1,
        "只在达到阈值那一轮报一次，不能每轮都报：{per_round:?}"
    );
    assert_eq!(
        per_round.last().copied(),
        Some(1),
        "阈值是最后一轮到达的：{per_round:?}"
    );
    let rows = segments(&service, tenant_id);
    assert_eq!(rows.len(), 1, "退役不删行");
    assert!(rows[0].stale_at.is_some(), "云上没了的子网要标退役");

    // 域跟着「一个活段都不剩」一起退役。
    let domain_id = rows[0].network_domain_id.expect("domain");
    let domain = CatalogStore::get_network_domain(service.store(), domain_id)
        .expect("get domain")
        .expect("domain row");
    assert!(domain.stale_at.is_some(), "段都没了，域也该退役");
}

#[test]
fn a_frame_that_does_not_report_networks_never_retires_them() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let mut first = envelope("ing-1", vpc("prod-a", "10.0.1.0/24"), tenant_id);
    first.payload_inline = Some({
        let mut payload = vpc("prod-a", "10.0.1.0/24");
        payload["network_segments"][0]["cloud_account"] = json!("tencent:a");
        payload["network_domains"][0]["cloud_account"] = json!("tencent:a");
        payload
    });
    declare_full_account_coverage(&mut first, "tencent:a");
    service.submit_and_materialize(first).expect("ingest");

    for round in 0..crate::service::materialize::NETWORK_RETIRE_AFTER_COMPLETE_SNAPSHOTS + 1 {
        // 没有 network_segments / ip_addresses / ips 任一键 = 本帧不表态。
        // coverage 元数据仍齐全，所以这里唯一拦着退役的就是 `reported` 闸。
        let mut quiet = envelope(
            &format!("ing-nokeys-{round}"),
            json!({ "hosts": [] }),
            tenant_id,
        );
        declare_full_account_coverage(&mut quiet, "tencent:a");
        let summary = service.submit_and_materialize(quiet).expect("ingest").1;
        assert_eq!(summary.stale_network_segment_count, 0, "没观测 ≠ 删除");
    }

    let rows = segments(&service, tenant_id);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].stale_at.is_none(), "没观测 ≠ 删除");
    assert_eq!(rows[0].miss_count, 0, "miss_count 一次都不该被推进");
}
