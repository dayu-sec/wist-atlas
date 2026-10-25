use chrono::Utc;

use super::{sql, *};

#[test]
fn core_upserts_are_idempotent() {
    for statement in [
        sql::UPSERT_HOST,
        sql::UPSERT_NETWORK_DOMAIN,
        sql::UPSERT_NETWORK_SEGMENT,
        sql::UPSERT_HOST_NET_ASSOC,
        sql::UPSERT_SUBJECT,
        sql::UPSERT_RESPONSIBILITY_ASSIGNMENT,
        sql::UPSERT_INGEST_JOB,
    ] {
        assert!(
            statement.contains("ON CONFLICT"),
            "upsert statement must be idempotent: {statement}"
        );
    }
}

#[test]
fn core_queries_include_pagination_where_expected() {
    for statement in [
        sql::LIST_HOSTS,
        sql::LIST_NETWORK_SEGMENTS,
        sql::LIST_HOST_NET_ASSOCS,
        sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET,
    ] {
        assert!(statement.contains("LIMIT"));
    }
}

#[test]
fn postgres_store_runs_migrations_and_records_exec_calls() {
    let executor = RecordingExecutor::default();
    let store = PostgresTopologyStore::new(executor.clone());

    store.run_migrations().unwrap();

    let calls = executor.calls();
    assert!(!calls.is_empty());
    assert!(
        calls[0]
            .0
            .contains("CREATE TABLE IF NOT EXISTS schema_migrations")
    );
}

#[test]
fn postgres_store_uses_ingest_job_upsert_sql() {
    let executor = RecordingExecutor::default();
    let store = PostgresTopologyStore::new(executor.clone());

    store
        .record_ingest_job(IngestJobEntry {
            ingest_id: "ing-1".to_string(),
            tenant_id: TenantId(Uuid::new_v4()),
            source_name: "demo".to_string(),
            source_kind: "batch_import".to_string(),
            received_at: Utc::now(),
            status: "accepted".to_string(),
            payload_ref: None,
            error: None,
        })
        .unwrap();

    let calls = executor.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.contains("INSERT INTO ingest_job"));
}

#[test]
fn memory_postgres_executor_round_trips_minimal_p0_records() {
    let executor = MemoryPostgresExecutor::default();
    let store = PostgresTopologyStore::new(executor);
    let tenant_id = TenantId(Uuid::new_v4());
    let now = Utc::now();
    let host = HostInventory {
        host_id: Uuid::new_v4(),
        tenant_id,
        environment_id: None,
        host_name: "node-01".to_string(),
        machine_id: Some("machine-01".to_string()),
        os_name: Some("linux".to_string()),
        os_version: Some("6.8".to_string()),
        created_at: now,
        last_inventory_at: now,
    };
    let service = ServiceEntity {
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
    };
    let domain = NetworkDomain {
        network_domain_id: Uuid::new_v4(),
        tenant_id,
        environment_id: None,
        name: "default".to_string(),
        kind: atlas_domain::NetworkDomainKind::Unknown,
        description: None,
        created_at: now,
        updated_at: now,
    };
    let segment = NetworkSegment {
        network_segment_id: Uuid::new_v4(),
        tenant_id,
        network_domain_id: Some(domain.network_domain_id),
        environment_id: None,
        name: "10.0.0.0/24".to_string(),
        cidr: Some("10.0.0.0/24".to_string()),
        gateway_ip: Some("10.0.0.1".to_string()),
        address_family: atlas_domain::AddressFamily::Ipv4,
        created_at: now,
        updated_at: now,
    };
    let subject = Subject {
        subject_id: Uuid::new_v4(),
        tenant_id,
        subject_type: atlas_domain::SubjectType::User,
        display_name: "alice".to_string(),
        external_ref: None,
        email: Some("alice@example.com".to_string()),
        is_active: true,
        created_at: now,
        updated_at: now,
    };
    let assignment = ResponsibilityAssignment {
        assignment_id: Uuid::new_v4(),
        tenant_id,
        subject_id: subject.subject_id,
        target_kind: ObjectKind::Host,
        target_id: host.host_id,
        role: atlas_domain::ResponsibilityRole::Owner,
        source: "batch_import".to_string(),
        validity: atlas_domain::ValidityWindow {
            valid_from: now,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };
    let host_runtime = HostRuntimeState {
        host_id: host.host_id,
        observed_at: atlas_domain::ObservedAt(now),
        boot_id: Some("boot-1".to_string()),
        uptime_seconds: Some(42),
        loadavg_1m: Some(0.25),
        loadavg_5m: None,
        loadavg_15m: None,
        cpu_usage_pct: None,
        memory_used_bytes: Some(1024),
        memory_available_bytes: Some(2048),
        process_count: Some(3),
        container_count: Some(0),
        agent_health: atlas_domain::AgentHealth::Healthy,
    };
    let process = ProcessRuntimeState {
        process_id: Uuid::new_v4(),
        tenant_id,
        host_id: host.host_id,
        container_id: None,
        external_ref: Some("hostname:node-01:pid:123:start:abc".to_string()),
        pid: 123,
        executable: "/usr/sbin/sshd".to_string(),
        command_line: Some("/usr/sbin/sshd -D".to_string()),
        process_state: Some("S".to_string()),
        memory_rss_kib: Some(7456),
        started_at: now,
        observed_at: atlas_domain::ObservedAt(now),
    };
    let instance = ServiceInstance {
        instance_id: Uuid::new_v4(),
        tenant_id,
        service_id: Uuid::new_v4(),
        workload_id: None,
        started_at: now,
        ended_at: None,
        last_seen_at: now,
    };
    let binding = RuntimeBinding {
        binding_id: Uuid::new_v4(),
        instance_id: instance.instance_id,
        object_type: RuntimeObjectType::Process,
        object_id: process.process_id,
        scope: atlas_domain::BindingScope::Observed,
        confidence: atlas_domain::Confidence::Medium,
        source: "edge".to_string(),
        validity: atlas_domain::ValidityWindow {
            valid_from: now,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };

    CatalogStore::upsert_host(&store, &host).unwrap();
    CatalogStore::upsert_service(&store, &service).unwrap();
    CatalogStore::upsert_network_domain(&store, &domain).unwrap();
    CatalogStore::upsert_network_segment(&store, &segment).unwrap();
    RuntimeStore::insert_host_runtime_state(&store, &host_runtime).unwrap();
    RuntimeStore::upsert_process_runtime_state(&store, &process).unwrap();
    RuntimeStore::upsert_service_instance(&store, &instance).unwrap();
    RuntimeStore::upsert_runtime_binding(&store, &binding).unwrap();
    CatalogStore::upsert_subject(&store, &subject).unwrap();
    GovernanceStore::upsert_responsibility_assignment(&store, &assignment).unwrap();
    IngestStore::record_ingest_job(
        &store,
        IngestJobEntry {
            ingest_id: "ing-1".to_string(),
            tenant_id,
            source_name: "demo".to_string(),
            source_kind: "batch_import".to_string(),
            received_at: now,
            status: "accepted".to_string(),
            payload_ref: None,
            error: None,
        },
    )
    .unwrap();

    assert!(
        CatalogStore::get_host(&store, host.host_id)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        CatalogStore::list_hosts(&store, tenant_id, Page::default())
            .unwrap()
            .len(),
        1
    );
    assert!(
        CatalogStore::get_service(&store, service.service_id)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        CatalogStore::list_services(&store, tenant_id, Page::default())
            .unwrap()
            .len(),
        1
    );
    assert!(
        CatalogStore::get_network_domain(&store, domain.network_domain_id)
            .unwrap()
            .is_some()
    );
    assert!(
        CatalogStore::get_network_segment(&store, segment.network_segment_id)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        RuntimeStore::list_host_runtime_states(&store, host.host_id, Page::default())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        RuntimeStore::list_process_runtime_states(&store, host.host_id, Page::default())
            .unwrap()
            .len(),
        1
    );
    assert!(
        RuntimeStore::get_service_instance(&store, instance.instance_id)
            .unwrap()
            .is_some()
    );
    assert!(
        RuntimeStore::get_runtime_binding(&store, binding.binding_id)
            .unwrap()
            .is_some()
    );
    assert!(
        CatalogStore::get_subject(&store, subject.subject_id)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        GovernanceStore::list_responsibility_assignments_for_target(
            &store,
            ObjectKind::Host,
            host.host_id,
            Page::default(),
        )
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
