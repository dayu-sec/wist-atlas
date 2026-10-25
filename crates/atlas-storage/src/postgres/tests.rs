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
fn process_upsert_carries_the_presence_columns() {
    // 写不进去：清扫标记永远看不见。读回不包含：复活永远清不掉标记。
    assert!(sql::UPSERT_PROCESS_RUNTIME_STATE.contains("stale_at"));
    assert!(sql::UPSERT_PROCESS_RUNTIME_STATE.contains("ingest_scope"));
    // 两个新参数只能**追加**在末尾 —— 已有 12 个位次被所有调用方按序号依赖。
    assert!(sql::UPSERT_PROCESS_RUNTIME_STATE.contains("$13"));
    assert!(sql::UPSERT_PROCESS_RUNTIME_STATE.contains("$14"));

    assert!(sql::LIST_PROCESS_RUNTIME_STATES.contains("stale_at"));
    assert!(sql::LIST_PROCESS_RUNTIME_STATES.contains("ingest_scope"));
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
fn process_presence_columns_round_trip_and_revive() {
    let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
    let tenant_id = TenantId(Uuid::new_v4());
    let now = Utc::now();
    let host_id = Uuid::new_v4();
    let host = HostInventory {
        host_id,
        tenant_id,
        environment_id: None,
        host_name: "node-presence".to_string(),
        machine_id: Some("machine-presence".to_string()),
        os_name: None,
        os_version: None,
        arch: None,
        created_at: now,
        last_inventory_at: now,
        stale_at: None,
        miss_count: 0,
        ingest_scope: None,
    };
    CatalogStore::upsert_host(&store, &host).unwrap();

    let mut process = ProcessRuntimeState {
        process_id: Uuid::new_v4(),
        tenant_id,
        host_id,
        container_id: None,
        external_ref: Some("machine-presence:pid:1:a".to_string()),
        pid: 1,
        executable: "/sbin/init".to_string(),
        command_line: None,
        process_state: None,
        memory_rss_kib: None,
        started_at: now,
        observed_at: atlas_domain::ObservedAt(now),
        stale_at: Some(now),
        ingest_scope: Some("atlas.in.edge.v1:wist:agent-1:tenant:".to_string()),
    };
    RuntimeStore::upsert_process_runtime_state(&store, &process).unwrap();

    let stored = RuntimeStore::list_process_runtime_states(&store, host_id, Page::default())
        .unwrap()
        .pop()
        .expect("process row");
    assert_eq!(stored.stale_at, Some(now));
    assert_eq!(
        stored.ingest_scope.as_deref(),
        Some("atlas.in.edge.v1:wist:agent-1:tenant:")
    );

    // 重新出现：同一 process_id + stale_at = None ⇒ 标记被清掉。
    process.stale_at = None;
    RuntimeStore::upsert_process_runtime_state(&store, &process).unwrap();
    let revived = RuntimeStore::list_process_runtime_states(&store, host_id, Page::default())
        .unwrap()
        .pop()
        .expect("process row");
    assert_eq!(revived.stale_at, None, "复活必须清掉 stale 标记");
}

#[test]
fn instance_endpoint_upsert_list_and_close() {
    let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
    let tenant_id = TenantId(Uuid::new_v4());
    let instance_id = Uuid::new_v4();
    let valid_from = Utc::now();
    let endpoint = InstanceEndpoint {
        endpoint_id: Uuid::new_v4(),
        tenant_id,
        instance_id,
        address: "10.20.1.11".to_string(),
        port: 8080,
        protocol: "http".to_string(),
        source: "runtime_probe".to_string(),
        valid_from,
        valid_to: None,
        created_at: valid_from,
        updated_at: valid_from,
    };

    RuntimeStore::upsert_instance_endpoint(&store, &endpoint).unwrap();
    // 同一个 endpoint_id 再写一次是覆盖，不是新增。
    RuntimeStore::upsert_instance_endpoint(&store, &endpoint).unwrap();
    assert_eq!(
        RuntimeStore::list_instance_endpoints(&store, instance_id, Page::default()).unwrap(),
        vec![endpoint.clone()]
    );

    let closed_at = valid_from + chrono::Duration::seconds(60);
    RuntimeStore::close_instance_endpoint(&store, endpoint.endpoint_id, closed_at).unwrap();

    // 关闭后不再出现在「当前地址」里；历史行仍在库里（不删行）。
    assert!(
        RuntimeStore::list_instance_endpoints(&store, instance_id, Page::default())
            .unwrap()
            .is_empty(),
        "closed endpoint must not be listed as live"
    );
    RuntimeStore::close_instance_endpoint(&store, endpoint.endpoint_id, closed_at).unwrap();
}

#[test]
fn instance_endpoint_upsert_keeps_the_window_start_until_it_is_closed() {
    let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
    let tenant_id = TenantId(Uuid::new_v4());
    let instance_id = Uuid::new_v4();
    let started = Utc::now();
    let endpoint = InstanceEndpoint {
        endpoint_id: Uuid::new_v4(),
        tenant_id,
        instance_id,
        address: "10.20.1.12".to_string(),
        port: 9090,
        protocol: "tcp".to_string(),
        source: "runtime_probe".to_string(),
        valid_from: started,
        valid_to: None,
        created_at: started,
        updated_at: started,
    };
    RuntimeStore::upsert_instance_endpoint(&store, &endpoint).unwrap();

    // 每份快照都会重写同一行。窗口还开着时，起点必须留在第一次看到的那一刻，
    // 否则 `valid_from` 就退化成「上次上报时间」，`last_seen_at` 才是那个语义。
    let later = endpoint.clone();
    let later = InstanceEndpoint {
        valid_from: started + chrono::Duration::hours(3),
        updated_at: started + chrono::Duration::hours(3),
        ..later
    };
    RuntimeStore::upsert_instance_endpoint(&store, &later).unwrap();
    let live = RuntimeStore::list_instance_endpoints(&store, instance_id, Page::default()).unwrap();
    assert_eq!(live[0].valid_from, started);
    assert_eq!(live[0].updated_at, started + chrono::Duration::hours(3));

    // 关掉再打开属于**新**一段窗口，起点要重新计。
    let closed_at = started + chrono::Duration::hours(4);
    RuntimeStore::close_instance_endpoint(&store, endpoint.endpoint_id, closed_at).unwrap();
    let reopened_at = started + chrono::Duration::hours(5);
    let reopened = InstanceEndpoint {
        valid_from: reopened_at,
        valid_to: None,
        updated_at: reopened_at,
        ..endpoint.clone()
    };
    RuntimeStore::upsert_instance_endpoint(&store, &reopened).unwrap();
    let live = RuntimeStore::list_instance_endpoints(&store, instance_id, Page::default()).unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].valid_from, reopened_at);
}

#[test]
fn list_instance_ids_for_host_skips_stale_processes_and_dedups() {
    let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
    let tenant_id = TenantId(Uuid::new_v4());
    let host_id = Uuid::new_v4();
    let now = Utc::now();

    let process = |stale_at: Option<chrono::DateTime<Utc>>| ProcessRuntimeState {
        process_id: Uuid::new_v4(),
        tenant_id,
        host_id,
        container_id: None,
        external_ref: None,
        pid: 1,
        executable: "/usr/sbin/sshd".to_string(),
        command_line: None,
        process_state: None,
        memory_rss_kib: None,
        started_at: now,
        observed_at: atlas_domain::ObservedAt(now),
        stale_at,
        ingest_scope: None,
    };
    let live = process(None);
    let stale = process(Some(now));

    let binding =
        |instance_id: Uuid, object_type: RuntimeObjectType, object_id: Uuid| RuntimeBinding {
            binding_id: Uuid::new_v4(),
            instance_id,
            object_type,
            object_id,
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

    let live_instance_id = Uuid::new_v4();
    let stale_instance_id = Uuid::new_v4();
    let container_instance_id = Uuid::new_v4();

    RuntimeStore::upsert_process_runtime_state(&store, &live).unwrap();
    RuntimeStore::upsert_process_runtime_state(&store, &stale).unwrap();
    // 同一实例两个 binding：结果必须去重。
    RuntimeStore::upsert_runtime_binding(
        &store,
        &binding(
            live_instance_id,
            RuntimeObjectType::Process,
            live.process_id,
        ),
    )
    .unwrap();
    RuntimeStore::upsert_runtime_binding(
        &store,
        &binding(
            live_instance_id,
            RuntimeObjectType::Process,
            live.process_id,
        ),
    )
    .unwrap();
    // 进程已 stale：它的实例不算活着。
    RuntimeStore::upsert_runtime_binding(
        &store,
        &binding(
            stale_instance_id,
            RuntimeObjectType::Process,
            stale.process_id,
        ),
    )
    .unwrap();
    // 只认 Process 绑定，容器绑定不参与。
    RuntimeStore::upsert_runtime_binding(
        &store,
        &binding(
            container_instance_id,
            RuntimeObjectType::Container,
            live.process_id,
        ),
    )
    .unwrap();

    assert_eq!(
        RuntimeStore::list_instance_ids_for_host(&store, host_id).unwrap(),
        vec![live_instance_id]
    );
    assert!(
        RuntimeStore::list_instance_ids_for_host(&store, Uuid::new_v4())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn list_host_process_counts_only_counts_live_processes() {
    let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
    let tenant_id = TenantId(Uuid::new_v4());
    let now = Utc::now();
    let busy_host = Uuid::new_v4();
    let quiet_host = Uuid::new_v4();
    let retired_host = Uuid::new_v4();
    let empty_host = Uuid::new_v4();

    let process = |host_id: Uuid, stale_at: Option<chrono::DateTime<Utc>>| ProcessRuntimeState {
        process_id: Uuid::new_v4(),
        tenant_id,
        host_id,
        container_id: None,
        external_ref: None,
        pid: 1,
        executable: "/usr/sbin/sshd".to_string(),
        command_line: None,
        process_state: None,
        memory_rss_kib: None,
        started_at: now,
        observed_at: atlas_domain::ObservedAt(now),
        stale_at,
        ingest_scope: None,
    };

    for stale_at in [None, None, Some(now)] {
        RuntimeStore::upsert_process_runtime_state(&store, &process(busy_host, stale_at)).unwrap();
    }
    RuntimeStore::upsert_process_runtime_state(&store, &process(quiet_host, None)).unwrap();
    // 进程已全部退役的主机不该出现。
    RuntimeStore::upsert_process_runtime_state(&store, &process(retired_host, Some(now))).unwrap();

    let mut expected = vec![(busy_host, 2), (quiet_host, 1)];
    expected.sort();
    let counts = RuntimeStore::list_host_process_counts(&store, Page::default()).unwrap();
    assert_eq!(counts, expected);
    // 从未见过进程的主机也不该出现。
    assert!(!counts.iter().any(|(host_id, _)| *host_id == empty_host));

    // 分页跟在 ORDER BY host_id 之后，与 SQL 的 LIMIT/OFFSET 对齐。
    assert_eq!(
        RuntimeStore::list_host_process_counts(
            &store,
            Page {
                limit: 1,
                offset: 1
            }
        )
        .unwrap(),
        vec![expected[1]]
    );
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
        arch: Some("x86_64".to_string()),
        created_at: now,
        last_inventory_at: now,
        stale_at: None,
        miss_count: 0,
        ingest_scope: None,
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
        stale_at: None,
        ingest_scope: Some("atlas.in.edge.v1:wist:agent-1:tenant:env".to_string()),
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
