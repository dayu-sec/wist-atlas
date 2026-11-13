use atlas_api::TopologyQueryService;
use atlas_domain::RuntimeObjectType;
use atlas_storage::{
    AsyncCatalogStore, AsyncRuntimeStore, CatalogStore, GovernanceStore, Page, RuntimeStore,
};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{AppResult, materialization_missing};

mod async_build;
mod network;
mod overview;
mod service;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub(crate) struct HostProcessTopologyGraph {
    nodes: Vec<HostProcessTopologyNode>,
    edges: Vec<HostProcessTopologyEdge>,
    pub(crate) metadata: HostProcessTopologyMetadata,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyNode {
    id: String,
    #[schema(value_type = String)]
    object_kind: &'static str,
    object_id: String,
    #[schema(value_type = String)]
    layer: &'static str,
    label: String,
    #[schema(value_type = Object)]
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyEdge {
    id: String,
    #[schema(value_type = String)]
    edge_kind: &'static str,
    source: String,
    target: String,
    label: Option<String>,
    #[schema(value_type = Object)]
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostProcessTopologyMetadata {
    query_time: String,
    pub(crate) host_count: usize,
    pub(crate) process_count: usize,
}

pub(crate) use async_build::{
    build_detailed_fleet_graph_async, build_host_process_topology_graph_async,
};
pub(crate) use network::build_network_topology_graph_async;
pub(crate) use overview::{build_asset_overview_graph, build_asset_overview_graph_async};
pub(crate) use service::build_service_topology_graph_async;

/// 无焦点主机时要建哪一种图。
///
/// 这条分岔不是“优化开关”，是对两个不同产品语义的承认：
/// - [`FleetShape::Canvas`]：**画布首屏**。站点有多台主机时，画布只画主机卡 + 共享网段
///   （`buildOverviewDisplay`），所以只给资产总览；单主机站点仍是完整图。
/// - [`FleetShape::Detailed`]：**离线导出/排查**（`export-visualization`）。无论几台主机
///   都导完整图 —— 导出是给人翻的，不该因为“画布不画”就把进程树抽掉。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FleetShape {
    Canvas,
    Detailed,
}

/// 主机节点上的「端侧（agentd）观测」标记。
///
/// 判据：**这台主机有没有运行时报文**。启动时间 / 负载 / 内存只有 agent 能采，
/// 云 API 那边只给静态资产属性 —— 所以「有运行时就说明这台机上有 agentd 在跑」。
/// 时钟只有一期：`agentdObservedAt` 让前端能把「装过但早就不报了」和「正在报」分开。
///
/// 放在一个函数里而不是每个视图各写一遍：主机节点有 5 个构建点，
/// 漏一个就会出现「同一台机在总览里有标、在网段视图里没有」。
pub(crate) fn insert_agentd_properties(
    properties: &mut serde_json::Map<String, Value>,
    runtime: Option<&atlas_domain::HostRuntimeState>,
) {
    properties.insert("agentd".to_string(), Value::Bool(runtime.is_some()));
    let Some(runtime) = runtime else {
        return;
    };
    properties.insert(
        "agentdHealth".to_string(),
        Value::String(
            match runtime.agent_health {
                atlas_domain::AgentHealth::Healthy => "healthy",
                atlas_domain::AgentHealth::Degraded => "degraded",
                atlas_domain::AgentHealth::Protect => "protect",
                atlas_domain::AgentHealth::Unavailable => "unavailable",
            }
            .to_string(),
        ),
    );
    properties.insert(
        "agentdObservedAt".to_string(),
        Value::String(runtime.observed_at.0.to_rfc3339()),
    );
}

/// 导出用：始终完整图。
pub(crate) fn build_visualization_graph<S>(store: S) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    build_graph(store, None, FleetShape::Detailed)
}

/// 画布 / CLI 打印用：跟首屏接口看到的一样（多主机时是资产总览）。
pub(crate) fn build_host_process_topology_graph<S>(
    store: S,
    focus_host_id: Option<Uuid>,
) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    build_graph(store, focus_host_id, FleetShape::Canvas)
}

fn build_graph<S>(
    store: S,
    focus_host_id: Option<Uuid>,
    shape: FleetShape,
) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    let mut hosts = Vec::new();
    if let Some(host_id) = focus_host_id {
        let host = store
            .get_host(host_id)
            .conv_err()?
            .ok_or_else(|| materialization_missing(format!("host {host_id} was not found")))?;
        hosts.push(host);
    } else {
        let mut offset = 0;
        let page_limit = 200;
        loop {
            let page = Page {
                limit: page_limit,
                offset,
            };
            let batch = list_all_hosts_page(&store, page)?;
            if batch.is_empty() {
                break;
            }
            offset += batch.len() as u32;
            hosts.extend(batch);
            if hosts.len() % page_limit as usize != 0 {
                break;
            }
        }
    }

    if shape == FleetShape::Canvas && focus_host_id.is_none() && hosts.len() > 1 {
        return build_asset_overview_graph(&store, hosts);
    }

    let query = TopologyQueryService::new(store.clone());
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut process_count = 0usize;

    for host in hosts {
        let Some(view) = query.host_topology_view(host.host_id).conv_err()? else {
            continue;
        };

        let host_node_id = format!("host:{}", view.host.host_id);
        let mut host_props = serde_json::Map::new();
        host_props.insert(
            "hostName".to_string(),
            Value::String(view.host.host_name.clone()),
        );
        if let Some(machine_id) = &view.host.machine_id {
            host_props.insert("machineId".to_string(), Value::String(machine_id.clone()));
        }
        if let Some(os_name) = &view.host.os_name {
            host_props.insert("osName".to_string(), Value::String(os_name.clone()));
        }
        if let Some(os_version) = &view.host.os_version {
            host_props.insert("osVersion".to_string(), Value::String(os_version.clone()));
        }
        if let Some(runtime) = &view.latest_runtime {
            host_props.insert(
                "observedAt".to_string(),
                Value::String(runtime.observed_at.0.to_rfc3339()),
            );
            if let Some(loadavg) = runtime.loadavg_1m {
                host_props.insert("loadavg1m".to_string(), Value::from(loadavg));
            }
            if let Some(memory_used_bytes) = runtime.memory_used_bytes {
                host_props.insert(
                    "memoryUsedBytes".to_string(),
                    Value::from(memory_used_bytes),
                );
            }
            if let Some(processes) = runtime.process_count {
                host_props.insert("processCount".to_string(), Value::from(processes));
            }
        }
        insert_agentd_properties(&mut host_props, view.latest_runtime.as_ref());
        nodes.push(HostProcessTopologyNode {
            id: host_node_id.clone(),
            object_kind: "HostInventory",
            object_id: view.host.host_id.to_string(),
            layer: "resource",
            label: view.host.host_name.clone(),
            properties: host_props,
        });

        let summary_node_id = format!("process-summary:{}", host_node_id);
        let mut summary_props = serde_json::Map::new();
        summary_props.insert(
            "totalProcesses".to_string(),
            Value::from(view.processes.len() as i64),
        );
        summary_props.insert(
            "totalPrograms".to_string(),
            Value::from(view.process_groups.len() as i64),
        );
        summary_props.insert(
            "topPrograms".to_string(),
            Value::String(
                view.process_groups
                    .iter()
                    .take(5)
                    .map(|group| format!("{} x{}", group.display_name, group.process_count))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        );
        nodes.push(HostProcessTopologyNode {
            id: summary_node_id.clone(),
            object_kind: "ProcessSummary",
            object_id: view.host.host_id.to_string(),
            layer: "resource",
            label: format!("processes: {}", view.processes.len()),
            properties: summary_props,
        });
        edges.push(HostProcessTopologyEdge {
            id: format!("edge:{}:process-summary", view.host.host_id),
            edge_kind: "host_process_assoc",
            source: host_node_id.clone(),
            target: summary_node_id.clone(),
            label: None,
            properties: serde_json::Map::new(),
        });

        append_network_graph(&mut nodes, &mut edges, &host_node_id, &view);

        for group in &view.process_groups {
            let group_node_id = format!("process-group:{}:{}", host_node_id, group.executable);
            let mut group_props = serde_json::Map::new();
            group_props.insert(
                "executable".to_string(),
                Value::String(group.executable.clone()),
            );
            group_props.insert(
                "processCount".to_string(),
                Value::from(group.process_count as i64),
            );
            group_props.insert(
                "totalMemoryRssKiB".to_string(),
                Value::from(group.total_memory_rss_kib),
            );
            group_props.insert(
                "totalMemoryRssMiB".to_string(),
                Value::from(((group.total_memory_rss_kib as f64) / 1024.0 * 10.0).round() / 10.0),
            );
            if let Some(state) = &group.dominant_state {
                group_props.insert("dominantState".to_string(), Value::String(state.clone()));
            }
            group_props.insert(
                "states".to_string(),
                Value::String(
                    group
                        .state_summary
                        .iter()
                        .take(3)
                        .map(|item| format!("{}:{}", item.state, item.count))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            nodes.push(HostProcessTopologyNode {
                id: group_node_id.clone(),
                object_kind: "ProcessGroup",
                object_id: view.host.host_id.to_string(),
                layer: "resource",
                label: format!("{} x{}", group.display_name, group.process_count),
                properties: group_props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", summary_node_id, group.executable),
                edge_kind: "host_process_assoc",
                source: summary_node_id.clone(),
                target: group_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });
        }

        for process in view.processes {
            process_count += 1;
            let process_node_id = format!("process:{}", process.process_id);
            let mut process_props = serde_json::Map::new();
            process_props.insert("pid".to_string(), Value::from(process.pid));
            process_props.insert(
                "executable".to_string(),
                Value::String(process.executable.clone()),
            );
            if let Some(command_line) = &process.command_line {
                process_props.insert(
                    "commandLine".to_string(),
                    Value::String(command_line.clone()),
                );
            }
            if let Some(state) = &process.process_state {
                process_props.insert("processState".to_string(), Value::String(state.clone()));
            }
            if let Some(memory_rss_kib) = process.memory_rss_kib {
                process_props.insert("memoryRssKiB".to_string(), Value::from(memory_rss_kib));
            }
            process_props.insert(
                "observedAt".to_string(),
                Value::String(process.observed_at.0.to_rfc3339()),
            );
            nodes.push(HostProcessTopologyNode {
                id: process_node_id.clone(),
                object_kind: "ProcessRuntime",
                object_id: process.process_id.to_string(),
                layer: "resource",
                label: format!("{} ({})", basename(&process.executable), process.pid),
                properties: process_props,
            });
            let group_node_id = format!("process-group:{}:{}", host_node_id, process.executable);
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", group_node_id, process.process_id),
                edge_kind: "host_process_assoc",
                source: group_node_id,
                target: process_node_id,
                label: None,
                properties: serde_json::Map::new(),
            });
        }

        // 容器层：主机上跑着哪些容器；实例落在哪个容器里，靠端口证据写的 Container 绑定。
        let mut instances_by_container: std::collections::BTreeMap<Uuid, Vec<String>> =
            std::collections::BTreeMap::new();

        for service_view in &view.services {
            let service_node_id = format!("service:{}", service_view.service.service_id);
            let mut service_props = serde_json::Map::new();
            service_props.insert(
                "serviceName".to_string(),
                Value::String(service_view.service.name.clone()),
            );
            if let Some(external_ref) = &service_view.service.external_ref {
                service_props.insert(
                    "externalRef".to_string(),
                    Value::String(external_ref.clone()),
                );
            }
            service_props.insert(
                "serviceType".to_string(),
                Value::String(format!("{:?}", service_view.service.service_type)),
            );
            service_props.insert(
                "boundary".to_string(),
                Value::String(format!("{:?}", service_view.service.boundary)),
            );
            // 服务卡上直接给出「在哪儿听」：一个服务可能多个实例、每个实例多个端点。
            // 没有这个，服务节点在图上就只是一个名字，看不出暴露面。
            let service_endpoints = collect_endpoints(&service_view.instances);
            if !service_endpoints.is_empty() {
                service_props.insert(
                    "endpoints".to_string(),
                    Value::Array(
                        service_endpoints
                            .iter()
                            .map(|(protocol, address, port)| {
                                serde_json::json!({
                                    "protocol": protocol,
                                    "address": address,
                                    "port": port,
                                })
                            })
                            .collect(),
                    ),
                );
            }
            nodes.push(HostProcessTopologyNode {
                id: service_node_id.clone(),
                object_kind: "ServiceEntity",
                object_id: service_view.service.service_id.to_string(),
                layer: "resource",
                label: service_view.service.name.clone(),
                properties: service_props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", host_node_id, service_view.service.service_id),
                edge_kind: "host_service_assoc",
                source: host_node_id.clone(),
                target: service_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });

            for instance_view in &service_view.instances {
                let instance_node_id =
                    format!("service-instance:{}", instance_view.instance.instance_id);
                for binding in &instance_view.bindings {
                    if binding.object_type == RuntimeObjectType::Container
                        && binding.validity.valid_to.is_none()
                    {
                        instances_by_container
                            .entry(binding.object_id)
                            .or_default()
                            .push(instance_node_id.clone());
                    }
                }
                let mut instance_props = serde_json::Map::new();
                let instance_endpoints: Vec<String> = instance_view
                    .endpoints
                    .iter()
                    .map(|endpoint| {
                        format!(
                            "{}/{} {}",
                            endpoint.protocol, endpoint.port, endpoint.address
                        )
                    })
                    .collect();
                if !instance_endpoints.is_empty() {
                    instance_props.insert(
                        "endpoints".to_string(),
                        Value::Array(
                            instance_view
                                .endpoints
                                .iter()
                                .map(|endpoint| {
                                    serde_json::json!({
                                        "protocol": endpoint.protocol,
                                        "address": endpoint.address,
                                        "port": endpoint.port,
                                    })
                                })
                                .collect(),
                        ),
                    );
                }
                instance_props.insert(
                    "lastSeenAt".to_string(),
                    Value::String(instance_view.instance.last_seen_at.to_rfc3339()),
                );
                instance_props.insert(
                    "startedAt".to_string(),
                    Value::String(instance_view.instance.started_at.to_rfc3339()),
                );
                nodes.push(HostProcessTopologyNode {
                    id: instance_node_id.clone(),
                    object_kind: "ServiceInstance",
                    object_id: instance_view.instance.instance_id.to_string(),
                    layer: "resource",
                    label: if instance_endpoints.is_empty() {
                        // 没有端口就退回 id 前缀：至少能区分两个实例。
                        format!(
                            "instance {}",
                            &instance_view.instance.instance_id.to_string()[..8]
                        )
                    } else {
                        instance_endpoints.join(" · ")
                    },
                    properties: instance_props,
                });
                edges.push(HostProcessTopologyEdge {
                    id: format!(
                        "edge:{}:{}",
                        instance_view.instance.instance_id, service_view.service.service_id
                    ),
                    edge_kind: "service_instance_assoc",
                    source: instance_node_id.clone(),
                    target: service_node_id.clone(),
                    label: None,
                    properties: serde_json::Map::new(),
                });

                for process in &instance_view.processes {
                    edges.push(HostProcessTopologyEdge {
                        id: format!(
                            "edge:{}:{}",
                            process.process_id, instance_view.instance.instance_id
                        ),
                        edge_kind: "process_service_assoc",
                        source: format!("process:{}", process.process_id),
                        target: instance_node_id.clone(),
                        label: None,
                        properties: serde_json::Map::new(),
                    });
                }
            }
        }

        append_container_graph(
            &mut nodes,
            &mut edges,
            &host_node_id,
            &view.containers,
            &instances_by_container,
        );
    }

    Ok(HostProcessTopologyGraph {
        metadata: HostProcessTopologyMetadata {
            query_time: Utc::now().to_rfc3339(),
            host_count: nodes
                .iter()
                .filter(|node| node.object_kind == "HostInventory")
                .count(),
            process_count,
        },
        nodes,
        edges,
    })
}

/// 网段层：这台机器接在哪些网段上（节点 + 关联边）。
///
/// 节点/边的形状由 `graph::overview::segment_node` / `assoc_edge` 定，与资产总览共用一套 ——
/// 两处各写一遍必定漂：总览卡上有 `cidr`、聚焦卡上没有这种小差异最难查。
pub(in crate::graph) fn append_network_graph(
    nodes: &mut Vec<HostProcessTopologyNode>,
    edges: &mut Vec<HostProcessTopologyEdge>,
    host_node_id: &str,
    view: &atlas_domain::HostTopologyView,
) {
    for segment in &view.network_segments {
        nodes.push(overview::segment_node(segment));
    }

    for assoc in &view.network_assocs {
        edges.push(overview::assoc_edge(host_node_id, assoc));
    }
}

/// 容器层：主机上跑着哪些容器，以及哪些服务实例落在容器里。
///
/// 容器是主机下的独立对象（`host_container_assoc`）：它不属于某个服务，
/// 一个容器可以监听多个端口。实例→容器的边来自端口证据（`container_service_assoc`）——
/// 没有这条边时，容器只是主机上的一个孤立对象，这正是未接入归属前的样子。
pub(in crate::graph) fn append_container_graph(
    nodes: &mut Vec<HostProcessTopologyNode>,
    edges: &mut Vec<HostProcessTopologyEdge>,
    host_node_id: &str,
    containers: &[atlas_domain::ContainerRuntime],
    instance_nodes_by_container: &std::collections::BTreeMap<Uuid, Vec<String>>,
) {
    let mut container_node_ids: std::collections::BTreeMap<Uuid, String> =
        std::collections::BTreeMap::new();

    for container in containers {
        let container_node_id = format!("container:{}", container.container_id);
        nodes.push(container_node(container));
        edges.push(HostProcessTopologyEdge {
            id: format!("edge:{}:{}", host_node_id, container.container_id),
            edge_kind: "host_container_assoc",
            source: host_node_id.to_string(),
            target: container_node_id.clone(),
            label: None,
            properties: serde_json::Map::new(),
        });
        container_node_ids.insert(container.container_id, container_node_id);
    }

    for (container_id, instance_node_ids) in instance_nodes_by_container {
        // 绑定指向的容器本轮没采到（已退役）时不画边，避免悬空连线。
        let Some(container_node_id) = container_node_ids.get(container_id) else {
            continue;
        };
        for instance_node_id in instance_node_ids {
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", container_node_id, instance_node_id),
                edge_kind: "container_service_assoc",
                source: container_node_id.clone(),
                target: instance_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });
        }
    }
}

/// 容器节点的构造。主机视图与服务焦点视图都用它 —— 两处的属性必须一模一样，
/// 否则同一张卡片在两个视图里显示不同的信息。
pub(in crate::graph) fn container_node(
    container: &atlas_domain::ContainerRuntime,
) -> HostProcessTopologyNode {
    let mut props = serde_json::Map::new();
    if let Some(name) = &container.container_name {
        props.insert("containerName".to_string(), Value::String(name.clone()));
    }
    props.insert(
        "runtimeType".to_string(),
        Value::String(format!("{:?}", container.runtime_type)),
    );
    if let Some(image) = &container.image_ref {
        props.insert("imageRef".to_string(), Value::String(image.clone()));
    }
    if let Some(namespace) = &container.runtime_namespace {
        props.insert(
            "runtimeNamespace".to_string(),
            Value::String(namespace.clone()),
        );
    }
    if let Some(pod_name) = &container.pod_name {
        props.insert("podName".to_string(), Value::String(pod_name.clone()));
    }
    if let Some(namespace) = &container.k8s_namespace {
        props.insert("k8sNamespace".to_string(), Value::String(namespace.clone()));
    }
    if let Some(external_ref) = &container.external_ref {
        props.insert(
            "externalRef".to_string(),
            Value::String(external_ref.clone()),
        );
    }

    HostProcessTopologyNode {
        id: format!("container:{}", container.container_id),
        object_kind: "ContainerRuntime",
        object_id: container.container_id.to_string(),
        layer: "resource",
        // 卡片标题用容器名；没采到名字时退回 id 前 12 位，至少能区分两个容器。
        label: container
            .container_name
            .clone()
            .unwrap_or_else(|| container.container_id.to_string()[..12].to_string()),
        properties: props,
    }
}

fn basename(path: &str) -> String {
    path.rsplit('/')
        .next()
        .filter(|item| !item.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// 把一个服务的所有实例的端点去重后拼成一份。
/// 同一台主机上一个服务只该有一个实例，但多实例/多端口是常态，
/// 服务卡上给全集比只给第一个诚实。
pub(in crate::graph) fn collect_endpoints(
    instances: &[atlas_domain::HostServiceInstanceView],
) -> Vec<(String, String, i32)> {
    let mut endpoints: Vec<(String, String, i32)> = instances
        .iter()
        .flat_map(|instance| instance.endpoints.iter())
        .map(|endpoint| {
            (
                endpoint.protocol.clone(),
                endpoint.address.clone(),
                endpoint.port,
            )
        })
        .collect();
    endpoints.sort();
    endpoints.dedup();
    endpoints
}

fn list_all_hosts_page<S>(store: &S, page: Page) -> AppResult<Vec<atlas_domain::HostInventory>>
where
    S: CatalogStore,
{
    store.list_all_hosts(page).conv_err()
}

pub(super) async fn list_all_hosts_page_async<S>(
    store: &S,
    page: Page,
) -> AppResult<Vec<atlas_domain::HostInventory>>
where
    S: AsyncCatalogStore,
{
    AsyncCatalogStore::list_all_hosts(store, page)
        .await
        .conv_err()
}

/// 一次取多少台主机的进程计数（与 `atlas-api` 的列表查询同口径）。
const HOST_COUNT_PAGE_LIMIT: u32 = 200;

/// 每台主机的活进程数：一条 `GROUP BY host_id` 聚合，按页取全。
///
/// 资产总览 / 网段视图的卡片只需要这个数 —— 不要为了几个数字去逐台机读进程树。
/// 只取第一页会让第 201 台之后的主机显示成 0 进程。
pub(super) fn host_process_counts<S>(store: &S) -> AppResult<std::collections::BTreeMap<Uuid, i64>>
where
    S: RuntimeStore,
{
    let mut counts = std::collections::BTreeMap::new();
    let mut offset = 0;
    loop {
        let batch = store
            .list_host_process_counts(Page {
                limit: HOST_COUNT_PAGE_LIMIT,
                offset,
            })
            .conv_err()?;
        let len = batch.len();
        counts.extend(batch);
        if len < HOST_COUNT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(counts)
}

/// 同上，异步版。
pub(super) async fn host_process_counts_async<S>(
    store: &S,
) -> AppResult<std::collections::BTreeMap<Uuid, i64>>
where
    S: AsyncRuntimeStore,
{
    let mut counts = std::collections::BTreeMap::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_host_process_counts(
            store,
            Page {
                limit: HOST_COUNT_PAGE_LIMIT,
                offset,
            },
        )
        .await
        .conv_err()?;
        let len = batch.len();
        counts.extend(batch);
        if len < HOST_COUNT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(counts)
}
