use atlas_domain::{AtlasInputEnvelope, IngestEnvelope};
use atlas_storage::AsyncIngestStore;
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    IngestStore, RuntimeStore,
};
use chrono::{DateTime, Utc};
use orion_error::conversion::ConvErr;
use serde::{Deserialize, Serialize};

use crate::error::ApiResult;
use crate::ingest::{
    IngestJobRecord, extract_business_catalog_candidates, extract_host_candidates,
    extract_host_telemetry_candidates, extract_network_segment_candidates,
    extract_process_runtime_candidates, extract_process_telemetry_candidates,
    extract_responsibility_assignment_candidates, extract_subject_candidates,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineRunSummary {
    pub ingest_id: String,
    pub accepted_at: DateTime<Utc>,
    pub host_count: usize,
    pub network_count: usize,
    pub assoc_count: usize,
    /// 本次快照替换清扫标成 stale 的进程数（非 snapshot 模式恒为 0）。
    pub stale_process_count: u64,
}

pub struct TopologyIngestService<S> {
    store: S,
}

impl<S> TopologyIngestService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> TopologyIngestService<S>
where
    S: CatalogStore + RuntimeStore + IngestStore + GovernanceStore,
{
    pub fn submit_and_materialize(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record(&self.store, &envelope)?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let networks = extract_network_segment_candidates(&envelope)?.candidates;
        let processes = extract_process_runtime_candidates(&envelope)?.candidates;
        let business_catalog = extract_business_catalog_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let host_telemetry = extract_host_telemetry_candidates(&envelope)?.candidates;
        let process_telemetry = extract_process_telemetry_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog(&self.store, envelope.tenant_id).conv_err()?;
        let scope = envelope.ingest_scope();
        materialize_business_catalog(&self.store, business_catalog, accepted_at).conv_err()?;
        let materialized =
            materialize_candidates(&self.store, &mut catalog, hosts, networks, accepted_at)
                .conv_err()?;
        let process_sweep =
            materialize_processes(&self.store, &mut catalog, processes, accepted_at, &scope)
                .conv_err()?;
        let stale_process_count =
            sweep_stale_processes(&self.store, &envelope, &process_sweep, &scope, accepted_at)
                .conv_err()?;
        materialize_host_telemetry(&self.store, &mut catalog, host_telemetry).conv_err()?;
        materialize_process_telemetry(&self.store, &mut catalog, process_telemetry).conv_err()?;
        materialize_subjects_and_assignments(
            &self.store,
            envelope.tenant_id,
            subjects,
            assignments,
            accepted_at,
        )
        .conv_err()?;

        Ok((
            record,
            PipelineRunSummary {
                ingest_id: envelope.ingest_id,
                accepted_at,
                host_count: materialized.0,
                network_count: materialized.1,
                assoc_count: materialized.2,
                stale_process_count,
            },
        ))
    }

    pub fn submit_atlas_input_and_materialize(
        &self,
        input: AtlasInputEnvelope,
        tenant_id: atlas_domain::TenantId,
        environment_id: Option<atlas_domain::EnvironmentId>,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        input.validate().conv_err()?;
        self.submit_and_materialize(input.into_ingest_envelope(
            tenant_id,
            environment_id,
            Utc::now(),
        ))
    }
}

impl<S> TopologyIngestService<S>
where
    S: AsyncCatalogStore + AsyncRuntimeStore + AsyncIngestStore + AsyncGovernanceStore,
{
    pub async fn submit_and_materialize_async(
        &self,
        envelope: IngestEnvelope,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        let accepted_at = envelope.received_at;
        let record = validate_and_record_async(&self.store, &envelope).await?;

        let hosts = extract_host_candidates(&envelope)?.candidates;
        let networks = extract_network_segment_candidates(&envelope)?.candidates;
        let processes = extract_process_runtime_candidates(&envelope)?.candidates;
        let business_catalog = extract_business_catalog_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let host_telemetry = extract_host_telemetry_candidates(&envelope)?.candidates;
        let process_telemetry = extract_process_telemetry_candidates(&envelope)?.candidates;
        let subjects = extract_subject_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();
        let assignments = extract_responsibility_assignment_candidates(&envelope)
            .map(|result| result.candidates)
            .unwrap_or_default();

        let mut catalog = hydrate_catalog_async(&self.store, envelope.tenant_id)
            .await
            .conv_err()?;
        let scope = envelope.ingest_scope();
        materialize_business_catalog_async(&self.store, business_catalog, accepted_at)
            .await
            .conv_err()?;
        let materialized =
            materialize_candidates_async(&self.store, &mut catalog, hosts, networks, accepted_at)
                .await
                .conv_err()?;
        let process_sweep =
            materialize_processes_async(&self.store, &mut catalog, processes, accepted_at, &scope)
                .await
                .conv_err()?;
        let stale_process_count = sweep_stale_processes_async(
            &self.store,
            &envelope,
            &process_sweep,
            &scope,
            accepted_at,
        )
        .await
        .conv_err()?;
        materialize_host_telemetry_async(&self.store, &mut catalog, host_telemetry)
            .await
            .conv_err()?;
        materialize_process_telemetry_async(&self.store, &mut catalog, process_telemetry)
            .await
            .conv_err()?;
        materialize_subjects_and_assignments_async(
            &self.store,
            envelope.tenant_id,
            subjects,
            assignments,
            accepted_at,
        )
        .await
        .conv_err()?;

        Ok((
            record,
            PipelineRunSummary {
                ingest_id: envelope.ingest_id,
                accepted_at,
                host_count: materialized.0,
                network_count: materialized.1,
                assoc_count: materialized.2,
                stale_process_count,
            },
        ))
    }

    pub async fn submit_atlas_input_and_materialize_async(
        &self,
        input: AtlasInputEnvelope,
        tenant_id: atlas_domain::TenantId,
        environment_id: Option<atlas_domain::EnvironmentId>,
    ) -> ApiResult<(IngestJobRecord, PipelineRunSummary)> {
        input.validate().conv_err()?;
        self.submit_and_materialize_async(input.into_ingest_envelope(
            tenant_id,
            environment_id,
            Utc::now(),
        ))
        .await
    }
}

mod materialize;
use materialize::{
    ProcessSweep, hydrate_catalog, hydrate_catalog_async, mark_stale_processes,
    mark_stale_processes_async, materialize_business_catalog, materialize_business_catalog_async,
    materialize_candidates, materialize_candidates_async, materialize_host_telemetry,
    materialize_host_telemetry_async, materialize_process_telemetry,
    materialize_process_telemetry_async, materialize_processes, materialize_processes_async,
    materialize_subjects_and_assignments, materialize_subjects_and_assignments_async,
    validate_and_record, validate_and_record_async,
};

/// 只在 `snapshot` 模式下做替换清扫。
///
/// `incremental` / `window` / `correction` 描述的都是**部分**观测，拿去当「全集」比对会把
/// 没投递的对象全标死。
fn sweep_stale_processes<S>(
    store: &S,
    envelope: &IngestEnvelope,
    sweep: &ProcessSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: RuntimeStore,
{
    if envelope.ingest_mode != atlas_domain::IngestMode::Snapshot {
        return Ok(0);
    }
    Ok(mark_stale_processes(store, sweep, scope, at)
        .conv_err()?
        .marked)
}

async fn sweep_stale_processes_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
    sweep: &ProcessSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> ApiResult<u64>
where
    S: AsyncRuntimeStore,
{
    if envelope.ingest_mode != atlas_domain::IngestMode::Snapshot {
        return Ok(0);
    }
    Ok(mark_stale_processes_async(store, sweep, scope, at)
        .await
        .conv_err()?
        .marked)
}

#[cfg(test)]
mod tests;
