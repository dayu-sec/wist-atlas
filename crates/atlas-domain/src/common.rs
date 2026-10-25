use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TenantId(pub Uuid);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EnvironmentId(pub Uuid);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservedAt(pub DateTime<Utc>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidityWindow {
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceKind {
    Manual,
    BatchImport,
    EdgeDiscovery,
    ExternalSync,
    TelemetrySummary,
    RuleDerived,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResolutionStatus {
    Matched,
    Created,
    Unresolved,
    Conflicting,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectKind {
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRef {
    pub kind: ObjectKind,
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentifierMatch {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolutionResult {
    pub object_kind: ObjectKind,
    pub status: ResolutionStatus,
    pub matched_id: Option<Uuid>,
    pub confidence: Confidence,
    pub rule_name: String,
    pub matched_identifiers: Vec<IdentifierMatch>,
    pub conflicting_ids: Vec<Uuid>,
}
