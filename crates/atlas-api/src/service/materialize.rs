use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use atlas_domain::{
    AgentHealth, BindingScope, BusinessCatalogCandidate, Confidence, HostCandidate,
    HostRuntimeState, HostTelemetryCandidate, IngestEnvelope, NetworkSegmentCandidate,
    ProcessRuntimeCandidate, ProcessRuntimeState, ProcessTelemetryCandidate,
    ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType, ServiceEntity, ServiceInstance,
    Subject, SubjectCandidate, ValidityWindow,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncIngestStore, AsyncRuntimeStore, CatalogStore,
    InMemoryTopologyStore, IngestJobEntry, IngestStore, RuntimeStore, StorageResult,
};
use chrono::{DateTime, Utc};
use orion_error::prelude::SourceErr;
use uuid::Uuid;

use super::TopologyIngestService;
use crate::error::{ApiReason, ApiResult, missing_payload, unsupported_ingest_mode};
use crate::ingest::{IngestJobRecord, IngestJobStatus};
use crate::pipeline::{InMemoryCatalog, materialize_host_network, resolve_host_candidate};
use crate::recorder_failed;

pub(super) fn validate_and_record<S>(
    store: &S,
    envelope: &IngestEnvelope,
) -> ApiResult<IngestJobRecord>
where
    S: IngestStore,
{
    if envelope.payload_inline.is_none() && envelope.payload_ref.is_none() {
        return Err(missing_payload());
    }

    if envelope.ingest_mode == atlas_domain::IngestMode::Delta {
        return Err(unsupported_ingest_mode());
    }

    let record = IngestJobRecord {
        ingest_id: envelope.ingest_id.clone(),
        tenant_id: envelope.tenant_id,
        source_kind: envelope.source_kind,
        source_name: envelope.source_name.clone(),
        received_at: envelope.received_at,
        status: IngestJobStatus::Accepted,
        payload_ref: envelope.payload_ref.clone(),
        error: None,
    };

    store
        .record_ingest_job(IngestJobEntry {
            ingest_id: record.ingest_id.clone(),
            tenant_id: record.tenant_id,
            source_name: record.source_name.clone(),
            source_kind: format!("{:?}", record.source_kind).to_lowercase(),
            received_at: record.received_at,
            status: "accepted".to_string(),
            payload_ref: record.payload_ref.clone(),
            error: None,
        })
        .source_err(ApiReason::IngestRejected, "record ingest job")?;

    Ok(record)
}

pub(super) async fn validate_and_record_async<S>(
    store: &S,
    envelope: &IngestEnvelope,
) -> ApiResult<IngestJobRecord>
where
    S: AsyncIngestStore,
{
    if envelope.payload_inline.is_none() && envelope.payload_ref.is_none() {
        return Err(missing_payload());
    }

    if envelope.ingest_mode == atlas_domain::IngestMode::Delta {
        return Err(unsupported_ingest_mode());
    }

    let record = IngestJobRecord {
        ingest_id: envelope.ingest_id.clone(),
        tenant_id: envelope.tenant_id,
        source_kind: envelope.source_kind,
        source_name: envelope.source_name.clone(),
        received_at: envelope.received_at,
        status: IngestJobStatus::Accepted,
        payload_ref: envelope.payload_ref.clone(),
        error: None,
    };

    atlas_storage::AsyncIngestStore::record_ingest_job(
        store,
        IngestJobEntry {
            ingest_id: record.ingest_id.clone(),
            tenant_id: record.tenant_id,
            source_name: record.source_name.clone(),
            source_kind: format!("{:?}", record.source_kind).to_lowercase(),
            received_at: record.received_at,
            status: "accepted".to_string(),
            payload_ref: record.payload_ref.clone(),
            error: None,
        },
    )
    .await
    .map_err(|err| recorder_failed(err.to_string()))?;

    Ok(record)
}

pub(super) fn hydrate_catalog<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<InMemoryCatalog>
where
    S: CatalogStore + RuntimeStore,
{
    let hosts = store.list_hosts(tenant_id, atlas_storage::Page::default())?;
    let network_segments =
        store.list_network_segments(tenant_id, atlas_storage::Page::default())?;

    let mut host_net_assocs = Vec::new();
    for host in &hosts {
        host_net_assocs
            .extend(store.list_host_net_assocs(host.host_id, atlas_storage::Page::default())?);
    }

    Ok(InMemoryCatalog {
        hosts,
        network_domains: Vec::new(),
        network_segments,
        host_net_assocs,
    })
}

pub(super) async fn hydrate_catalog_async<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<InMemoryCatalog>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let hosts = atlas_storage::AsyncCatalogStore::list_hosts(
        store,
        tenant_id,
        atlas_storage::Page::default(),
    )
    .await?;
    let network_segments = atlas_storage::AsyncCatalogStore::list_network_segments(
        store,
        tenant_id,
        atlas_storage::Page::default(),
    )
    .await?;

    let mut host_net_assocs = Vec::new();
    for host in &hosts {
        host_net_assocs.extend(
            atlas_storage::AsyncRuntimeStore::list_host_net_assocs(
                store,
                host.host_id,
                atlas_storage::Page::default(),
            )
            .await?,
        );
    }

    Ok(InMemoryCatalog {
        hosts,
        network_domains: Vec::new(),
        network_segments,
        host_net_assocs,
    })
}

mod business;
mod governance;
mod host;
mod process;
mod telemetry;

pub(super) use business::{materialize_business_catalog, materialize_business_catalog_async};
pub(super) use governance::{
    materialize_subjects_and_assignments, materialize_subjects_and_assignments_async,
};
pub(super) use host::{
    HOST_STALE_AFTER_MISSES, HostStaleSweep, MaterializedHosts, mark_stale_hosts,
    mark_stale_hosts_async, materialize_candidates, materialize_candidates_async,
};
pub(super) use process::{
    ProcessSweep, StaleSweep, mark_stale_processes, mark_stale_processes_async,
    materialize_processes, materialize_processes_async,
};
pub(super) use telemetry::{
    materialize_host_telemetry, materialize_host_telemetry_async, materialize_process_telemetry,
    materialize_process_telemetry_async,
};

pub(super) fn stable_uuid(namespace: &str, key: &str) -> Uuid {
    let mut h1 = DefaultHasher::new();
    ("wist-atlas", namespace, key, "a").hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    ("wist-atlas", namespace, key, "b").hash(&mut h2);
    let hi = h1.finish().to_be_bytes();
    let lo = h2.finish().to_be_bytes();
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hi);
    bytes[8..].copy_from_slice(&lo);
    Uuid::from_bytes(bytes)
}

pub(super) fn find_service_by_ref<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
    service_ref: &str,
) -> StorageResult<Option<ServiceEntity>>
where
    S: CatalogStore,
{
    let services = store.list_services(tenant_id, atlas_storage::Page::default())?;
    Ok(services.into_iter().find(|service| {
        service.external_ref.as_deref() == Some(service_ref) || service.name == service_ref
    }))
}

pub(super) async fn find_service_by_ref_async<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
    service_ref: &str,
) -> StorageResult<Option<ServiceEntity>>
where
    S: AsyncCatalogStore,
{
    let services = atlas_storage::AsyncCatalogStore::list_services(
        store,
        tenant_id,
        atlas_storage::Page::default(),
    )
    .await?;
    Ok(services.into_iter().find(|service| {
        service.external_ref.as_deref() == Some(service_ref) || service.name == service_ref
    }))
}

impl TopologyIngestService<InMemoryTopologyStore> {
    pub fn new_in_memory() -> Self {
        Self {
            store: InMemoryTopologyStore::default(),
        }
    }

    pub fn store(&self) -> &InMemoryTopologyStore {
        &self.store
    }
}
