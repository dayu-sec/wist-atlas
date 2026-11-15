use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, ServiceEntity, ServiceInstance,
    Subject, TenantId,
};
use uuid::Uuid;

use crate::{
    CatalogStore, GovernanceStore, IngestStats, IngestStore, Page, RuntimeStore, StorageResult,
    not_configured,
};

use super::{InMemoryTopologyStore, IngestJobEntry};

impl IngestStore for InMemoryTopologyStore {
    fn ingest_stats(
        &self,
        tenant_id: TenantId,
        since: chrono::DateTime<chrono::Utc>,
    ) -> StorageResult<IngestStats> {
        self.with_state(|state| {
            let mut stats = IngestStats::default();
            for job in state
                .ingest_jobs
                .iter()
                .filter(|job| job.tenant_id == tenant_id && job.received_at >= since)
            {
                stats.received += 1;
                stats.last_received_at = Some(match stats.last_received_at {
                    Some(current) if current >= job.received_at => current,
                    _ => job.received_at,
                });
            }
            stats
        })
    }

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
