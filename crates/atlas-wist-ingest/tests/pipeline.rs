//! 端到端：真实形状的 sink 文件 → 增量读取 → 转换 → 入库（含 stale 清扫）。

use std::fs;
use std::path::PathBuf;

use atlas_api::TopologyIngestService;
use atlas_domain::TenantId;
use atlas_storage::{CatalogStore, Page, RuntimeStore};
use atlas_wist_ingest::{DiscoveryLogTail, convert_sink_line};
use serde_json::json;

const HOST_NAME: &str = "mac-ingest-01";

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("atlas-wist-ingest-e2e-{name}-{suffix}"));
        fs::create_dir_all(&dir).expect("mkdir");
        Self(dir)
    }

    fn log(&self) -> PathBuf {
        self.0.join("agent-discovery.jsonl")
    }

    fn checkpoint(&self) -> PathBuf {
        self.0.join("offset.json")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// 造一行**真形状**的 sink 记录：`body` 是报告 JSON 的**字符串**，外面再套 wparse 的 OML 信封。
fn sink_line(snap_id: &str, observed_at: &str, pids: &[i32]) -> String {
    let mut resources = vec![json!({
        "resource_id": "machine-ingest-01",
        "kind": "host",
        "origin_idx": 0,
        "attributes": {"host.id": "machine-ingest-01", "host.name": HOST_NAME},
        "discovered_at": observed_at,
        "last_seen_at": observed_at,
        "health": "healthy",
        "source": "host"
    })];
    resources.extend(pids.iter().map(|pid| {
        json!({
            "resource_id": format!("machine-ingest-01:pid:{pid}:ps_lstart:{pid}"),
            "kind": "process",
            "origin_idx": 3,
            "attributes": {
                "process.pid": pid.to_string(),
                "process.executable.name": format!("/bin/proc-{pid}")
            },
            "discovered_at": observed_at,
            "last_seen_at": observed_at,
            "health": "healthy",
            "source": "process"
        })
    }));

    let targets: Vec<serde_json::Value> = pids
        .iter()
        .map(|pid| {
            json!({
                "target_id": format!("machine-ingest-01:pid:{pid}:process"),
                "kind": "process",
                "origin_idx": 3,
                "resource_ref": format!("machine-ingest-01:pid:{pid}:ps_lstart:{pid}"),
                "execution_hints": {"process.identity": format!("ps_lstart:{pid}")},
                "state": "active"
            })
        })
        .collect();

    let report = json!({
        "api_version": "v1",
        "kind": "report_discovery_snapshot",
        "report_id": format!("disc_{snap_id}"),
        "agent_id": "agent-e2e",
        "instance_id": "agent-e2e",
        "snapshot_id": snap_id,
        "revision": 1,
        "generated_at": observed_at,
        "report_attempt": 1,
        "report_mode": "full_snapshot",
        "reported_at": observed_at,
        "snapshot": {
            "schema_version": "v1",
            "snapshot_id": snap_id,
            "revision": 1,
            "generated_at": observed_at,
            "origins": [],
            "resources": resources,
            "targets": targets
        }
    });

    json!({
        "schema": "v1",
        "agent_id": "agent-e2e",
        "observed_at": observed_at,
        "seq": 1,
        "category": "agent.discovery",
        "log_desc": "Agent 发现快照",
        "body": report.to_string(),
        "biz": "agent-discovery",
        "env": "dev",
        "sink": "json",
        "wp_oml_name": "agent_discovery_snapshot"
    })
    .to_string()
}

fn append(dir: &TempDir, line: &str) {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.log())
        .expect("open log");
    file.write_all(line.as_bytes()).expect("write line");
    file.write_all(b"\n").expect("newline");
}

fn drain(
    tail: &mut DiscoveryLogTail,
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    tenant_id: TenantId,
) -> (usize, u64) {
    let mut ingested = 0;
    let mut stale = 0;
    for line in tail.poll().expect("poll") {
        let input = convert_sink_line(&line.text, "tenant-demo", None).expect("convert");
        let (_record, summary) = service
            .submit_atlas_input_and_materialize(input, tenant_id, None)
            .expect("ingest");
        stale += summary.stale_process_count;
        ingested += 1;
        tail.commit(line.next_offset).expect("commit");
    }
    (ingested, stale)
}

#[test]
fn a_real_shaped_sink_file_drives_ingest_and_stale_sweep() {
    let dir = TempDir::new("pipeline");
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(uuid::Uuid::new_v4());

    append(
        &dir,
        &sink_line("snap-1", "2026-10-25T09:00:00Z", &[100, 200, 300]),
    );
    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");

    assert_eq!(drain(&mut tail, &service, tenant_id), (1, 0));

    // 200 退出了。
    append(
        &dir,
        &sink_line("snap-2", "2026-10-25T09:05:00Z", &[100, 300]),
    );
    assert_eq!(
        drain(&mut tail, &service, tenant_id),
        (1, 1),
        "第二帧应把消失的 200 标成 stale"
    );

    // 库里三行都在，但 200 是 stale。
    let host = CatalogStore::list_hosts(service.store(), tenant_id, Page::default())
        .expect("list hosts")
        .into_iter()
        .find(|host| host.host_name == HOST_NAME)
        .expect("host");
    let mut processes = RuntimeStore::list_process_runtime_states(
        service.store(),
        host.host_id,
        Page {
            limit: 500,
            offset: 0,
        },
    )
    .expect("list processes");
    processes.sort_by_key(|process| process.pid);
    assert_eq!(
        processes
            .iter()
            .map(|process| (process.pid, process.stale_at.is_some()))
            .collect::<Vec<_>>(),
        vec![(100, false), (200, true), (300, false)]
    );

    // 再轮询：没有新行，也没有重复入库。
    assert_eq!(drain(&mut tail, &service, tenant_id), (0, 0));
}

#[test]
fn a_partial_trailing_line_waits_for_the_newline() {
    // 网关正在写一帧时不能解析半行 —— 否则一次半写就能把半张快照塞进库。
    let dir = TempDir::new("partial");
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(uuid::Uuid::new_v4());

    let line = sink_line("snap-partial", "2026-10-25T09:00:00Z", &[100]);
    let (head, rest) = line.split_at(line.len() / 2);
    fs::write(dir.log(), head).expect("write partial");

    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    assert_eq!(drain(&mut tail, &service, tenant_id), (0, 0), "半行不算");

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(dir.log())
        .expect("open");
    file.write_all(rest.as_bytes()).expect("rest");
    file.write_all(b"\n").expect("newline");

    assert_eq!(drain(&mut tail, &service, tenant_id), (1, 0));
}

#[test]
fn a_corrupt_line_is_skippable_and_does_not_block_the_next_frame() {
    let dir = TempDir::new("corrupt");
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(uuid::Uuid::new_v4());

    append(&dir, "{ this is not json");
    append(&dir, &sink_line("snap-ok", "2026-10-25T09:00:00Z", &[100]));

    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    let lines = tail.poll().expect("poll");
    assert_eq!(lines.len(), 2);

    assert!(
        convert_sink_line(&lines[0].text, "tenant-demo", None).is_err(),
        "坏行必须被判定为不可转换"
    );
    tail.commit(lines[0].next_offset).expect("skip bad line");

    let input = convert_sink_line(&lines[1].text, "tenant-demo", None).expect("good line");
    service
        .submit_atlas_input_and_materialize(input, tenant_id, None)
        .expect("ingest");
    tail.commit(lines[1].next_offset).expect("commit");

    let hosts =
        CatalogStore::list_hosts(service.store(), tenant_id, Page::default()).expect("list");
    assert_eq!(hosts.len(), 1, "坏行不应影响后面的正常帧");
}

#[test]
fn conversion_rejects_a_line_without_a_body() {
    let line = json!({"agent_id": "a", "category": "agent.discovery"}).to_string();
    let err = convert_sink_line(&line, "tenant-demo", None).expect_err("must fail");
    assert!(err.contains("body"), "{err}");
}

#[test]
fn the_line_env_is_used_unless_overridden() {
    let dir = TempDir::new("env");
    let service = TopologyIngestService::new_in_memory();
    let tenant_id = TenantId(uuid::Uuid::new_v4());
    append(&dir, &sink_line("snap-env", "2026-10-25T09:00:00Z", &[100]));

    let mut tail = DiscoveryLogTail::open(dir.log(), dir.checkpoint()).expect("open");
    let lines = tail.poll().expect("poll");

    let input = convert_sink_line(&lines[0].text, "tenant-demo", None).expect("convert");
    assert_eq!(input.source.env.as_deref(), Some("dev"), "行上的部署环境");

    let overridden =
        convert_sink_line(&lines[0].text, "tenant-demo", Some("prod")).expect("convert");
    assert_eq!(
        overridden.source.env.as_deref(),
        Some("prod"),
        "CLI 覆盖优先"
    );

    service
        .submit_atlas_input_and_materialize(input, tenant_id, None)
        .expect("ingest");
}
