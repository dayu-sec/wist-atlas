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
        cloud_instance_id: None,
        cloud_account: None,
        os_name: Some("linux".to_string()),
        os_version: Some("6.8".to_string()),
        arch: Some("x86_64".to_string()),
        created_at: Utc::now(),
        last_inventory_at: Utc::now(),
        stale_at: None,
        miss_count: 0,
        ingest_scope: None,
    };
    let domain = NetworkDomain {
        network_domain_id: Uuid::new_v4(),
        tenant_id,
        environment_id: None,
        external_ref: None,
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
        external_ref: None,
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

mod software {
    use atlas_domain::{
        AnchorKey, AnchorKind, ClaimedSource, FingerprintStatus, SoftwareArtifact,
        SoftwareFingerprint, SoftwareInstance, SoftwareInstanceObservation, SoftwareRole,
    };
    use chrono::{Duration, Utc};

    use crate::{Page, SoftwareStore};

    use super::*;

    fn instance(host_id: Uuid, tenant_id: TenantId, instance_id: Uuid) -> SoftwareInstance {
        let now = Utc::now();
        SoftwareInstance {
            instance_id,
            tenant_id,
            host_id,
            anchor_kind: AnchorKind::SystemdUnit,
            anchor_value: "nginx.service".to_string(),
            current_artifact_id: Some("sha256:aaaa".to_string()),
            identity_missing_reason: None,
            claimed_package_name: Some("nginx".to_string()),
            claimed_product_id: Some("nginx/nginx".to_string()),
            claimed_version: Some("1.24".to_string()),
            claimed_source: Some(ClaimedSource::PathPackage),
            role: vec![SoftwareRole::Middleware],
            reason: vec![atlas_domain::SoftwareReason::Listener],
            status: FingerprintStatus::Observed,
            first_seen_at: now,
            last_seen_at: now,
            stale_at: None,
        }
    }

    #[test]
    fn upserting_by_anchor_keeps_the_first_instance_id() {
        // 中心按 anchor 归并，agentd 每轮都可能带一个新建的 id。
        // 如果归并时把 id 换掉，观测时间线与路径明细就会挂在一个不存在的实例上。
        let store = InMemoryTopologyStore::default();
        let tenant_id = TenantId(Uuid::new_v4());
        let host_id = Uuid::new_v4();
        let first_id = Uuid::new_v4();

        SoftwareStore::upsert_instance(&store, &instance(host_id, tenant_id, first_id)).unwrap();

        let mut upgraded = instance(host_id, tenant_id, Uuid::new_v4());
        upgraded.current_artifact_id = Some("sha256:bbbb".to_string());
        upgraded.claimed_version = Some("1.25".to_string());
        SoftwareStore::upsert_instance(&store, &upgraded).unwrap();

        let rows =
            SoftwareStore::list_instances_for_host(&store, host_id, Page::default()).unwrap();
        assert_eq!(rows.len(), 1, "同一个部署位只能有一行");
        assert_eq!(rows[0].instance_id, first_id, "instance_id 必须保留");
        assert_eq!(
            rows[0].current_artifact_id.as_deref(),
            Some("sha256:bbbb"),
            "内容要更新到最新一轮"
        );
    }

    #[test]
    fn retiring_marks_only_the_anchors_missing_from_this_round() {
        let store = InMemoryTopologyStore::default();
        let tenant_id = TenantId(Uuid::new_v4());
        let host_id = Uuid::new_v4();

        SoftwareStore::upsert_instance(&store, &instance(host_id, tenant_id, Uuid::new_v4()))
            .unwrap();
        let mut other = instance(host_id, tenant_id, Uuid::new_v4());
        other.anchor_value = "postgresql@16-main.service".to_string();
        SoftwareStore::upsert_instance(&store, &other).unwrap();

        // 本轮只看到 nginx：postgres 该退役，nginx 不该。
        let seen = vec![AnchorKey::new(AnchorKind::SystemdUnit, "nginx.service")];
        let marked =
            SoftwareStore::mark_instances_stale(&store, host_id, &seen, Utc::now()).unwrap();
        assert_eq!(marked, 1);

        let live =
            SoftwareStore::list_instances_for_host(&store, host_id, Page::default()).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].anchor_value, "nginx.service");

        // 退役行不删：历史要查得到。
        let retired = SoftwareStore::get_instance_by_anchor(
            &store,
            host_id,
            &AnchorKey::new(AnchorKind::SystemdUnit, "postgresql@16-main.service"),
        )
        .unwrap()
        .expect("退役行应保留");
        assert!(!retired.is_live());
    }

    #[test]
    fn seeing_an_instance_again_revives_it() {
        let store = InMemoryTopologyStore::default();
        let tenant_id = TenantId(Uuid::new_v4());
        let host_id = Uuid::new_v4();
        SoftwareStore::upsert_instance(&store, &instance(host_id, tenant_id, Uuid::new_v4()))
            .unwrap();

        SoftwareStore::mark_instances_stale(&store, host_id, &[], Utc::now()).unwrap();
        assert_eq!(
            SoftwareStore::list_instances_for_host(&store, host_id, Page::default())
                .unwrap()
                .len(),
            0
        );

        // 下一轮又出现了：复活（stale_at 由 upsert 带回来，这里是 None）。
        SoftwareStore::upsert_instance(&store, &instance(host_id, tenant_id, Uuid::new_v4()))
            .unwrap();
        assert_eq!(
            SoftwareStore::list_instances_for_host(&store, host_id, Page::default())
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn observations_are_deduped_and_returned_newest_first() {
        let store = InMemoryTopologyStore::default();
        let instance_id = Uuid::new_v4();
        let now = Utc::now();
        let older = SoftwareInstanceObservation {
            instance_id,
            artifact_id: "sha256:old".to_string(),
            observed_at: now - Duration::hours(2),
            version_running: Some("1.23".to_string()),
            version_installed: Some("1.24".to_string()),
        };
        let newer = SoftwareInstanceObservation {
            instance_id,
            artifact_id: "sha256:new".to_string(),
            observed_at: now,
            version_running: Some("1.24".to_string()),
            version_installed: Some("1.24".to_string()),
        };
        SoftwareStore::record_observation(&store, &older).unwrap();
        SoftwareStore::record_observation(&store, &newer).unwrap();
        // 重投同一帧不能多出一行。
        SoftwareStore::record_observation(&store, &newer).unwrap();

        let rows =
            SoftwareStore::list_observations_for_instance(&store, instance_id, Page::default())
                .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].artifact_id, "sha256:new");
        // 升级没重启那一轮：跑 1.23、装 1.24。
        assert_eq!(rows[1].version_running.as_deref(), Some("1.23"));
        assert_eq!(rows[1].version_installed.as_deref(), Some("1.24"));
    }

    #[test]
    fn fingerprint_keeps_the_first_seen_time() {
        let store = InMemoryTopologyStore::default();
        let tenant_id = TenantId(Uuid::new_v4());
        let host_id = Uuid::new_v4();
        let now = Utc::now();
        let mut row = SoftwareFingerprint {
            tenant_id,
            host_id,
            artifact_id: "sha256:aaaa".to_string(),
            size_bytes: 1024,
            build_id: None,
            first_seen_at: now - Duration::days(3),
            last_seen_at: now - Duration::days(3),
        };
        SoftwareStore::upsert_fingerprint(&store, &row).unwrap();

        row.last_seen_at = now;
        SoftwareStore::upsert_fingerprint(&store, &row).unwrap();

        let rows =
            SoftwareStore::list_fingerprints_for_host(&store, host_id, Page::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].first_seen_at,
            now - Duration::days(3),
            "第一次见到的时间是事实，不能被后来的轮次覆盖"
        );
        assert_eq!(rows[0].last_seen_at, now);
    }

    #[test]
    fn artifacts_are_keyed_by_content_not_by_name() {
        let store = InMemoryTopologyStore::default();
        let artifact = SoftwareArtifact {
            artifact_id: SoftwareArtifact::content_id("aaaa"),
            sha256: "aaaa".to_string(),
            artifact_kind: atlas_domain::ArtifactKind::Executable,
            version_id: None,
            size_bytes: Some(1572864),
            build_id: None,
            package_manager: Some("dpkg".to_string()),
            package_name: Some("nginx".to_string()),
            purl: None,
            interpreter_artifact_id: None,
            coordinate: Some(atlas_domain::ArtifactCoordinate {
                distro_id: Some("alpine".to_string()),
                distro_version: Some("3.20".to_string()),
                ..Default::default()
            }),
            first_seen_at: None,
            last_seen_at: None,
        };
        SoftwareStore::upsert_artifact(&store, &artifact).unwrap();
        let loaded = SoftwareStore::get_artifact(&store, "sha256:aaaa")
            .unwrap()
            .expect("按内容寻址取回");
        assert_eq!(loaded.artifact_id, "sha256:aaaa");
        assert_eq!(
            loaded
                .coordinate
                .and_then(|value| value.distro_id)
                .as_deref(),
            Some("alpine")
        );
    }
}
