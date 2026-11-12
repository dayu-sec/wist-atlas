use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    Confidence, EnvironmentId, ObservedAt, ResponsibilityRole, ServiceBoundary, ServiceType,
    SourceKind, SubjectType, TenantId, ValidityWindow, WorkloadKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessCatalogCandidate {
    pub tenant_id: TenantId,
    pub source_kind: SourceKind,
    pub external_ref: Option<String>,
    pub business_name: String,
    pub system_name: Option<String>,
    pub subsystem_name: Option<String>,
    pub service_name: Option<String>,
    pub service_type: Option<ServiceType>,
    pub boundary: Option<ServiceBoundary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElasticIpCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    /// 云侧地址 ID（腾讯云 `AddressId`）。**重新绑定时不变**，是稳定键。
    pub external_ref: String,
    /// 地址本身。
    pub address: String,
    /// 云侧原始状态（`BIND` / `UNBIND` …），归一化在物料化层做。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charge_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandwidth_mbps: Option<i64>,
    /// 账号归属（与主机同一个键）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_account: Option<String>,
    /// 当前绑定的目标（`InstanceId`）。没绑就是 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_instance_ref: Option<String>,
    /// 目标类型（`CVM` / `NAT` / `CLB` …）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_instance_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub external_ref: Option<String>,
    pub host_name: String,
    pub machine_id: Option<String>,
    /// 云实例 ID。与 `machine_id` 并列：前者是云平台给的，后者是机器自证的。
    /// resolver 用它把"云 API 报的主机"和"agentd 报的同一台主机"并成一个对象。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_instance_id: Option<String>,
    /// 云端账号归属。云 API 采集器负责给；agentd 不知道也不需要知道。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_account: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub arch: Option<String>,
    /// 主网卡的私网地址（云 API 的 `PrivateIpAddresses[0]`）。
    /// 有了它才能在拓扑里回答「这台机在哪个子网、私网 IP 是多少」。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_addr: Option<String>,
    /// 云侧所属子网 ID。给了就优先用它定位子网，否则按 CIDR 包含匹配。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subnet_external_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessRuntimeCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub pid: i32,
    pub executable: String,
    pub command_line: Option<String>,
    pub identity: Option<String>,
    /// 采集侧直接报的容器 id（`/proc/<pid>/cgroup` 推出来的）。
    /// 没有时仍可由端口证据反推，但那对不监听端口的进程不成立。
    pub container_ref: Option<String>,
    pub service_ref: Option<String>,
    pub instance_key: Option<String>,
    pub observed_at: Option<ObservedAt>,
}

/// 一个网络域候选（云侧 VPC / 机房的 LAN）。
///
/// 为什么要有它：VPC 是**域名空间**，不是网段。以前把 VPC 也塞进
/// `network_segments[]`，只能靠 CIDR 猜归属；有了显式的域，子网才能稳定挂进去。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkDomainCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    /// 云侧 VPC ID：域的**稳定键**（改名不换身份）。
    pub external_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkSegmentCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub segment_name: Option<String>,
    pub cidr: Option<String>,
    pub gateway_ip: Option<String>,
    pub ip_addr: Option<String>,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub iface_name: Option<String>,
    /// 云侧子网 ID：段的**稳定键**（`SubnetId`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<String>,
    /// 所属域的云侧 ID（`VpcId`）。有它就能稳定挂进对应的 `NetworkDomain`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_external_ref: Option<String>,
}

/// 一个观测到的监听地址（模型里的 `InstEp` 候选）。
///
/// 这是「运行时事实」，不是业务服务定义：`executable` 只是用来给中心侧推断一个
/// 可读的服务名，不表达「这就是某个业务服务」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceEndpointCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub protocol: String,
    pub address: String,
    pub port: i32,
    /// 持有 socket 的进程。拿不到时只记地址。
    pub pid: Option<i32>,
    pub process_executable: Option<String>,
    pub process_identity: Option<String>,
    pub container_id: Option<String>,
    pub binding_confidence: Option<String>,
    pub observed_at: Option<ObservedAt>,
}

/// 一个观测到的容器（模型里的 `ContainerRuntime` 候选）。
///
/// `container_key` 是采集侧的容器 id 原值（`3f2c9f3c1b8e…`）。资产 id 由它派生出稳定
/// UUID，所以这里必须保留：丢了它就只能靠（主机 + 容器名）猜身份，同名重建会被当成同一个容器。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerRuntimeCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub container_key: String,
    /// `docker` / `containerd` / `cri-o`。解析不了的类型当 `process` 处理不如直接丢弃。
    pub runtime_type: String,
    pub runtime_namespace: Option<String>,
    pub container_name: Option<String>,
    pub image_ref: Option<String>,
    /// K8s 线索（平铺后）：`k8s.namespace` / `k8s.pod_uid` / `k8s.pod_name` / `k8s.container_name`。
    pub k8s_namespace: Option<String>,
    pub k8s_pod_uid: Option<String>,
    pub k8s_pod_name: Option<String>,
    pub k8s_container_name: Option<String>,
    pub started_at: Option<ObservedAt>,
    pub observed_at: Option<ObservedAt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostTelemetryCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub observed_at: ObservedAt,
    pub metric_name: String,
    pub value_i64: Option<i64>,
    pub value_f64: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessTelemetryCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub host_name: Option<String>,
    pub machine_id: Option<String>,
    pub process_ref: String,
    pub pid: i32,
    pub observed_at: ObservedAt,
    pub metric_name: String,
    pub value_i64: Option<i64>,
    pub value_string: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubjectCandidate {
    pub tenant_id: TenantId,
    pub source_kind: SourceKind,
    pub subject_type: SubjectType,
    pub external_ref: Option<String>,
    pub display_name: String,
    pub email: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub cluster_name: String,
    pub namespace_name: String,
    pub workload_kind: WorkloadKind,
    pub workload_name: String,
    pub service_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponsibilityAssignmentCandidate {
    pub tenant_id: TenantId,
    pub source_kind: SourceKind,
    pub subject_display_name: Option<String>,
    pub subject_external_ref: Option<String>,
    pub subject_email: Option<String>,
    pub target_kind: crate::ObjectKind,
    pub target_external_ref: Option<String>,
    pub role: ResponsibilityRole,
    pub validity: ValidityWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolutionCandidate {
    pub source_kind: SourceKind,
    pub rule_hints: Vec<String>,
    pub matched_identifiers: BTreeMap<String, String>,
    pub confidence: Confidence,
}
