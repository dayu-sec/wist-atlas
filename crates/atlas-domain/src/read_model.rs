use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    BusinessDomain, ContainerRuntime, HostInventory, HostNetAssoc, HostRuntimeState,
    InstanceEndpoint, NetworkSegment, ProcessRuntimeState, ResponsibilityAssignment,
    RuntimeBinding, ServiceEntity, ServiceInstance, Subject,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogSummary {
    pub businesses: usize,
    pub systems: usize,
    pub services: usize,
    pub hosts: usize,
    pub subjects: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessOverviewView {
    pub business: BusinessDomain,
    pub services: Vec<ServiceEntity>,
    pub hosts: Vec<HostInventory>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostTopologyView {
    pub host: HostInventory,
    pub latest_runtime: Option<HostRuntimeState>,
    pub processes: Vec<ProcessRuntimeState>,
    pub process_groups: Vec<HostProcessGroupView>,
    pub network_segments: Vec<NetworkSegment>,
    pub network_assocs: Vec<HostNetAssoc>,
    pub services: Vec<HostServiceView>,
    /// 这台主机上跑着的容器（已退役的不算）。
    ///
    /// 容器不进画布：一台开发机常见的容器数是 0~2，为它单独开一层节点类型、
    /// 一套卡片和一条版式车道不划算；先在主机详情里列清楚。
    #[serde(default)]
    pub containers: Vec<ContainerRuntime>,
    pub assignments: Vec<ResponsibilityAssignment>,
    pub generated_at: DateTime<Utc>,
}

/// 主机列表条目。
///
/// 只放「选主机 / 判断这台机值不值得看」需要的东西：进程数与网段数都是规模指标，
/// 不放进程明细 —— 列表里每台机都带全量数据会把首页变成几 MB。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostSummaryView {
    pub host: HostInventory,
    /// 未退役的进程数（`stale_at IS NULL`）。
    pub process_count: i64,
    /// 关联网段数。进程多不等于暴露面大，两个数一起看才能选主机。
    pub segment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProcessOverviewView {
    pub host: HostInventory,
    pub total_processes: usize,
    pub total_groups: usize,
    pub top_groups: Vec<HostProcessGroupView>,
    pub truncated_group_count: usize,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProcessGroupsPageView {
    pub host: HostInventory,
    pub total_processes: usize,
    pub total_groups: usize,
    pub groups: Vec<HostProcessGroupView>,
    pub limit: usize,
    pub offset: usize,
    pub has_more: bool,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostServiceView {
    pub service: ServiceEntity,
    pub instances: Vec<HostServiceInstanceView>,
}

/// 服务列表条目。
///
/// 与主机列表同一取舍：只放「选服务」需要的规模指标，不放端点明细 ——
/// 端点属于服务焦点视图，塞进列表会把首页变成几十份端口清单。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSummaryView {
    pub service: ServiceEntity,
    /// 仍有存活进程绑定的实例数。采集侧停掉服务后这里会归零，而不是留着旧实例。
    pub instance_count: usize,
    /// 这些实例分布在几台主机上。这是「这套服务铺得多广」的唯一直接读数。
    pub host_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostServiceInstanceView {
    pub instance: ServiceInstance,
    pub bindings: Vec<RuntimeBinding>,
    pub processes: Vec<ProcessRuntimeState>,
    /// 这个实例当前实际监听的地址（`InstEp`）。空表示实例在跑但没开端口。
    #[serde(default)]
    pub endpoints: Vec<InstanceEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProcessGroupView {
    pub executable: String,
    pub display_name: String,
    pub process_count: usize,
    pub total_memory_rss_kib: i64,
    pub dominant_state: Option<String>,
    pub state_summary: Vec<ProcessStateCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessStateCount {
    pub state: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkTopologyView {
    pub segment: NetworkSegment,
    pub hosts: Vec<HostInventory>,
    pub host_assocs: Vec<HostNetAssoc>,
    pub assignments: Vec<ResponsibilityAssignment>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceTopologyView {
    pub service: ServiceEntity,
    pub instances: Vec<ServiceInstance>,
    pub hosts: Vec<HostInventory>,
    pub assignments: Vec<ResponsibilityAssignment>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveResponsibilityView {
    pub subject: Subject,
    pub assignment: ResponsibilityAssignment,
    pub generated_at: DateTime<Utc>,
}
