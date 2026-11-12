//! sync / async 双份实现的等价性。
//!
//! 生产路径是 async（`atlas-wist-ingest`、HTTP 写入、`serve` 全走它），sync 只给 CLI
//! 和内存替身用。两条路径各写一份代码，**只改一边编译器不会响**。
//!
//! 真账号 smoke 就是这么翻车的：A2 给 `materialize_candidates` 加了 presence 与
//! `network_segment_ids`，忘了同名的 `_async`。结果是「单测全绿、上线后子网行里
//! `cloud_account` 和 `ingest_scope` 全是 NULL，退役清扫永远打不到它们」。
//!
//! 所以这里不逐个字段挑，而是**把同一份信封各跑一遍，然后比对两个库里能读出来的
//! 全部对象**。以后再加一类对象，这个用例会自动覆盖到。

use atlas_domain::{IngestEnvelope, IngestMode, SourceKind, TenantId};
use atlas_storage::{CatalogStore, Page, RuntimeStore};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::TopologyIngestService;

const ACCOUNT: &str = "tencent:100041355715";

/// 一帧尽量「什么都有」的云资产快照：主机 + EIP + VPC + 子网 + 主机↔子网关联。
fn payload() -> Value {
    json!({
        "network_domains": [{
            "external_ref": "vpc-1",
            "name": "prod-vpc",
            "cidr": "10.0.0.0/16",
            "cloud_account": ACCOUNT,
        }],
        "network_segments": [{
            "segment_name": "prod-a",
            "cidr": "10.0.1.0/24",
            "gateway_ip": "10.0.1.1",
            "external_ref": "subnet-1",
            "parent_external_ref": "vpc-1",
            "cloud_account": ACCOUNT,
        }],
        "hosts": [{
            "host_name": "node-a",
            "cloud_instance_id": "ins-1",
            "cloud_account": ACCOUNT,
            "os_name": "Ubuntu Server 24.04 LTS 64bit",
            "ip": "10.0.1.10",
            "ip_addr": "10.0.1.10",
            "subnet_external_ref": "subnet-1",
        }],
        "elastic_ips": [{
            "external_ref": "eip-1",
            "address": "114.132.225.16",
            "status": "BIND",
            "charge_type": "TRAFFIC_POSTPAID_BY_HOUR",
            "bandwidth_mbps": 200,
            "cloud_account": ACCOUNT,
            "bound_instance_ref": "ins-1",
            "bound_instance_type": "CVM",
        }],
    })
}

fn envelope(ingest_id: &str, tenant_id: TenantId) -> IngestEnvelope {
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
        payload_inline: Some(payload()),
        metadata: Default::default(),
    };
    envelope
        .metadata
        .insert("collect.coverage.scope".to_string(), "account".to_string());
    envelope
        .metadata
        .insert("collect.coverage.complete".to_string(), "true".to_string());
    envelope
        .metadata
        .insert("collect.coverage.account".to_string(), ACCOUNT.to_string());
    envelope
}

/// 从库里读回来的全量视图（不含时间戳：两边各自 `Utc::now()`，微秒级必然不同）。
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    hosts: Vec<(String, Option<String>, Option<String>, bool, i64)>,
    elastic_ips: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        bool,
        i64,
        Option<String>,
    )>,
    domains: Vec<(String, Option<String>, Option<String>, bool, i64)>,
    segments: Vec<(
        String,
        String,
        Option<String>,
        Option<String>,
        bool,
        i64,
        Option<String>,
    )>,
    assoc_count: usize,
}

fn snapshot_of(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant: TenantId,
) -> Snapshot {
    let page = Page::default();
    let hosts = CatalogStore::list_hosts(service.store(), tenant, page)
        .expect("list hosts")
        .into_iter()
        .map(|host| {
            (
                host.host_name,
                host.cloud_instance_id,
                host.cloud_account,
                host.stale_at.is_some(),
                host.miss_count,
            )
        })
        .collect();
    let elastic_ips = CatalogStore::list_elastic_ips(service.store(), tenant, page)
        .expect("list eips")
        .into_iter()
        .map(|ip| {
            (
                ip.external_ref,
                ip.address,
                ip.status.as_str().to_string(),
                ip.cloud_account,
                ip.binding
                    .as_ref()
                    .and_then(|binding| binding.external_target_ref.clone()),
                ip.bandwidth_mbps,
                ip.stale_at.is_some(),
                ip.miss_count,
                ip.ingest_scope,
            )
        })
        .collect();
    let domains = CatalogStore::list_network_domains(service.store(), tenant, page)
        .expect("list domains")
        .into_iter()
        .map(|domain| {
            (
                domain.name,
                domain.cidr,
                domain.cloud_account,
                domain.stale_at.is_some(),
                domain.miss_count,
            )
        })
        .collect();
    let segments = CatalogStore::list_network_segments(service.store(), tenant, page)
        .expect("list segments")
        .into_iter()
        .map(|segment| {
            (
                segment.external_ref.unwrap_or_default(),
                segment.name,
                segment.cloud_account,
                segment.ingest_scope,
                segment.stale_at.is_some(),
                segment.miss_count,
                segment.cidr,
            )
        })
        .collect();
    let assoc_count = CatalogStore::list_hosts(service.store(), tenant, page)
        .expect("list hosts")
        .iter()
        .map(|host| {
            RuntimeStore::list_host_net_assocs(service.store(), host.host_id, page)
                .expect("list assocs")
                .len()
        })
        .sum();
    Snapshot {
        hosts,
        elastic_ips,
        domains,
        segments,
        assoc_count,
    }
}

#[tokio::test]
async fn the_async_path_persists_exactly_what_the_sync_path_persists() {
    let tenant = TenantId(Uuid::new_v4());

    let sync_service = TopologyIngestService::new_in_memory();
    sync_service
        .submit_and_materialize(envelope("ing-1", tenant))
        .expect("sync ingest");

    let async_service = TopologyIngestService::new_in_memory();
    async_service
        .submit_and_materialize_async(envelope("ing-1", tenant))
        .await
        .expect("async ingest");

    let from_sync = snapshot_of(&sync_service, tenant);
    let from_async = snapshot_of(&async_service, tenant);

    // 先确认这条用例真的采到了东西（空对空也能「相等」，那是最坏的一种绿）。
    assert_eq!(from_sync.hosts.len(), 1, "{from_sync:?}");
    assert_eq!(from_sync.elastic_ips.len(), 1, "{from_sync:?}");
    assert_eq!(from_sync.domains.len(), 1, "{from_sync:?}");
    assert_eq!(from_sync.segments.len(), 1, "{from_sync:?}");
    assert_eq!(from_sync.assoc_count, 1, "{from_sync:?}");

    assert_eq!(
        from_async, from_sync,
        "sync / async 落库结果必须逐字段一致（生产走 async，sync 只给 CLI）"
    );
}

#[tokio::test]
async fn the_async_path_writes_presence_for_every_object_kind() {
    // 上一条用例依赖「两边一样」这个间接证据；这条直接钉住「生产路径写了 presence」。
    let tenant = TenantId(Uuid::new_v4());
    let service = TopologyIngestService::new_in_memory();
    service
        .submit_and_materialize_async(envelope("ing-1", tenant))
        .await
        .expect("ingest");

    let snapshot = snapshot_of(&service, tenant);
    // (…, cloud_account, stale, miss_count)
    assert_eq!(
        snapshot.hosts[0].2.as_deref(),
        Some(ACCOUNT),
        "主机缺账号键"
    );
    assert!(!snapshot.hosts[0].3);
    assert_eq!(snapshot.hosts[0].4, 0);

    assert_eq!(
        snapshot.elastic_ips[0].3.as_deref(),
        Some(ACCOUNT),
        "EIP 缺账号键"
    );
    assert!(!snapshot.elastic_ips[0].6);
    assert_eq!(snapshot.elastic_ips[0].7, 0);
    assert!(
        snapshot.elastic_ips[0].8.is_some(),
        "EIP 缺 ingest_scope：诊断「这一行最后是谁写的」就没线索了"
    );

    assert_eq!(
        snapshot.domains[0].2.as_deref(),
        Some(ACCOUNT),
        "域缺账号键：域退役靠「域下还有没有活段」，但账号闸先把它筛掉了"
    );
    assert!(!snapshot.domains[0].3);

    assert_eq!(
        snapshot.segments[0].2.as_deref(),
        Some(ACCOUNT),
        "段缺账号键（真账号 smoke 踩过的那个）"
    );
    assert!(
        snapshot.segments[0].3.is_some(),
        "段缺 ingest_scope（真账号 smoke 踩过的那个）"
    );
    assert!(!snapshot.segments[0].4);
    assert_eq!(snapshot.segments[0].5, 0);
}
