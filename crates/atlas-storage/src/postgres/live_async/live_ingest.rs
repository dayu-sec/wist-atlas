use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment, ObjectKind,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncIngestStore, AsyncRuntimeStore, Page,
    StorageResult, not_configured,
};

use super::super::{LivePostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

impl AsyncIngestStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn record_ingest_job(&self, entry: IngestJobEntry) -> StorageResult<()> {
        self.executor
            .exec_async(
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
            )
            .await?;
        Ok(())
    }

    async fn get_ingest_job(&self, ingest_id: &str) -> StorageResult<Option<IngestJobEntry>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_INGEST_JOB, &[ingest_id.to_string()])
            .await?;
        Ok(rows
            .into_iter()
            .next()
            .map(|row| decode_ingest_job(&row))
            .transpose()?)
    }
}
