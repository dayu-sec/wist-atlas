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

use super::business::{materialize_process_binding, materialize_process_binding_async};
use super::host::resolve_host_candidate_for_process;
use super::stable_uuid;
use crate::pipeline::InMemoryCatalog;

pub(crate) fn materialize_processes<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    processes: Vec<ProcessRuntimeCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + RuntimeStore,
{
    for candidate in processes {
        let Some(host_candidate) = resolve_host_candidate_for_process(catalog, &candidate) else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let process = ProcessRuntimeState {
            process_id: stable_uuid(
                "process_runtime",
                &candidate
                    .machine_id
                    .as_deref()
                    .zip(candidate.identity.as_deref())
                    .map(|(machine_id, identity)| {
                        format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                    })
                    .unwrap_or_else(|| {
                        format!(
                            "{}:{}:{}",
                            host.host_id, candidate.pid, candidate.executable
                        )
                    }),
            ),
            tenant_id: candidate.tenant_id,
            host_id: host.host_id,
            container_id: None,
            external_ref: candidate
                .machine_id
                .as_ref()
                .zip(candidate.identity.as_ref())
                .map(|(machine_id, identity)| {
                    format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                }),
            pid: candidate.pid,
            executable: candidate.executable.clone(),
            command_line: candidate.command_line.clone(),
            process_state: None,
            memory_rss_kib: None,
            started_at: candidate.observed_at.map(|item| item.0).unwrap_or(now),
            observed_at: candidate
                .observed_at
                .unwrap_or(atlas_domain::ObservedAt(now)),
        };
        store.upsert_process_runtime_state(&process)?;
        materialize_process_binding(store, &candidate, &process, now)?;
    }

    Ok(())
}

pub(crate) async fn materialize_processes_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    processes: Vec<ProcessRuntimeCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    for candidate in processes {
        let Some(host_candidate) = resolve_host_candidate_for_process(catalog, &candidate) else {
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            continue;
        };

        let process = ProcessRuntimeState {
            process_id: stable_uuid(
                "process_runtime",
                &candidate
                    .machine_id
                    .as_deref()
                    .zip(candidate.identity.as_deref())
                    .map(|(machine_id, identity)| {
                        format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                    })
                    .unwrap_or_else(|| {
                        format!(
                            "{}:{}:{}",
                            host.host_id, candidate.pid, candidate.executable
                        )
                    }),
            ),
            tenant_id: candidate.tenant_id,
            host_id: host.host_id,
            container_id: None,
            external_ref: candidate
                .machine_id
                .as_ref()
                .zip(candidate.identity.as_ref())
                .map(|(machine_id, identity)| {
                    format!("{machine_id}:pid:{}:{identity}", candidate.pid)
                }),
            pid: candidate.pid,
            executable: candidate.executable.clone(),
            command_line: candidate.command_line.clone(),
            process_state: None,
            memory_rss_kib: None,
            started_at: candidate.observed_at.map(|item| item.0).unwrap_or(now),
            observed_at: candidate
                .observed_at
                .unwrap_or(atlas_domain::ObservedAt(now)),
        };
        atlas_storage::AsyncRuntimeStore::upsert_process_runtime_state(store, &process).await?;
        materialize_process_binding_async(store, &candidate, &process, now).await?;
    }

    Ok(())
}
