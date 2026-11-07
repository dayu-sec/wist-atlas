//! 只用于生成 OpenAPI schema 的「文档 DTO」。
//!
//! 为什么需要它们：响应体里直接引用 `atlas-domain` 的类型（`HostRuntimeState`、
//! `ProcessRuntimeState`…）。给 domain 加 `ToSchema` 会把「怎么对外描述」这件事漏到领域层，
//! 而让 utoipa 反射出 schema 又必须有一个类型载体 —— 于是这里放一组**与 domain 同形状**的
//! 结构体，只给 `#[schema(value_type = …)]` 用，不参与任何序列化。
//!
//! 它们必须与 domain 的真实 serde 形状一致（字段名、可选性、嵌套）。这条约束不是靠人眼
//! 守的：[`crate::http`] 里的 `schema_matches_response` 测试把**真实响应 JSON**拿去对
//! spec 校验，多一个字段 / 少一个字段都会红。
//!
//! 约定：
//! - 字段名与 domain 一致（domain 全部是 snake_case，没有 `rename_all`）；
//! - `TenantId` / `EnvironmentId`（`struct X(Uuid)`）序列化成 uuid 字符串 → 这里写 `Uuid`；
//! - `ObservedAt(DateTime<Utc>)` 序列化成 RFC3339 → 这里写 `DateTime<Utc>`；
//! - `serde(skip_serializing_if)` 的字段在 JSON 里可能缺 → 文档里标成可选（`Option`）。
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(ToSchema)]
pub(crate) enum AgentHealthDoc {
    Healthy,
    Degraded,
    Protect,
    Unavailable,
}

#[derive(ToSchema)]
pub(crate) struct HostRuntimeStateDoc {
    pub host_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub boot_id: Option<String>,
    pub uptime_seconds: Option<i64>,
    pub loadavg_1m: Option<f64>,
    pub loadavg_5m: Option<f64>,
    pub loadavg_15m: Option<f64>,
    pub cpu_usage_pct: Option<f64>,
    pub memory_used_bytes: Option<i64>,
    pub memory_available_bytes: Option<i64>,
    pub process_count: Option<i64>,
    pub container_count: Option<i64>,
    pub agent_health: AgentHealthDoc,
}

#[derive(ToSchema)]
pub(crate) struct ProcessStateCountDoc {
    pub state: String,
    pub count: usize,
}

#[derive(ToSchema)]
pub(crate) struct HostProcessGroupViewDoc {
    pub executable: String,
    pub display_name: String,
    pub process_count: usize,
    pub total_memory_rss_kib: i64,
    pub dominant_state: Option<String>,
    pub state_summary: Vec<ProcessStateCountDoc>,
}

#[derive(ToSchema)]
pub(crate) struct ProcessRuntimeStateDoc {
    pub process_id: Uuid,
    pub tenant_id: Uuid,
    pub host_id: Uuid,
    pub container_id: Option<Uuid>,
    pub external_ref: Option<String>,
    pub pid: i32,
    pub executable: String,
    pub command_line: Option<String>,
    pub process_state: Option<String>,
    pub memory_rss_kib: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub stale_at: Option<DateTime<Utc>>,
    pub ingest_scope: Option<String>,
}

#[derive(ToSchema)]
pub(crate) enum AddressFamilyDoc {
    Ipv4,
    Ipv6,
}

#[derive(ToSchema)]
pub(crate) struct NetworkSegmentDoc {
    pub network_segment_id: Uuid,
    pub tenant_id: Uuid,
    pub network_domain_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub name: String,
    pub cidr: Option<String>,
    pub gateway_ip: Option<String>,
    pub address_family: AddressFamilyDoc,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(ToSchema)]
pub(crate) struct ValidityWindowDoc {
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
}

#[derive(ToSchema)]
pub(crate) struct HostNetAssocDoc {
    pub assoc_id: Uuid,
    pub tenant_id: Uuid,
    pub host_id: Uuid,
    pub network_segment_id: Uuid,
    pub ip_addr: String,
    pub iface_name: Option<String>,
    pub validity: ValidityWindowDoc,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(ToSchema)]
pub(crate) enum ServiceTypeDoc {
    Application,
    Data,
    Platform,
    Shared,
}

#[derive(ToSchema)]
pub(crate) enum ServiceBoundaryDoc {
    Internal,
    External,
    Partner,
    Saas,
}

#[derive(ToSchema)]
pub(crate) struct ServiceEntityDoc {
    pub service_id: Uuid,
    pub tenant_id: Uuid,
    pub business_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub subsystem_id: Option<Uuid>,
    pub name: String,
    pub namespace: Option<String>,
    pub service_type: ServiceTypeDoc,
    pub boundary: ServiceBoundaryDoc,
    pub provider: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(ToSchema)]
pub(crate) struct ServiceInstanceDoc {
    pub instance_id: Uuid,
    pub tenant_id: Uuid,
    pub service_id: Uuid,
    pub workload_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(ToSchema)]
pub(crate) enum RuntimeObjectTypeDoc {
    Process,
    Container,
    Pod,
}

#[derive(ToSchema)]
pub(crate) enum BindingScopeDoc {
    Declared,
    Observed,
    Inferred,
}

#[derive(ToSchema)]
pub(crate) enum ConfidenceDoc {
    High,
    Medium,
    Low,
}

#[derive(ToSchema)]
pub(crate) struct RuntimeBindingDoc {
    pub binding_id: Uuid,
    pub instance_id: Uuid,
    pub object_type: RuntimeObjectTypeDoc,
    pub object_id: Uuid,
    pub scope: BindingScopeDoc,
    pub confidence: ConfidenceDoc,
    pub source: String,
    pub validity: ValidityWindowDoc,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(ToSchema)]
pub(crate) struct InstanceEndpointDoc {
    pub endpoint_id: Uuid,
    pub tenant_id: Uuid,
    pub instance_id: Uuid,
    pub address: String,
    pub port: i32,
    pub protocol: String,
    pub source: String,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(ToSchema)]
pub(crate) struct HostServiceInstanceViewDoc {
    pub instance: ServiceInstanceDoc,
    pub bindings: Vec<RuntimeBindingDoc>,
    pub processes: Vec<ProcessRuntimeStateDoc>,
    pub endpoints: Vec<InstanceEndpointDoc>,
}

#[derive(ToSchema)]
pub(crate) struct HostServiceViewDoc {
    pub service: ServiceEntityDoc,
    pub instances: Vec<HostServiceInstanceViewDoc>,
}

#[derive(ToSchema)]
pub(crate) enum RuntimeTypeDoc {
    Containerd,
    Docker,
    CriO,
    Process,
    Unknown,
}

#[derive(ToSchema)]
pub(crate) struct ContainerRuntimeDoc {
    pub container_id: Uuid,
    pub tenant_id: Uuid,
    pub pod_id: Option<Uuid>,
    pub host_id: Uuid,
    pub runtime_type: RuntimeTypeDoc,
    pub runtime_namespace: Option<String>,
    pub container_name: Option<String>,
    pub image_ref: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_seen_at: DateTime<Utc>,
    pub external_ref: Option<String>,
    pub pod_uid: Option<String>,
    pub pod_name: Option<String>,
    pub k8s_namespace: Option<String>,
    pub stale_at: Option<DateTime<Utc>>,
    pub ingest_scope: Option<String>,
}

#[derive(ToSchema)]
pub(crate) enum ObjectKindDoc {
    Business,
    System,
    Subsystem,
    Service,
    NetworkDomain,
    NetworkSegment,
    HostNetAssoc,
    Cluster,
    Namespace,
    Workload,
    Pod,
    Host,
    Subject,
    ServiceInstance,
    RuntimeBinding,
}

#[derive(ToSchema)]
pub(crate) enum ResponsibilityRoleDoc {
    Owner,
    Maintainer,
    Oncall,
    Security,
}

#[derive(ToSchema)]
pub(crate) struct ResponsibilityAssignmentDoc {
    pub assignment_id: Uuid,
    pub tenant_id: Uuid,
    pub subject_id: Uuid,
    pub target_kind: ObjectKindDoc,
    pub target_id: Uuid,
    pub role: ResponsibilityRoleDoc,
    pub source: String,
    pub validity: ValidityWindowDoc,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
