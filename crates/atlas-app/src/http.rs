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
use crate::graph::{
    HostProcessTopologyGraph, build_host_process_topology_graph_async,
    build_network_topology_graph_async, build_service_topology_graph_async,
};

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
    arch: Option<String>,
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
struct HostListQuery {
    /// 默认不开：退役主机保留行是为了历史，不该出现在「现在有哪几台机」。
    include_stale: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostListHttpEnvelope {
    status: &'static str,
    data: HostListHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostListHttpView {
    hosts: Vec<HostSummaryHttpItem>,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostSummaryHttpItem {
    id: Uuid,
    host_name: String,
    machine_id: Option<String>,
    os_name: Option<String>,
    os_version: Option<String>,
    arch: Option<String>,
    process_count: i64,
    segment_count: usize,
    last_inventory_at: String,
    stale: bool,
    stale_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HostProcessGroupsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ServiceListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListHttpEnvelope {
    status: &'static str,
    data: ServiceListHttpView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListHttpView {
    services: Vec<ServiceSummaryHttpItem>,
    limit: u32,
    offset: u32,
}

/// 服务列表只给规模指标：端点、实例明细都在服务焦点视图里。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceSummaryHttpItem {
    id: Uuid,
    name: String,
    service_type: String,
    boundary: String,
    external_ref: Option<String>,
    instance_count: usize,
    host_count: usize,
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
                        arch: view.host.arch,
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
    // 已退役的主机不能代表「当前这台机」—— 保留行是为了历史，不是为了出现在现役视图里。
    let Some(host) = hosts.into_iter().find(|host| host.stale_at.is_none()) else {
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
                        arch: view.host.arch,
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
                        arch: view.host.arch,
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

async fn get_network_topology(
    State(state): State<HttpAppState>,
    AxumPath(segment_id): AxumPath<String>,
) -> Response {
    let segment_id = match parse_uuid_arg(&segment_id) {
        Ok(segment_id) => segment_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };

    match &state.app {
        TopologyMonolith::Memory(store) => {
            network_topology_response(store.clone(), segment_id).await
        }
        TopologyMonolith::PostgresMock(store) => {
            network_topology_response(store.clone(), segment_id).await
        }
        TopologyMonolith::PostgresLive(store) => {
            network_topology_response(store.clone(), segment_id).await
        }
    }
}

/// 网段焦点图：一个网段 + 连在它上面的主机。
///
/// 与主机焦点图方向相反（那边是「这台机接在哪些网段」），但共用同一套图形状，
/// 前端不需要新的节点类型。
async fn network_topology_response<S>(store: S, segment_id: Uuid) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    match build_network_topology_graph_async(store, segment_id).await {
        Ok(Some(graph)) => {
            let payload = VisualizationEnvelope {
                status: "ok",
                data: graph,
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            format!("network segment {segment_id} not found"),
        ),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn get_service_topology(
    State(state): State<HttpAppState>,
    AxumPath(service_id): AxumPath<String>,
) -> Response {
    let service_id = match parse_uuid_arg(&service_id) {
        Ok(service_id) => service_id,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err.to_string()),
    };

    match &state.app {
        TopologyMonolith::Memory(store) => {
            service_topology_response(store.clone(), service_id).await
        }
        TopologyMonolith::PostgresMock(store) => {
            service_topology_response(store.clone(), service_id).await
        }
        TopologyMonolith::PostgresLive(store) => {
            service_topology_response(store.clone(), service_id).await
        }
    }
}

/// 服务焦点图：一个服务 + 它的实例 + 监听进程 + 所在主机。
///
/// 与主机焦点图方向相反（那边是「这台机上有哪些服务」），但共用同一套节点 id
/// 约定与边类型，前端不需要新的渲染分支。
async fn service_topology_response<S>(store: S, service_id: Uuid) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    match build_service_topology_graph_async(store, service_id).await {
        Ok(Some(graph)) => {
            let payload = VisualizationEnvelope {
                status: "ok",
                data: graph,
            };
            (StatusCode::OK, Json(payload)).into_response()
        }
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            format!("service {service_id} not found"),
        ),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn get_services(
    State(state): State<HttpAppState>,
    Query(query): Query<ServiceListQuery>,
) -> Response {
    // 服务数量是「不同监听程序」的量级（百级），默认一次给足；分页参数保留给大集群。
    let limit = query.limit.unwrap_or(200).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);

    match &state.app {
        TopologyMonolith::Memory(store) => {
            service_list_response(store.clone(), limit, offset).await
        }
        TopologyMonolith::PostgresMock(store) => {
            service_list_response(store.clone(), limit, offset).await
        }
        TopologyMonolith::PostgresLive(store) => {
            service_list_response(store.clone(), limit, offset).await
        }
    }
}

async fn service_list_response<S>(store: S, limit: u32, offset: u32) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    let tenant_id = match query.default_tenant_id_async().await {
        Ok(Some(tenant_id)) => tenant_id,
        // 空库不是错误：前端拿空列表就够了。
        Ok(None) => {
            return (
                StatusCode::OK,
                Json(ServiceListHttpEnvelope {
                    status: "ok",
                    data: ServiceListHttpView {
                        services: Vec::new(),
                        limit,
                        offset,
                    },
                }),
            )
                .into_response();
        }
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };

    match query
        .service_summaries_async(tenant_id, Page { limit, offset })
        .await
    {
        Ok(views) => {
            let services: Vec<ServiceSummaryHttpItem> = views
                .into_iter()
                .map(|view| ServiceSummaryHttpItem {
                    id: view.service.service_id,
                    name: view.service.name,
                    service_type: format!("{:?}", view.service.service_type),
                    boundary: format!("{:?}", view.service.boundary),
                    external_ref: view.service.external_ref,
                    instance_count: view.instance_count,
                    host_count: view.host_count,
                })
                .collect();
            (
                StatusCode::OK,
                Json(ServiceListHttpEnvelope {
                    status: "ok",
                    data: ServiceListHttpView {
                        services,
                        limit,
                        offset,
                    },
                }),
            )
                .into_response()
        }
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
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

async fn get_hosts(
    State(state): State<HttpAppState>,
    Query(query): Query<HostListQuery>,
) -> Response {
    let include_stale = query.include_stale.unwrap_or(false);
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0);

    match &state.app {
        TopologyMonolith::Memory(store) => {
            host_list_response(store.clone(), include_stale, limit, offset).await
        }
        TopologyMonolith::PostgresMock(store) => {
            host_list_response(store.clone(), include_stale, limit, offset).await
        }
        TopologyMonolith::PostgresLive(store) => {
            host_list_response(store.clone(), include_stale, limit, offset).await
        }
    }
}

async fn host_list_response<S>(store: S, include_stale: bool, limit: u32, offset: u32) -> Response
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let query = TopologyQueryService::new(store);
    match query
        .host_summaries_async(Page { limit, offset }, include_stale)
        .await
    {
        Ok(views) => {
            let hosts: Vec<HostSummaryHttpItem> = views
                .into_iter()
                .map(|view| HostSummaryHttpItem {
                    id: view.host.host_id,
                    host_name: view.host.host_name,
                    machine_id: view.host.machine_id,
                    os_name: view.host.os_name,
                    os_version: view.host.os_version,
                    arch: view.host.arch,
                    process_count: view.process_count,
                    segment_count: view.segment_count,
                    last_inventory_at: view.host.last_inventory_at.to_rfc3339(),
                    stale: view.host.stale_at.is_some(),
                    stale_at: view.host.stale_at.map(|at| at.to_rfc3339()),
                })
                .collect();
            (
                StatusCode::OK,
                Json(HostListHttpEnvelope {
                    status: "ok",
                    data: HostListHttpView {
                        hosts,
                        limit,
                        offset,
                    },
                }),
            )
                .into_response()
        }
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

pub(crate) fn build_http_router(state: HttpAppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/hosts", get(get_hosts))
        .route("/api/services", get(get_services))
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
        .route("/api/topology/network/{id}", get(get_network_topology))
        .route("/api/topology/service/{id}", get(get_service_topology))
        .with_state(state)
}
