use atlas_api::TopologyQueryService;
use atlas_storage::{AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, Page};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use serde_json::Value;
use uuid::Uuid;

use super::{
    HostProcessTopologyEdge, HostProcessTopologyGraph, HostProcessTopologyMetadata,
    HostProcessTopologyNode, append_network_graph, basename, list_all_hosts_page_async,
};
use crate::{AppResult, materialization_missing};

pub(crate) async fn build_host_process_topology_graph_async<S>(
    store: S,
    focus_host_id: Option<Uuid>,
) -> AppResult<HostProcessTopologyGraph>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let mut hosts = Vec::new();
    if let Some(host_id) = focus_host_id {
        let host = AsyncCatalogStore::get_host(&store, host_id)
            .await
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
            let batch = list_all_hosts_page_async(&store, page).await?;
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
        let Some(view) = query
            .host_topology_view_async(host.host_id)
            .await
            .conv_err()?
        else {
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
                    label: format!(
                        "instance {}",
                        &instance_view.instance.instance_id.to_string()[..8]
                    ),
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
