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
pub struct HostCandidate {
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub source_kind: SourceKind,
    pub external_ref: Option<String>,
    pub host_name: String,
    pub machine_id: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub arch: Option<String>,
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
