//! 退役权归属：一个云账号只认一个采集器。
//!
//! 两个生产者同时声明同一账号的账号级完整覆盖时，每帧都会给缺行 `miss_count + 1`，
//! 退役速度直接翻倍（3 轮变 1.5 轮）—— 方向是错的：采集器抖动一次就可能把还在的机器
//! 判死。所以退役权显式登记：先到先得，持有者每帧刷心跳，其他人只写数据不推进退役。

use atlas_domain::{IngestEnvelope, IngestMode, SourceKind, TenantId};
use atlas_storage::CatalogStore;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::TopologyIngestService;
use crate::service::materialize::NETWORK_RETIRE_AFTER_COMPLETE_SNAPSHOTS;

const ACCOUNT: &str = "tencent:a";

fn envelope(
    ingest_id: &str,
    producer: &str,
    payload: serde_json::Value,
    tenant_id: TenantId,
) -> IngestEnvelope {
    let mut envelope = IngestEnvelope {
        ingest_id: ingest_id.to_string(),
        source_kind: SourceKind::ExternalSync,
        source_name: format!("wist-harvest:{producer}"),
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
        .insert("collect.coverage.scope".to_string(), "account".to_string());
    envelope
        .metadata
        .insert("collect.coverage.complete".to_string(), "true".to_string());
    envelope
        .metadata
        .insert("collect.coverage.account".to_string(), ACCOUNT.to_string());
    envelope
}

fn subnet_payload() -> serde_json::Value {
    json!({
        "network_segments": [{
            "segment_name": "prod-a",
            "cidr": "10.0.1.0/24",
            "external_ref": "subnet-1",
            "cloud_account": ACCOUNT,
        }],
    })
}

/// 缺席快照：什么都没观测到，但仍是「账号级完整」，所以它带着退役意图。
fn empty_payload() -> serde_json::Value {
    json!({ "network_segments": [] })
}

fn miss_count(service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>) -> i64 {
    CatalogStore::list_network_segments(service.store(), TenantId(*TENANT), Default::default())
        .expect("list")
        .first()
        .map(|segment| segment.miss_count)
        .unwrap_or(-1)
}

// 每个用例一个固定租户，避免共享内存 store 的串扰。
const TENANT: &Uuid = &Uuid::nil();

#[test]
fn another_producer_cannot_advance_the_miss_counter() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(*TENANT);

    service
        .submit_and_materialize(envelope("a-1", "harvest-a", subnet_payload(), tenant_id))
        .expect("producer a");

    // A 声明了完整覆盖 → 持有退役权。B 随后也来声明同一账号。
    let (_, summary) = service
        .submit_and_materialize(envelope("b-1", "harvest-b", empty_payload(), tenant_id))
        .expect("producer b");
    assert_eq!(
        summary.retirement_held_by.as_deref(),
        Some("wist-harvest:harvest-a"),
        "第二个生产者必须被报出来，否则运维看不见「两个采集器抢一个账号」"
    );
    assert_eq!(summary.stale_network_segment_count, 0);
    assert_eq!(miss_count(&service), 0, "非持有者的帧不能推进 miss_count");

    // A 的缺席帧才推进。
    service
        .submit_and_materialize(envelope("a-2", "harvest-a", empty_payload(), tenant_id))
        .expect("producer a");
    assert_eq!(miss_count(&service), 1);

    // B 继续报缺席也不推进。
    service
        .submit_and_materialize(envelope("b-2", "harvest-b", empty_payload(), tenant_id))
        .expect("producer b");
    assert_eq!(miss_count(&service), 1, "B 报几轮都不能让退役更快");
}

#[test]
fn the_holder_still_retires_on_its_own_cadence() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(*TENANT);

    service
        .submit_and_materialize(envelope("a-1", "harvest-a", subnet_payload(), tenant_id))
        .expect("seed");

    let mut per_round = Vec::new();
    for round in 0..NETWORK_RETIRE_AFTER_COMPLETE_SNAPSHOTS {
        let (_, summary) = service
            .submit_and_materialize(envelope(
                &format!("a-miss-{round}"),
                "harvest-a",
                empty_payload(),
                tenant_id,
            ))
            .expect("miss");
        per_round.push(summary.stale_network_segment_count);
    }
    assert_eq!(
        per_round.iter().sum::<u64>(),
        1,
        "持有者仍按原节奏（3 轮）退役：{per_round:?}"
    );
}

#[test]
fn a_silent_holder_is_replaced_after_the_ttl() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(*TENANT);

    service
        .submit_and_materialize(envelope("a-1", "harvest-a", subnet_payload(), tenant_id))
        .expect("seed");

    // 把持有者的心跳推到 TTL 之前，模拟「A 已经不跑了」。
    let mut owner = CatalogStore::get_retirement_owner(service.store(), tenant_id, ACCOUNT)
        .expect("get owner")
        .expect("owner");
    owner.claimed_at =
        Utc::now() - chrono::Duration::hours(atlas_domain::RETIREMENT_OWNER_TTL_HOURS + 1);
    CatalogStore::upsert_retirement_owner(service.store(), &owner).expect("stale owner");

    // B 现在可以接管：不报 held_by，并且它的缺席帧会推进。
    let (_, summary) = service
        .submit_and_materialize(envelope("b-1", "harvest-b", empty_payload(), tenant_id))
        .expect("takeover");
    assert!(
        summary.retirement_held_by.is_none(),
        "过期持有者不该再拦着接管"
    );
    assert_eq!(miss_count(&service), 1, "接管后由新持有者推进");

    let owner = CatalogStore::get_retirement_owner(service.store(), tenant_id, ACCOUNT)
        .expect("get owner")
        .expect("owner");
    assert_eq!(owner.producer, "wist-harvest:harvest-b");
}

#[test]
fn a_frame_without_account_coverage_never_registers_an_owner() {
    // 端侧 / partial 帧根本不该参与退役权，也就没有登记。
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(*TENANT);
    let mut envelope = envelope("edge-1", "agentd", subnet_payload(), tenant_id);
    envelope.metadata.remove("collect.coverage.account");
    service
        .submit_and_materialize(envelope)
        .expect("edge frame");

    assert!(
        CatalogStore::get_retirement_owner(service.store(), tenant_id, ACCOUNT)
            .expect("get owner")
            .is_none()
    );
}

/// 休眠：不同用例共用一个固定租户时，内存 store 各自独立，但保留这个断言提醒。
#[test]
fn each_service_has_its_own_store() {
    let a = TopologyIngestService::new_in_memory();
    let b = TopologyIngestService::new_in_memory();
    a.submit_and_materialize(envelope(
        "a-1",
        "harvest-a",
        subnet_payload(),
        TenantId(*TENANT),
    ))
    .expect("ingest");
    assert!(
        CatalogStore::list_network_segments(b.store(), TenantId(*TENANT), Default::default())
            .expect("list")
            .is_empty()
    );
}
