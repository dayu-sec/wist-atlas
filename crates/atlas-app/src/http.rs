use atlas_api::TopologyQueryService;
use atlas_storage::{AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, Page};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use uuid::Uuid;

use crate::TopologyMonolith;
use crate::cli::parse_uuid_arg;
use crate::graph::{
    HostProcessTopologyGraph, build_host_process_topology_graph_async,
    build_network_topology_graph_async, build_service_topology_graph_async,
};
use crate::http_schema::*;
use crate::ingest_http::{MAX_INGEST_BODY_BYTES, ingest_atlas};
use utoipa::OpenApi as _;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct VisualizationEnvelope {
    #[schema(value_type = String)]
    pub(crate) status: &'static str,
    pub(crate) data: HostProcessTopologyGraph,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpEnvelope {
    #[schema(value_type = String)]
    status: &'static str,
    data: HostTopologyHttpView,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpEnvelope {
    #[schema(value_type = String)]
    status: &'static str,
    data: HostProcessOverviewHttpView,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpEnvelope {
    #[schema(value_type = String)]
    status: &'static str,
    data: HostProcessGroupsPageHttpView,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpView {
    host: HostTopologyHostDto,
    #[schema(value_type = Option<HostRuntimeStateDoc>)]
    latest_runtime: Option<atlas_domain::HostRuntimeState>,
    #[schema(value_type = Vec<HostProcessGroupViewDoc>)]
    process_groups: Vec<atlas_domain::HostProcessGroupView>,
    #[schema(value_type = Vec<ProcessRuntimeStateDoc>)]
    processes: Vec<atlas_domain::ProcessRuntimeState>,
    #[schema(value_type = Vec<NetworkSegmentDoc>)]
    network_segments: Vec<atlas_domain::NetworkSegment>,
    #[schema(value_type = Vec<HostNetAssocDoc>)]
    network_assocs: Vec<atlas_domain::HostNetAssoc>,
    #[schema(value_type = Vec<HostServiceViewDoc>)]
    services: Vec<atlas_domain::HostServiceView>,
    #[schema(value_type = Vec<ContainerRuntimeDoc>)]
    containers: Vec<atlas_domain::ContainerRuntime>,
    #[schema(value_type = Vec<ResponsibilityAssignmentDoc>)]
    assignments: Vec<atlas_domain::ResponsibilityAssignment>,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    #[schema(value_type = Vec<HostProcessGroupViewDoc>)]
    top_groups: Vec<atlas_domain::HostProcessGroupView>,
    truncated_group_count: usize,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    #[schema(value_type = Vec<HostProcessGroupViewDoc>)]
    groups: Vec<atlas_domain::HostProcessGroupView>,
    limit: usize,
    offset: usize,
    has_more: bool,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
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
    /// `Some` = 写探针已启用（由 serve 启动时开）——`/healthz` 多一个 `probe` 块。
    probe: Option<crate::probe::SharedProbe>,
}

impl HttpAppState {
    pub(crate) fn new(app: TopologyMonolith) -> Self {
        Self { app, probe: None }
    }

    /// 打开写探针状态位：`/healthz` 会把最近一次探针结果一并报出去。
    pub(crate) fn with_probe(mut self, probe: crate::probe::SharedProbe) -> Self {
        self.probe = Some(probe);
        self
    }

    pub(crate) fn app(&self) -> &TopologyMonolith {
        &self.app
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct HostProcessOverviewQuery {
    top_n: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct HostListQuery {
    /// 默认不开：退役主机保留行是为了历史，不该出现在「现在有哪几台机」。
    include_stale: Option<bool>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostListHttpEnvelope {
    #[schema(value_type = String)]
    status: &'static str,
    data: HostListHttpView,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostListHttpView {
    hosts: Vec<HostSummaryHttpItem>,
    limit: u32,
    offset: u32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
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

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct HostProcessGroupsQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct ServiceListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ServiceListHttpEnvelope {
    #[schema(value_type = String)]
    status: &'static str,
    data: ServiceListHttpView,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ServiceListHttpView {
    services: Vec<ServiceSummaryHttpItem>,
    limit: u32,
    offset: u32,
}

/// 服务列表只给规模指标：端点、实例明细都在服务焦点视图里。
#[derive(Debug, Clone, Serialize, ToSchema)]
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

async fn healthz(State(state): State<HttpAppState>) -> Json<serde_json::Value> {
    let mut body = serde_json::json!({ "status": "ok" });
    // 开了写探针就把最近一次结果带出去：上游能区分「进程活着」与「真写得进去」。
    // 字段形状与 `wist-evidence` 的 `probe` 块保持一致，星象台的接口可用性页只认这一份。
    if let Some(probe) = state.probe.as_ref()
        && let Ok(status) = probe.read()
        && let Ok(value) = serde_json::to_value(&*status)
    {
        body["probe"] = value;
    }
    Json(body)
}

/// 主机进程拓扑图（节点 + 边）。
#[utoipa::path(
    get,
    path = "/api/topology/host/{id}/processes",
    tag = "topology",
    summary = "主机进程拓扑图",
    params(("id" = String, Path, description = "主机 UUID")),
    responses(
        (status = 200, description = "拓扑图", body = VisualizationEnvelope),
        (status = 400, description = "id 不是合法 UUID"),
        (status = 404, description = "主机不存在"),
    )
)]
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

/// 第一台主机的进程拓扑图（节点 + 边）。
#[utoipa::path(
    get,
    path = "/api/topology/host/first/processes",
    tag = "topology",
    summary = "第一台主机的进程拓扑图",
    responses(
        (status = 200, description = "拓扑图", body = VisualizationEnvelope),
        (status = 404, description = "还没有任何主机"),
    )
)]
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

/// 主机焦点视图：一台机器的运行时状态 / 进程 / 网段 / 服务 / 容器 / 责任归属。
#[utoipa::path(
    get,
    path = "/api/topology/host/{id}",
    tag = "topology",
    summary = "主机焦点视图",
    params(("id" = String, Path, description = "主机 UUID")),
    responses(
        (status = 200, description = "主机焦点视图", body = HostTopologyHttpEnvelope),
        (status = 400, description = "id 不是合法 UUID"),
        (status = 404, description = "主机不存在或已物化失败"),
    )
)]
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

/// 第一台主机的焦点视图：给画布首屏用，省一次「先列主机再取详情」。
#[utoipa::path(
    get,
    path = "/api/topology/host/first",
    tag = "topology",
    summary = "第一台主机的焦点视图",
    responses(
        (status = 200, description = "主机焦点视图", body = HostTopologyHttpEnvelope),
        (status = 404, description = "还没有任何主机"),
    )
)]
async fn get_first_host_topology(State(state): State<HttpAppState>) -> Response {
    match &state.app {
        TopologyMonolith::Memory(store) => first_host_topology_response(store.clone()).await,
        TopologyMonolith::PostgresMock(store) => first_host_topology_response(store.clone()).await,
        TopologyMonolith::PostgresLive(store) => first_host_topology_response(store.clone()).await,
    }
}

/// 进程概览：只给前 top_n 个进程组与总数，给画布首屏用。
#[utoipa::path(
    get,
    path = "/api/topology/host/{id}/process-overview",
    tag = "topology",
    summary = "主机进程概览（截断）",
    params(
        ("id" = String, Path, description = "主机 UUID"),
        HostProcessOverviewQuery,
    ),
    responses(
        (status = 200, description = "进程概览", body = HostProcessOverviewHttpEnvelope),
        (status = 400, description = "id 不是合法 UUID"),
    )
)]
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

/// 进程组分页：与 `process-overview` 同一份数据，多了 `limit` / `offset`。
#[utoipa::path(
    get,
    path = "/api/topology/host/{id}/process-groups",
    tag = "topology",
    summary = "主机进程组分页",
    params(
        ("id" = String, Path, description = "主机 UUID"),
        HostProcessGroupsQuery,
    ),
    responses(
        (status = 200, description = "进程组分页", body = HostProcessGroupsPageHttpEnvelope),
        (status = 400, description = "id 不是合法 UUID"),
    )
)]
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
                    containers: view.containers,
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

/// 网段拓扑图：段 + 主机 + 归属关系。
#[utoipa::path(
    get,
    path = "/api/topology/network/{id}",
    tag = "topology",
    summary = "网段拓扑图",
    params(("id" = String, Path, description = "网段 UUID")),
    responses(
        (status = 200, description = "拓扑图", body = VisualizationEnvelope),
        (status = 400, description = "id 不是合法 UUID"),
        (status = 404, description = "网段不存在"),
    )
)]
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

/// 服务拓扑图：实例 + 主机 + 归属关系。
#[utoipa::path(
    get,
    path = "/api/topology/service/{id}",
    tag = "topology",
    summary = "服务拓扑图",
    params(("id" = String, Path, description = "服务 UUID")),
    responses(
        (status = 200, description = "拓扑图", body = VisualizationEnvelope),
        (status = 400, description = "id 不是合法 UUID"),
        (status = 404, description = "服务不存在"),
    )
)]
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

/// 服务列表（分页）：只给规模指标，端点 / 实例明细在服务焦点视图里。
#[utoipa::path(
    get,
    path = "/api/services",
    tag = "topology",
    summary = "服务列表（分页）",
    params(ServiceListQuery),
    responses((status = 200, description = "服务列表", body = ServiceListHttpEnvelope))
)]
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

/// 主机列表（分页）：「现在有哪几台机」，写探针主机已剔除。
#[utoipa::path(
    get,
    path = "/api/hosts",
    tag = "topology",
    summary = "主机列表（分页）",
    params(HostListQuery),
    responses(
        (status = 200, description = "主机列表", body = HostListHttpEnvelope),
    )
)]
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
                // 写探针的主机不进展示：它不是资产，只是「写入能跑通」的证据。
                // 与 evidence 把 `_probe` 从列表里藏起来是同一条纪律。
                .filter(|view| view.host.host_name != crate::probe::PROBE_HOST)
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
    // 读接口全部走 `routes!`：路径只有一份源（handler 上的 `#[utoipa::path]`），
    // Router 与 spec 不会各写一份。一个 `.routes()` 只对应**一条路径**
    // （utoipa-axum 把同一路径上的多个方法合成一个 MethodRouter），所以逐个写。
    let (router, _documented) = documented_routes()
        .route("/healthz", get(healthz))
        // 数据面写入入口：`{cust_id, content}`（证据服务那边多一个 cmd_id，是它的幂等键）。
        // body 上限单独放宽（axum 默认 2 MB，放不下 0.9 MB 的快照加上多进程主机）。
        // 它不是对外接口，**有意**不进 spec。
        .route(
            "/internal/atlas",
            post(ingest_atlas).layer(DefaultBodyLimit::max(MAX_INGEST_BODY_BYTES)),
        )
        .split_for_parts();

    let openapi = openapi_spec();

    router
        // 机器可读的那份：前端 / 网关 / 测试都直接拿它，不要去 grep 文档。
        .route(
            "/api/openapi.json",
            get(move || {
                let openapi = openapi.clone();
                async move { Json(openapi) }
            }),
        )
        // 人看的那份：自包含 HTML（不引 CDN，计算区可能没有外网）。
        .route("/api/docs", get(openapi_docs))
        .with_state(state)
}

/// `GET /api/docs` —— 一个不依赖外网的 spec 阅读页。
///
/// 为什么不引 Scalar / Swagger UI：它们从 CDN 拉脚本，计算区常常没有外网；
/// 这一页只需要 spec 本身，先把「接口清单可读」这件事做好。
async fn openapi_docs() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        DOCS_HTML,
    )
}

const DOCS_HTML: &str = include_str!("openapi_docs.html");

/// 所有**对外**读接口（带 `#[utoipa::path]` 标注）。
///
/// 只在一个地方列：Router 和 spec 都从这里来。新增接口时只加一行，两边同时生效。
fn documented_routes() -> OpenApiRouter<HttpAppState> {
    OpenApiRouter::new()
        .routes(routes!(get_hosts))
        .routes(routes!(get_services))
        .routes(routes!(get_first_host_topology))
        .routes(routes!(get_host_topology))
        .routes(routes!(get_host_process_overview))
        .routes(routes!(get_host_process_groups_page))
        .routes(routes!(get_first_host_process_topology))
        .routes(routes!(get_host_process_topology))
        .routes(routes!(get_network_topology))
        .routes(routes!(get_service_topology))
}

/// 完整 spec：路径来自 `routes!`（标注了就在），描述与复用 schema 来自 `ApiDoc`。
fn openapi_spec() -> utoipa::openapi::OpenApi {
    let (_, documented) = documented_routes().split_for_parts();
    let mut openapi = ApiDoc::openapi();
    openapi.paths.paths.extend(documented.paths.paths);
    if let Some(components) = documented.components {
        match openapi.components.as_mut() {
            Some(current) => current.schemas.extend(components.schemas),
            None => openapi.components = Some(components),
        }
    }
    openapi
}

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "wist-atlas 只读 API",
        version = env!("CARGO_PKG_VERSION"),
        description = "资产与运行拓扑的只读接口（写入口 `/internal/atlas` 不在本文档里）。\n\n这份 spec 由 handler 标注与 DTO 的 `ToSchema` 在**编译期**生成：字段名跟 serde 走，接口路径跟 Router 走。"
    ),
    tags(
        (name = "topology", description = "资产与运行拓扑（只读）")
    ),
    components(schemas(
        HostListHttpEnvelope,
        HostListHttpView,
        HostSummaryHttpItem,
        HostTopologyHttpEnvelope,
        HostTopologyHttpView,
        HostTopologyHostDto,
        HostProcessOverviewHttpEnvelope,
        HostProcessOverviewHttpView,
        HostProcessGroupsPageHttpEnvelope,
        HostProcessGroupsPageHttpView,
        ServiceListHttpEnvelope,
        ServiceListHttpView,
        ServiceSummaryHttpItem,
        VisualizationEnvelope,
        crate::graph::HostProcessTopologyGraph,
        crate::graph::HostProcessTopologyMetadata,
        ContainerRuntimeDoc,
        HostNetAssocDoc,
        HostProcessGroupViewDoc,
        HostRuntimeStateDoc,
        HostServiceViewDoc,
        NetworkSegmentDoc,
        ProcessRuntimeStateDoc,
        ResponsibilityAssignmentDoc,
        AgentHealthDoc,
        AddressFamilyDoc,
        BindingScopeDoc,
        ConfidenceDoc,
        HostServiceInstanceViewDoc,
        InstanceEndpointDoc,
        ObjectKindDoc,
        ProcessStateCountDoc,
        ResponsibilityRoleDoc,
        RuntimeBindingDoc,
        RuntimeObjectTypeDoc,
        RuntimeTypeDoc,
        ServiceBoundaryDoc,
        ServiceEntityDoc,
        ServiceInstanceDoc,
        ServiceTypeDoc,
        ValidityWindowDoc,
    ))
)]
struct ApiDoc;

#[cfg(test)]
mod openapi_tests {
    use super::*;
    use atlas_storage::{CatalogStore, Page};
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::util::ServiceExt;

    /// 对外读接口清单：少一个就是忘进 `documented_routes()`。
    const DOCUMENTED_PATHS: &[&str] = &[
        "/api/hosts",
        "/api/services",
        "/api/topology/host/first",
        "/api/topology/host/{id}",
        "/api/topology/host/{id}/process-overview",
        "/api/topology/host/{id}/process-groups",
        "/api/topology/host/first/processes",
        "/api/topology/host/{id}/processes",
        "/api/topology/network/{id}",
        "/api/topology/service/{id}",
    ];

    /// 路径由 `routes!` 收集，不是手抄的字符串：新增一个带 `#[utoipa::path]` 的 handler
    /// 忘了进 `documented_routes()` 时这里会红。
    #[test]
    fn spec_documents_every_read_route() {
        let spec = openapi_spec();
        for path in DOCUMENTED_PATHS {
            assert!(spec.paths.paths.contains_key(*path), "spec 里没有 {path}");
        }
        assert_eq!(spec.info.title, "wist-atlas 只读 API");
        // 字段名跟 serde 走（`camelCase`），不是手写的 `snake_case`。
        let schemas = serde_json::to_value(&spec.components.as_ref().expect("components").schemas)
            .expect("schemas to json");
        let props = schemas["HostSummaryHttpItem"]["properties"]
            .as_object()
            .expect("properties");
        assert!(props.contains_key("hostName"), "{props:?}");
        assert!(props.contains_key("lastInventoryAt"), "{props:?}");
    }

    /// 写入口是内部接口，不应该被当成对外文档暴露。
    #[test]
    fn write_endpoint_is_not_documented() {
        let spec = openapi_spec();
        assert!(!spec.paths.paths.contains_key("/internal/atlas"));
        assert!(!spec.paths.paths.contains_key("/healthz"));
    }

    /// spec 里每个 `$ref` 都要能解析 —— 漏注册一个 schema 就会留下悬空引用。
    #[test]
    fn every_ref_resolves() {
        let spec = openapi_spec();
        let value = serde_json::to_value(&spec).expect("spec to json");
        let schemas = value["components"]["schemas"].clone();
        let mut missing = Vec::new();
        collect_refs(&value, &schemas, &mut missing);
        assert!(missing.is_empty(), "悬空 $ref：{missing:?}");
    }

    fn collect_refs(value: &Value, schemas: &Value, missing: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                    let name = reference.rsplit('/').next().unwrap_or_default();
                    if schemas.get(name).is_none() {
                        missing.push(reference.to_string());
                    }
                }
                for child in map.values() {
                    collect_refs(child, schemas, missing);
                }
            }
            Value::Array(items) => {
                for item in items {
                    collect_refs(item, schemas, missing);
                }
            }
            _ => {}
        }
    }

    /// 真实响应 vs spec：多一个字段、少一个必填字段、类型不对，都算文档说错了。
    ///
    /// 这是「文档 DTO」的防腐层：`http_schema.rs` 里那些镜像结构体没有编译期的
    /// 绑定，只能靠这一层把漂移钉住。
    #[tokio::test]
    async fn responses_match_the_spec() {
        let app = TopologyMonolith::new_in_memory();
        for fixture in [
            "atlas_edge_service_binding_sample.jsonl",
            "atlas_edge_host_process_sample.jsonl",
            "atlas_edge_network_hosts_sample.jsonl",
            "atlas_edge_container_sample.jsonl",
            "atlas_telemetry_host_sample.jsonl",
            "atlas_telemetry_process_sample.jsonl",
        ] {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(fixture);
            app.replay_jsonl(path).expect("fixture replays");
        }

        let (host_ids, tenants) = {
            let store = app.store().expect("memory store");
            let hosts = CatalogStore::list_all_hosts(store, Page::default()).expect("hosts");
            let tenants: Vec<_> = {
                let mut seen = Vec::new();
                for host in &hosts {
                    if !seen.contains(&host.tenant_id) {
                        seen.push(host.tenant_id);
                    }
                }
                seen
            };
            let ids: Vec<_> = hosts.iter().map(|host| host.host_id).collect();
            assert!(!ids.is_empty(), "fixtures 里应该有主机");
            (ids, tenants)
        };
        // 几个 fixture 不一定同一个租户：逐个找，只要有就跑。
        let (segment_id, service_id) = {
            let store = app.store().expect("memory store");
            let segment = tenants
                .iter()
                .find_map(|tenant| {
                    CatalogStore::list_network_segments(store, *tenant, Page::default())
                        .ok()
                        .and_then(|segments| segments.first().map(|s| s.network_segment_id))
                })
                .expect("a segment in the fixtures");
            let service = tenants
                .iter()
                .find_map(|tenant| {
                    CatalogStore::list_services(store, *tenant, Page::default())
                        .ok()
                        .and_then(|services| services.first().map(|s| s.service_id))
                })
                .expect("a service in the fixtures");
            (segment, service)
        };

        let router = build_http_router(HttpAppState::new(app));
        let spec = openapi_spec();
        let spec_json = serde_json::to_value(&spec).expect("spec to json");
        let schemas = spec_json["components"]["schemas"].clone();

        let mut requests: Vec<(String, &str)> = vec![
            ("/api/hosts".to_string(), "/api/hosts"),
            ("/api/services".to_string(), "/api/services"),
            (
                "/api/topology/host/first".to_string(),
                "/api/topology/host/first",
            ),
            (
                "/api/topology/host/first/processes".to_string(),
                "/api/topology/host/first/processes",
            ),
            (
                format!("/api/topology/network/{segment_id}"),
                "/api/topology/network/{id}",
            ),
            (
                format!("/api/topology/service/{service_id}"),
                "/api/topology/service/{id}",
            ),
        ];
        // 每台主机都跑一遍：不同 fixture 覆盖不同的嵌套（运行时 / 容器 / 服务绑定）。
        for host_id in &host_ids {
            requests.push((
                format!("/api/topology/host/{host_id}"),
                "/api/topology/host/{id}",
            ));
            requests.push((
                format!("/api/topology/host/{host_id}/process-overview?top_n=5"),
                "/api/topology/host/{id}/process-overview",
            ));
            requests.push((
                format!("/api/topology/host/{host_id}/process-groups?limit=5"),
                "/api/topology/host/{id}/process-groups",
            ));
            requests.push((
                format!("/api/topology/host/{host_id}/processes"),
                "/api/topology/host/{id}/processes",
            ));
        }

        let mut saw_runtime = false;
        let mut saw_processes = false;
        for (uri, template) in requests {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let payload: Value = serde_json::from_slice(&body).expect("json body");
            let schema = &spec_json["paths"][template]["get"]["responses"]["200"]["content"]["application/json"]
                ["schema"];
            let mut errors = Vec::new();
            validate(&payload, schema, &schemas, template, &mut errors);
            assert!(errors.is_empty(), "{template} 与响应不一致：{errors:#?}");

            // 深度视图不能是空壳 —— 否则这个测试看着绿、实际没验到镜像类型。
            if template == "/api/topology/host/{id}" {
                let data = &payload["data"];
                saw_runtime |= !data["latestRuntime"].is_null();
                saw_processes |= data["processes"]
                    .as_array()
                    .is_some_and(|processes| !processes.is_empty());
            }
        }

        assert!(
            saw_runtime,
            "没有任何一台主机带 latestRuntime，嵌套 schema 没被验到"
        );
        assert!(
            saw_processes,
            "没有任何一台主机带进程，嵌套 schema 没被验到"
        );
    }

    /// 极简校验器：只看「响应的字段 spec 里有没有、类型对不对」。
    /// 不实现完整 JSON Schema —— 这里要的是漂移告警，不是通用验证器。
    fn validate(
        value: &Value,
        schema: &Value,
        schemas: &Value,
        at: &str,
        errors: &mut Vec<String>,
    ) {
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            let name = reference.rsplit('/').next().unwrap_or_default();
            match schemas.get(name) {
                Some(target) => validate(value, target, schemas, at, errors),
                None => errors.push(format!("{at}: $ref {reference} 解析不到")),
            }
            return;
        }
        if let Some(options) = schema
            .get("oneOf")
            .or_else(|| schema.get("anyOf"))
            .and_then(Value::as_array)
        {
            let mut branch_errors = Vec::new();
            for option in options {
                let mut candidate = Vec::new();
                validate(value, option, schemas, at, &mut candidate);
                if candidate.is_empty() {
                    return;
                }
                branch_errors.push(candidate);
            }
            errors.push(format!("{at}: 不匹配 anyOf/oneOf（{branch_errors:?}）"));
            return;
        }
        let types: Vec<&str> = match schema.get("type") {
            Some(Value::String(kind)) => vec![kind.as_str()],
            Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if value.is_null() {
            if !types.is_empty() && !types.contains(&"null") {
                errors.push(format!("{at}: 值为 null，但 spec 没标可空"));
            }
            return;
        }
        if types.is_empty() {
            return;
        }
        if types.contains(&"object") {
            let Some(map) = value.as_object() else {
                errors.push(format!("{at}: 不是对象"));
                return;
            };
            let properties = schema.get("properties").and_then(Value::as_object);
            for (key, child) in map {
                match properties.and_then(|props| props.get(key)) {
                    Some(child_schema) => {
                        validate(child, child_schema, schemas, &format!("{at}.{key}"), errors)
                    }
                    None => {
                        // `properties` 为空 = 自由形状（如 `Object` 占位），不比对。
                        if properties.is_some() {
                            errors.push(format!("{at}.{key}: 响应里有、spec 里没有"));
                        }
                    }
                }
            }
            for required in schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !map.contains_key(required) {
                    errors.push(format!("{at}.{required}: spec 标了必填、响应里没有"));
                }
            }
            return;
        }
        if types.contains(&"array") {
            let Some(items) = value.as_array() else {
                errors.push(format!("{at}: 不是数组"));
                return;
            };
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter().enumerate() {
                    validate(
                        item,
                        item_schema,
                        schemas,
                        &format!("{at}[{index}]"),
                        errors,
                    );
                }
            }
            return;
        }
        let matched = types.iter().any(|kind| match *kind {
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => {
                value.as_i64().is_some()
                    || value.as_u64().is_some()
                    || value.as_f64().is_some_and(|number| number.fract() == 0.0)
            }
            "number" => value.is_number(),
            _ => true,
        });
        if !matched {
            errors.push(format!("{at}: {value} 不是 {types:?}"));
        }
    }
}
