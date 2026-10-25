use atlas_api::TopologyQueryService;
use atlas_storage::{AsyncCatalogStore, CatalogStore, GovernanceStore, Page, RuntimeStore};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{AppResult, materialization_missing};

mod async_build;
mod network;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostProcessTopologyGraph {
    nodes: Vec<HostProcessTopologyNode>,
    edges: Vec<HostProcessTopologyEdge>,
    pub(crate) metadata: HostProcessTopologyMetadata,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyNode {
    id: String,
    object_kind: &'static str,
    object_id: String,
    layer: &'static str,
    label: String,
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostProcessTopologyEdge {
    id: String,
    edge_kind: &'static str,
    source: String,
    target: String,
    label: Option<String>,
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostProcessTopologyMetadata {
    query_time: String,
    pub(crate) host_count: usize,
    pub(crate) process_count: usize,
}

pub(crate) use async_build::build_host_process_topology_graph_async;
pub(crate) use network::build_network_topology_graph_async;

pub(crate) fn build_visualization_graph<S>(store: S) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    build_host_process_topology_graph(store, None)
}

pub(crate) fn build_host_process_topology_graph<S>(
    store: S,
    focus_host_id: Option<Uuid>,
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

pub(in crate::graph) fn append_network_graph(
    nodes: &mut Vec<HostProcessTopologyNode>,
    edges: &mut Vec<HostProcessTopologyEdge>,
    host_node_id: &str,
    view: &atlas_domain::HostTopologyView,
) {
    for segment in &view.network_segments {
        let node_id = format!("network:{}", segment.network_segment_id);
        let mut properties = serde_json::Map::new();
        if let Some(cidr) = &segment.cidr {
            properties.insert("cidr".to_string(), Value::String(cidr.clone()));
        }
        properties.insert(
            "addressFamily".to_string(),
            Value::String(format!("{:?}", segment.address_family)),
        );
        if let Some(gateway_ip) = &segment.gateway_ip {
            properties.insert("gatewayIp".to_string(), Value::String(gateway_ip.clone()));
        }

        nodes.push(HostProcessTopologyNode {
            id: node_id,
            object_kind: "NetworkSegment",
            object_id: segment.network_segment_id.to_string(),
            layer: "resource",
            label: segment.name.clone(),
            properties,
        });
    }

    for assoc in &view.network_assocs {
        let target = format!("network:{}", assoc.network_segment_id);
        let mut properties = serde_json::Map::new();
        properties.insert("ipAddr".to_string(), Value::String(assoc.ip_addr.clone()));
        if let Some(iface_name) = &assoc.iface_name {
            properties.insert("ifaceName".to_string(), Value::String(iface_name.clone()));
        }
        properties.insert(
            "validFrom".to_string(),
            Value::String(assoc.validity.valid_from.to_rfc3339()),
        );
        if let Some(valid_to) = assoc.validity.valid_to {
            properties.insert("validTo".to_string(), Value::String(valid_to.to_rfc3339()));
        }

        edges.push(HostProcessTopologyEdge {
            id: format!("edge:{}:{}", host_node_id, assoc.assoc_id),
            edge_kind: "host_network_assoc",
            source: host_node_id.to_string(),
            target,
            label: assoc.iface_name.clone(),
            properties,
        });
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
