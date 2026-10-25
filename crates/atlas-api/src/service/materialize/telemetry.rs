use atlas_domain::{
    AgentHealth, BindingScope, BusinessCatalogCandidate, Confidence, HostCandidate,
    HostRuntimeState, HostTelemetryCandidate, NetworkSegmentCandidate, ProcessRuntimeCandidate,
    ProcessRuntimeState, ProcessTelemetryCandidate, ResponsibilityAssignment, RuntimeBinding,
    RuntimeObjectType, ServiceEntity, ServiceInstance, Subject, SubjectCandidate, ValidityWindow,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore,
    InMemoryTopologyStore, RuntimeStore, StorageResult,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{find_service_by_ref, find_service_by_ref_async, stable_uuid};
use crate::pipeline::{InMemoryCatalog, materialize_host_network, resolve_host_candidate};

pub(crate) fn materialize_process_telemetry<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<ProcessTelemetryCandidate>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    let full_page = atlas_storage::Page {
        limit: i32::MAX as u32,
        offset: 0,
    };
    for candidate in metrics {
        let Some(host_candidate) =
            resolve_host_candidate_for_process_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let Some(mut process) = store
            .list_process_runtime_states(host.host_id, full_page)?
            .into_iter()
            .find(|state| {
                state.external_ref.as_deref() == Some(candidate.process_ref.as_str())
                    || (state.pid == candidate.pid
                        && state.external_ref.as_deref().is_some_and(|value| {
                            value.starts_with(&format!(
                                "{}:pid:{}:",
                                candidate.machine_id.as_deref().unwrap_or(""),
                                candidate.pid
                            ))
                        }))
            })
        else {
            continue;
        };

        match candidate.metric_name.as_str() {
            "process.state" => process.process_state = candidate.value_string.clone(),
            "process.memory.rss" => process.memory_rss_kib = candidate.value_i64,
            _ => continue,
        }
        process.observed_at = candidate.observed_at;

        store.upsert_process_runtime_state(&process)?;
    }

    Ok(())
}

pub(crate) async fn materialize_process_telemetry_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<ProcessTelemetryCandidate>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let full_page = atlas_storage::Page {
        limit: i32::MAX as u32,
        offset: 0,
    };
    for candidate in metrics {
        let Some(host_candidate) =
            resolve_host_candidate_for_process_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let Some(mut process) = atlas_storage::AsyncRuntimeStore::list_process_runtime_states(
            store,
            host.host_id,
            full_page,
        )
        .await?
        .into_iter()
        .find(|state| {
            state.external_ref.as_deref() == Some(candidate.process_ref.as_str())
                || (state.pid == candidate.pid
                    && state.external_ref.as_deref().is_some_and(|value| {
                        value.starts_with(&format!(
                            "{}:pid:{}:",
                            candidate.machine_id.as_deref().unwrap_or(""),
                            candidate.pid
                        ))
                    }))
        }) else {
            continue;
        };

        match candidate.metric_name.as_str() {
            "process.state" => process.process_state = candidate.value_string.clone(),
            "process.memory.rss" => process.memory_rss_kib = candidate.value_i64,
            _ => continue,
        }
        process.observed_at = candidate.observed_at;

        atlas_storage::AsyncRuntimeStore::upsert_process_runtime_state(store, &process).await?;
    }

    Ok(())
}

pub(crate) fn materialize_host_telemetry<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<HostTelemetryCandidate>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    for candidate in metrics {
        let Some(host_candidate) = resolve_host_candidate_for_host_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let mut runtime = store
            .list_host_runtime_states(host.host_id, atlas_storage::Page::default())?
            .into_iter()
            .find(|state| state.observed_at == candidate.observed_at)
            .unwrap_or(HostRuntimeState {
                host_id: host.host_id,
                observed_at: candidate.observed_at,
                boot_id: None,
                uptime_seconds: None,
                loadavg_1m: None,
                loadavg_5m: None,
                loadavg_15m: None,
                cpu_usage_pct: None,
                memory_used_bytes: None,
                memory_available_bytes: None,
                process_count: None,
                container_count: None,
                agent_health: AgentHealth::Healthy,
            });

        match candidate.metric_name.as_str() {
            "system.target.count" => runtime.process_count = candidate.value_i64,
            "system.load_average.1m" => runtime.loadavg_1m = candidate.value_f64,
            "system.load_average.5m" => runtime.loadavg_5m = candidate.value_f64,
            "system.load_average.15m" => runtime.loadavg_15m = candidate.value_f64,
            "system.memory.used" | "system.memory.used_bytes" => {
                runtime.memory_used_bytes = candidate.value_i64
            }
            "system.memory.available" | "system.memory.available_bytes" => {
                runtime.memory_available_bytes = candidate.value_i64
            }
            "system.container.count" => runtime.container_count = candidate.value_i64,
            _ => continue,
        }

        store.insert_host_runtime_state(&runtime)?;
    }

    Ok(())
}

pub(crate) async fn materialize_host_telemetry_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    metrics: Vec<HostTelemetryCandidate>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    for candidate in metrics {
        let Some(host_candidate) = resolve_host_candidate_for_host_telemetry(catalog, &candidate)
        else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let mut runtime = atlas_storage::AsyncRuntimeStore::list_host_runtime_states(
            store,
            host.host_id,
            atlas_storage::Page::default(),
        )
        .await?
        .into_iter()
        .find(|state| state.observed_at == candidate.observed_at)
        .unwrap_or(HostRuntimeState {
            host_id: host.host_id,
            observed_at: candidate.observed_at,
            boot_id: None,
            uptime_seconds: None,
            loadavg_1m: None,
            loadavg_5m: None,
            loadavg_15m: None,
            cpu_usage_pct: None,
            memory_used_bytes: None,
            memory_available_bytes: None,
            process_count: None,
            container_count: None,
            agent_health: AgentHealth::Healthy,
        });

        match candidate.metric_name.as_str() {
            "system.target.count" => runtime.process_count = candidate.value_i64,
            "system.load_average.1m" => runtime.loadavg_1m = candidate.value_f64,
            "system.load_average.5m" => runtime.loadavg_5m = candidate.value_f64,
            "system.load_average.15m" => runtime.loadavg_15m = candidate.value_f64,
            "system.memory.used" | "system.memory.used_bytes" => {
                runtime.memory_used_bytes = candidate.value_i64
            }
            "system.memory.available" | "system.memory.available_bytes" => {
                runtime.memory_available_bytes = candidate.value_i64
            }
            "system.container.count" => runtime.container_count = candidate.value_i64,
            _ => continue,
        }

        atlas_storage::AsyncRuntimeStore::insert_host_runtime_state(store, &runtime).await?;
    }

    Ok(())
}

pub(crate) fn resolve_host_candidate_for_host_telemetry(
    catalog: &InMemoryCatalog,
    candidate: &HostTelemetryCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    if let Some(host_name) = candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    None
}

pub(crate) fn resolve_host_candidate_for_process_telemetry(
    catalog: &InMemoryCatalog,
    candidate: &ProcessTelemetryCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    if let Some(host_name) = candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
                arch: host.arch.clone(),
            });
        }
    }

    None
}
