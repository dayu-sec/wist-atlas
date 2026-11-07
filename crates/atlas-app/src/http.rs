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
use crate::ingest_http::{MAX_INGEST_BODY_BYTES, ingest_atlas};
use utoipa::OpenApi as _;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VisualizationEnvelope {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessGroupsPageHttpEnvelope {
    status: &'static str,
    data: HostProcessGroupsPageHttpView,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostTopologyHttpView {
    host: HostTopologyHostDto,
    // 域类型直接引用：给它们加 `ToSchema` 会把 schema 关注点漏到 domain 层，
    // P0 先以 `Object` 占位（字段名仍由 serde 决定），后续再逐个展开。
    #[schema(value_type = Option<Object>)]
    latest_runtime: Option<atlas_domain::HostRuntimeState>,
    #[schema(value_type = Vec<Object>)]
    process_groups: Vec<atlas_domain::HostProcessGroupView>,
    #[schema(value_type = Vec<Object>)]
    processes: Vec<atlas_domain::ProcessRuntimeState>,
    #[schema(value_type = Vec<Object>)]
    network_segments: Vec<atlas_domain::NetworkSegment>,
    #[schema(value_type = Vec<Object>)]
    network_assocs: Vec<atlas_domain::HostNetAssoc>,
    #[schema(value_type = Vec<Object>)]
    services: Vec<atlas_domain::HostServiceView>,
    #[schema(value_type = Vec<Object>)]
    containers: Vec<atlas_domain::ContainerRuntime>,
    #[schema(value_type = Vec<Object>)]
    assignments: Vec<atlas_domain::ResponsibilityAssignment>,
    generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessOverviewHttpView {
    host: HostTopologyHostDto,
    total_processes: usize,
    total_groups: usize,
    #[schema(value_type = Vec<Object>)]
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
    // 有 `#[utoipa::path]` 标注的接口走 `routes!`：路径只有一份源（标注里那份），
    // 不会出现「文档写了 /api/hosts、Router 里是 /api/host」这种漂移。
    let (router, _documented) = OpenApiRouter::new()
        // 一个 `.routes()` 只对应**一条路径**（utoipa-axum 把同一路径上的多个方法
        // 合成一个 MethodRouter）；不同路径要分开写。
        .routes(routes!(get_hosts))
        .routes(routes!(get_host_topology))
        .routes(routes!(get_host_process_overview))
        .route("/healthz", get(healthz))
        // 数据面写入入口：`{cust_id, content}`（证据服务那边多一个 cmd_id，是它的幂等键）。
        // body 上限单独放宽（axum 默认 2 MB，放不下 0.9 MB 的快照加上多进程主机）。
        .route(
            "/internal/atlas",
            post(ingest_atlas).layer(DefaultBodyLimit::max(MAX_INGEST_BODY_BYTES)),
        )
        .route("/api/services", get(get_services))
        .route("/api/topology/host/first", get(get_first_host_topology))
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

/// 完整 spec：路径与描述各管一半。
///
/// - 路径来自 `routes!`：handler 上有 `#[utoipa::path]` 就会自动进来，忘不了；
/// - info / tags / 复用 schema 来自 `ApiDoc`。
fn openapi_spec() -> utoipa::openapi::OpenApi {
    let (_, documented) = OpenApiRouter::new()
        .routes(routes!(get_hosts))
        .routes(routes!(get_host_topology))
        .routes(routes!(get_host_process_overview))
        .split_for_parts();
    let mut openapi = ApiDoc::openapi();
    openapi.paths.paths.extend(documented.paths.paths);
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
    paths(get_hosts, get_host_topology, get_host_process_overview),
    components(schemas(
        HostListHttpEnvelope,
        HostListHttpView,
        HostSummaryHttpItem,
        HostTopologyHttpEnvelope,
        HostTopologyHttpView,
        HostTopologyHostDto,
        HostProcessOverviewHttpEnvelope,
        HostProcessOverviewHttpView,
    ))
)]
struct ApiDoc;

#[cfg(test)]
mod openapi_tests {
    use super::*;

    /// 钉住「标注了 = 文档里有」：路径由 `routes!` 收集，不是手抄的字符串，
    /// 所以新增一个带 `#[utoipa::path]` 的 handler 忘了加进 `routes!` 时这里会红。
    #[test]
    fn spec_documents_the_annotated_routes() {
        let spec = openapi_spec();
        for path in [
            "/api/hosts",
            "/api/topology/host/{id}",
            "/api/topology/host/{id}/process-overview",
        ] {
            assert!(spec.paths.paths.contains_key(path), "spec 里没有 {path}");
        }
        assert_eq!(spec.info.title, "wist-atlas 只读 API");
        // 字段名跟 serde 走（`camelCase`），不是手写的 `snake_case`。
        let host_summary =
            &spec.components.as_ref().expect("components").schemas["HostSummaryHttpItem"];
        let json = serde_json::to_value(host_summary).expect("schema to json");
        let props = json["properties"].as_object().expect("properties");
        assert!(props.contains_key("hostName"), "{props:?}");
        assert!(props.contains_key("lastInventoryAt"), "{props:?}");
    }

    /// 写入口是内部接口，不应该被当成对外文档暴露。
    #[test]
    fn write_endpoint_is_not_documented() {
        assert!(!openapi_spec().paths.paths.contains_key("/internal/atlas"));
    }
}
