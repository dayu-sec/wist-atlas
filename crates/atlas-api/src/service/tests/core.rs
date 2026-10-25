use atlas_domain::{AtlasInputEnvelope, IngestEnvelope, IngestMode, SourceKind, TenantId};
use atlas_storage::{CatalogStore, IngestStore};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::IngestJobStatus;
use crate::TopologyIngestService;
use crate::query::TopologyQueryService;

#[test]
fn submit_and_materialize_persists_minimal_host_network_closure() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let envelope = IngestEnvelope {
        ingest_id: "ing-1".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "fixture".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "hosts": [{
                "host_name": "node-01",
                "machine_id": "machine-01",
                "os_name": "linux"
            }],
            "ips": [{
                "ip": "10.0.0.12",
                "cidr": "10.0.0.0/24",
                "host_name": "node-01",
                "machine_id": "machine-01",
                "iface_name": "eth0"
            }]
        })),
        metadata: Default::default(),
    };

    let (record, summary) = service.submit_and_materialize(envelope).unwrap();

    assert_eq!(record.status, IngestJobStatus::Accepted);
    assert_eq!(summary.host_count, 1);
    assert_eq!(summary.network_count, 1);
    assert_eq!(summary.assoc_count, 1);
    assert_eq!(
        CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default(),)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        CatalogStore::list_network_segments(
            service.store(),
            tenant_id,
            atlas_storage::Page::default(),
        )
        .unwrap()
        .len(),
        1
    );
    assert!(
        IngestStore::get_ingest_job(service.store(), "ing-1")
            .unwrap()
            .is_some()
    );
}

#[test]
fn submit_and_materialize_can_be_queried_as_topology_views() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let envelope = IngestEnvelope {
        ingest_id: "ing-2".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "fixture".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "hosts": [{
                "host_name": "node-02",
                "machine_id": "machine-02",
                "os_name": "linux"
            }],
            "ips": [{
                "ip": "10.1.0.12",
                "cidr": "10.1.0.0/24",
                "host_name": "node-02",
                "machine_id": "machine-02",
                "iface_name": "eth0"
            }]
        })),
        metadata: Default::default(),
    };

    service.submit_and_materialize(envelope).unwrap();

    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let segment = CatalogStore::list_network_segments(
        service.store(),
        tenant_id,
        atlas_storage::Page::default(),
    )
    .unwrap()
    .into_iter()
    .next()
    .unwrap();

    let host_view = TopologyQueryService::new(service.store().clone())
        .host_topology_view(host.host_id)
        .unwrap()
        .unwrap();
    let network_view = TopologyQueryService::new(service.store().clone())
        .network_topology_view(segment.network_segment_id)
        .unwrap()
        .unwrap();

    assert_eq!(host_view.host.host_name, "node-02");
    assert_eq!(host_view.network_segments.len(), 1);
    assert_eq!(host_view.network_assocs.len(), 1);
    assert_eq!(network_view.segment.name, "10.1.0.0/24");
    assert_eq!(network_view.hosts.len(), 1);
    assert_eq!(network_view.host_assocs.len(), 1);
}

#[test]
fn submit_and_materialize_builds_effective_responsibility_view() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let envelope = IngestEnvelope {
        ingest_id: "ing-3".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "fixture".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "hosts": [{
                "host_name": "node-03",
                "machine_id": "machine-03",
                "os_name": "linux"
            }],
            "ips": [{
                "ip": "10.3.0.12",
                "cidr": "10.3.0.0/24",
                "host_name": "node-03",
                "machine_id": "machine-03",
                "iface_name": "eth0"
            }],
            "subjects": [{
                "display_name": "alice",
                "email": "alice@example.com",
                "subject_type": "user"
            }],
            "responsibility_assignments": [{
                "subject_display_name": "alice",
                "subject_email": "alice@example.com",
                "target_kind": "host",
                "target_external_ref": "node-03",
                "role": "owner"
            }]
        })),
        metadata: Default::default(),
    };

    service.submit_and_materialize(envelope).unwrap();

    let host = CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let views = TopologyQueryService::new(service.store().clone())
        .effective_responsibility_view(atlas_domain::ObjectKind::Host, host.host_id)
        .unwrap();

    assert_eq!(views.len(), 1);
    assert_eq!(views[0].subject.display_name, "alice");
    assert!(matches!(
        views[0].assignment.role,
        atlas_domain::ResponsibilityRole::Owner
    ));
}

#[test]
fn submit_atlas_input_and_materialize_accepts_target_edge_envelope() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let input: AtlasInputEnvelope = serde_json::from_value(json!({
        "schema": "atlas.in.edge.v1",
        "source": {
            "system": "warp-insight",
            "producer": "agent-01",
            "tenant": "tenant-demo",
            "env": "prod"
        },
        "collect": {
            "mode": "snapshot",
            "snap_id": "snap-001",
            "observed_at": "2026-04-26T02:20:30Z"
        },
        "payload": {
            "hosts": [{
                "hostname": "node-04",
                "machine_id": "machine-04",
                "os": { "name": "linux", "version": "6.8.0" }
            }],
            "interfaces": [{
                "host_ref": "node-04",
                "name": "eth0",
                "addresses": [{
                    "family": "ipv4",
                    "ip": "10.4.0.12",
                    "prefix": 24,
                    "gateway": "10.4.0.1"
                }]
            }]
        }
    }))
    .unwrap();

    let (record, summary) = service
        .submit_atlas_input_and_materialize(input, tenant_id, None)
        .unwrap();

    assert_eq!(
        record.ingest_id,
        "atlas.in.edge.v1:warp-insight:agent-01:tenant-demo:prod:snap-001"
    );
    assert_eq!(summary.host_count, 1);
    assert_eq!(summary.network_count, 1);
    assert_eq!(summary.assoc_count, 1);

    let segment = CatalogStore::list_network_segments(
        service.store(),
        tenant_id,
        atlas_storage::Page::default(),
    )
    .unwrap()
    .into_iter()
    .next()
    .unwrap();
    assert_eq!(segment.name, "10.4.0.0/24");
    assert_eq!(segment.cidr.as_deref(), Some("10.4.0.0/24"));
}

#[test]
fn submit_and_materialize_persists_host_without_network_candidates() {
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(Uuid::new_v4());
    let envelope = IngestEnvelope {
        ingest_id: "ing-host-only".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "fixture".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None,
        observed_at: None,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(json!({
            "hosts": [{
                "host_name": "node-host-only",
                "machine_id": "machine-host-only",
                "external_ref": "machine-host-only"
            }]
        })),
        metadata: Default::default(),
    };

    let (record, summary) = service.submit_and_materialize(envelope).unwrap();

    assert_eq!(record.status, IngestJobStatus::Accepted);
    assert_eq!(summary.host_count, 1);
    assert_eq!(summary.network_count, 0);
    assert_eq!(summary.assoc_count, 0);

    let hosts =
        CatalogStore::list_hosts(service.store(), tenant_id, atlas_storage::Page::default())
            .unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].host_name, "node-host-only");
    assert_eq!(hosts[0].machine_id.as_deref(), Some("machine-host-only"));
}
