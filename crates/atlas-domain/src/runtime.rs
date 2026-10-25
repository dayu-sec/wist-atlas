use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Confidence, ObservedAt, TenantId, ValidityWindow};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentHealth {
    Healthy,
    Degraded,
    Protect,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeType {
    Containerd,
    Docker,
    CriO,
    Process,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuntimeObjectType {
    Process,
    Container,
    Pod,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BindingScope {
    Declared,
    Observed,
    Inferred,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BindingEvidenceType {
    ExternalId,
    Label,
    Annotation,
    WorkloadRef,
    ProcessFingerprint,
    RuntimeMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostRuntimeState {
    pub host_id: Uuid,
    pub observed_at: ObservedAt,
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
    pub agent_health: AgentHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceInstance {
    pub instance_id: Uuid,
    pub tenant_id: TenantId,
    pub service_id: Uuid,
    pub workload_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContainerRuntime {
    pub container_id: Uuid,
    pub tenant_id: TenantId,
    pub pod_id: Option<Uuid>,
    pub host_id: Uuid,
    pub runtime_type: RuntimeType,
    pub runtime_namespace: Option<String>,
    pub container_name: Option<String>,
    pub image_ref: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessRuntimeState {
    pub process_id: Uuid,
    pub tenant_id: TenantId,
    pub host_id: Uuid,
    pub container_id: Option<Uuid>,
    pub external_ref: Option<String>,
    pub pid: i32,
    pub executable: String,
    pub command_line: Option<String>,
    pub process_state: Option<String>,
    pub memory_rss_kib: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub observed_at: ObservedAt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBinding {
    pub binding_id: Uuid,
    pub instance_id: Uuid,
    pub object_type: RuntimeObjectType,
    pub object_id: Uuid,
    pub scope: BindingScope,
    pub confidence: Confidence,
    pub source: String,
    pub validity: ValidityWindow,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBindingEvidence {
    pub evidence_id: Uuid,
    pub binding_id: Uuid,
    pub evidence_type: BindingEvidenceType,
    pub evidence_value: String,
    pub score: Option<i32>,
    pub observed_at: Option<ObservedAt>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadPodMembership {
    pub membership_id: Uuid,
    pub workload_id: Uuid,
    pub pod_id: Uuid,
    pub validity: ValidityWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PodPlacement {
    pub placement_id: Uuid,
    pub pod_id: Uuid,
    pub host_id: Uuid,
    pub validity: ValidityWindow,
}
