//! fixture 端到端：一条裁剪过的 macOS 快照 → `atlas.in.edge.v1` 逐字节比对。
//!
//! 输入是**真帧的形状**（取自 `mac-local-01` 的 DSNAP 正文，裁剪到
//! 1 host / 2 网卡 / 3 进程），不是手造的理想数据。它钉住三件事：
//!
//! 1. 契约字段名（`host.id` / `net.if.addr` / `process.pid` / `execution_hints.process.identity`）
//!    一旦改名就红，不会静默产出空结果；
//! 2. 过滤语义（`127.0.0.1` 的 lo0 整卡丢弃、只有 ip_address 的 en1 被补出来）；
//! 3. **字段顺序**（序列化顺序即声明顺序，输出可 diff、可进 git）。

use std::path::PathBuf;

use atlas_wist_adapter::{EdgeEnvelope, envelope_from_report_json};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

#[test]
fn trimmed_macos_snapshot_matches_the_golden_envelope() {
    let report = read(&fixture("macos-trimmed.json"));
    let envelope = envelope_from_report_json(&report, None, "tenant-demo", Some("dev"))
        .expect("convert fixture");

    let actual = serde_json::to_string(&envelope).expect("serialize");
    let expected = read(&fixture("macos-trimmed.expected.json"));
    assert_eq!(
        actual,
        expected.trim_end_matches('\n'),
        "fixture 输出与 golden 不一致：改了映射规则就同步重新生成 expected"
    );
}

#[test]
fn golden_envelope_round_trips_through_its_own_type() {
    // 输出的形状必须能用自己的类型读回来：否则 `--jsonl` 喂给 `replay-jsonl` 时才会发现。
    let raw = read(&fixture("macos-trimmed.expected.json"));
    let envelope: EdgeEnvelope = serde_json::from_str(raw.trim_end()).expect("decode golden");
    assert_eq!(envelope.payload.hosts.len(), 1);
    assert_eq!(envelope.payload.interfaces.len(), 2);
    assert_eq!(envelope.payload.processes.len(), 3);
}

#[test]
fn trimmed_fixture_shape_is_what_the_doc_says() {
    // 这条断言是给人看的：fixture 一旦被改大/改小，用例名里的数字就不再撒谎。
    let report = read(&fixture("macos-trimmed.json"));
    let envelope = envelope_from_report_json(&report, None, "tenant-demo", Some("dev"))
        .expect("convert fixture");

    let names: Vec<&str> = envelope
        .payload
        .interfaces
        .iter()
        .map(|interface| interface.name.as_str())
        .collect();
    assert_eq!(names, vec!["en0", "en1"], "lo0 只有回环地址，应被整卡丢弃");

    let pids: Vec<i64> = envelope
        .payload
        .processes
        .iter()
        .map(|process| process.pid)
        .collect();
    assert_eq!(pids, vec![1, 210, 300]);
    assert!(
        envelope
            .payload
            .processes
            .iter()
            .all(|process| process.identity.is_some()),
        "三条进程都有 process target，identity 应全部带上"
    );
}

#[test]
fn a_state_dir_without_discovery_files_reports_the_missing_path() {
    let missing = std::env::temp_dir().join("atlas-wist-adapter-no-such-state");
    let err = atlas_wist_adapter::envelope_from_state_dir(&missing, "agent-1", "tenant-demo", None)
        .expect_err("missing state dir must fail");
    let message = err.to_string();
    assert!(message.contains("resources.json"), "{message}");
}
