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
        cloud_instance_id: None,
        cloud_account: None,
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
        cloud_instance_id: None,
        cloud_account: None,
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

mod software {
    use atlas_domain::{
        AnchorKind, ClaimedSource, FingerprintStatus, SoftwareArtifact, SoftwareFingerprint,
        SoftwareInstance, SoftwareInstanceObservation, SoftwareReason, SoftwareRole,
    };
    use chrono::{Duration, Utc};

    use crate::{Page, SoftwareStore};

    use super::*;

    fn sample_instance(
        host_id: Uuid,
        tenant_id: TenantId,
        now: chrono::DateTime<Utc>,
    ) -> SoftwareInstance {
        SoftwareInstance {
            instance_id: Uuid::new_v4(),
            tenant_id,
            host_id,
            anchor_kind: AnchorKind::SystemdUnit,
            anchor_value: "nginx.service".to_string(),
            current_artifact_id: Some("sha256:aaaa".to_string()),
            identity_missing_reason: None,
            claimed_package_name: Some("nginx".to_string()),
            claimed_product_id: None,
            claimed_version: Some("1.27.5-r1".to_string()),
            claimed_source: Some(ClaimedSource::PathPackage),
            role: vec![SoftwareRole::Middleware, SoftwareRole::ExposedService],
            reason: vec![SoftwareReason::Listener],
            status: FingerprintStatus::Observed,
            first_seen_at: now,
            last_seen_at: now,
            stale_at: None,
        }
    }

    /// 一条真回归：**UPSERT 的参数顺序必须与 SELECT 的列顺序一致**。
    ///
    /// 内存替身按参数向量存行、按列下标解码，所以顺序一错这里立刻炸；
    /// 而真 PG 会默默把值写到错的列上。
    #[test]
    fn software_rows_round_trip_through_the_sql_shape() {
        let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
        let tenant_id = TenantId(Uuid::new_v4());
        let host_id = Uuid::new_v4();
        let now = Utc::now();

        let instance = sample_instance(host_id, tenant_id, now);
        SoftwareStore::upsert_instance(&store, &instance).unwrap();

        let loaded = SoftwareStore::get_instance(&store, instance.instance_id)
            .unwrap()
            .expect("按 id 取回");
        assert_eq!(loaded.anchor_kind, AnchorKind::SystemdUnit);
        assert_eq!(loaded.anchor_value, "nginx.service");
        assert_eq!(loaded.claimed_package_name.as_deref(), Some("nginx"));
        assert_eq!(loaded.claimed_version.as_deref(), Some("1.27.5-r1"));
        assert_eq!(loaded.claimed_source, Some(ClaimedSource::PathPackage));
        assert_eq!(loaded.status, FingerprintStatus::Observed);
        // 闭集列表往返：这两个列存的是 JSON 数组。
        assert_eq!(
            loaded.role,
            vec![SoftwareRole::Middleware, SoftwareRole::ExposedService]
        );
        assert_eq!(loaded.reason, vec![SoftwareReason::Listener]);
        assert!(loaded.stale_at.is_none());
    }

    #[test]
    fn retiring_through_sql_only_touches_this_host() {
        let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
        let tenant_id = TenantId(Uuid::new_v4());
        let now = Utc::now();
        let host_a = Uuid::new_v4();
        let host_b = Uuid::new_v4();

        SoftwareStore::upsert_instance(&store, &sample_instance(host_a, tenant_id, now)).unwrap();
        SoftwareStore::upsert_instance(&store, &sample_instance(host_b, tenant_id, now)).unwrap();

        // 本轮 host_a 什么都没看到。
        let retired = SoftwareStore::mark_instances_stale(&store, host_a, &[], now).unwrap();
        assert_eq!(retired, 1);

        assert_eq!(
            SoftwareStore::list_instances_for_host(&store, host_a, Page::default())
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            SoftwareStore::list_instances_for_host(&store, host_b, Page::default())
                .unwrap()
                .len(),
            1,
            "别的机器的实例不能被一起清掉"
        );
    }

    #[test]
    fn artifact_coordinate_round_trips_as_json() {
        let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
        let artifact = SoftwareArtifact {
            artifact_id: SoftwareArtifact::content_id("d3f1c9e2"),
            sha256: "d3f1c9e2".to_string(),
            artifact_kind: atlas_domain::ArtifactKind::Executable,
            version_id: Some("postgresql@16.15".to_string()),
            size_bytes: Some(7340032),
            build_id: None,
            package_manager: Some("dpkg".to_string()),
            package_name: Some("postgresql-16".to_string()),
            purl: None,
            interpreter_artifact_id: None,
            coordinate: Some(atlas_domain::ArtifactCoordinate {
                distro_id: Some("debian".to_string()),
                distro_version: Some("12".to_string()),
                component: Some("main".to_string()),
                arch: Some("amd64".to_string()),
                libc: Some("glibc".to_string()),
            }),
            first_seen_at: Some(Utc::now()),
            last_seen_at: None,
        };
        SoftwareStore::upsert_artifact(&store, &artifact).unwrap();
        let loaded = SoftwareStore::get_artifact(&store, "sha256:d3f1c9e2")
            .unwrap()
            .expect("按内容寻址取回");
        let coordinate = loaded.coordinate.expect("coordinate 应往返");
        assert_eq!(coordinate.distro_id.as_deref(), Some("debian"));
        assert_eq!(coordinate.arch.as_deref(), Some("amd64"));
        // 只有 first_seen 有值、last_seen 为空也要能解码。
        assert!(loaded.first_seen_at.is_some());
        assert!(loaded.last_seen_at.is_none());
    }

    #[test]
    fn observation_and_fingerprint_round_trip() {
        let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
        let tenant_id = TenantId(Uuid::new_v4());
        let host_id = Uuid::new_v4();
        let instance_id = Uuid::new_v4();
        let now = Utc::now();

        SoftwareStore::record_observation(
            &store,
            &SoftwareInstanceObservation {
                instance_id,
                artifact_id: "sha256:aaaa".to_string(),
                observed_at: now,
                // 升级没重启：跑 1.23、装 1.24。
                version_running: Some("1.23".to_string()),
                version_installed: Some("1.24".to_string()),
            },
        )
        .unwrap();
        let observations =
            SoftwareStore::list_observations_for_instance(&store, instance_id, Page::default())
                .unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].version_running.as_deref(), Some("1.23"));

        SoftwareStore::upsert_fingerprint(
            &store,
            &SoftwareFingerprint {
                tenant_id,
                host_id,
                artifact_id: "sha256:aaaa".to_string(),
                size_bytes: 1024,
                build_id: None,
                first_seen_at: now - Duration::days(1),
                last_seen_at: now,
            },
        )
        .unwrap();
        let fingerprints =
            SoftwareStore::list_fingerprints_for_host(&store, host_id, Page::default()).unwrap();
        assert_eq!(fingerprints.len(), 1);
        assert_eq!(fingerprints[0].size_bytes, 1024);
    }
}

#[test]
fn host_upsert_preserves_the_identity_of_a_cloud_created_row() {
    // 云侧先建行（没有 machine_id），端侧后到把 machine_id 补上。
    // 这类「同一个 host_id、后补身份字段」的写入一旦按 machine_id 仲裁，真 PG 会撞主键
    // —— 插入时的 PK 冲突不在 ON CONFLICT 的仲裁范围内。内存替身按 host_id 覆盖，
    // 所以这条用例守的是「语句的仲裁列必须是 host_id」。
    let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
    let tenant_id = TenantId(Uuid::new_v4());
    let now = Utc::now();
    let host_id = Uuid::new_v4();

    let mut host = HostInventory {
        host_id,
        tenant_id,
        environment_id: None,
        host_name: "node-cloud".to_string(),
        machine_id: None,
        cloud_instance_id: Some("ins-1".to_string()),
        cloud_account: Some("tencent:1300000000".to_string()),
        os_name: None,
        os_version: None,
        arch: None,
        created_at: now,
        last_inventory_at: now,
        stale_at: None,
        miss_count: 0,
        ingest_scope: Some("atlas.in.inventory.v1:wist-harvest:harvest-1::".to_string()),
    };
    CatalogStore::upsert_host(&store, &host).unwrap();

    // 端侧看到同一台机：同一个 host_id，多了 machine_id / os。
    host.machine_id = Some("machine-1".to_string());
    host.os_name = Some("Ubuntu".to_string());
    CatalogStore::upsert_host(&store, &host).unwrap();

    let stored = CatalogStore::get_host(&store, host_id)
        .unwrap()
        .expect("host");
    assert_eq!(stored.machine_id.as_deref(), Some("machine-1"));
    assert_eq!(stored.os_name.as_deref(), Some("Ubuntu"));
    assert_eq!(
        stored.cloud_account.as_deref(),
        Some("tencent:1300000000"),
        "端侧不报账号，账号归属不能被抹掉"
    );
    assert_eq!(
        stored.cloud_instance_id.as_deref(),
        Some("ins-1"),
        "云实例 ID 同样不能被抹掉"
    );
    assert_eq!(
        CatalogStore::list_hosts(&store, tenant_id, Page::default())
            .unwrap()
            .len(),
        1,
        "同一个 host_id 不能变成两行"
    );
}

#[test]
fn host_upsert_is_arbitered_on_the_primary_key() {
    // 仲裁列必须是主键 `host_id`：云侧先建行（machine_id 为空）、端侧后补 machine_id 时，
    // 插入会撞 PK，而按 machine_id 仲裁覆盖不到这个冲突 —— 真 PG 会直接报错。
    assert!(sql::UPSERT_HOST.contains("ON CONFLICT (host_id) DO UPDATE"));
    assert!(
        !sql::UPSERT_HOST.contains("ON CONFLICT (machine_id)"),
        "按 machine_id 仲裁会在「同 host_id、后补身份字段」时撞主键"
    );

    // 账号归属：能写进去，也能读回来。写不进去 => 退役永远匹配不上账号。
    assert!(sql::UPSERT_HOST.contains("cloud_account"));
    assert!(sql::UPSERT_HOST.contains("$15"));
    assert!(sql::UPSERT_HOST.contains("COALESCE(EXCLUDED.cloud_account"));
    for statement in [sql::LIST_HOSTS, sql::LIST_ALL_HOSTS, sql::GET_HOST] {
        assert!(
            statement.contains("cloud_account"),
            "读路径少了 cloud_account，退役目标就永远是空的"
        );
    }
}

/// 真 PostgreSQL 上的主机身份用例。
///
/// 默认 `#[ignore]`：常规 CI 的 PG 用例走内存替身，而「按 machine_id 仲裁会撞主键」
/// 正是内存替身看不出来的那一类问题。要跑真库：
///
/// ```bash
/// WIST_ATLAS_TEST_DATABASE_URL='postgres://atlas:atlas@127.0.0.1:55432/wist_atlas' \
///   cargo test -p atlas-storage -- --ignored real_postgres
/// ```
///
/// 在**独立 schema** 里跑完整迁移与写入，结束时删掉；不动库里既有的表。
#[test]
#[ignore = "需要真 PostgreSQL；设 WIST_ATLAS_TEST_DATABASE_URL 后加 --ignored"]
fn real_postgres_host_upsert_keeps_one_row_per_identity() {
    let Ok(url) =
        std::env::var("WIST_ATLAS_TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL"))
    else {
        eprintln!("skip: WIST_ATLAS_TEST_DATABASE_URL / DATABASE_URL 未设置");
        return;
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    runtime.block_on(async move {
        let executor = LivePostgresExecutor::new(url)
            .await
            .expect("connect postgres");
        let schema = format!(
            "atlas_probe_{}",
            Uuid::new_v4().to_string().replace('-', "")
        );
        executor
            .exec_batch_async(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create schema");
        executor
            .exec_batch_async(&format!("SET search_path TO {schema}"))
            .await
            .expect("set search_path");

        let store = PostgresTopologyStore::new(executor.clone());
        store.run_migrations_async().await.expect("migrations");

        let tenant_id = TenantId(Uuid::new_v4());
        let now = Utc::now();
        let host_id = Uuid::new_v4();
        let mut host = HostInventory {
            host_id,
            tenant_id,
            environment_id: None,
            host_name: "node-cloud".to_string(),
            machine_id: None,
            cloud_instance_id: Some("ins-1".to_string()),
            cloud_account: Some("tencent:1300000000".to_string()),
            os_name: None,
            os_version: None,
            arch: None,
            created_at: now,
            last_inventory_at: now,
            stale_at: None,
            miss_count: 0,
            ingest_scope: Some("atlas.in.inventory.v1:wist-harvest:harvest-1::".to_string()),
        };
        // 云侧先建行（machine_id 为空）。
        AsyncCatalogStore::upsert_host(&store, &host)
            .await
            .expect("cloud-first upsert");

        // 端侧后到：同一个 host_id，补上 machine_id / os / arch。
        host.machine_id = Some("machine-1".to_string());
        host.os_name = Some("Ubuntu 24.04".to_string());
        host.arch = Some("x86_64".to_string());
        host.last_inventory_at = now;
        AsyncCatalogStore::upsert_host(&store, &host)
            .await
            .expect("agent upsert must not violate the primary key");

        let stored = AsyncCatalogStore::get_host(&store, host_id)
            .await
            .expect("get host")
            .expect("host row");
        assert_eq!(stored.machine_id.as_deref(), Some("machine-1"));
        assert_eq!(stored.os_name.as_deref(), Some("Ubuntu 24.04"));
        assert_eq!(stored.arch.as_deref(), Some("x86_64"));
        assert_eq!(
            stored.cloud_account.as_deref(),
            Some("tencent:1300000000"),
            "COALESCE：端侧不报账号，账号归属不能被抹掉"
        );
        assert_eq!(stored.cloud_instance_id.as_deref(), Some("ins-1"));

        let rows = AsyncCatalogStore::list_hosts(&store, tenant_id, Page::default())
            .await
            .expect("list hosts");
        assert_eq!(rows.len(), 1, "同一次身份不能变成两行");

        executor
            .exec_batch_async(&format!("DROP SCHEMA {schema} CASCADE"))
            .await
            .ok();
    });
}
