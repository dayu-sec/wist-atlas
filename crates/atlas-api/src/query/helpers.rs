use atlas_domain::{
    HostProcessGroupView, HostProcessGroupsPageView, HostProcessOverviewView,
    HostServiceInstanceView, HostServiceView, HostTopologyView, ProcessRuntimeState,
    ProcessStateCount, RuntimeObjectType,
};
use atlas_storage::{AsyncCatalogStore, AsyncRuntimeStore, CatalogStore, Page, RuntimeStore};
use chrono::Utc;
use orion_error::conversion::ConvErr;
use uuid::Uuid;

use crate::error::ApiResult;

pub(super) fn list_all_process_runtime_states<S>(
    store: &S,
    host_id: Uuid,
) -> ApiResult<Vec<atlas_domain::ProcessRuntimeState>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let page_limit = 500;
    let mut offset = 0;

    loop {
        let batch = store
            .list_process_runtime_states(
                host_id,
                Page {
                    limit: page_limit,
                    offset,
                },
            )
            .conv_err()?;
        let batch_len = batch.len();
        if batch_len == 0 {
            break;
        }
        items.extend(batch);
        if batch_len < page_limit as usize {
            break;
        }
        offset += batch_len as u32;
    }

    Ok(items)
}

pub(super) async fn list_all_process_runtime_states_async<S>(
    store: &S,
    host_id: Uuid,
) -> ApiResult<Vec<atlas_domain::ProcessRuntimeState>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let page_limit = 500;
    let mut offset = 0;

    loop {
        let batch = AsyncRuntimeStore::list_process_runtime_states(
            store,
            host_id,
            Page {
                limit: page_limit,
                offset,
            },
        )
        .await
        .conv_err()?;
        let batch_len = batch.len();
        if batch_len == 0 {
            break;
        }
        items.extend(batch);
        if batch_len < page_limit as usize {
            break;
        }
        offset += batch_len as u32;
    }

    Ok(items)
}

pub(super) fn build_host_services<S>(
    store: &S,
    processes: &[ProcessRuntimeState],
) -> ApiResult<Vec<HostServiceView>>
where
    S: CatalogStore + RuntimeStore,
{
    let mut views = Vec::new();
    let mut seen_instance_ids = std::collections::BTreeSet::new();
    let process_by_id: std::collections::BTreeMap<Uuid, ProcessRuntimeState> = processes
        .iter()
        .cloned()
        .map(|process| (process.process_id, process))
        .collect();

    for process in processes {
        let bindings = store
            .list_runtime_bindings_for_object(
                RuntimeObjectType::Process,
                process.process_id,
                Page::default(),
            )
            .conv_err()?;
        for binding in bindings {
            if !seen_instance_ids.insert(binding.instance_id) {
                continue;
            }
            let Some(instance) = store.get_service_instance(binding.instance_id).conv_err()? else {
                continue;
            };
            let Some(service) = store.get_service(instance.service_id).conv_err()? else {
                continue;
            };

            let instance_bindings = store
                .list_runtime_bindings_for_instance(instance.instance_id, Page::default())
                .conv_err()?;
            let instance_processes = collect_bound_processes(&process_by_id, &instance_bindings);
            push_host_service_view(
                &mut views,
                service,
                HostServiceInstanceView {
                    instance,
                    bindings: instance_bindings,
                    processes: instance_processes,
                },
            );
        }
    }

    sort_host_service_views(&mut views);
    Ok(views)
}

pub(super) async fn build_host_services_async<S>(
    store: &S,
    processes: &[ProcessRuntimeState],
) -> ApiResult<Vec<HostServiceView>>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut views = Vec::new();
    let mut seen_instance_ids = std::collections::BTreeSet::new();
    let process_by_id: std::collections::BTreeMap<Uuid, ProcessRuntimeState> = processes
        .iter()
        .cloned()
        .map(|process| (process.process_id, process))
        .collect();

    for process in processes {
        let bindings = AsyncRuntimeStore::list_runtime_bindings_for_object(
            store,
            RuntimeObjectType::Process,
            process.process_id,
            Page::default(),
        )
        .await
        .conv_err()?;
        for binding in bindings {
            if !seen_instance_ids.insert(binding.instance_id) {
                continue;
            }
            let Some(instance) =
                AsyncRuntimeStore::get_service_instance(store, binding.instance_id)
                    .await
                    .conv_err()?
            else {
                continue;
            };
            let Some(service) = AsyncCatalogStore::get_service(store, instance.service_id)
                .await
                .conv_err()?
            else {
                continue;
            };

            let instance_bindings = AsyncRuntimeStore::list_runtime_bindings_for_instance(
                store,
                instance.instance_id,
                Page::default(),
            )
            .await
            .conv_err()?;
            let instance_processes = collect_bound_processes(&process_by_id, &instance_bindings);
            push_host_service_view(
                &mut views,
                service,
                HostServiceInstanceView {
                    instance,
                    bindings: instance_bindings,
                    processes: instance_processes,
                },
            );
        }
    }

    sort_host_service_views(&mut views);
    Ok(views)
}

pub(super) fn collect_bound_processes(
    process_by_id: &std::collections::BTreeMap<Uuid, ProcessRuntimeState>,
    bindings: &[atlas_domain::RuntimeBinding],
) -> Vec<ProcessRuntimeState> {
    let mut items = Vec::new();
    for binding in bindings {
        if binding.object_type != RuntimeObjectType::Process {
            continue;
        }
        if let Some(process) = process_by_id.get(&binding.object_id) {
            items.push(process.clone());
        }
    }
    items.sort_by(|left, right| {
        left.pid
            .cmp(&right.pid)
            .then(left.process_id.cmp(&right.process_id))
    });
    items
}

pub(super) fn push_host_service_view(
    views: &mut Vec<HostServiceView>,
    service: atlas_domain::ServiceEntity,
    instance_view: HostServiceInstanceView,
) {
    if let Some(existing) = views
        .iter_mut()
        .find(|item| item.service.service_id == service.service_id)
    {
        existing.instances.push(instance_view);
        return;
    }
    views.push(HostServiceView {
        service,
        instances: vec![instance_view],
    });
}

pub(super) fn sort_host_service_views(views: &mut [HostServiceView]) {
    for view in views.iter_mut() {
        view.instances.sort_by(|left, right| {
            right
                .instance
                .last_seen_at
                .cmp(&left.instance.last_seen_at)
                .then(left.instance.instance_id.cmp(&right.instance.instance_id))
        });
    }
    views.sort_by(|left, right| left.service.name.cmp(&right.service.name));
}

pub(super) fn build_process_groups(processes: &[ProcessRuntimeState]) -> Vec<HostProcessGroupView> {
    let mut grouped: std::collections::BTreeMap<String, Vec<&ProcessRuntimeState>> =
        std::collections::BTreeMap::new();
    for process in processes {
        grouped
            .entry(process.executable.clone())
            .or_default()
            .push(process);
    }

    let mut views: Vec<_> = grouped
        .into_iter()
        .map(|(executable, members)| {
            let process_count = members.len();
            let total_memory_rss_kib = members.iter().filter_map(|item| item.memory_rss_kib).sum();

            let mut state_counts: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for member in &members {
                let state = member
                    .process_state
                    .clone()
                    .unwrap_or_else(|| "-".to_string());
                *state_counts.entry(state).or_insert(0) += 1;
            }

            let mut state_summary: Vec<ProcessStateCount> = state_counts
                .into_iter()
                .map(|(state, count)| ProcessStateCount { state, count })
                .collect();
            state_summary.sort_by(|left, right| {
                right
                    .count
                    .cmp(&left.count)
                    .then(left.state.cmp(&right.state))
            });

            HostProcessGroupView {
                display_name: executable
                    .rsplit('/')
                    .next()
                    .filter(|item| !item.is_empty())
                    .unwrap_or(executable.as_str())
                    .to_string(),
                executable,
                process_count,
                total_memory_rss_kib,
                dominant_state: state_summary.first().map(|item| item.state.clone()),
                state_summary,
            }
        })
        .collect();

    views.sort_by(|left, right| {
        right
            .process_count
            .cmp(&left.process_count)
            .then(right.total_memory_rss_kib.cmp(&left.total_memory_rss_kib))
            .then(left.display_name.cmp(&right.display_name))
    });
    views
}

pub(super) fn build_host_process_overview_view(
    view: HostTopologyView,
    top_n: usize,
) -> HostProcessOverviewView {
    let total_processes = view.processes.len();
    let total_groups = view.process_groups.len();
    let top_groups = view
        .process_groups
        .into_iter()
        .take(top_n)
        .collect::<Vec<_>>();
    let truncated_group_count = total_groups.saturating_sub(top_groups.len());

    HostProcessOverviewView {
        host: view.host,
        total_processes,
        total_groups,
        top_groups,
        truncated_group_count,
        generated_at: view.generated_at,
    }
}

pub(super) fn build_host_process_groups_page_view(
    view: HostTopologyView,
    offset: usize,
    limit: usize,
) -> HostProcessGroupsPageView {
    let total_processes = view.processes.len();
    let total_groups = view.process_groups.len();
    let groups = view
        .process_groups
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let has_more = offset.saturating_add(groups.len()) < total_groups;

    HostProcessGroupsPageView {
        host: view.host,
        total_processes,
        total_groups,
        groups,
        limit,
        offset,
        has_more,
        generated_at: view.generated_at,
    }
}
