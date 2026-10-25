use atlas_domain::{IngestEnvelope, IngestMode, RuntimeObjectType, SourceKind, TenantId};
use atlas_storage::{CatalogStore, RuntimeStore};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::TopologyIngestService;
use crate::query::TopologyQueryService;
use crate::service::materialize::stable_uuid;

#[test]
fn submit_and_materialize_links_network_fact_to_previously_materialized_host() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let host_envelope = IngestEnvelope {
        ingest_id: "ing-host-seed".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "host",
            "host_name": "node-07",
            "machine_id": "hostname:node-07",
            "external_ref": "hostname:node-07"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(host_envelope).unwrap();

    let network_envelope = IngestEnvelope {
        ingest_id: "ing-network-followup".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "network_interface",
            "host_name": "node-07",
            "machine_id": "hostname:node-07",
            "iface_name": "eth0",
            "ip": "10.7.0.12",
            "prefix": 24,
            "gateway": "10.7.0.1"
        })),
        metadata: Default::default(),
    };

    let (_record, summary) = service.submit_and_materialize(network_envelope).unwrap();

    assert_eq!(summary.host_count, 1);
    assert_eq!(summary.network_count, 1);
    assert_eq!(summary.assoc_count, 1);

    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-07")
        .unwrap();
    let host_view = TopologyQueryService::new(service.store().clone())
        .host_topology_view(host.host_id)
        .unwrap()
        .unwrap();

    assert_eq!(host_view.network_segments.len(), 1);
    assert_eq!(
        host_view.network_segments[0].cidr.as_deref(),
        Some("10.7.0.0/24")
    );
    assert_eq!(host_view.network_assocs.len(), 1);
    assert_eq!(host_view.network_assocs[0].ip_addr, "10.7.0.12");
}

#[test]
fn submit_and_materialize_persists_process_fact_for_existing_host() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let host_envelope = IngestEnvelope {
        ingest_id: "ing-host-for-process".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "host",
            "host_name": "node-09",
            "machine_id": "hostname:node-09",
            "external_ref": "hostname:node-09"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(host_envelope).unwrap();

    let process_envelope = IngestEnvelope {
        ingest_id: "ing-process-followup".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "process",
            "host_name": "node-09",
            "machine_id": "hostname:node-09",
            "pid": "231",
            "identity": "ps_lstart:Tue May 12 05:38:01 2026",
            "process_key": "hostname:node-09:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
            "executable_name": "/usr/sbin/sshd",
            "observed_at": "2026-05-12T03:16:03Z"
        })),
        metadata: Default::default(),
    };

    service.submit_and_materialize(process_envelope).unwrap();

    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-09")
        .unwrap();
    let processes = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        atlas_storage::Page::default(),
    )
    .unwrap();

    assert_eq!(processes.len(), 1);
    assert_eq!(processes[0].pid, 231);
    assert_eq!(processes[0].executable, "/usr/sbin/sshd");
}

#[test]
fn submit_and_materialize_links_process_fact_using_process_key_host_prefix() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let host_envelope = IngestEnvelope {
        ingest_id: "ing-host-for-derived-process".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "host",
            "host_name": "node-09",
            "machine_id": "hostname:node-09",
            "external_ref": "hostname:node-09"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(host_envelope).unwrap();

    let process_envelope = IngestEnvelope {
        ingest_id: "ing-process-followup-derived".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "process",
            "pid": "231",
            "identity": "ps_lstart:Tue May 12 05:38:01 2026",
            "process_key": "hostname:node-09:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
            "executable_name": "/usr/sbin/sshd",
            "observed_at": "2026-05-12T03:16:03Z"
        })),
        metadata: Default::default(),
    };

    service.submit_and_materialize(process_envelope).unwrap();

    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-09")
        .unwrap();
    let processes = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        atlas_storage::Page::default(),
    )
    .unwrap();

    assert_eq!(processes.len(), 1);
    assert_eq!(processes[0].pid, 231);
    assert_eq!(processes[0].executable, "/usr/sbin/sshd");
}

#[test]
fn submit_and_materialize_enriches_existing_process_from_telemetry() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());

    let host_envelope = IngestEnvelope {
        ingest_id: "ing-host-for-process-telemetry".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "host",
            "host_name": "node-11",
            "machine_id": "hostname:node-11",
            "external_ref": "hostname:node-11"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(host_envelope).unwrap();

    let process_envelope = IngestEnvelope {
        ingest_id: "ing-process-for-telemetry".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "process",
            "pid": "231",
            "identity": "ps_lstart:Tue May 12 05:38:01 2026",
            "process_key": "hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
            "executable_name": "/usr/sbin/sshd",
            "observed_at": "2026-05-12T03:16:03Z"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(process_envelope).unwrap();

    let telemetry_envelope = IngestEnvelope {
        ingest_id: "ing-process-telemetry".to_string(),
        source_kind: SourceKind::TelemetrySummary,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: Some(atlas_domain::ObservedAt(
            chrono::DateTime::parse_from_rfc3339("2026-05-12T03:16:04Z")
                .unwrap()
                .with_timezone(&Utc),
        )),
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "collection_kind": "process_metrics",
            "metric_name": "process.memory.rss",
            "resource_ref": "hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026",
            "target_ref": "hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026:process",
            "value": 7456
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(telemetry_envelope).unwrap();

    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-11")
        .unwrap();
    let processes = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        atlas_storage::Page::default(),
    )
    .unwrap();

    assert_eq!(processes.len(), 1);
    assert_eq!(processes[0].memory_rss_kib, Some(7456));
    assert_eq!(
        processes[0].external_ref.as_deref(),
        Some("hostname:node-11:pid:231:ps_lstart:Tue May 12 05:38:01 2026")
    );
}

#[test]
fn submit_and_materialize_creates_runtime_binding_when_service_ref_is_present() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let now = Utc::now();

    CatalogStore::upsert_service(
        service.store(),
        &atlas_domain::ServiceEntity {
            service_id: Uuid::new_v4(),
            tenant_id,
            business_id: None,
            system_id: None,
            subsystem_id: None,
            name: "sshd".to_string(),
            namespace: None,
            service_type: atlas_domain::ServiceType::Platform,
            boundary: atlas_domain::ServiceBoundary::Internal,
            provider: None,
            external_ref: Some("svc:sshd".to_string()),
            created_at: now,
            updated_at: now,
        },
    )
    .unwrap();

    let host_envelope = IngestEnvelope {
        ingest_id: "ing-host-for-binding".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: now,
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "host",
            "host_name": "node-10",
            "machine_id": "hostname:node-10",
            "external_ref": "hostname:node-10"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(host_envelope).unwrap();

    let process_envelope = IngestEnvelope {
        ingest_id: "ing-process-binding".to_string(),
        source_kind: SourceKind::EdgeDiscovery,
        source_name: "warp-insight:agent-01".to_string(),
        ingest_mode: IngestMode::Snapshot,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: now,
        payload_ref: None,
        payload_inline: Some(json!({
            "target_kind": "process",
            "host_name": "node-10",
            "machine_id": "hostname:node-10",
            "pid": "222",
            "identity": "sshd:instance-a",
            "process_key": "hostname:node-10:pid:222:sshd:instance-a",
            "executable_name": "/usr/sbin/sshd",
            "service_ref": "svc:sshd",
            "instance_key": "process:sshd:instance-a",
            "observed_at": "2026-05-12T03:16:03Z"
        })),
        metadata: Default::default(),
    };
    service.submit_and_materialize(process_envelope).unwrap();

    let service_entity =
        CatalogStore::list_services(service.store(), tenant_id, atlas_storage::Page::default())
            .unwrap()
            .into_iter()
            .find(|item| item.external_ref.as_deref() == Some("svc:sshd"))
            .unwrap();
    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .find(|host| host.host_name == "node-10")
        .unwrap();
    let processes = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        atlas_storage::Page::default(),
    )
    .unwrap();
    assert_eq!(processes.len(), 1);

    let instance_id = stable_uuid(
        "service_instance",
        &format!(
            "{}:{}:{}",
            tenant_id.0, service_entity.service_id, "process:sshd:instance-a"
        ),
    );
    let instance = RuntimeStore::get_service_instance(service.store(), instance_id)
        .unwrap()
        .expect("service instance should exist");
    assert_eq!(instance.service_id, service_entity.service_id);

    let binding_id = stable_uuid(
        "runtime_binding",
        &format!(
            "{}:{}:{}",
            instance.instance_id, processes[0].process_id, "process"
        ),
    );
    let binding = RuntimeStore::get_runtime_binding(service.store(), binding_id)
        .unwrap()
        .expect("runtime binding should exist");
    assert_eq!(binding.instance_id, instance.instance_id);
    assert_eq!(binding.object_id, processes[0].process_id);
    assert!(matches!(binding.object_type, RuntimeObjectType::Process));
}
