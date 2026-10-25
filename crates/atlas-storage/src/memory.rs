use std::sync::{Arc, Mutex};

use atlas_domain::{
    ContainerRuntime, HostInventory, HostNetAssoc, HostRuntimeState, InstanceEndpoint,
    NetworkDomain, NetworkSegment, ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding,
    ServiceEntity, ServiceInstance, Subject, TenantId,
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

pub trait IngestStore {
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()>;
    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>>;
}

#[allow(async_fn_in_trait)]
pub trait AsyncIngestStore: Sync {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()>;
    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>>;
}

#[derive(Debug, Default)]
struct MemoryState {
    hosts: Vec<HostInventory>,
    network_domains: Vec<NetworkDomain>,
    network_segments: Vec<NetworkSegment>,
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

#[cfg(test)]
mod tests;
