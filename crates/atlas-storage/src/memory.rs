use std::sync::{Arc, Mutex};

use atlas_domain::{
    ContainerRuntime, ElasticIp, HostInventory, HostNetAssoc, HostRuntimeState, InstanceEndpoint,
    NetworkDomain, NetworkSegment, ProcessRuntimeState, ResponsibilityAssignment, RetirementOwner,
    RuntimeBinding, ServiceEntity, ServiceInstance, SoftwareArtifact, SoftwareArtifactPath,
    SoftwareFingerprint, SoftwareInstance, SoftwareInstanceObservation, Subject, TenantId,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, Page, RuntimeStore, StorageResult, lock_failed, not_configured,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestJobEntry {
    pub ingest_id: String,
    pub tenant_id: TenantId,
    pub source_name: String,
    pub source_kind: String,
    pub received_at: DateTime<Utc>,
    pub status: String,
    pub payload_ref: Option<String>,
    pub error: Option<String>,
}

/// 窗口内的投递计数（看板用）。
///
/// 一行 `ingest_job` = 数据面发来的**一批**（一次 `POST /internal/atlas`，
/// `batch_size > 1` 时一批里有多条记录），所以这里的单位是批而不是条。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestStats {
    pub received: u64,
    pub last_received_at: Option<DateTime<Utc>>,
}

pub trait IngestStore {
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()>;
    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>>;
    /// 窗口内的投递批次计数（看板用）：`since` 之前的不算。
    fn ingest_stats(&self, tenant_id: TenantId, since: DateTime<Utc>)
    -> StorageResult<IngestStats>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncIngestStore: Sync {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()>;
    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>>;
    async fn ingest_stats(
        &self,
        tenant_id: TenantId,
        since: DateTime<Utc>,
    ) -> StorageResult<IngestStats>;
}

#[derive(Debug, Default)]
struct MemoryState {
    hosts: Vec<HostInventory>,
    elastic_ips: Vec<ElasticIp>,
    network_domains: Vec<NetworkDomain>,
    network_segments: Vec<NetworkSegment>,
    retirement_owners: Vec<RetirementOwner>,
    host_net_assocs: Vec<HostNetAssoc>,
    host_runtime_states: Vec<HostRuntimeState>,
    process_runtime_states: Vec<ProcessRuntimeState>,
    container_runtimes: Vec<ContainerRuntime>,
    services: Vec<ServiceEntity>,
    service_instances: Vec<ServiceInstance>,
    instance_endpoints: Vec<InstanceEndpoint>,
    runtime_bindings: Vec<RuntimeBinding>,
    subjects: Vec<Subject>,
    responsibility_assignments: Vec<ResponsibilityAssignment>,
    ingest_jobs: Vec<IngestJobEntry>,
    software_artifacts: Vec<SoftwareArtifact>,
    software_instances: Vec<SoftwareInstance>,
    software_fingerprints: Vec<SoftwareFingerprint>,
    software_instance_observations: Vec<SoftwareInstanceObservation>,
    software_artifact_paths: Vec<SoftwareArtifactPath>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryTopologyStore {
    state: Arc<Mutex<MemoryState>>,
}

impl InMemoryTopologyStore {
    fn with_state<T>(&self, f: impl FnOnce(&mut MemoryState) -> T) -> StorageResult<T> {
        let mut state = self.state.lock().map_err(|_| lock_failed())?;
        Ok(f(&mut state))
    }
}

mod async_impl;

mod catalog;
mod governance;
mod ingest;
mod runtime;
mod software;

#[cfg(test)]
mod tests;
