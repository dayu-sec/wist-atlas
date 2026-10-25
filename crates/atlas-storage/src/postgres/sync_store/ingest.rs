use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment, ObjectKind,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{
    CatalogStore, GovernanceStore, IngestStore, Page, RuntimeStore, StorageResult, not_configured,
};

use super::super::{PostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

impl<E> IngestStore for PostgresTopologyStore<E>
where
    E: PostgresExecutor,
{
    fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        self.executor.exec(
            sql::UPSERT_INGEST_JOB,
            &[
                entry.ingest_id,
                entry.tenant_id.0.to_string(),
                entry.source_kind,
                entry.source_name,
                entry.received_at.to_rfc3339(),
                entry.status,
                entry.payload_ref.unwrap_or_default(),
                entry.error.unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        let rows = self
            .executor
            .query_rows(sql::GET_INGEST_JOB, &[ingest_id.to_string()])?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_ingest_job(&row))
            .transpose()?)
    }
}
