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
async fn service_topology_includes_the_container_holding_the_instance() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_container_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let store = app.store().unwrap();
    let service_id = store
        .list_all_hosts(Page::default())
        .unwrap()
        .into_iter()
        .next()
        .map(|host| host.tenant_id)
        .map(|tenant_id| {
            store
                .list_services(tenant_id, Page::default())
                .unwrap()
                .into_iter()
                .find(|service| service.name == "nginx")
                .unwrap()
                .service_id
        })
        .unwrap();

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
    let nodes = payload["data"]["nodes"].as_array().unwrap();
    let edges = payload["data"]["edges"].as_array().unwrap();

    let container_nodes: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["objectKind"] == "ContainerRuntime")
        .collect();
    assert_eq!(container_nodes.len(), 1);
    assert_eq!(container_nodes[0]["label"], "nginx");

    let container_edges: Vec<&Value> = edges
        .iter()
        .filter(|edge| edge["edgeKind"] == "container_service_assoc")
        .collect();
    assert_eq!(container_edges.len(), 1);
    let instance_node = nodes
        .iter()
        .find(|node| node["id"] == container_edges[0]["target"])
        .unwrap();
    assert_eq!(instance_node["objectKind"], "ServiceInstance");
}

#[tokio::test]
async fn http_router_draws_the_container_layer_with_service_edges() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_container_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let host_id = app
        .store()
        .unwrap()
        .list_all_hosts(Page::default())
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .host_id;

    let router = build_http_router(HttpAppState::new(app));
    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/topology/host/{host_id}/processes"))
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
    let nodes = payload["data"]["nodes"].as_array().unwrap();
    let edges = payload["data"]["edges"].as_array().unwrap();

    let container_nodes: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["objectKind"] == "ContainerRuntime")
        .collect();
    assert_eq!(container_nodes.len(), 2);
    assert_eq!(container_nodes[0]["label"], "nginx");
    assert_eq!(
        container_nodes[0]["properties"]["runtimeType"],
        "Containerd"
    );
    assert_eq!(container_nodes[0]["properties"]["imageRef"], "nginx:1.27");

    // 容器挂在主机下
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge["edgeKind"] == "host_container_assoc")
            .count(),
        2
    );

    // nginx 实例落在 nginx 容器里：端口证据写的 Container 绑定被画成一条边
    let container_edges: Vec<&Value> = edges
        .iter()
        .filter(|edge| edge["edgeKind"] == "container_service_assoc")
        .collect();
    assert_eq!(container_edges.len(), 1);
    assert!(
        container_edges[0]["source"]
            .as_str()
            .unwrap()
            .starts_with("container:")
    );
    let instance_node = nodes
        .iter()
        .find(|node| node["id"] == container_edges[0]["target"])
        .unwrap();
    assert_eq!(instance_node["objectKind"], "ServiceInstance");
}

#[tokio::test]
async fn http_router_exposes_containers_on_the_host_view() {
    let app = TopologyMonolith::new_in_memory();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/atlas_edge_container_sample.jsonl");
    app.replay_jsonl(fixture).unwrap();

    let host_id = app
        .store()
        .unwrap()
        .list_all_hosts(Page::default())
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

    let containers = payload["data"]["containers"].as_array().unwrap();
    assert_eq!(containers.len(), 2);
    // 嵌套的 domain 对象保持 snake_case（与 services / processGroups 一致），
    // 只有 HTTP 自己的 DTO 字段是 camelCase。
    assert_eq!(containers[0]["container_name"], "nginx");
    assert_eq!(containers[0]["runtime_type"], "Containerd");
    assert_eq!(containers[0]["image_ref"], "nginx:1.27");
    assert_eq!(containers[0]["pod_name"], "nginx-abc");
    assert_eq!(containers[1]["container_name"], "redis");
    assert!(containers[1]["pod_name"].is_null());
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

// ── HTTP 接收（DSNAP → atlas）────────────────────────────────

const INGEST_TENANT: &str = "4d3c2b1a-0000-4000-8000-000000000001";

/// 造一行**真形状**的 sink 记录：`body` 是 `ReportDiscoverySnapshot` 的 JSON 字符串，
/// 外面再套 wparse 的 OML 信封。与网关 `agent-discovery.jsonl` 的一行同构。
fn dsnap_sink_line(host_name: &str) -> String {
    let observed_at = "2026-10-26T02:20:30Z";
    let report = serde_json::json!({
        "api_version": "v1",
        "kind": "report_discovery_snapshot",
        "report_id": format!("disc_{host_name}"),
        "agent_id": format!("agent-{host_name}"),
        "instance_id": format!("agent-{host_name}"),
        "snapshot_id": format!("snap-{host_name}"),
        "revision": 1,
        "generated_at": observed_at,
        "report_attempt": 1,
        "report_mode": "full_snapshot",
        "reported_at": observed_at,
        "snapshot": {
            "schema_version": "v1",
            "snapshot_id": format!("snap-{host_name}"),
            "revision": 1,
            "generated_at": observed_at,
            "origins": [],
            "resources": [
                {
                    "resource_id": format!("machine-{host_name}"),
                    "kind": "host",
                    "origin_idx": 0,
                    "attributes": {
                        "host.id": format!("machine-{host_name}"),
                        "host.name": host_name,
                        "os.name": "Ubuntu",
                        "os.type": "linux",
                        "os.version": "6.8.0",
                        "host.arch": "x86_64"
                    },
                    "discovered_at": observed_at,
                    "last_seen_at": observed_at,
                    "health": "healthy",
                    "source": "host"
                }
            ],
            "targets": []
        }
    });

    serde_json::json!({
        "schema": "v1",
        "agent_id": format!("agent-{host_name}"),
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

/// 三字段信封：`{cust_id, cmd_id, content}`（与 `wist-evidence` 同形）。
fn ingest_request(cust_id: &str, content: Value) -> Request<Body> {
    let body = serde_json::json!({
        "cust_id": cust_id,
        "cmd_id": "cmd-discovery-1",
        "content": content,
    });
    Request::builder()
        .method("POST")
        .uri("/internal/atlas")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// 把 sink 行字符串转回 JSON 对象（`content` 里放一个对象）。
fn dsnap_content(host_name: &str) -> Value {
    serde_json::from_str(&dsnap_sink_line(host_name)).unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn http_ingest_accepts_a_dsnap_content_and_writes_the_host() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));

    let response = router
        .clone()
        .oneshot(ingest_request(INGEST_TENANT, dsnap_content("build-01")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = json_body(response).await;
    assert_eq!(payload["status"], "accepted");
    assert_eq!(payload["count"], 1);
    assert_eq!(payload["ingested"], 1);
    assert_eq!(payload["rejected"], 0);
    assert_eq!(payload["hosts"], 1);

    // 走同一个 router 的读接口确认真的落库了（不是只回了个 200）。
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload = json_body(response).await;
    let hosts = payload["data"]["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0]["hostName"], "build-01");
    assert_eq!(hosts[0]["osName"], "Ubuntu");
}

#[tokio::test]
async fn http_ingest_accepts_an_array_in_content() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));
    let content = Value::Array(vec![dsnap_content("build-01"), dsnap_content("build-02")]);

    let response = router
        .oneshot(ingest_request(INGEST_TENANT, content))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = json_body(response).await;
    assert_eq!(payload["count"], 2);
    assert_eq!(payload["ingested"], 2);
    assert_eq!(payload["hosts"], 2);
}

#[tokio::test]
async fn http_ingest_rejects_a_non_uuid_cust_id() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));

    let response = router
        .oneshot(ingest_request("tenant-demo", dsnap_content("build-01")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = json_body(response).await;
    assert_eq!(payload["status"], "error");
    assert!(
        payload["message"].as_str().unwrap().contains("UUID"),
        "{payload}"
    );
}

#[tokio::test]
async fn http_ingest_rejects_a_body_that_is_not_the_three_fields() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));
    // 旧信封（sink 行直接当 body）不再是契约：缺 cust_id / cmd_id / content 就 400。
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/atlas")
                .header("content-type", "application/json")
                .body(Body::from(dsnap_sink_line("build-01")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = json_body(response).await;
    assert!(
        payload["message"]
            .as_str()
            .unwrap()
            .contains("cust_id, cmd_id, content"),
        "{payload}"
    );
}

#[tokio::test]
async fn http_ingest_rejects_an_empty_cmd_id() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));
    let body = serde_json::json!({
        "cust_id": INGEST_TENANT,
        "cmd_id": "   ",
        "content": dsnap_content("build-01"),
    });

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/atlas")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(response).await["message"], "cmd_id must not be empty");
}

#[tokio::test]
async fn http_ingest_reports_a_record_without_body_as_permanent() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));
    // 少了 `body`：数据面这一帧永远修不好，回 400 让它进 rescue 而不是无限重试。
    let record = serde_json::json!({
        "schema": "v1",
        "agent_id": "agent-x",
        "env": "dev"
    });

    let response = router
        .oneshot(ingest_request(INGEST_TENANT, record))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = json_body(response).await;
    assert_eq!(payload["status"], "rejected");
    assert_eq!(payload["ingested"], 0);
    assert_eq!(payload["rejected"], 1);
    assert_eq!(payload["failures"][0]["kind"], "permanent");
    assert_eq!(payload["failures"][0]["index"], 0);
}

#[tokio::test]
async fn http_ingest_rejects_an_empty_content_array() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));

    let response = router
        .oneshot(ingest_request(INGEST_TENANT, Value::Array(Vec::new())))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json_body(response).await["message"]
            .as_str()
            .unwrap()
            .contains("empty content")
    );
}

#[tokio::test]
async fn http_ingest_is_idempotent_for_the_same_snapshot() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));
    let content = dsnap_content("build-01");

    for _ in 0..2 {
        let response = router
            .clone()
            .oneshot(ingest_request(INGEST_TENANT, content.clone()))
            .await
            .unwrap();
        // 重复投递也是「已收到」：判重在库里（`ingest_id` 含 `snap_id`），不在回执上。
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload = json_body(response).await;
    assert_eq!(payload["data"]["hosts"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn healthz_reports_the_write_probe_block_when_enabled() {
    let probe = crate::probe::new_shared();
    let router = build_http_router(
        HttpAppState::new(TopologyMonolith::new_in_memory()).with_probe(probe),
    );

    let response = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = json_body(response).await;
    assert_eq!(payload["status"], "ok");
    // 还没跑过探针：`last_ok` 是 null，而不是假装成功。
    assert!(payload["probe"].is_object(), "{payload}");
    assert_eq!(payload["probe"]["last_ok"], Value::Null);
}

#[tokio::test]
async fn healthz_has_no_probe_block_when_disabled() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));

    let response = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload = json_body(response).await;
    assert!(payload.get("probe").is_none(), "{payload}");
}

#[tokio::test]
async fn host_list_hides_the_probe_host() {
    let router = build_http_router(HttpAppState::new(TopologyMonolith::new_in_memory()));
    let content = Value::Array(vec![
        dsnap_content("build-01"),
        dsnap_content(crate::probe::PROBE_HOST),
    ]);
    let response = router
        .clone()
        .oneshot(ingest_request(INGEST_TENANT, content))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/hosts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload = json_body(response).await;
    let names: Vec<&str> = payload["data"]["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| host["hostName"].as_str().unwrap_or_default())
        .collect();
    // 探针主机是「写入能跑通」的证据，不是资产：不出现在列表里，但数据仍在库里。
    assert_eq!(names, vec!["build-01"], "{payload}");
}
