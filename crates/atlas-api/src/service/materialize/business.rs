use atlas_domain::{
    BindingScope, BusinessCatalogCandidate, Confidence, ProcessRuntimeCandidate,
    ProcessRuntimeState, RuntimeBinding, RuntimeObjectType, ServiceEntity, ServiceInstance,
    ValidityWindow,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncRuntimeStore, CatalogStore, RuntimeStore, StorageResult,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{find_service_by_ref, find_service_by_ref_async, stable_uuid};

pub(crate) fn materialize_process_binding<S>(
    store: &S,
    candidate: &ProcessRuntimeCandidate,
    process: &ProcessRuntimeState,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    let Some(service_ref) = candidate.service_ref.as_deref() else {
        return Ok(());
    };

    let service = find_service_by_ref(store, candidate.tenant_id, service_ref)?;
    let Some(service) = service else {
        return Ok(());
    };

    let instance_key = candidate
        .instance_key
        .as_deref()
        .or(candidate.identity.as_deref())
        .unwrap_or("process");
    let instance_id = stable_uuid(
        "service_instance",
        &format!(
            "{}:{}:{}",
            candidate.tenant_id.0, service.service_id, instance_key
        ),
    );
    let instance_started_at = process.started_at;
    let binding_valid_from = process.observed_at.0;
    let instance = ServiceInstance {
        instance_id,
        tenant_id: candidate.tenant_id,
        service_id: service.service_id,
        workload_id: None,
        started_at: instance_started_at,
        ended_at: None,
        last_seen_at: process.observed_at.0,
    };
    store.upsert_service_instance(&instance)?;

    let binding_id = stable_uuid(
        "runtime_binding",
        &format!(
            "{}:{}:{}",
            instance.instance_id, process.process_id, "process"
        ),
    );
    let binding = RuntimeBinding {
        binding_id,
        instance_id: instance.instance_id,
        object_type: RuntimeObjectType::Process,
        object_id: process.process_id,
        scope: BindingScope::Observed,
        confidence: Confidence::Medium,
        source: format!("{:?}", candidate.source_kind).to_lowercase(),
        validity: ValidityWindow {
            valid_from: binding_valid_from,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };
    store.upsert_runtime_binding(&binding)?;

    Ok(())
}

pub(crate) fn materialize_business_catalog<S>(
    store: &S,
    candidates: Vec<BusinessCatalogCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore,
{
    for candidate in candidates {
        let Some(service_name) = candidate.service_name.as_deref() else {
            continue;
        };
        let service_ref = candidate
            .external_ref
            .clone()
            .unwrap_or_else(|| service_name.to_string());
        let service = ServiceEntity {
            service_id: stable_uuid(
                "service",
                &format!("{}:{}", candidate.tenant_id.0, service_ref),
            ),
            tenant_id: candidate.tenant_id,
            business_id: None,
            system_id: None,
            subsystem_id: None,
            name: service_name.to_string(),
            namespace: None,
            service_type: candidate
                .service_type
                .unwrap_or(atlas_domain::ServiceType::Application),
            boundary: candidate
                .boundary
                .unwrap_or(atlas_domain::ServiceBoundary::Internal),
            provider: None,
            external_ref: candidate.external_ref.clone(),
            created_at: now,
            updated_at: now,
        };
        store.upsert_service(&service)?;
    }
    Ok(())
}

pub(crate) async fn materialize_business_catalog_async<S>(
    store: &S,
    candidates: Vec<BusinessCatalogCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore,
{
    for candidate in candidates {
        let Some(service_name) = candidate.service_name.as_deref() else {
            continue;
        };
        let service_ref = candidate
            .external_ref
            .clone()
            .unwrap_or_else(|| service_name.to_string());
        let service = ServiceEntity {
            service_id: stable_uuid(
                "service",
                &format!("{}:{}", candidate.tenant_id.0, service_ref),
            ),
            tenant_id: candidate.tenant_id,
            business_id: None,
            system_id: None,
            subsystem_id: None,
            name: service_name.to_string(),
            namespace: None,
            service_type: candidate
                .service_type
                .unwrap_or(atlas_domain::ServiceType::Application),
            boundary: candidate
                .boundary
                .unwrap_or(atlas_domain::ServiceBoundary::Internal),
            provider: None,
            external_ref: candidate.external_ref.clone(),
            created_at: now,
            updated_at: now,
        };
        atlas_storage::AsyncCatalogStore::upsert_service(store, &service).await?;
    }
    Ok(())
}

pub(crate) async fn materialize_process_binding_async<S>(
    store: &S,
    candidate: &ProcessRuntimeCandidate,
    process: &ProcessRuntimeState,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let Some(service_ref) = candidate.service_ref.as_deref() else {
        return Ok(());
    };

    let service = find_service_by_ref_async(store, candidate.tenant_id, service_ref).await?;
    let Some(service) = service else {
        return Ok(());
    };

    let instance_key = candidate
        .instance_key
        .as_deref()
        .or(candidate.identity.as_deref())
        .unwrap_or("process");
    let instance_id = stable_uuid(
        "service_instance",
        &format!(
            "{}:{}:{}",
            candidate.tenant_id.0, service.service_id, instance_key
        ),
    );
    let instance_started_at = process.started_at;
    let binding_valid_from = process.observed_at.0;
    let instance = ServiceInstance {
        instance_id,
        tenant_id: candidate.tenant_id,
        service_id: service.service_id,
        workload_id: None,
        started_at: instance_started_at,
        ended_at: None,
        last_seen_at: process.observed_at.0,
    };
    atlas_storage::AsyncRuntimeStore::upsert_service_instance(store, &instance).await?;

    let binding_id = stable_uuid(
        "runtime_binding",
        &format!(
            "{}:{}:{}",
            instance.instance_id, process.process_id, "process"
        ),
    );
    let binding = RuntimeBinding {
        binding_id,
        instance_id: instance.instance_id,
        object_type: RuntimeObjectType::Process,
        object_id: process.process_id,
        scope: BindingScope::Observed,
        confidence: Confidence::Medium,
        source: format!("{:?}", candidate.source_kind).to_lowercase(),
        validity: ValidityWindow {
            valid_from: binding_valid_from,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };
    atlas_storage::AsyncRuntimeStore::upsert_runtime_binding(store, &binding).await?;

    Ok(())
}
