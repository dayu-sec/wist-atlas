use atlas_api::TopologyQueryService;
use atlas_storage::{AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, Page};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use uuid::Uuid;

use crate::TopologyMonolith;
use crate::cli::parse_uuid_arg;
use crate::graph::{HostProcessTopologyGraph, build_host_process_topology_graph_async};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VisualizationEnvelope {
    pub(crate) status: &'static str,
    pub(crate) data: HostProcessTopologyGraph,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpEnvelope {
    status: &'static str,
    data: HostTopologyHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpEnvelope {
    status: &'static str,
    data: HostProcessOverviewHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpEnvelope {
    status: &'static str,
    data: HostProcessGroupsPageHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpView {
    host: HostTopologyHostDto,
    latest_runtime: Option<atlas_domain::HostRuntimeState>,
    process_groups: Vec<atlas_domain::HostProcessGroupView>,
    processes: Vec<atlas_domain::ProcessRuntimeState>,
    network_segments: Vec<atlas_domain::NetworkSegment>,
    network_assocs: Vec<atlas_domain::HostNetAssoc>,
    services: Vec<atlas_domain::HostServiceView>,
    assignments: Vec<atlas_domain::ResponsibilityAssignment>,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    top_groups: Vec<atlas_domain::HostProcessGroupView>,
    truncated_group_count: usize,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    groups: Vec<atlas_domain::HostProcessGroupView>,
    limit: usize,
    offset: usize,
    has_more: bool,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHostDto {
    id: Uuid,
    host_name: String,
    machine_id: Option<String>,
    os_name: Option<String>,
    os_version: Option<String>,
}

#[derive(Clone)]
pub(crate) struct HttpAppState {
    app: TopologyMonolith,
}

impl HttpAppState {
    pub(crate) fn new(app: TopologyMonolith) -> Self {
        Self { app }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HostProcessOverviewQuery {
    top_n: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HostProcessGroupsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn healthz() -> &'static str {
    "ok"
}

async fn get_host_process_topology(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_topology_response(store.clone(), Some(host_id)).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_topology_response(store.clone(), Some(host_id)).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_topology_response(store.clone(), Some(host_id)).await
        }
    }
}

async fn get_first_host_process_topology(State(state): State<HttpAppState>) -> Response {
    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_topology_response(store.clone(), None).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_topology_response(store.clone(), None).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_topology_response(store.clone(), None).await
        }
    }
}

async fn get_host_topology(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };

    match &state.app {
        TopologyMonolith::Memory(store) => host_topology_response(store.clone(), host_id).await,
        TopologyMonolith::PostgresMock(store) => {
            host_topology_response(store.clone(), host_id).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_topology_response(store.clone(), host_id).await
        }
    }
}

async fn get_first_host_topology(State(state): State<HttpAppState>) -> Response {
    match &state.app {
        TopologyMonolith::Memory(store) => first_host_topology_response(store.clone()).await,
        TopologyMonolith::PostgresMock(store) => first_host_topology_response(store.clone()).await,
        TopologyMonolith::PostgresLive(store) => first_host_topology_response(store.clone()).await,
    }
}

async fn get_host_process_overview(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
    Query(query): Query<HostProcessOverviewQuery>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    let top_n = query.top_n.unwrap_or(12).clamp(1, 200);

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_overview_response(store.clone(), host_id, top_n).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_overview_response(store.clone(), host_id, top_n).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_overview_response(store.clone(), host_id, top_n).await
        }
    }
}

async fn get_host_process_groups_page(
    State(state): State<HttpAppState>,
    AxumPath(host_id): AxumPath<String>,
    Query(query): Query<HostProcessGroupsQuery>,
) -> Response {
    let host_id = match parse_uuid_arg(&host_id) {
        Ok(host_id) => host_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_process_groups_page_response(store.clone(), host_id, offset, limit).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_process_groups_page_response(store.clone(), host_id, offset, limit).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_process_groups_page_response(store.clone(), host_id, offset, limit).await
        }
    }
}

async fn host_topology_response<S>(store: S, host_id: Uuid) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query.host_topology_view_async(host_id).await {
        Ok(Some(view)) => {
            let payload = HostTopologyHttpEnvelope {
                status: "ok",
                data: HostTopologyHttpView {
                    host: HostTopologyHostDto {
                        id: view.host.host_id,
                        host_name: view.host.host_name,
                        machine_id: view.host.machine_id,
                        os_name: view.host.os_name,
                        os_version: view.host.os_version,
                    },
                    latest_runtime: view.latest_runtime,
                    process_groups: view.process_groups,
                    processes: view.processes,
                    network_segments: view.network_segments,
                    network_assocs: view.network_assocs,
                    services: view.services,
                    assignments: view.assignments,
                    generated_at: view.generated_at,
                },
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("host {host_id} not found")),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn first_host_topology_response<S>(store: S) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let hosts = match AsyncCatalogStore::list_all_hosts(&store, Page::default()).await {
        Ok(hosts) => hosts,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };
    let Some(host) = hosts.into_iter().next() else {
        return json_error(StatusCode::NOT_FOUND, "no host found".to_string());
    };
    host_topology_response(store, host.host_id).await
}

async fn host_process_overview_response<S>(store: S, host_id: Uuid, top_n: usize) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query.host_process_overview_view_async(host_id, top_n).await {
        Ok(Some(view)) => {
            let payload = HostProcessOverviewHttpEnvelope {
                status: "ok",
                data: HostProcessOverviewHttpView {
                    host: HostTopologyHostDto {
                        id: view.host.host_id,
                        host_name: view.host.host_name,
                        machine_id: view.host.machine_id,
                        os_name: view.host.os_name,
                        os_version: view.host.os_version,
                    },
                    total_processes: view.total_processes,
                    total_groups: view.total_groups,
                    top_groups: view.top_groups,
                    truncated_group_count: view.truncated_group_count,
                    generated_at: view.generated_at,
                },
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("host {host_id} not found")),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn host_process_groups_page_response<S>(
    store: S,
    host_id: Uuid,
    offset: usize,
    limit: usize,
) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query
        .host_process_groups_page_view_async(host_id, offset, limit)
        .await
    {
        Ok(Some(view)) => {
            let payload = HostProcessGroupsPageHttpEnvelope {
                status: "ok",
                data: HostProcessGroupsPageHttpView {
                    host: HostTopologyHostDto {
                        id: view.host.host_id,
                        host_name: view.host.host_name,
                        machine_id: view.host.machine_id,
                        os_name: view.host.os_name,
                        os_version: view.host.os_version,
                    },
                    total_processes: view.total_processes,
                    total_groups: view.total_groups,
                    groups: view.groups,
                    limit: view.limit,
                    offset: view.offset,
                    has_more: view.has_more,
                    generated_at: view.generated_at,
                },
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("host {host_id} not found")),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn host_process_topology_response<S>(store: S, focus_host_id: Option<Uuid>) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    match build_host_process_topology_graph_async(store, focus_host_id).await {
        Ok(graph) => {
            let payload = VisualizationEnvelope {
                status: "ok",
                data: graph,
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(err) => json_error(StatusCode::NOT_FOUND, err.to_string()),
    }
}

fn json_error(status: StatusCode, message: String) -> Response {
    (
        status,
        Json(serde_json::json!({
            "status": "error",
            "code": status.as_u16().to_string(),
            "message": message,
        })),
    )
        .into_response()
}

pub(crate) fn build_http_router(state: HttpAppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/topology/host/first", get(get_first_host_topology))
        .route("/api/topology/host/{id}", get(get_host_topology))
        .route(
            "/api/topology/host/{id}/process-overview",
            get(get_host_process_overview),
        )
        .route(
            "/api/topology/host/{id}/process-groups",
            get(get_host_process_groups_page),
        )
        .route(
            "/api/topology/host/first/processes",
            get(get_first_host_process_topology),
        )
        .route(
            "/api/topology/host/{id}/processes",
            get(get_host_process_topology),
        )
        .with_state(state)
}
