use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance,
    Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult, not_configured,
};

use super::{InMemoryTopologyStore, IngestJobEntry};

impl IngestStore for InMemoryTopologyStore {
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .ingest_jobs
                .iter_mut()
                .find(|item| item.ingest_id == entry.ingest_id)
            {
                *existing = entry;
            } else {
                state.ingest_jobs.push(entry);
            }
        })
    }

    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        self.with_state(|state| {
            state
                .ingest_jobs
                .iter()
                .find(|item| item.ingest_id == ingest_id)
                .cloned()
        })
    }
}
