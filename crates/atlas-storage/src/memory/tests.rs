use atlas_domain::{AddressFamily, EnvironmentId, NetworkDomainKind};
use chrono::Utc;

use super::*;

#[test]
fn in_memory_store_persists_host_network_and_ingest_job() {
    let store = InMemoryTopologyStore::default();
    let tenant_id = TenantId(Uuid::new_v4());
    let host = HostInventory {
        host_id: Uuid::new_v4(),
        tenant_id,
        environment_id: Some(EnvironmentId(Uuid::new_v4())),
        host_name: "node-01".to_string(),
        machine_id: Some("machine-01".to_string()),
        os_name: Some("linux".to_string()),
        os_version: Some("6.8".to_string()),
        created_at: Utc::now(),
        last_inventory_at: Utc::now(),
    };
    let domain = NetworkDomain {
        network_domain_id: Uuid::new_v4(),
        tenant_id,
        environment_id: None,
        name: "default".to_string(),
        kind: NetworkDomainKind::Unknown,
        description: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let segment = NetworkSegment {
        network_segment_id: Uuid::new_v4(),
        tenant_id,
        network_domain_id: Some(domain.network_domain_id),
        environment_id: None,
        name: "office".to_string(),
        cidr: Some("192.168.0.0/24".to_string()),
        gateway_ip: Some("192.168.0.1".to_string()),
        address_family: AddressFamily::Ipv4,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    CatalogStore::upsert_host(&store, &host).unwrap();
    CatalogStore::upsert_network_domain(&store, &domain).unwrap();
    CatalogStore::upsert_network_segment(&store, &segment).unwrap();
    IngestStore::record_ingest_job(
        &store,
        IngestJobEntry {
            ingest_id: "ing-1".to_string(),
            tenant_id,
            source_name: "fixture".to_string(),
            source_kind: "batch_import".to_string(),
            received_at: Utc::now(),
            status: "accepted".to_string(),
            payload_ref: None,
            error: None,
        },
    )
    .unwrap();

    assert_eq!(
        CatalogStore::list_hosts(&store, tenant_id, Page::default())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        CatalogStore::list_network_segments(&store, tenant_id, Page::default())
            .unwrap()
            .len(),
        1
    );
    assert!(
        IngestStore::get_ingest_job(&store, "ing-1")
            .unwrap()
            .is_some()
    );
}
