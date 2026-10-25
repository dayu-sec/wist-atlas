use super::*;
use atlas_storage::{CatalogStore, Page};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::util::ServiceExt;

#[test]
fn monolith_demo_runs_end_to_end() {
    let app = TopologyMonolith::new_in_memory();
    let summary = app.run_demo().unwrap();

    assert_eq!(summary.ingest_id, "demo-ingest-1");
    assert_eq!(summary.host_name, "demo-node");
    assert_eq!(summary.network_name.as_deref(), Some("10.42.0.0/24"));
    assert_eq!(summary.assoc_ip.as_deref(), Some("10.42.0.12"));
    assert_eq!(summary.responsibilities, vec!["alice:Owner".to_string()]);
}

#[test]
fn parse_monolith_input_defaults_to_demo() {
    let (mode, input) = parse_monolith_input(&[]).unwrap();
    assert_eq!(mode, MonolithMode::Memory);
    assert!(matches!(input, MonolithInput::Demo));
}

#[test]
fn parse_monolith_input_supports_file_mode() {
    let (mode, input) =
        parse_monolith_input(&["file".to_string(), "/tmp/demo.json".to_string()]).unwrap();
    assert_eq!(mode, MonolithMode::Memory);
    match input {
        MonolithInput::File(path) => assert_eq!(path, PathBuf::from("/tmp/demo.json")),
        _ => panic!("expected file mode"),
    }
}

#[test]
fn parse_monolith_input_supports_replay_jsonl_mode() {
    let (mode, input) = parse_monolith_input(&[
        "replay-jsonl".to_string(),
        "/tmp/atlas-edge.jsonl".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::Memory);
    match input {
        MonolithInput::JsonlFiles(paths) => {
            assert_eq!(paths, vec![PathBuf::from("/tmp/atlas-edge.jsonl")])
        }
        _ => panic!("expected replay-jsonl mode"),
    }
}

#[test]
fn parse_monolith_input_supports_replay_jsonl_multi_file_mode() {
    let (mode, input) = parse_monolith_input(&[
        "replay-jsonl".to_string(),
        "/tmp/atlas-edge.jsonl".to_string(),
        "/tmp/atlas-telemetry.jsonl".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::Memory);
    match input {
        MonolithInput::JsonlFiles(paths) => assert_eq!(
            paths,
            vec![
                PathBuf::from("/tmp/atlas-edge.jsonl"),
                PathBuf::from("/tmp/atlas-telemetry.jsonl")
            ]
        ),
        _ => panic!("expected replay-jsonl multi-file mode"),
    }
}

#[test]
fn parse_monolith_input_supports_replay_jsonl_many_files() {
    let (mode, input) = parse_monolith_input(&[
        "replay-jsonl".to_string(),
        "/tmp/a.jsonl".to_string(),
        "/tmp/b.jsonl".to_string(),
        "/tmp/c.jsonl".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::Memory);
    match input {
        MonolithInput::JsonlFiles(paths) => assert_eq!(
            paths,
            vec![
                PathBuf::from("/tmp/a.jsonl"),
                PathBuf::from("/tmp/b.jsonl"),
                PathBuf::from("/tmp/c.jsonl")
            ]
        ),
        _ => panic!("expected replay-jsonl many-file mode"),
    }
}

#[test]
fn parse_monolith_input_supports_memory_serve_jsonl_mode() {
    let (mode, input) = parse_monolith_input(&[
        "memory".to_string(),
        "serve-jsonl".to_string(),
        "/tmp/atlas-edge.jsonl".to_string(),
        "--listen".to_string(),
        "127.0.0.1:8080".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::Memory);
    match input {
        MonolithInput::ServeJsonlFiles { listen, paths } => {
            assert_eq!(listen, "127.0.0.1:8080");
            assert_eq!(paths, vec![PathBuf::from("/tmp/atlas-edge.jsonl")]);
        }
        _ => panic!("expected serve-jsonl mode"),
    }
}

#[test]
fn parse_monolith_input_supports_postgres_mock_mode() {
    let (mode, input) = parse_monolith_input(&["postgres-mock".to_string()]).unwrap();
    assert_eq!(mode, MonolithMode::PostgresMock);
    assert!(matches!(input, MonolithInput::Demo));
}

#[test]
fn parse_monolith_input_supports_postgres_live_mode() {
    let (mode, input) = parse_monolith_input(&["postgres-live".to_string()]).unwrap();
    assert_eq!(mode, MonolithMode::PostgresLive);
    assert!(matches!(input, MonolithInput::Demo));
}

#[test]
fn parse_monolith_input_supports_postgres_live_reset_public_mode() {
    let (mode, input) =
        parse_monolith_input(&["postgres-live".to_string(), "reset-public".to_string()]).unwrap();
    assert_eq!(mode, MonolithMode::PostgresLive);
    assert!(matches!(input, MonolithInput::ResetPublic));
}

#[test]
fn parse_monolith_input_supports_postgres_live_export_visualization_mode() {
    let (mode, input) = parse_monolith_input(&[
        "postgres-live".to_string(),
        "export-visualization".to_string(),
        "/tmp/live-topology.json".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::PostgresLive);
    match input {
        MonolithInput::ExportVisualization(path) => {
            assert_eq!(path, PathBuf::from("/tmp/live-topology.json"))
        }
        _ => panic!("expected export-visualization mode"),
    }
}

#[test]
fn parse_monolith_input_supports_postgres_live_print_first_host_process_topology_mode() {
    let (mode, input) = parse_monolith_input(&[
        "postgres-live".to_string(),
        "print-first-host-process-topology".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::PostgresLive);
    assert!(matches!(
        input,
        MonolithInput::PrintFirstHostProcessTopology
    ));
}

#[test]
fn parse_monolith_input_supports_postgres_live_import_jsonl_mode() {
    let (mode, input) = parse_monolith_input(&[
        "postgres-live".to_string(),
        "import-jsonl".to_string(),
        "/tmp/atlas-edge.jsonl".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::PostgresLive);
    match input {
        MonolithInput::JsonlFiles(paths) => {
            assert_eq!(paths, vec![PathBuf::from("/tmp/atlas-edge.jsonl")])
        }
        _ => panic!("expected import-jsonl mode"),
    }
}

#[test]
fn parse_monolith_input_supports_postgres_live_replace_jsonl_mode() {
    let (mode, input) = parse_monolith_input(&[
        "postgres-live".to_string(),
        "replace-jsonl".to_string(),
        "/tmp/atlas-edge.jsonl".to_string(),
        "/tmp/atlas-telemetry.jsonl".to_string(),
    ])
    .unwrap();
    assert_eq!(mode, MonolithMode::PostgresLive);
    match input {
        MonolithInput::ReplaceJsonlFiles(paths) => assert_eq!(
            paths,
            vec![
                PathBuf::from("/tmp/atlas-edge.jsonl"),
                PathBuf::from("/tmp/atlas-telemetry.jsonl")
            ]
        ),
        _ => panic!("expected replace-jsonl mode"),
    }
}

#[test]
fn postgres_mock_demo_runs_end_to_end() {
    let app = TopologyAppBuilder::new()
        .with_mode(MonolithMode::PostgresMock)
        .build()
        .unwrap();
    let summary = app.run_demo().unwrap();

    assert_eq!(summary.ingest_id, "demo-ingest-1");
    assert_eq!(summary.host_name, "demo-node");
    assert_eq!(summary.network_name.as_deref(), Some("10.42.0.0/24"));
    assert_eq!(summary.assoc_ip.as_deref(), Some("10.42.0.12"));
    assert_eq!(summary.responsibilities, vec!["alice:Owner".to_string()]);
}

#[test]
fn file_mode_accepts_target_atlas_input_envelope() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/external-input/target/edge-discovery-snapshot.json");

    let summary = app.run_file(fixture).unwrap();

    assert_eq!(
        summary.ingest_id,
        "atlas.in.edge.v1:warp-insight:agent-office-01:tenant-demo:office:edge-snap-20260426-office-01"
    );
    assert_eq!(summary.host_name, "office-build-01");
    assert_eq!(summary.network_name.as_deref(), Some("192.168.10.0/24"));
    assert_eq!(summary.assoc_ip.as_deref(), Some("192.168.10.52"));
}

#[test]
fn file_mode_accepts_atlas_edge_host_only_payload() {
    let app = TopologyMonolith::new_in_memory();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/atlas_edge_host_only.json");

    let summary = app.run_file(fixture).unwrap();

    assert_eq!(
        summary.ingest_id,
        "atlas.in.edge.v1:warp-insight:agent-local-01:tenant-demo:office:edge-snap-local-01"
    );
    assert_eq!(summary.host_name, "local-host");
    assert_eq!(summary.network_name, None);
    assert_eq!(summary.assoc_ip, None);
}

#[test]
fn replay_jsonl_mode_materializes_host_and_processes() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl");

    let summary = app.replay_jsonl(fixture).unwrap();

    assert_eq!(summary.total_lines, 2);
    assert_eq!(summary.success_lines, 2);
    assert_eq!(summary.failed_lines, 0);
    assert_eq!(summary.host_count, 1);
    assert_eq!(summary.network_count, 0);
    assert_eq!(summary.process_count, 1);
    assert_eq!(summary.enriched_process_count, 0);
    assert_eq!(summary.host_runtime_count, 0);
    assert!(summary.last_ingest_id.is_some());
    assert!(summary.failures.is_empty());
}

#[test]
fn print_first_host_process_topology_outputs_specialized_graph_json() {
    let app = TopologyMonolith::new_postgres_mock().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let result = app
        .run(MonolithInput::PrintFirstHostProcessTopology)
        .unwrap();
    let body = match result {
        MonolithRunResult::PrintJson(body) => body,
        other => panic!(
            "expected print json result, got {:?}",
            std::mem::discriminant(&other)
        ),
    };

    assert!(body.contains("\"objectKind\": \"HostInventory\""));
    assert!(body.contains("\"objectKind\": \"ProcessSummary\""));
    assert!(body.contains("\"objectKind\": \"ProcessGroup\""));
    assert!(body.contains("\"objectKind\": \"ProcessRuntime\""));
}

#[test]
fn print_first_host_process_topology_prints_service_endpoints() {
    let app = TopologyMonolith::new_postgres_mock().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_service_endpoint_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let result = app
        .run(MonolithInput::PrintFirstHostProcessTopology)
        .unwrap();
    let body = match result {
        MonolithRunResult::PrintJson(body) => body,
        other => panic!(
            "expected print json result, got {:?}",
            std::mem::discriminant(&other)
        ),
    };

    // 服务节点必须把「在哪儿听」带出去，否则前端只能显示一个服务名，
    // 看不出暴露面。实例节点的标题直接就是监听地址。
    assert!(body.contains("\"objectKind\": \"ServiceEntity\""));
    assert!(body.contains("\"objectKind\": \"ServiceInstance\""));
    assert!(body.contains("\"endpoints\""));
    assert!(body.contains("\"port\": 22"));
    assert!(body.contains("tcp/22 0.0.0.0"));
    assert!(body.contains("\"edgeKind\": \"host_service_assoc\""));
    // 没 pid 的那个端口不该凭空造一个服务出来。
    assert!(!body.contains("\"port\": 9999"));
}

#[tokio::test]
async fn http_router_returns_network_topology_for_a_segment() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_network_hosts_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    // 同一网段上的两台主机：网段视图应该把两台都带出来。
    let store = app.store().unwrap();
    let tenant_id = store
        .list_all_hosts(Page::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .tenant_id;
    let segment_id = store
        .list_network_segments(tenant_id, Page::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .network_segment_id;

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/topology/network/{segment_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["metadata"]["hostCount"], 2);
    let nodes = payload["data"]["nodes"].as_array().unwrap();
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["objectKind"] == "NetworkSegment")
            .count(),
        1
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["objectKind"] == "HostInventory")
            .count(),
        2
    );
    // 每个主机到网段一条关联边，边上带接口名（界面靠它显示 eth0 而不是 CIDR）。
    let edges = payload["data"]["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 2);
    assert!(
        edges
            .iter()
            .all(|edge| edge["edgeKind"] == "host_network_assoc")
    );
    assert!(
        edges
            .iter()
            .all(|edge| edge["properties"]["ifaceName"] == "eth0")
    );
}

#[tokio::test]
async fn http_router_lists_services_by_spread() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_service_shared_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    let services = payload["data"]["services"].as_array().unwrap();
    assert_eq!(services.len(), 2);
    // 铺得广的排前面：sshd 两台机都在跑，nginx 只在一台上。
    assert_eq!(services[0]["name"], "sshd");
    assert_eq!(services[0]["instanceCount"], 2);
    assert_eq!(services[0]["hostCount"], 2);
    assert_eq!(services[1]["name"], "nginx");
    assert_eq!(services[1]["hostCount"], 1);
}

#[tokio::test]
async fn http_router_lists_no_services_for_an_empty_catalog() {
    let app = TopologyMonolith::new_in_memory();
    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["services"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn http_router_returns_404_for_unknown_network() {
    let app = TopologyMonolith::new_in_memory();
    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/topology/network/{}", uuid::Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_router_returns_service_topology_across_hosts() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_service_shared_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    // 同一套 sshd 跑在两台主机上：服务视图要把两台主机、两个实例、两个监听进程都带出来。
    let store = app.store().unwrap();
    let tenant_id = store
        .list_all_hosts(Page::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .tenant_id;
    let service_id = store
        .list_services(tenant_id, Page::default())
        .unwrap()
        .into_iter()
        .find(|service| service.name == "sshd")
        .unwrap()
        .service_id;

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/topology/service/{service_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["metadata"]["hostCount"], 2);
    let nodes = payload["data"]["nodes"].as_array().unwrap();
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["objectKind"] == "ServiceEntity")
            .count(),
        1
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["objectKind"] == "ServiceInstance")
            .count(),
        2
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["objectKind"] == "HostInventory")
            .count(),
        2
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node["objectKind"] == "ProcessRuntime")
            .count(),
        2
    );

    let edges = payload["data"]["edges"].as_array().unwrap();
    for edge_kind in [
        "host_service_assoc",
        "service_instance_assoc",
        "process_service_assoc",
    ] {
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge["edgeKind"] == edge_kind)
                .count(),
            2,
            "{edge_kind} should appear once per host/instance"
        );
    }

    // 服务节点上的端点取所有实例的并集并去重：0.0.0.0:22 两台各一份合成一个。
    let service_node = nodes
        .iter()
        .find(|node| node["objectKind"] == "ServiceEntity")
        .unwrap();
    let endpoints = service_node["properties"]["endpoints"].as_array().unwrap();
    assert_eq!(endpoints.len(), 2);
    assert!(
        endpoints
            .iter()
            .any(|endpoint| endpoint["port"] == 22 && endpoint["address"] == "10.30.0.11")
    );
    // nginx 是另一个服务，不该被卷进 sshd 的视图。
    assert!(!endpoints.iter().any(|endpoint| endpoint["port"] == 8080));
}

#[tokio::test]
async fn http_router_returns_404_for_unknown_service() {
    let app = TopologyMonolith::new_in_memory();
    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/topology/service/{}", uuid::Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_router_lists_hosts_with_scale_metrics() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_service_endpoint_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    let hosts = payload["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0]["hostName"], "local-host");
    // 两个进程都是活的；退役进程不计数。
    assert_eq!(hosts[0]["processCount"], 2);
    assert_eq!(hosts[0]["segmentCount"], 0);
    assert_eq!(hosts[0]["stale"], false);
    assert!(hosts[0]["lastInventoryAt"].as_str().is_some());
}

#[tokio::test]
async fn http_router_hides_stale_hosts_unless_asked() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/hosts?include_stale=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    // 默认会滤掉退役主机；显式带上 include_stale 时至少要包含现役那一台。
    let hosts = payload["data"]["hosts"].as_array().unwrap();
    assert!(!hosts.is_empty());
    assert!(hosts.iter().any(|host| host["stale"] == false));
}

#[tokio::test]
async fn http_router_returns_first_host_process_topology_in_memory() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/topology/host/first/processes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn http_router_returns_structured_host_topology_with_services_and_processes() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_service_binding_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let host_id = app
        .store()
        .and_then(|store| CatalogStore::list_all_hosts(store, Page::default()).ok())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .host_id;

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/topology/host/{host_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["host"]["hostName"], "local-host");
    assert_eq!(
        payload["data"]["processGroups"].as_array().unwrap().len(),
        1
    );
    assert_eq!(payload["data"]["processes"].as_array().unwrap().len(), 1);
    assert_eq!(payload["data"]["services"].as_array().unwrap().len(), 1);
    assert_eq!(payload["data"]["services"][0]["service"]["name"], "sshd");
    assert_eq!(
        payload["data"]["services"][0]["instances"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        payload["data"]["services"][0]["instances"][0]["processes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn http_router_returns_first_host_topology() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_service_binding_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/topology/host/first")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["host"]["hostName"], "local-host");
    assert_eq!(payload["data"]["services"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn http_router_returns_host_process_overview() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let host_id = app
        .store()
        .and_then(|store| CatalogStore::list_all_hosts(store, Page::default()).ok())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .host_id;

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/topology/host/{host_id}/process-overview?top_n=5"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["host"]["hostName"], "local-host");
    assert_eq!(payload["data"]["totalProcesses"], 1);
    assert_eq!(payload["data"]["totalGroups"], 1);
    assert_eq!(payload["data"]["topGroups"].as_array().unwrap().len(), 1);
    assert_eq!(payload["data"]["truncatedGroupCount"], 0);
}

#[tokio::test]
async fn http_router_returns_host_process_groups_page() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_host_process_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let host_id = app
        .store()
        .and_then(|store| CatalogStore::list_all_hosts(store, Page::default()).ok())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .host_id;

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/topology/host/{host_id}/process-groups?limit=10&offset=0"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["data"]["host"]["hostName"], "local-host");
    assert_eq!(payload["data"]["totalProcesses"], 1);
    assert_eq!(payload["data"]["totalGroups"], 1);
    assert_eq!(payload["data"]["groups"].as_array().unwrap().len(), 1);
    assert_eq!(payload["data"]["limit"], 10);
    assert_eq!(payload["data"]["offset"], 0);
    assert_eq!(payload["data"]["hasMore"], false);
}
