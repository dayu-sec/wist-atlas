use std::collections::BTreeSet;

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

pub(crate) fn materialize_candidates<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    hosts: Vec<HostCandidate>,
    networks: Vec<NetworkSegmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<(usize, usize, usize)>
where
    S: CatalogStore + RuntimeStore,
{
    let mut host_count = 0;
    let mut network_count = 0;
    let mut assoc_count = 0;
    let mut materialized_host_ids = BTreeSet::new();

    for network_candidate in networks {
        let host_candidate =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate);
        if let Some(host_candidate) = host_candidate {
            let materialized =
                materialize_host_network(catalog, &host_candidate, &network_candidate, now);

            for domain in &catalog.network_domains {
                store.upsert_network_domain(domain)?;
            }
            store.upsert_host(&materialized.host)?;
            store.upsert_network_segment(&materialized.segment)?;
            if let Some(assoc) = &materialized.assoc {
                store.upsert_host_net_assoc(assoc)?;
                assoc_count += 1;
            }
            materialized_host_ids.insert(materialized.host.host_id);
            host_count += 1;
            network_count += 1;
        }
    }

    for host_candidate in hosts {
        let host_resolution = resolve_host_candidate(catalog, &host_candidate, now);
        if materialized_host_ids.contains(&host_resolution.host.host_id) {
            continue;
        }

        upsert_catalog_host(catalog, host_resolution.host.clone());
        store.upsert_host(&host_resolution.host)?;
        materialized_host_ids.insert(host_resolution.host.host_id);
        host_count += 1;
    }

    Ok((host_count, network_count, assoc_count))
}

pub(crate) async fn materialize_candidates_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    hosts: Vec<HostCandidate>,
    networks: Vec<NetworkSegmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<(usize, usize, usize)>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut host_count = 0;
    let mut network_count = 0;
    let mut assoc_count = 0;
    let mut materialized_host_ids = BTreeSet::new();

    for network_candidate in networks {
        let host_candidate =
            resolve_host_candidate_for_network(&hosts, catalog, &network_candidate);
        if let Some(host_candidate) = host_candidate {
            let materialized =
                materialize_host_network(catalog, &host_candidate, &network_candidate, now);

            for domain in &catalog.network_domains {
                atlas_storage::AsyncCatalogStore::upsert_network_domain(store, domain).await?;
            }
            atlas_storage::AsyncCatalogStore::upsert_host(store, &materialized.host).await?;
            atlas_storage::AsyncCatalogStore::upsert_network_segment(store, &materialized.segment)
                .await?;
            if let Some(assoc) = &materialized.assoc {
                atlas_storage::AsyncRuntimeStore::upsert_host_net_assoc(store, assoc).await?;
                assoc_count += 1;
            }
            materialized_host_ids.insert(materialized.host.host_id);
            host_count += 1;
            network_count += 1;
        }
    }

    for host_candidate in hosts {
        let host_resolution = resolve_host_candidate(catalog, &host_candidate, now);
        if materialized_host_ids.contains(&host_resolution.host.host_id) {
            continue;
        }

        upsert_catalog_host(catalog, host_resolution.host.clone());
        atlas_storage::AsyncCatalogStore::upsert_host(store, &host_resolution.host).await?;
        materialized_host_ids.insert(host_resolution.host.host_id);
        host_count += 1;
    }

    Ok((host_count, network_count, assoc_count))
}

pub(crate) fn resolve_host_candidate_for_network(
    hosts: &[HostCandidate],
    catalog: &InMemoryCatalog,
    network_candidate: &NetworkSegmentCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = network_candidate.machine_id.as_ref() {
        if let Some(host) = hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(host.clone());
        }

        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: network_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    if let Some(host_name) = network_candidate.host_name.as_ref() {
        if let Some(host) = hosts.iter().find(|host| &host.host_name == host_name) {
            return Some(host.clone());
        }

        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: network_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    None
}

pub(crate) fn resolve_host_candidate_for_process(
    catalog: &InMemoryCatalog,
    process_candidate: &ProcessRuntimeCandidate,
) -> Option<HostCandidate> {
    if let Some(machine_id) = process_candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: process_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    if let Some(host_name) = process_candidate.host_name.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| &host.host_name == host_name)
        {
            return Some(HostCandidate {
                tenant_id: host.tenant_id,
                environment_id: host.environment_id,
                source_kind: process_candidate.source_kind,
                external_ref: None,
                host_name: host.host_name.clone(),
                machine_id: host.machine_id.clone(),
                os_name: host.os_name.clone(),
                os_version: host.os_version.clone(),
            });
        }
    }

    None
}

pub(crate) fn upsert_catalog_host(
    catalog: &mut InMemoryCatalog,
    host: atlas_domain::HostInventory,
) {
    if let Some(existing) = catalog
        .hosts
        .iter_mut()
        .find(|existing| existing.host_id == host.host_id)
    {
        *existing = host;
    } else {
        catalog.hosts.push(host);
    }
}
