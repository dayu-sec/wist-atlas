use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{EnvironmentId, TenantId, ValidityWindow};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceBoundary {
    Internal,
    External,
    Partner,
    Saas,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceType {
    Application,
    Data,
    Platform,
    Shared,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkloadKind {
    Deployment,
    StatefulSet,
    DaemonSet,
    Job,
    CronJob,
    BareProcess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubjectType {
    User,
    Team,
    Rotation,
    ServiceAccount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkDomainKind {
    Lan,
    Wan,
    Vpc,
    Vnet,
    Vlan,
    Overlay,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessDomain {
    pub business_id: Uuid,
    pub tenant_id: TenantId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemBoundary {
    pub system_id: Uuid,
    pub tenant_id: TenantId,
    pub business_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subsystem {
    pub subsystem_id: Uuid,
    pub tenant_id: TenantId,
    pub system_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceEntity {
    pub service_id: Uuid,
    pub tenant_id: TenantId,
    pub business_id: Option<Uuid>,
    pub system_id: Option<Uuid>,
    pub subsystem_id: Option<Uuid>,
    pub name: String,
    pub namespace: Option<String>,
    pub service_type: ServiceType,
    pub boundary: ServiceBoundary,
    pub provider: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClusterInventory {
    pub cluster_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub name: String,
    pub provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamespaceInventory {
    pub namespace_id: Uuid,
    pub tenant_id: TenantId,
    pub cluster_id: Uuid,
    pub name: String,
    pub environment_id: Option<EnvironmentId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadEntity {
    pub workload_id: Uuid,
    pub tenant_id: TenantId,
    pub cluster_id: Uuid,
    pub namespace_id: Uuid,
    pub service_id: Option<Uuid>,
    pub kind: WorkloadKind,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PodInventory {
    pub pod_id: Uuid,
    pub tenant_id: TenantId,
    pub cluster_id: Uuid,
    pub namespace_id: Uuid,
    pub workload_id: Option<Uuid>,
    pub pod_uid: String,
    pub pod_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostInventory {
    pub host_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub host_name: String,
    pub machine_id: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_inventory_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkDomain {
    pub network_domain_id: Uuid,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub name: String,
    pub kind: NetworkDomainKind,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkSegment {
    pub network_segment_id: Uuid,
    pub tenant_id: TenantId,
    pub network_domain_id: Option<Uuid>,
    pub environment_id: Option<EnvironmentId>,
    pub name: String,
    pub cidr: Option<String>,
    pub gateway_ip: Option<String>,
    pub address_family: AddressFamily,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostNetAssoc {
    pub assoc_id: Uuid,
    pub tenant_id: TenantId,
    pub host_id: Uuid,
    pub network_segment_id: Uuid,
    pub ip_addr: String,
    pub iface_name: Option<String>,
    pub validity: ValidityWindow,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subject {
    pub subject_id: Uuid,
    pub tenant_id: TenantId,
    pub subject_type: SubjectType,
    pub display_name: String,
    pub external_ref: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
