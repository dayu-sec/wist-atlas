//! `atlas-wist-adapter` 的映射规则测试。
//!
//! 与 `lib.rs` 分开的原因只有一个：本仓约定单文件 500 行以下。
//! 断言本身全部是同步纯函数，没有网络也没有时钟依赖。

use super::*;

const OBSERVED_AT: &str = "2026-10-25T07:56:34.061342Z";

fn attrs(pairs: &[(&str, &str)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
        .collect()
}

fn resource(resource_id: &str, kind: &str, attributes: &[(&str, &str)]) -> SnapshotResource {
    SnapshotResource {
        resource_id: resource_id.to_string(),
        kind: kind.to_string(),
        attributes: attrs(attributes),
        discovered_at: OBSERVED_AT.to_string(),
    }
}

fn target(resource_ref: &str, kind: &str, hints: &[(&str, &str)]) -> SnapshotTarget {
    SnapshotTarget {
        kind: kind.to_string(),
        resource_ref: resource_ref.to_string(),
        execution_hints: attrs(hints),
    }
}

fn host_resource() -> SnapshotResource {
    resource(
        "machine-abc",
        HOST_KIND,
        &[("host.id", "machine-abc"), ("host.name", "node-1")],
    )
}

fn envelope(resources: &[SnapshotResource], targets: &[SnapshotTarget]) -> EdgeEnvelope {
    build_envelope(
        resources,
        targets,
        SnapshotMeta {
            producer: "agent-1",
            tenant: "tenant-demo",
            env: Some("dev"),
            snapshot_id: "snap-1",
            revision: 7,
            generated_at: OBSERVED_AT,
            collected_at: OBSERVED_AT,
        },
    )
    .expect("build envelope")
}

// ── 主机身份 ──────────────────────────────────────────

#[test]
fn real_machine_id_is_used_as_both_ref_and_machine_id() {
    let built = envelope(&[host_resource()], &[]);
    let host = &built.payload.hosts[0];
    assert_eq!(host.external_ref, "machine-abc");
    assert_eq!(host.machine_id.as_deref(), Some("machine-abc"));
    assert_eq!(host.hostname, "node-1");
}

#[test]
fn hostname_fallback_is_not_promoted_to_machine_id() {
    // agentd 读不到 /etc/machine-id 时退化成 `hostname:<name>`。当 machine_id 用会让
    // 「同名不同机」在中心侧合并成一台。
    let built = envelope(
        &[resource(
            "hostname:mac-01",
            HOST_KIND,
            &[("host.name", "mac-01")],
        )],
        &[],
    );
    let host = &built.payload.hosts[0];
    assert_eq!(host.external_ref, "hostname:mac-01");
    assert_eq!(host.machine_id, None);
    assert_eq!(host.hostname, "mac-01");
}

#[test]
fn host_name_falls_back_to_the_resource_id() {
    let built = envelope(&[resource("hostname:mac-01", HOST_KIND, &[])], &[]);
    assert_eq!(built.payload.hosts[0].hostname, "mac-01");
}

#[test]
fn a_snapshot_without_a_host_resource_is_rejected() {
    let err = build_envelope(
        &[resource("if-1", IFACE_KIND, &[("net.if.name", "en0")])],
        &[],
        SnapshotMeta {
            producer: "agent-1",
            tenant: "t",
            env: None,
            snapshot_id: "s",
            revision: 1,
            generated_at: OBSERVED_AT,
            collected_at: OBSERVED_AT,
        },
    )
    .unwrap_err();
    assert!(matches!(err, AdapterError::MissingHost), "{err}");
}

#[test]
fn os_attributes_are_mapped_onto_the_host() {
    // 没有这两个字段，中心侧的资产卡片就只有「一台叫 node-1 的机器」。
    let built = envelope(
        &[resource(
            "machine-abc",
            HOST_KIND,
            &[
                ("host.id", "machine-abc"),
                ("host.name", "node-1"),
                ("os.name", "macOS"),
                ("os.version", "26.6.2"),
                ("os.type", "macos"),
            ],
        )],
        &[],
    );
    let host = &built.payload.hosts[0];
    assert_eq!(host.os_name.as_deref(), Some("macOS"));
    assert_eq!(host.os_version.as_deref(), Some("26.6.2"));
}

#[test]
fn a_host_before_os_collection_still_converts() {
    // `os.*` 是后加的：旧 agentd 的快照没有这几个键，不能因此报错，也不能输出空串。
    let built = envelope(&[host_resource()], &[]);
    let host = &built.payload.hosts[0];
    assert_eq!(host.os_name, None);
    assert_eq!(host.os_version, None);

    let rendered = serde_json::to_string(host).expect("serialize");
    assert!(!rendered.contains("os_name"), "{rendered}");
}

// ── 网卡与地址 ────────────────────────────────────────

#[test]
fn link_local_and_loopback_addresses_are_dropped() {
    let resources = vec![
        host_resource(),
        resource(
            "if-0",
            IFACE_KIND,
            &[("net.if.name", "lo0"), ("net.if.mac", "00:00:00:00:00:00")],
        ),
        resource(
            "a0",
            ADDR_KIND,
            &[("net.if.name", "lo0"), ("net.if.addr", "127.0.0.1")],
        ),
        resource(
            "a1",
            ADDR_KIND,
            &[("net.if.name", "lo0"), ("net.if.addr", "::1")],
        ),
        resource(
            "a2",
            ADDR_KIND,
            &[
                ("net.if.name", "lo0"),
                ("net.if.addr", "fe80::1"),
                ("net.if.addr.scope", "link_local"),
            ],
        ),
        resource(
            "a3",
            ADDR_KIND,
            &[("net.if.name", "lo0"), ("net.if.addr", "169.254.1.1")],
        ),
    ];
    // 只剩回环/链路本地 → 该网卡整体不进 payload（没有地址的网卡对拓扑无意义）。
    assert!(envelope(&resources, &[]).payload.interfaces.is_empty());
}

#[test]
fn uppercase_link_local_is_also_dropped() {
    // IPv6 十六进制在不同探针上大小写不一致；`FE80:` 与 `fe80:` 是同一个东西。
    let resources = vec![
        host_resource(),
        resource(
            "a0",
            ADDR_KIND,
            &[("net.if.name", "awdl0"), ("net.if.addr", "FE80::1")],
        ),
    ];
    assert!(envelope(&resources, &[]).payload.interfaces.is_empty());
}

#[test]
fn a_scope_marked_link_local_is_dropped_even_with_a_public_looking_address() {
    // 不给地址字符串猜：探针标了 scope 就按 scope 走。
    let resources = vec![
        host_resource(),
        resource(
            "a0",
            ADDR_KIND,
            &[
                ("net.if.name", "en0"),
                ("net.if.addr", "192.168.3.181"),
                ("net.if.addr.scope", "link_local"),
            ],
        ),
    ];
    assert!(envelope(&resources, &[]).payload.interfaces.is_empty());
}

#[test]
fn routable_address_keeps_the_interface_with_full_address_detail() {
    let resources = vec![
        host_resource(),
        resource(
            "if-1",
            IFACE_KIND,
            &[("net.if.name", "en0"), ("net.if.mac", "aa:bb:cc:dd:ee:ff")],
        ),
        resource(
            "a1",
            ADDR_KIND,
            &[
                ("net.if.name", "en0"),
                ("net.if.addr", "192.168.3.181"),
                ("net.if.prefix", "24"),
                ("net.if.cidr", "192.168.3.181/24"),
                ("net.if.gateway", "192.168.3.1"),
            ],
        ),
    ];
    let interfaces = envelope(&resources, &[]).payload.interfaces;
    assert_eq!(interfaces.len(), 1);

    let interface = &interfaces[0];
    assert_eq!(interface.name, "en0");
    assert_eq!(interface.mac.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
    assert_eq!(interface.machine_id.as_deref(), Some("machine-abc"));
    assert_eq!(
        interface.addresses,
        vec![Address {
            family: "ipv4".to_string(),
            ip: "192.168.3.181".to_string(),
            prefix: Some(24),
            cidr: Some("192.168.3.181/24".to_string()),
            gateway: Some("192.168.3.1".to_string()),
        }]
    );
}

#[test]
fn ipv6_address_is_typed_as_ipv6() {
    let resources = vec![
        host_resource(),
        resource(
            "a1",
            ADDR_KIND,
            &[("net.if.name", "en0"), ("net.if.addr", "2001:db8::1")],
        ),
    ];
    let interfaces = envelope(&resources, &[]).payload.interfaces;
    assert_eq!(interfaces[0].addresses[0].family, "ipv6");
}

#[test]
fn address_without_a_network_interface_resource_still_creates_one() {
    // 探针可能只产出 ip_address（没有单独的 network_interface 资源）。
    let resources = vec![
        host_resource(),
        resource(
            "a1",
            ADDR_KIND,
            &[("net.if.name", "en1"), ("net.if.addr", "10.0.0.5")],
        ),
    ];
    let interfaces = envelope(&resources, &[]).payload.interfaces;
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].name, "en1");
    assert_eq!(interfaces[0].mac, None);
}

#[test]
fn a_non_numeric_prefix_is_an_error_rather_than_a_silent_drop() {
    let resources = vec![
        host_resource(),
        resource(
            "a1",
            ADDR_KIND,
            &[
                ("net.if.name", "en0"),
                ("net.if.addr", "10.0.0.5"),
                ("net.if.prefix", "wide"),
            ],
        ),
    ];
    let err = build_envelope(
        &resources,
        &[],
        SnapshotMeta {
            producer: "agent-1",
            tenant: "t",
            env: None,
            snapshot_id: "s",
            revision: 1,
            generated_at: OBSERVED_AT,
            collected_at: OBSERVED_AT,
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            AdapterError::NotNumeric {
                field: "net.if.prefix",
                ..
            }
        ),
        "{err}"
    );
}

// ── 进程 ──────────────────────────────────────────────

fn process_resource(resource_id: &str, pid: &str, executable: &str) -> SnapshotResource {
    resource(
        resource_id,
        PROCESS_KIND,
        &[
            ("process.pid", pid),
            ("process.executable.name", executable),
        ],
    )
}

#[test]
fn identity_is_read_from_the_target_not_from_resource_attributes() {
    // 契约里 `process.identity` 在 `targets[].execution_hints`，resource attributes 里没有。
    // 只读 resource 会让所有进程退化成「同一台机上的同 pid」，去重时互相踩。
    let resources = vec![
        host_resource(),
        process_resource("p1", "210", "/usr/sbin/sshd"),
    ];
    let targets = vec![target(
        "p1",
        PROCESS_KIND,
        &[("process.identity", "ps_lstart:9f2c")],
    )];
    let processes = envelope(&resources, &targets).payload.processes;
    assert_eq!(processes[0].identity.as_deref(), Some("ps_lstart:9f2c"));
}

#[test]
fn process_without_identity_still_produces_a_row() {
    let resources = vec![
        host_resource(),
        process_resource("p1", "210", "/usr/sbin/sshd"),
    ];
    let processes = envelope(&resources, &[]).payload.processes;
    assert_eq!(processes.len(), 1);
    assert_eq!(processes[0].identity, None);
    assert_eq!(processes[0].observed_at, OBSERVED_AT);
}

#[test]
fn rows_without_pid_or_executable_are_skipped() {
    let resources = vec![
        host_resource(),
        resource("p1", PROCESS_KIND, &[("process.pid", "210")]),
        resource(
            "p2",
            PROCESS_KIND,
            &[("process.executable.name", "/bin/ls")],
        ),
    ];
    assert!(envelope(&resources, &[]).payload.processes.is_empty());
}

#[test]
fn processes_are_deduplicated_by_pid_and_identity_then_sorted() {
    let resources = vec![
        host_resource(),
        process_resource("p-b", "300", "/bin/b"),
        process_resource("p-a", "100", "/bin/a"),
        // 同 pid + 同 identity（同一个观测量）：只留一条。
        process_resource("p-a", "100", "/bin/a"),
    ];
    let processes = envelope(&resources, &[]).payload.processes;
    let pids: Vec<i64> = processes.iter().map(|process| process.pid).collect();
    assert_eq!(pids, vec![100, 300]);
}

#[test]
fn same_pid_with_different_identity_is_kept_twice() {
    // pid 复用：同一 pid、不同启动时刻，是两个真实进程。
    let resources = vec![
        host_resource(),
        process_resource("p1", "210", "/bin/a"),
        process_resource("p2", "210", "/bin/a"),
    ];
    let targets = vec![
        target("p1", PROCESS_KIND, &[("process.identity", "ps_lstart:aaa")]),
        target("p2", PROCESS_KIND, &[("process.identity", "ps_lstart:bbb")]),
    ];
    assert_eq!(envelope(&resources, &targets).payload.processes.len(), 2);
}

#[test]
fn a_numeric_pid_attribute_is_accepted() {
    // 契约里 attributes 是 String -> String，但生产端把 pid 写成 `1` 是常见手误。
    let mut process = resource("p1", PROCESS_KIND, &[("process.executable.name", "/bin/a")]);
    process
        .attributes
        .insert("process.pid".to_string(), Value::from(4242));
    let resources = vec![host_resource(), process];
    assert_eq!(envelope(&resources, &[]).payload.processes[0].pid, 4242);
}

#[test]
fn a_non_numeric_pid_is_an_error_naming_the_resource() {
    let resources = vec![
        host_resource(),
        process_resource("p1", "not-a-pid", "/bin/a"),
    ];
    let err = build_envelope(
        &resources,
        &[],
        SnapshotMeta {
            producer: "agent-1",
            tenant: "t",
            env: None,
            snapshot_id: "s",
            revision: 1,
            generated_at: OBSERVED_AT,
            collected_at: OBSERVED_AT,
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            AdapterError::NotNumeric {
                field: "process.pid",
                ..
            }
        ),
        "{err}"
    );
    assert!(err.to_string().contains("p1"), "{err}");
}

#[test]
fn a_process_without_discovered_at_falls_back_to_the_snapshot_time() {
    let mut process = process_resource("p1", "1", "/sbin/launchd");
    process.discovered_at = String::new();
    let resources = vec![host_resource(), process];
    assert_eq!(
        envelope(&resources, &[]).payload.processes[0].observed_at,
        OBSERVED_AT
    );
}

// ── 信封 ──────────────────────────────────────────────

#[test]
fn envelope_carries_source_collect_and_payload() {
    let built = envelope(&[host_resource()], &[]);
    assert_eq!(built.schema, EDGE_SCHEMA);
    assert_eq!(
        built.source,
        Source {
            system: "wist".to_string(),
            producer: "agent-1".to_string(),
            tenant: "tenant-demo".to_string(),
            env: Some("dev".to_string()),
        }
    );
    assert_eq!(built.collect.mode, "snapshot");
    assert_eq!(built.collect.snap_id, "snap-1");
    assert_eq!(built.collect.res_ver, "7");
    assert_eq!(built.collect.observed_at, OBSERVED_AT);
}

#[test]
fn env_is_omitted_when_not_configured() {
    let built = build_envelope(
        &[host_resource()],
        &[],
        SnapshotMeta {
            producer: "agent-1",
            tenant: "tenant-demo",
            env: None,
            snapshot_id: "snap-1",
            revision: 1,
            generated_at: OBSERVED_AT,
            collected_at: OBSERVED_AT,
        },
    )
    .expect("build envelope");
    let rendered = serde_json::to_string(&built).expect("serialize");
    assert!(!rendered.contains("\"env\""), "{rendered}");
}

// ── report 模式 ────────────────────────────────────────

fn report_json() -> String {
    serde_json::json!({
        "api_version": "v1",
        "kind": "report_discovery_snapshot",
        "report_id": "disc_agent-1_1",
        "agent_id": "agent-from-report",
        "instance_id": "inst-1",
        "snapshot_id": "discovery:5:2026-10-25T07:56:36Z",
        "revision": 5,
        "generated_at": OBSERVED_AT,
        "report_attempt": 1,
        "report_mode": "full_snapshot",
        "reported_at": "2026-10-25T07:56:38.736786Z",
        "snapshot": {
            "schema_version": "v1",
            "snapshot_id": "discovery:5:2026-10-25T07:56:36Z",
            "revision": 5,
            "generated_at": OBSERVED_AT,
            "origins": [],
            "resources": [{
                "resource_id": "machine-abc",
                "kind": "host",
                "origin_idx": 0,
                "attributes": { "host.id": "machine-abc", "host.name": "node-1" },
                "discovered_at": OBSERVED_AT,
                "last_seen_at": OBSERVED_AT,
                "health": "healthy",
                "source": "host"
            }],
            "targets": []
        }
    })
    .to_string()
}

#[test]
fn snapshot_fields_drive_the_collect_block() {
    let built = envelope_from_report_json(&report_json(), None, "tenant-demo", Some("dev"))
        .expect("convert");
    assert_eq!(built.source.producer, "agent-from-report");
    assert_eq!(built.collect.snap_id, "discovery:5:2026-10-25T07:56:36Z");
    assert_eq!(built.collect.res_ver, "5");
    assert_eq!(built.collect.observed_at, OBSERVED_AT);
    // collected_at 是「我们收到它的时刻」，与快照自带的 generated_at 不同。
    assert_eq!(built.collect.collected_at, "2026-10-25T07:56:38.736786Z");
}

#[test]
fn agent_id_override_wins() {
    let built =
        envelope_from_report_json(&report_json(), Some("override-1"), "t", None).expect("convert");
    assert_eq!(built.source.producer, "override-1");
}

#[test]
fn a_report_without_snapshot_is_rejected() {
    let broken = serde_json::json!({ "agent_id": "a", "kind": "report_discovery_snapshot" });
    let err = envelope_from_report_json(&broken.to_string(), None, "t", None).unwrap_err();
    assert!(matches!(err, AdapterError::MissingReportSnapshot), "{err}");
}

#[test]
fn a_report_without_any_agent_id_is_rejected() {
    let mut document: Value = serde_json::from_str(&report_json()).expect("json");
    document.as_object_mut().expect("object").remove("agent_id");
    let err = envelope_from_report_json(&document.to_string(), None, "t", None).unwrap_err();
    assert!(matches!(err, AdapterError::MissingProducer), "{err}");
}

#[test]
fn a_report_missing_reported_at_falls_back_to_now() {
    let mut document: Value = serde_json::from_str(&report_json()).expect("json");
    document
        .as_object_mut()
        .expect("object")
        .remove("reported_at");
    let built = envelope_from_report_json(&document.to_string(), None, "t", None).expect("convert");
    // 不能是空串：空串会让中心侧按 observed_at 排序时把这条排到最前。
    assert!(!built.collect.collected_at.trim().is_empty());
    assert!(built.collect.collected_at.ends_with('Z'));
}

#[test]
fn unknown_fields_are_ignored() {
    // 这是**不**直接用 wist-contracts 类型的理由：新版 agentd 加字段不该让
    // 旧版适配器整帧打回。加完后必须仍然解析成功且结果不变。
    let mut document: Value = serde_json::from_str(&report_json()).expect("json");
    let object = document.as_object_mut().expect("object");
    object.insert("brand_new_envelope_field".to_string(), Value::from(1));
    let snapshot = object
        .get_mut("snapshot")
        .and_then(Value::as_object_mut)
        .expect("snapshot");
    snapshot.insert("brand_new_snapshot_field".to_string(), Value::from("x"));
    let host = snapshot
        .get_mut("resources")
        .and_then(Value::as_array_mut)
        .and_then(|resources| resources.first_mut())
        .and_then(Value::as_object_mut)
        .expect("resource");
    host.insert("new_resource_field".to_string(), Value::from(true));

    let built = envelope_from_report_json(&document.to_string(), None, "t", None).expect("convert");
    assert_eq!(built.payload.hosts.len(), 1);
    assert_eq!(built.collect.res_ver, "5");
}
