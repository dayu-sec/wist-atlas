use atlas_domain::{IngestEnvelope, IngestMode, SourceKind, TenantId};
use chrono::Utc;
use orion_error::reason::ErrorIdentityProvider;
use serde_json::json;
use uuid::Uuid;

use super::{
    InMemoryIngestJobRecorder, IngestJobStatus, IngestService, extract_business_catalog_candidates,
    extract_host_candidates, extract_host_telemetry_candidates, extract_network_segment_candidates,
    extract_process_runtime_candidates, extract_process_telemetry_candidates,
    extract_responsibility_assignment_candidates, extract_subject_candidates,
};

fn envelope(payload_inline: serde_json::Value) -> IngestEnvelope {
    IngestEnvelope {
        ingest_id: "ing-1".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "test".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id: TenantId(Uuid::new_v4()),
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(payload_inline),
        metadata: Default::default(),
    }
}

#[test]
fn submit_records_ingest_job() {
    let recorder = InMemoryIngestJobRecorder::default();
    let service = IngestService::new(recorder.clone());
    let record = service.submit(envelope(json!({ "items": [] }))).unwrap();

    assert_eq!(record.status, IngestJobStatus::Accepted);
    assert_eq!(recorder.records().unwrap().len(), 1);
}

#[test]
fn submit_rejects_delta_until_supported() {
    let recorder = InMemoryIngestJobRecorder::default();
    let service = IngestService::new(recorder);
    let mut env = envelope(json!({ "items": [] }));
    env.ingest_mode = IngestMode::Delta;

    let err = service.submit(env).unwrap_err();
    assert_eq!(
        err.reason().stable_code(),
        "biz.atlas.api.ingest_mode_unsupported"
    );
}

#[test]
fn extract_business_catalog_from_items() {
    let extracted = extract_business_catalog_candidates(&envelope(json!({
        "items": [{
            "external_ref": "svc-1",
            "business_name": "payments",
            "system_name": "checkout",
            "service_name": "billing",
            "service_type": "application",
            "boundary": "internal"
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.business_name, "payments");
    assert_eq!(candidate.service_name.as_deref(), Some("billing"));
}

#[test]
fn extract_hosts_from_hosts_field() {
    let extracted = extract_host_candidates(&envelope(json!({
        "hosts": [{
            "external_ref": "host-1",
            "host_name": "node-01",
            "machine_id": "machine-01",
            "os_name": "linux"
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name, "node-01");
    assert_eq!(candidate.machine_id.as_deref(), Some("machine-01"));
}

#[test]
fn extract_hosts_from_target_edge_rows() {
    let extracted = extract_host_candidates(&envelope(json!({
        "hosts": [{
            "hostname": "node-01",
            "machine_id": "machine-01",
            "os": {
                "name": "linux",
                "version": "6.8.0"
            }
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name, "node-01");
    assert_eq!(candidate.os_name.as_deref(), Some("linux"));
    assert_eq!(candidate.os_version.as_deref(), Some("6.8.0"));
}

#[test]
fn extract_hosts_from_edge_host_fact_payload() {
    let extracted = extract_host_candidates(&envelope(json!({
        "target_kind": "host",
        "target_ref": "hostname:node-05",
        "external_ref": "hostname:node-05",
        "host_name": "node-05",
        "machine_id": "hostname:node-05"
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name, "node-05");
    assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-05"));
    assert_eq!(candidate.external_ref.as_deref(), Some("hostname:node-05"));
}

#[test]
fn extract_hosts_does_not_treat_edge_process_fact_as_host() {
    let extracted = extract_host_candidates(&envelope(json!({
        "target_kind": "process",
        "target_ref": "hostname:node-05:pid:123",
        "external_ref": "hostname:node-05:pid:123",
        "pid": "123"
    })))
    .unwrap();

    assert!(extracted.candidates.is_empty());
}

#[test]
fn extract_network_segments_from_segment_rows() {
    let extracted = extract_network_segment_candidates(&envelope(json!({
        "network_segments": [{
            "segment_name": "office-lan",
            "cidr": "192.168.10.0/24",
            "gateway_ip": "192.168.10.1"
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.segment_name.as_deref(), Some("office-lan"));
    assert_eq!(candidate.cidr.as_deref(), Some("192.168.10.0/24"));
    assert_eq!(candidate.ip_addr, None);
}

#[test]
fn extract_network_segments_from_ip_rows() {
    let extracted = extract_network_segment_candidates(&envelope(json!({
        "ips": [{
            "ip": "10.0.0.12",
            "iface_name": "eth0"
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.ip_addr.as_deref(), Some("10.0.0.12"));
    assert_eq!(candidate.iface_name.as_deref(), Some("eth0"));
    assert_eq!(candidate.cidr, None);
}

#[test]
fn extract_network_segments_from_target_interfaces() {
    let extracted = extract_network_segment_candidates(&envelope(json!({
        "hosts": [{
            "hostname": "node-01",
            "machine_id": "machine-01"
        }],
        "interfaces": [{
            "host_ref": "node-01",
            "name": "eth0",
            "addresses": [{
                "family": "ipv4",
                "ip": "192.168.10.52",
                "prefix": 24,
                "gateway": "192.168.10.1"
            }]
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name.as_deref(), Some("node-01"));
    assert_eq!(candidate.iface_name.as_deref(), Some("eth0"));
    assert_eq!(candidate.ip_addr.as_deref(), Some("192.168.10.52"));
    assert_eq!(candidate.cidr.as_deref(), Some("192.168.10.0/24"));
    assert_eq!(candidate.segment_name.as_deref(), Some("192.168.10.0/24"));
}

#[test]
fn extract_network_segments_from_edge_network_fact_payload() {
    let extracted = extract_network_segment_candidates(&envelope(json!({
        "target_kind": "network_interface",
        "host_name": "node-06",
        "machine_id": "hostname:node-06",
        "iface_name": "eth0",
        "ip": "192.168.20.15",
        "prefix": 24,
        "gateway": "192.168.20.1"
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name.as_deref(), Some("node-06"));
    assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-06"));
    assert_eq!(candidate.iface_name.as_deref(), Some("eth0"));
    assert_eq!(candidate.ip_addr.as_deref(), Some("192.168.20.15"));
    assert_eq!(candidate.cidr.as_deref(), Some("192.168.20.0/24"));
    assert_eq!(candidate.gateway_ip.as_deref(), Some("192.168.20.1"));
    assert_eq!(candidate.segment_name.as_deref(), Some("192.168.20.0/24"));
}

#[test]
fn extract_network_segments_does_not_treat_edge_process_fact_as_network() {
    let extracted = extract_network_segment_candidates(&envelope(json!({
        "target_kind": "process",
        "pid": "123",
        "external_ref": "hostname:node-06:pid:123"
    })))
    .unwrap();

    assert!(extracted.candidates.is_empty());
}

#[test]
fn extract_process_runtime_from_edge_process_fact_payload() {
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "target_kind": "process",
        "host_name": "node-08",
        "machine_id": "hostname:node-08",
        "pid": "231",
        "identity": "ps_lstart:Tue May 12 05:38:01 2026",
        "process_key": "hostname:node-08:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
        "executable_name": "/usr/sbin/sshd",
        "observed_at": "2026-05-12T03:16:03Z"
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name.as_deref(), Some("node-08"));
    assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-08"));
    assert_eq!(candidate.pid, 231);
    assert_eq!(candidate.executable, "/usr/sbin/sshd");
    assert_eq!(
        candidate.identity.as_deref(),
        Some("ps_lstart:Tue May 12 05:38:01 2026")
    );
    assert_eq!(candidate.service_ref, None);
    assert!(candidate.observed_at.is_some());
}

#[test]
fn extract_process_runtime_from_processes_array() {
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "processes": [
            {
                "pid": "231",
                "host_name": "node-08",
                "machine_id": "machine-08",
                "identity": "ps_lstart:a",
                "executable_name": "/usr/sbin/sshd"
            },
            {
                "pid": "232",
                "host_ref": "node-08",
                "machine_id": "machine-08",
                "identity": "ps_lstart:b",
                "executable_name": "/usr/bin/nginx"
            }
        ]
    })))
    .unwrap();

    assert_eq!(extracted.candidates.len(), 2);
    assert_eq!(extracted.candidates[0].pid, 231);
    assert_eq!(extracted.candidates[1].pid, 232);
    assert_eq!(extracted.candidates[1].executable, "/usr/bin/nginx");
    assert_eq!(
        extracted.candidates[1].host_name.as_deref(),
        Some("node-08")
    );
    assert_eq!(
        extracted.candidates[1].machine_id.as_deref(),
        Some("machine-08")
    );
}

#[test]
fn extract_process_runtime_from_empty_processes_array_yields_nothing() {
    // 空数组是「这个快照里没有进程」，**不是**「没有 processes 字段」。
    // 不能落回单对象路径 —— 那里会去看顶层 pid，把信封自己的字段当成一个进程。
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "processes": [],
        "pid": "999",
        "process_key": "machine-08:pid:999:start:1",
        "executable_name": "/usr/sbin/sshd"
    })))
    .unwrap();

    assert!(extracted.candidates.is_empty());
}

#[test]
fn extract_process_runtime_treats_a_null_processes_field_as_absent() {
    // 生产端把可选列表序列化成 `null` 是常态；一个 `null` 不该让整个信封被打回。
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "processes": null,
        "pid": "999",
        "process_key": "machine-08:pid:999:start:1",
        "executable_name": "/usr/sbin/sshd"
    })))
    .unwrap();

    assert_eq!(extracted.candidates.len(), 1);
    assert_eq!(extracted.candidates[0].pid, 999);
}

#[test]
fn extract_process_runtime_processes_array_wins_over_top_level_fact() {
    // 两种形状同时在（网关补字段是常态）：数组是权威，不能两个路径都跑一遍导致重复计数。
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "pid": "1",
        "process_key": "machine-01:pid:1:start:1",
        "executable_name": "/sbin/launchd",
        "processes": [
            { "pid": "231", "executable_name": "/usr/sbin/sshd" },
            { "pid": "232", "executable_name": "/usr/bin/nginx" }
        ]
    })))
    .unwrap();

    assert_eq!(extracted.candidates.len(), 2);
    let pids: Vec<i32> = extracted.candidates.iter().map(|c| c.pid).collect();
    assert_eq!(pids, vec![231, 232]);
}

#[test]
fn extract_process_runtime_accepts_numeric_pid_in_a_processes_row() {
    // 目标态 `atlas.in.edge.v1` 里 `pid` 是数字，不是字符串。
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "processes": [{ "pid": 210, "executable_name": "/usr/sbin/sshd" }]
    })))
    .unwrap();

    assert_eq!(extracted.candidates.len(), 1);
    assert_eq!(extracted.candidates[0].pid, 210);
}

#[test]
fn extract_process_runtime_accepts_numeric_pid_in_an_edge_process_fact() {
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "pid": 4242,
        "process_key": "machine-08:pid:4242:start:9",
        "executable_name": "/usr/bin/nginx"
    })))
    .unwrap();

    assert_eq!(extracted.candidates.len(), 1);
    assert_eq!(extracted.candidates[0].pid, 4242);
    assert_eq!(extracted.candidates[0].executable, "/usr/bin/nginx");
}

#[test]
fn extract_process_runtime_rejects_a_non_object_processes_row() {
    let err = extract_process_runtime_candidates(&envelope(json!({
        "processes": [{ "pid": 1, "executable_name": "/a" }, "not-an-object"]
    })))
    .unwrap_err();

    assert_eq!(err.reason().stable_code(), "biz.atlas.api.field_invalid");
}

#[test]
fn extract_process_runtime_still_rejects_a_scalar_processes_field() {
    // `null` 宽容了，但类型错（比如误写成字符串）必须继续硬失败：
    // 静默当成空列表会把一个缩进错的生产端伪装成「这台机没进程」。
    let err = extract_process_runtime_candidates(&envelope(json!({
        "processes": "oops"
    })))
    .unwrap_err();

    assert_eq!(err.reason().stable_code(), "biz.atlas.api.field_invalid");
}

#[test]
fn extract_process_runtime_rejects_an_unparsable_pid() {
    let err = extract_process_runtime_candidates(&envelope(json!({
        "processes": [{ "pid": "not-a-pid", "executable_name": "/a" }]
    })))
    .unwrap_err();

    assert_eq!(err.reason().stable_code(), "biz.atlas.api.field_invalid");
}

#[test]
fn extract_process_runtime_reports_a_missing_pid() {
    let err = extract_process_runtime_candidates(&envelope(json!({
        "processes": [{ "executable_name": "/a" }]
    })))
    .unwrap_err();

    assert_eq!(err.reason().stable_code(), "biz.atlas.api.field_missing");
}

#[test]
fn extract_process_runtime_derives_host_locator_from_process_key() {
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "target_kind": "process",
        "pid": "231",
        "identity": "ps_lstart:Tue May 12 05:38:01 2026",
        "process_key": "hostname:node-08:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
        "executable_name": "/usr/sbin/sshd",
        "observed_at": "2026-05-12T03:16:03Z"
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name.as_deref(), Some("node-08"));
    assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-08"));
    assert_eq!(candidate.pid, 231);
}

#[test]
fn extract_process_runtime_does_not_treat_host_fact_as_process() {
    let extracted = extract_process_runtime_candidates(&envelope(json!({
        "target_kind": "host",
        "host_name": "node-08",
        "machine_id": "hostname:node-08"
    })))
    .unwrap();

    assert!(extracted.candidates.is_empty());
}

#[test]
fn extract_host_telemetry_from_atlas_telemetry_payload() {
    let extracted = extract_host_telemetry_candidates(&IngestEnvelope {
        ingest_id: "telemetry-1".to_string(),
        source_kind: SourceKind::TelemetrySummary,
        source_name: "test".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id: TenantId(Uuid::new_v4()),
        environment_id: None,
        observed_at: Some(atlas_domain::ObservedAt(Utc::now())),
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "collection_kind": "host_metrics",
            "metric_name": "system.target.count",
            "target_ref": "hostname:node-11:host",
            "resource_ref": "hostname:node-11",
            "value": 1
        })),
        metadata: Default::default(),
    })
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name.as_deref(), Some("node-11"));
    assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-11"));
    assert_eq!(candidate.metric_name, "system.target.count");
    assert_eq!(candidate.value_i64, Some(1));
}

#[test]
fn extract_process_telemetry_from_atlas_telemetry_payload() {
    let extracted = extract_process_telemetry_candidates(&IngestEnvelope {
        ingest_id: "telemetry-process-1".to_string(),
        source_kind: SourceKind::TelemetrySummary,
        source_name: "test".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id: TenantId(Uuid::new_v4()),
        environment_id: None,
        observed_at: Some(atlas_domain::ObservedAt(Utc::now())),
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "collection_kind": "process_metrics",
            "metric_name": "process.memory.rss",
            "target_ref": "hostname:node-11:pid:231:started:abc:process",
            "resource_ref": "hostname:node-11:pid:231:started:abc",
            "value": 7456
        })),
        metadata: Default::default(),
    })
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.host_name.as_deref(), Some("node-11"));
    assert_eq!(candidate.machine_id.as_deref(), Some("hostname:node-11"));
    assert_eq!(candidate.pid, 231);
    assert_eq!(
        candidate.process_ref,
        "hostname:node-11:pid:231:started:abc"
    );
    assert_eq!(candidate.metric_name, "process.memory.rss");
    assert_eq!(candidate.value_i64, Some(7456));
}

#[test]
fn extract_subjects_from_rows() {
    let extracted = extract_subject_candidates(&envelope(json!({
        "subjects": [{
            "display_name": "alice",
            "email": "alice@example.com",
            "subject_type": "user"
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.display_name, "alice");
    assert_eq!(candidate.email.as_deref(), Some("alice@example.com"));
}

#[test]
fn extract_subjects_from_target_users_and_groups() {
    let extracted = extract_subject_candidates(&envelope(json!({
        "users": [{
            "external_id": "user-alice",
            "display_name": "Alice",
            "email": "alice@example.com"
        }],
        "groups": [{
            "external_id": "team-platform",
            "name": "platform",
            "group_type": "team"
        }]
    })))
    .unwrap();

    assert_eq!(extracted.candidates.len(), 2);
    assert_eq!(
        extracted.candidates[0].external_ref.as_deref(),
        Some("user-alice")
    );
    assert_eq!(extracted.candidates[1].display_name, "platform");
    assert!(matches!(
        extracted.candidates[1].subject_type,
        atlas_domain::SubjectType::Team
    ));
}

#[test]
fn extract_responsibility_assignments_from_rows() {
    let extracted = extract_responsibility_assignment_candidates(&envelope(json!({
        "responsibility_assignments": [{
            "subject_display_name": "alice",
            "subject_email": "alice@example.com",
            "target_kind": "host",
            "target_external_ref": "node-01",
            "role": "owner"
        }]
    })))
    .unwrap();

    let candidate = &extracted.candidates[0];
    assert_eq!(candidate.subject_display_name.as_deref(), Some("alice"));
    assert_eq!(
        candidate.subject_email.as_deref(),
        Some("alice@example.com")
    );
    assert!(matches!(
        candidate.target_kind,
        atlas_domain::ObjectKind::Host
    ));
}
