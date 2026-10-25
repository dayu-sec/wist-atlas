//! 网段焦点图：一个网段 + 连在它上面的主机。
//!
//! 与主机焦点图正好相反 —— 那边问的是「这台机器接在哪些网段上」，这边问的是
//! 「这个网段里有哪些机器」。两者共用同一套 `HostProcessTopologyGraph` 形状，
//! 画布与卡片都不用新增节点类型。

use atlas_api::TopologyQueryService;
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

/// 一次取多少个主机的进程计数（与 `atlas-api` 的列表查询同口径）。
const HOST_COUNT_PAGE_LIMIT: u32 = 200;

pub(crate) async fn build_network_topology_graph_async<S>(
    store: S,
    segment_id: Uuid,
) -> AppResult<Option<HostProcessTopologyGraph>>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let view = TopologyQueryService::new(store.clone())
        .network_topology_view_async(segment_id)
        .await
        .conv_err()?;
    let Some(view) = view else {
        return Ok(None);
    };

    let counts = host_process_counts(&store).await?;
    let segment = &view.segment;

    let segment_node_id = format!("segment:{}", segment.network_segment_id);
    let mut segment_props = serde_json::Map::new();
    if let Some(cidr) = &segment.cidr {
        segment_props.insert("cidr".to_string(), Value::String(cidr.clone()));
    }
    if let Some(gateway) = &segment.gateway_ip {
        segment_props.insert("gatewayIp".to_string(), Value::String(gateway.clone()));
    }
    segment_props.insert(
        "addressFamily".to_string(),
        Value::String(format!("{:?}", segment.address_family)),
    );
    segment_props.insert(
        "hostCount".to_string(),
        Value::Number(serde_json::Number::from(view.hosts.len() as u64)),
    );

    let mut nodes = vec![HostProcessTopologyNode {
        id: segment_node_id.clone(),
        object_kind: "NetworkSegment",
        object_id: segment.network_segment_id.to_string(),
        layer: "resource",
        label: segment.cidr.clone().unwrap_or_else(|| segment.name.clone()),
        properties: segment_props,
    }];
    let mut edges = Vec::new();
    let mut process_total = 0usize;

    for host in &view.hosts {
        let process_count = counts.get(&host.host_id).copied().unwrap_or(0);
        process_total += process_count as usize;

        let host_node_id = format!("host:{}", host.host_id);
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
        // 卡片在网段视图里没有「当前主机」的聚合可借，所以把规模挂在节点上。
        props.insert(
            "processCount".to_string(),
            Value::Number(serde_json::Number::from(process_count as u64)),
        );

        nodes.push(HostProcessTopologyNode {
            id: host_node_id.clone(),
            object_kind: "HostInventory",
            object_id: host.host_id.to_string(),
            layer: "resource",
            label: host.host_name.clone(),
            properties: props,
        });

        for assoc in view
            .host_assocs
            .iter()
            .filter(|assoc| assoc.host_id == host.host_id)
        {
            let mut edge_props = serde_json::Map::new();
            edge_props.insert("ipAddr".to_string(), Value::String(assoc.ip_addr.clone()));
            if let Some(iface_name) = &assoc.iface_name {
                edge_props.insert("ifaceName".to_string(), Value::String(iface_name.clone()));
            }
            edges.push(HostProcessTopologyEdge {
                id: format!("edge:{}:{}", host_node_id, assoc.assoc_id),
                edge_kind: "host_network_assoc",
                source: host_node_id.clone(),
                target: segment_node_id.clone(),
                label: assoc.iface_name.clone(),
                properties: edge_props,
            });
        }
    }

    Ok(Some(HostProcessTopologyGraph {
        nodes,
        edges,
        metadata: HostProcessTopologyMetadata {
            query_time: Utc::now().to_rfc3339(),
            host_count: view.hosts.len(),
            process_count: process_total,
        },
    }))
}

/// 主机进程计数按页取全：只取第一页会让第 101 台之后的主机在卡片上显示成 0 进程。
async fn host_process_counts<S>(store: &S) -> AppResult<std::collections::BTreeMap<Uuid, i64>>
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
        for (host_id, count) in batch {
            counts.insert(host_id, count);
        }
        if len < HOST_COUNT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(counts)
}
