use chrono::Utc;
use orion_error::reason::ErrorIdentityProvider;
use serde_json::json;
use uuid::Uuid;

use crate::TenantId;

use super::{AtlasInputEnvelope, AtlasInputMode};

#[test]
fn atlas_input_validate_accepts_target_snapshot() {
    let input: AtlasInputEnvelope = serde_json::from_value(json!({
        "schema": "atlas.in.edge.v1",
        "source": {
            "system": "warp-insight",
            "producer": "agent-01",
            "tenant": "demo",
            "env": "prod"
        },
        "collect": {
            "mode": "snapshot",
            "snap_id": "snap-001",
            "observed_at": "2026-04-26T02:20:30Z"
        },
        "payload": {
            "hosts": []
        }
    }))
    .unwrap();

    input.validate().unwrap();
    assert_eq!(input.schema_family(), Some("edge"));
    assert!(matches!(input.collect.mode, AtlasInputMode::Snapshot));
}

#[test]
fn atlas_input_accepts_legacy_dayu_schema_prefix() {
    // 兼容别名：旧 `dayu.in.*` 仍可解析，避免升级时把在路上的数据整批拒掉。
    let input: AtlasInputEnvelope = serde_json::from_value(json!({
        "schema": "dayu.in.edge.v1",
        "source": {
            "system": "warp-insight",
            "producer": "agent-01",
            "tenant": "demo",
            "env": "prod"
        },
        "collect": {
            "mode": "snapshot",
            "snap_id": "snap-001",
            "observed_at": "2026-04-26T02:20:30Z"
        },
        "payload": {
            "hosts": []
        }
    }))
    .unwrap();

    input.validate().unwrap();
    assert_eq!(input.schema_family(), Some("edge"));
}

#[test]
fn atlas_input_rejects_snapshot_without_snap_id() {
    let input: AtlasInputEnvelope = serde_json::from_value(json!({
        "schema": "atlas.in.edge.v1",
        "source": {
            "system": "warp-insight",
            "producer": "agent-01",
            "tenant": "demo"
        },
        "collect": {
            "mode": "snapshot",
            "observed_at": "2026-04-26T02:20:30Z"
        },
        "payload": {}
    }))
    .unwrap();

    let err = input.validate().unwrap_err();
    assert_eq!(err.reason().stable_code(), "biz.atlas.domain.field_missing");
    assert!(
        err.detail()
            .as_deref()
            .is_some_and(|detail| detail.contains("collect.snap_id"))
    );
}

#[test]
fn atlas_input_uses_standard_snapshot_idempotency_key_as_ingest_id() {
    let input: AtlasInputEnvelope = serde_json::from_value(json!({
        "schema": "atlas.in.edge.v1",
        "source": {
            "system": "warp-insight",
            "producer": "agent-01",
            "tenant": "demo",
            "env": "prod"
        },
        "collect": {
            "mode": "snapshot",
            "snap_id": "snap-001",
            "observed_at": "2026-04-26T02:20:30Z"
        },
        "payload": {}
    }))
    .unwrap();

    let ingest = input.into_ingest_envelope(TenantId(Uuid::new_v4()), None, Utc::now());

    assert_eq!(
        ingest.ingest_id,
        "atlas.in.edge.v1:warp-insight:agent-01:demo:prod:snap-001"
    );
    assert_eq!(
        ingest.metadata.get("idempotency_key").map(String::as_str),
        Some("atlas.in.edge.v1:warp-insight:agent-01:demo:prod:snap-001")
    );
}

#[test]
fn atlas_input_accepts_short_source_aliases_and_numeric_res_ver() {
    let input: AtlasInputEnvelope = serde_json::from_value(json!({
        "schema": "atlas.in.edge.v1",
        "source": {
            "kind": "edge",
            "system": "warp-insight",
            "producer": "agent-local-01",
            "tenant_ref": "tenant-demo",
            "env_ref": "office"
        },
        "collect": {
            "mode": "snapshot",
            "snap_id": "edge-snap-local-01",
            "observed_at": "2026-05-12T03:16:04Z",
            "collected_at": "2026-05-12T03:16:05Z",
            "res_ver": 4
        },
        "payload": {
            "host_name": "local-host"
        }
    }))
    .unwrap();

    input.validate().unwrap();
    assert_eq!(input.source.tenant, "tenant-demo");
    assert_eq!(input.source.env.as_deref(), Some("office"));
    assert_eq!(input.collect.res_ver.as_deref(), Some("4"));
    assert_eq!(
        input.idempotency_key(),
        "atlas.in.edge.v1:warp-insight:agent-local-01:tenant-demo:office:edge-snap-local-01"
    );
}

/// 完整性声明是「能不能拿缺席当删除证据」的唯一开关。
#[test]
fn coverage_must_be_account_and_complete_to_be_authoritative() {
    let build = |coverage: serde_json::Value| -> AtlasInputEnvelope {
        serde_json::from_value(json!({
            "schema": "atlas.in.inventory.v1",
            "source": {
                "system": "wist-harvest",
                "producer": "tencent-cloud-prod",
                "tenant": "demo"
            },
            "collect": {
                "mode": "snapshot",
                "snap_id": "snap-1",
                "coverage": coverage
            },
            "payload": { "hosts": [] }
        }))
        .unwrap()
    };

    let declarations = |input: AtlasInputEnvelope| -> (bool, bool) {
        let authoritative = input
            .collect
            .coverage
            .as_ref()
            .is_some_and(|coverage| coverage.is_authoritative());
        let envelope = input.into_ingest_envelope(TenantId(Uuid::new_v4()), None, Utc::now());
        (authoritative, envelope.declares_full_account_coverage())
    };

    assert_eq!(
        declarations(build(json!({"scope": "account", "complete": true}))),
        (true, true)
    );
    assert_eq!(
        declarations(build(json!({"scope": "partial", "complete": true}))),
        (false, false),
        "手写地域子集：没有它什么也不证明"
    );
    assert_eq!(
        declarations(build(json!({"scope": "account", "complete": false}))),
        (false, false),
        "采集器自己说没采全"
    );
    // 没声明 = 什么也没说。老采集器/端侧都走这条路。
    assert_eq!(declarations(build(json!(null))), (false, false));
}
