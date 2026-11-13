//! 资产总览图（多主机形态）：主机卡 + 共享网段 + 每台机的规模。
//!
//! 多主机时画布**只画**「主机卡 + 共享网段卡」——进程、进程分组、服务、实例、
//! 容器一个都不上画布（`wist-atlas-graph` 的 `buildOverviewDisplay` 会把它们全丢掉）。
//! 但以前后端仍然逐台机把整棵进程树塞进响应：一个 12 台机 / 2800 个进程的站点
//! 是 ~2.8 MB、上千条 `list_runtime_bindings_for_object` 查询，浏览器下载完再全部丢弃。
//!
//! 总览真正需要的只有三样：主机行、网段关联、每台机的进程数（一条聚合 SQL）。
//! 单主机形态仍然走 [`super::build_host_process_topology_graph`] 的完整图 —— 那时候
//! 进程树是真的要画的（分组可展开），省不得。

use atlas_domain::{HostInventory, HostNetAssoc, HostRuntimeState, NetworkSegment};
use atlas_storage::{AsyncCatalogStore, AsyncRuntimeStore, CatalogStore, Page, RuntimeStore};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use serde_json::Value;

use super::{
    HostProcessTopologyEdge, HostProcessTopologyGraph, HostProcessTopologyMetadata,
    HostProcessTopologyNode,
};
use crate::AppResult;

pub(crate) async fn build_asset_overview_graph_async<S>(
    store: &S,
    hosts: Vec<HostInventory>,
) -> AppResult<HostProcessTopologyGraph>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let counts = super::host_process_counts_async(store).await?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen_segments = std::collections::BTreeSet::new();

    for host in &hosts {
        let assocs = AsyncRuntimeStore::list_host_net_assocs(store, host.host_id, Page::default())
            .await
            .conv_err()?;
        let latest_runtime =
            AsyncRuntimeStore::list_host_runtime_states(store, host.host_id, Page::default())
                .await
                .conv_err()?
                .into_iter()
                .max_by_key(|state| state.observed_at.0);

        nodes.push(host_node(
            host,
            latest_runtime.as_ref(),
            counts.get(&host.host_id).copied(),
            assocs.len(),
        ));
        let host_node_id = format!("host:{}", host.host_id);

        for assoc in &assocs {
            if seen_segments.insert(assoc.network_segment_id) {
                if let Some(segment) =
                    AsyncCatalogStore::get_network_segment(store, assoc.network_segment_id)
                        .await
                        .conv_err()?
                {
                    nodes.push(segment_node(&segment));
                }
            }
            edges.push(assoc_edge(&host_node_id, assoc));
        }
    }

    Ok(overview_graph(nodes, edges, &hosts, &counts))
}

pub(crate) fn build_asset_overview_graph<S>(
    store: &S,
    hosts: Vec<HostInventory>,
) -> AppResult<HostProcessTopologyGraph>
where
    S: CatalogStore + RuntimeStore,
{
    let counts = super::host_process_counts(store)?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen_segments = std::collections::BTreeSet::new();

    for host in &hosts {
        let assocs = store
            .list_host_net_assocs(host.host_id, Page::default())
            .conv_err()?;
        let latest_runtime = store
            .list_host_runtime_states(host.host_id, Page::default())
            .conv_err()?
            .into_iter()
            .max_by_key(|state| state.observed_at.0);

        nodes.push(host_node(
            host,
            latest_runtime.as_ref(),
            counts.get(&host.host_id).copied(),
            assocs.len(),
        ));
        let host_node_id = format!("host:{}", host.host_id);

        for assoc in &assocs {
            if seen_segments.insert(assoc.network_segment_id)
                && let Some(segment) = store
                    .get_network_segment(assoc.network_segment_id)
                    .conv_err()?
            {
                nodes.push(segment_node(&segment));
            }
            edges.push(assoc_edge(&host_node_id, assoc));
        }
    }

    Ok(overview_graph(nodes, edges, &hosts, &counts))
}

fn host_node(
    host: &HostInventory,
    latest_runtime: Option<&HostRuntimeState>,
    process_count: Option<i64>,
    segment_count: usize,
) -> HostProcessTopologyNode {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "hostName".to_string(),
        Value::String(host.host_name.clone()),
    );
    if let Some(machine_id) = &host.machine_id {
        properties.insert("machineId".to_string(), Value::String(machine_id.clone()));
    }
    if let Some(os_name) = &host.os_name {
        properties.insert("osName".to_string(), Value::String(os_name.clone()));
    }
    if let Some(os_version) = &host.os_version {
        properties.insert("osVersion".to_string(), Value::String(os_version.clone()));
    }
    if let Some(arch) = &host.arch {
        properties.insert("arch".to_string(), Value::String(arch.clone()));
    }
    // 卡片在总览里拿不到「当前主机」的聚合（多于一台机时 `HostStats` 不传），
    // 规模与网段数就挂在节点上，与网段视图同一口径。
    properties.insert(
        "segmentCount".to_string(),
        Value::from(segment_count as i64),
    );
    if let Some(runtime) = latest_runtime {
        properties.insert(
            "observedAt".to_string(),
            Value::String(runtime.observed_at.0.to_rfc3339()),
        );
        if let Some(loadavg) = runtime.loadavg_1m {
            properties.insert("loadavg1m".to_string(), Value::from(loadavg));
        }
        if let Some(memory_used_bytes) = runtime.memory_used_bytes {
            properties.insert(
                "memoryUsedBytes".to_string(),
                Value::from(memory_used_bytes),
            );
        }
        if let Some(processes) = runtime.process_count {
            properties.insert("processCount".to_string(), Value::from(processes));
        }
    }
    if !properties.contains_key("processCount")
        && let Some(count) = process_count
    {
        properties.insert("processCount".to_string(), Value::from(count));
    }

    super::insert_agentd_properties(&mut properties, latest_runtime);
    HostProcessTopologyNode {
        id: format!("host:{}", host.host_id),
        object_kind: "HostInventory",
        object_id: host.host_id.to_string(),
        layer: "resource",
        label: host.host_name.clone(),
        properties,
    }
}

pub(super) fn segment_node(segment: &NetworkSegment) -> HostProcessTopologyNode {
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

    HostProcessTopologyNode {
        id: format!("network:{}", segment.network_segment_id),
        object_kind: "NetworkSegment",
        object_id: segment.network_segment_id.to_string(),
        layer: "resource",
        label: segment.name.clone(),
        properties,
    }
}

pub(super) fn assoc_edge(host_node_id: &str, assoc: &HostNetAssoc) -> HostProcessTopologyEdge {
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

    HostProcessTopologyEdge {
        id: format!("edge:{}:{}", host_node_id, assoc.assoc_id),
        edge_kind: "host_network_assoc",
        source: host_node_id.to_string(),
        target: format!("network:{}", assoc.network_segment_id),
        label: assoc.iface_name.clone(),
        properties,
    }
}

fn overview_graph(
    nodes: Vec<HostProcessTopologyNode>,
    edges: Vec<HostProcessTopologyEdge>,
    hosts: &[HostInventory],
    counts: &std::collections::BTreeMap<uuid::Uuid, i64>,
) -> HostProcessTopologyGraph {
    let process_count = hosts
        .iter()
        .map(|host| counts.get(&host.host_id).copied().unwrap_or(0))
        .sum::<i64>()
        .max(0) as usize;

    HostProcessTopologyGraph {
        metadata: HostProcessTopologyMetadata {
            query_time: Utc::now().to_rfc3339(),
            host_count: hosts.len(),
            process_count,
        },
        nodes,
        edges,
    }
}
