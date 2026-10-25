//! 服务焦点图：一个服务 + 它的实例 + 监听进程 + 所在主机。
//!
//! 主机视图问「这台机器开了哪些端口」，这里反过来问「这个端口的**另一端**是谁」：
//! 哪个进程在听、那台机器是哪台、同一套服务还跑在哪些主机上。
//!
//! 实例到主机没有直接外键（`runtime_binding` 只连到进程），所以主机要靠进程行反查 ——
//! 这也是这里按主机拉一次进程表的原因。主机数量通常个位数，代价可控。

use std::collections::{BTreeMap, BTreeSet};

use atlas_api::list_all_process_runtime_states_async;
use atlas_domain::RuntimeObjectType;
use atlas_storage::{AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, Page};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use serde_json::Value;
use uuid::Uuid;

use super::{
    HostProcessTopologyEdge, HostProcessTopologyGraph, HostProcessTopologyMetadata,
    HostProcessTopologyNode,
};
use crate::AppResult;

/// 一次取多少台主机 / 多少个进程。
const PAGE_LIMIT: u32 = 200;

pub(crate) async fn build_service_topology_graph_async<S>(
    store: S,
    service_id: Uuid,
) -> AppResult<Option<HostProcessTopologyGraph>>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let Some(service) = AsyncCatalogStore::get_service(&store, service_id)
        .await
        .conv_err()?
    else {
        return Ok(None);
    };

    let instances = list_service_instances(&store, service_id).await?;

    // 先收集需要解析的 id，再一次性把进程/主机表读出来，避免每个实例各查一遍。
    let mut processes_by_instance: BTreeMap<Uuid, BTreeSet<Uuid>> = BTreeMap::new();
    let mut containers_by_instance: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    let mut endpoints_by_instance: BTreeMap<Uuid, Vec<(String, String, i32)>> = BTreeMap::new();
    let mut needed_processes: BTreeSet<Uuid> = BTreeSet::new();

    for instance in &instances {
        let bindings = list_instance_bindings(&store, instance.instance_id).await?;
        let process_ids: BTreeSet<Uuid> = bindings
            .iter()
            .filter(|binding| binding.object_type == RuntimeObjectType::Process)
            .filter(|binding| binding.validity.valid_to.is_none())
            .map(|binding| binding.object_id)
            .collect();
        needed_processes.extend(process_ids.iter().copied());
        processes_by_instance.insert(instance.instance_id, process_ids);
        // 容器绑定来自端口证据（`container_service_assoc` 的另一端）。
        let container_ids: Vec<Uuid> = bindings
            .iter()
            .filter(|binding| binding.object_type == RuntimeObjectType::Container)
            .filter(|binding| binding.validity.valid_to.is_none())
            .map(|binding| binding.object_id)
            .collect();
        containers_by_instance.insert(instance.instance_id, container_ids);

        let endpoints = list_endpoints(&store, instance.instance_id).await?;
        endpoints_by_instance.insert(
            instance.instance_id,
            endpoints
                .iter()
                .map(|endpoint| {
                    (
                        endpoint.protocol.clone(),
                        endpoint.address.clone(),
                        endpoint.port,
                    )
                })
                .collect(),
        );
    }

    let (hosts, process_rows, containers) = load_hosts_and_processes(&store).await?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut host_node_ids: BTreeSet<String> = BTreeSet::new();
    let mut resolved_hosts: BTreeSet<Uuid> = BTreeSet::new();
    // 实例 → 容器：端口证据写的 Container 绑定。服务视图里容器是「实例落脚的地方」，
    // 所以挂在实例旁边（容器 → 实例），不单独拉一条主机边。
    let mut instance_nodes_by_container: BTreeMap<Uuid, Vec<String>> = BTreeMap::new();

    // 服务节点：把端点全集摊平放在这里，卡片一眼能看出暴露面。
    let mut all_endpoints: Vec<(String, String, i32)> =
        endpoints_by_instance.values().flatten().cloned().collect();
    all_endpoints.sort();
    all_endpoints.dedup();

    let service_node_id = format!("service:{}", service.service_id);
    let mut service_props = serde_json::Map::new();
    service_props.insert(
        "serviceName".to_string(),
        Value::String(service.name.clone()),
    );
    service_props.insert(
        "serviceType".to_string(),
        Value::String(format!("{:?}", service.service_type)),
    );
    service_props.insert(
        "boundary".to_string(),
        Value::String(format!("{:?}", service.boundary)),
    );
    if let Some(external_ref) = &service.external_ref {
        service_props.insert(
            "externalRef".to_string(),
            Value::String(external_ref.clone()),
        );
    }
    service_props.insert(
        "instanceCount".to_string(),
        Value::Number(serde_json::Number::from(instances.len() as u64)),
    );
    if !all_endpoints.is_empty() {
        service_props.insert(
            "endpoints".to_string(),
            Value::Array(
                all_endpoints
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
        object_id: service.service_id.to_string(),
        layer: "resource",
        label: service.name.clone(),
        properties: service_props,
    });

    for instance in &instances {
        let instance_node_id = format!("service-instance:{}", instance.instance_id);
        let endpoints = endpoints_by_instance
            .get(&instance.instance_id)
            .cloned()
            .unwrap_or_default();

        // 实例挂在哪台主机上，只能顺着进程行反查。
        let instance_hosts: Vec<&atlas_domain::HostInventory> = processes_by_instance
            .get(&instance.instance_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| process_rows.get(id))
                    .filter_map(|process| hosts.get(&process.host_id))
                    .collect()
            })
            .unwrap_or_default();

        let mut instance_props = serde_json::Map::new();
        if !endpoints.is_empty() {
            instance_props.insert(
                "endpoints".to_string(),
                Value::Array(
                    endpoints
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
        if let Some(host) = instance_hosts.first() {
            instance_props.insert(
                "hostName".to_string(),
                Value::String(host.host_name.clone()),
            );
        }
        for container_id in containers_by_instance
            .get(&instance.instance_id)
            .into_iter()
            .flatten()
        {
            instance_nodes_by_container
                .entry(*container_id)
                .or_default()
                .push(instance_node_id.clone());
        }
        instance_props.insert(
            "lastSeenAt".to_string(),
            Value::String(instance.last_seen_at.to_rfc3339()),
        );
        instance_props.insert(
            "startedAt".to_string(),
            Value::String(instance.started_at.to_rfc3339()),
        );
        nodes.push(HostProcessTopologyNode {
            id: instance_node_id.clone(),
            object_kind: "ServiceInstance",
            object_id: instance.instance_id.to_string(),
            layer: "resource",
            label: if endpoints.is_empty() {
                format!("instance {}", &instance.instance_id.to_string()[..8])
            } else {
                endpoints
                    .iter()
                    .map(|(protocol, address, port)| format!("{protocol}/{port} {address}"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            },
            properties: instance_props,
        });
        edges.push(HostProcessTopologyEdge {
            id: format!("edge:{}:{}", instance.instance_id, service.service_id),
            edge_kind: "service_instance_assoc",
            source: instance_node_id.clone(),
            target: service_node_id.clone(),
            label: None,
            properties: serde_json::Map::new(),
        });

        // 主机节点：同一台机器只出现一次。
        for host in &instance_hosts {
            resolved_hosts.insert(host.host_id);
            let host_node_id = format!("host:{}", host.host_id);
            if host_node_ids.insert(host_node_id.clone()) {
                let mut props = serde_json::Map::new();
                props.insert(
                    "hostName".to_string(),
                    Value::String(host.host_name.clone()),
                );
                if let Some(os_name) = &host.os_name {
                    props.insert("osName".to_string(), Value::String(os_name.clone()));
                }
                if let Some(os_version) = &host.os_version {
                    props.insert("osVersion".to_string(), Value::String(os_version.clone()));
                }
                if let Some(arch) = &host.arch {
                    props.insert("arch".to_string(), Value::String(arch.clone()));
                }
                nodes.push(HostProcessTopologyNode {
                    id: host_node_id.clone(),
                    object_kind: "HostInventory",
                    object_id: host.host_id.to_string(),
                    layer: "resource",
                    label: host.host_name.clone(),
                    properties: props,
                });
                edges.push(HostProcessTopologyEdge {
                    id: format!("edge:{}:{}", host.host_id, service.service_id),
                    edge_kind: "host_service_assoc",
                    source: host_node_id.clone(),
                    target: service_node_id.clone(),
                    label: None,
                    properties: serde_json::Map::new(),
                });
            }
        }

        // 监听进程：谁在持有这个 socket。
        for process_id in processes_by_instance
            .get(&instance.instance_id)
            .cloned()
            .unwrap_or_default()
        {
            let Some(process) = process_rows.get(&process_id) else {
                continue;
            };
            let process_node_id = format!("process:{}", process.process_id);
            let mut props = serde_json::Map::new();
            props.insert(
                "pid".to_string(),
                Value::Number(serde_json::Number::from(process.pid)),
            );
            props.insert(
                "executable".to_string(),
                Value::String(process.executable.clone()),
            );
            nodes.push(HostProcessTopologyNode {
                id: process_node_id.clone(),
                object_kind: "ProcessRuntime",
                object_id: process.process_id.to_string(),
                layer: "resource",
                label: super::basename(&process.executable),
                properties: props,
            });
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", process.process_id, instance.instance_id),
                edge_kind: "process_service_assoc",
                source: process_node_id,
                target: instance_node_id.clone(),
                label: None,
                properties: serde_json::Map::new(),
            });
        }
    }

    // 容器节点只给「有实例落在里面」的容器：服务视图问的是「这个服务在哪」，
    // 不是「主机上还跑着什么」。
    for (container_id, instance_node_ids) in &instance_nodes_by_container {
        let Some(container) = containers.get(container_id) else {
            continue;
        };
        let container_node = super::container_node(container);
        let container_node_id = container_node.id.clone();
        nodes.push(container_node);
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

    Ok(Some(HostProcessTopologyGraph {
        nodes,
        edges,
        metadata: HostProcessTopologyMetadata {
            query_time: Utc::now().to_rfc3339(),
            host_count: resolved_hosts.len(),
            process_count: needed_processes.len(),
        },
    }))
}

async fn list_endpoints<S>(
    store: &S,
    instance_id: Uuid,
) -> AppResult<Vec<atlas_domain::InstanceEndpoint>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_instance_endpoints(
            store,
            instance_id,
            Page {
                limit: PAGE_LIMIT,
                offset,
            },
        )
        .await
        .conv_err()?;
        let len = batch.len();
        items.extend(batch);
        if len < PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

/// 服务的实例按页取全。agent 类服务往往每台机一个实例，只取第一页会让后面的主机凭空消失。
async fn list_service_instances<S>(
    store: &S,
    service_id: Uuid,
) -> AppResult<Vec<atlas_domain::ServiceInstance>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_service_instances(
            store,
            service_id,
            Page {
                limit: PAGE_LIMIT,
                offset,
            },
        )
        .await
        .conv_err()?;
        let len = batch.len();
        items.extend(batch);
        if len < PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

async fn list_instance_bindings<S>(
    store: &S,
    instance_id: Uuid,
) -> AppResult<Vec<atlas_domain::RuntimeBinding>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_runtime_bindings_for_instance(
            store,
            instance_id,
            Page {
                limit: PAGE_LIMIT,
                offset,
            },
        )
        .await
        .conv_err()?;
        let len = batch.len();
        items.extend(batch);
        if len < PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

/// 读主机表、全量进程表、以及这些主机上的容器（均按 id 建索引）。
///
/// 实例→主机没有直接外键，只能靠进程行的 `host_id` 反查；与其为每个进程 id 单独
/// 查一次，不如按主机拉一遍进程表 —— 主机数与单机进程量都在可控范围内。
async fn load_hosts_and_processes<S>(
    store: &S,
) -> AppResult<(
    BTreeMap<Uuid, atlas_domain::HostInventory>,
    BTreeMap<Uuid, atlas_domain::ProcessRuntimeState>,
    BTreeMap<Uuid, atlas_domain::ContainerRuntime>,
)>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut hosts = BTreeMap::new();
    let mut processes = BTreeMap::new();
    let mut containers = BTreeMap::new();
    let mut offset = 0;
    loop {
        let batch = AsyncCatalogStore::list_all_hosts(
            store,
            Page {
                limit: PAGE_LIMIT,
                offset,
            },
        )
        .await
        .conv_err()?;
        let len = batch.len();
        for host in batch {
            let process_rows = list_all_process_runtime_states_async(store, host.host_id)
                .await
                .conv_err()?;
            for process in process_rows {
                processes.insert(process.process_id, process);
            }
            for container in list_containers_async(store, host.host_id).await? {
                containers.insert(container.container_id, container);
            }
            hosts.insert(host.host_id, host);
        }
        if len < PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok((hosts, processes, containers))
}

async fn list_containers_async<S>(
    store: &S,
    host_id: Uuid,
) -> AppResult<Vec<atlas_domain::ContainerRuntime>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_container_runtimes_for_host(
            store,
            host_id,
            Page {
                limit: PAGE_LIMIT,
                offset,
            },
        )
        .await
        .conv_err()?;
        let len = batch.len();
        items.extend(batch);
        if len < PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}
