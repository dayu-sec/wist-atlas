use std::path::PathBuf;
use std::process::Command;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/p0_monolith_demo.json")
}

fn target_edge_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/external-input/target/edge-discovery-snapshot.json")
}

fn atlas_edge_host_only_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas_edge_host_only.json")
}

fn atlas_edge_host_only_jsonl_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas_edge_host_only.jsonl")
}

fn atlas_edge_host_process_jsonl_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl")
}

fn atlas_telemetry_host_jsonl_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_telemetry_host_sample.jsonl")
}

fn atlas_telemetry_process_jsonl_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_telemetry_process_sample.jsonl")
}

fn run_app(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_wist-atlas"))
        .args(args)
        .output()
        .expect("failed to run wist-atlas")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "process failed: status={:?}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout_text(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be valid utf-8")
}

#[test]
fn smoke_demo_mode_builds_minimal_closure() {
    let output = run_app(&[]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas monolith started"));
    assert!(stdout.contains("ingest_id=demo-ingest-1"));
    assert!(stdout.contains("host=demo-node"));
    assert!(stdout.contains("network=10.42.0.0/24"));
    assert!(stdout.contains("ip=10.42.0.12"));
    assert!(stdout.contains("responsibilities=alice:Owner"));
}

#[test]
fn smoke_file_mode_builds_minimal_closure_from_fixture() {
    let fixture = fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["file", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("ingest_id=demo-ingest-1"));
    assert!(stdout.contains("host=demo-node"));
    assert!(stdout.contains("network=10.42.0.0/24"));
    assert!(stdout.contains("ip=10.42.0.12"));
    assert!(stdout.contains("responsibilities=alice:Owner"));
}

#[test]
fn smoke_file_mode_accepts_target_atlas_input_envelope() {
    let fixture = target_edge_fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["file", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("ingest_id=atlas.in.edge.v1:warp-insight:agent-office-01:tenant-demo:office:edge-snap-20260426-office-01"));
    assert!(stdout.contains("host=office-build-01"));
    assert!(stdout.contains("network=192.168.10.0/24"));
    assert!(stdout.contains("ip=192.168.10.52"));
}

#[test]
fn smoke_file_mode_accepts_atlas_edge_host_only_payload() {
    let fixture = atlas_edge_host_only_fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["file", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains(
        "ingest_id=atlas.in.edge.v1:warp-insight:agent-local-01:tenant-demo:office:edge-snap-local-01"
    ));
    assert!(stdout.contains("host=local-host"));
    assert!(!stdout.contains("network="));
    assert!(!stdout.contains("ip="));
}

#[test]
fn smoke_replay_jsonl_mode_materializes_host_and_processes() {
    let fixture = atlas_edge_host_process_jsonl_fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["replay-jsonl", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas replay finished"));
    assert!(stdout.contains("lines_total=2"));
    assert!(stdout.contains("lines_ok=2"));
    assert!(stdout.contains("lines_failed=0"));
    assert!(stdout.contains("hosts=1"));
    assert!(stdout.contains("networks=0"));
    assert!(stdout.contains("processes=1"));
    assert!(stdout.contains("host_runtimes=0"));
}

#[test]
fn smoke_replay_jsonl_mode_materializes_host_runtime_from_telemetry() {
    let edge_fixture = atlas_edge_host_only_jsonl_fixture_path();
    let fixture = atlas_telemetry_host_jsonl_fixture_path();
    let edge_fixture = edge_fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["replay-jsonl", edge_fixture, fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas replay finished"));
    assert!(stdout.contains("lines_total=3"));
    assert!(stdout.contains("lines_ok=3"));
    assert!(stdout.contains("lines_failed=0"));
    assert!(stdout.contains("hosts=1"));
    assert!(stdout.contains("host_runtimes=1"));
}

#[test]
fn smoke_replay_jsonl_mode_enriches_process_from_telemetry() {
    let fixture = atlas_telemetry_process_jsonl_fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["replay-jsonl", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas replay finished"));
    assert!(stdout.contains("lines_total=3"));
    assert!(stdout.contains("lines_ok=3"));
    assert!(stdout.contains("lines_failed=0"));
    assert!(stdout.contains("hosts=1"));
    assert!(stdout.contains("processes=1"));
    assert!(stdout.contains("processes_enriched=1"));
}

#[test]
fn smoke_postgres_mock_mode_uses_same_ingest_query_path() {
    let output = run_app(&["postgres-mock"]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas monolith started"));
    assert!(stdout.contains("ingest_id=demo-ingest-1"));
    assert!(stdout.contains("host=demo-node"));
    assert!(stdout.contains("network=10.42.0.0/24"));
    assert!(stdout.contains("ip=10.42.0.12"));
    assert!(stdout.contains("responsibilities=alice:Owner"));
}

#[test]
fn smoke_postgres_mock_reset_public_reports_success() {
    let output = run_app(&["postgres-mock", "reset-public"]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas reset finished"));
    assert!(stdout.contains("status=public schema reset complete"));
}

#[test]
fn smoke_postgres_mock_replace_jsonl_replays_after_reset() {
    let fixture = atlas_edge_host_process_jsonl_fixture_path();
    let fixture = fixture
        .to_str()
        .expect("fixture path should be representable as utf-8");
    let output = run_app(&["postgres-mock", "replace-jsonl", fixture]);
    assert_success(&output);

    let stdout = stdout_text(&output);
    assert!(stdout.contains("wist-atlas replay finished"));
    assert!(stdout.contains("lines_total=2"));
    assert!(stdout.contains("lines_ok=2"));
    assert!(stdout.contains("lines_failed=0"));
    assert!(stdout.contains("hosts=1"));
    assert!(stdout.contains("processes=1"));
}
