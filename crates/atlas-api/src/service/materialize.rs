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

/// 目录页大小。这个模块里的目录读取**必须翻页取全** —— 库里有 100 台以上的主机、
/// 100 套以上的服务时，只取第一页会让新增/关联的对象静默失败（“主机解析不到”）。
const CATALOG_PAGE_LIMIT: u32 = 200;

pub(super) fn hydrate_catalog<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<InMemoryCatalog>
where
    S: CatalogStore + RuntimeStore,
{
    let hosts = list_all_hosts(store, tenant_id)?;
    let network_segments = list_all_network_segments(store, tenant_id)?;

    let mut host_net_assocs = Vec::new();
    for host in &hosts {
        host_net_assocs.extend(list_all_host_net_assocs(store, host.host_id)?);
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
    let hosts = list_all_hosts_async(store, tenant_id).await?;
    let network_segments = list_all_network_segments_async(store, tenant_id).await?;

    let mut host_net_assocs = Vec::new();
    for host in &hosts {
        host_net_assocs.extend(list_all_host_net_assocs_async(store, host.host_id).await?);
    }

    Ok(InMemoryCatalog {
        hosts,
        network_domains: Vec::new(),
        network_segments,
        host_net_assocs,
    })
}

pub(super) fn list_all_hosts<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<Vec<atlas_domain::HostInventory>>
where
    S: CatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_hosts(
            tenant_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) async fn list_all_hosts_async<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<Vec<atlas_domain::HostInventory>>
where
    S: AsyncCatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncCatalogStore::list_hosts(
            store,
            tenant_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) fn list_all_network_segments<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<Vec<atlas_domain::NetworkSegment>>
where
    S: CatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_network_segments(
            tenant_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) async fn list_all_network_segments_async<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<Vec<atlas_domain::NetworkSegment>>
where
    S: AsyncCatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncCatalogStore::list_network_segments(
            store,
            tenant_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) fn list_all_services<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<Vec<ServiceEntity>>
where
    S: CatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_services(
            tenant_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) async fn list_all_services_async<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
) -> StorageResult<Vec<ServiceEntity>>
where
    S: AsyncCatalogStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncCatalogStore::list_services(
            store,
            tenant_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) fn list_all_host_net_assocs<S>(
    store: &S,
    host_id: Uuid,
) -> StorageResult<Vec<atlas_domain::HostNetAssoc>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_host_net_assocs(
            host_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

pub(super) async fn list_all_host_net_assocs_async<S>(
    store: &S,
    host_id: Uuid,
) -> StorageResult<Vec<atlas_domain::HostNetAssoc>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_host_net_assocs(
            store,
            host_id,
            atlas_storage::Page {
                limit: CATALOG_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < CATALOG_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

mod business;
mod container;
mod endpoint;
mod governance;
mod host;
mod process;
mod telemetry;

pub(super) use business::{materialize_business_catalog, materialize_business_catalog_async};
pub(super) use container::{
    ContainerSweep, container_uuid, mark_stale_containers, mark_stale_containers_async,
    materialize_containers, materialize_containers_async,
};
pub(super) use endpoint::{
    EndpointSweep, materialize_service_endpoints, materialize_service_endpoints_async,
};
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
    // 必须翻页：库里的服务超过一页时，只取第一页会让后面那些服务永不被关联到，
    // 表现为「业务目录里写了服务名，但服务没挂上业务」。
    let services = list_all_services(store, tenant_id)?;
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
    let services = list_all_services_async(store, tenant_id).await?;
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
