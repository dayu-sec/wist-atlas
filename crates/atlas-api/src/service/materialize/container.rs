//! 容器物化：把 `containers[]` 候选写成 `container_runtime` 行。
//!
//! 与进程同一套 presence 语义：容器从某轮完整快照里消失时只标 `stale_at`，不删行 ——
//! 「上周这台机跑过哪些容器」是这个资产目录该能回答的问题。两条自我限制也照抄进程：
//!
//! 1. **只比同作用域的行**：一台主机可能被多个生产者看着，不按 `ingest_scope` 过滤
//!    就会把别人的容器一起清掉。
//! 2. **快照不完整就不清扫**：宿主解析不到、或者容器列表为空时直接返回。
//!    「看不懂」与「不存在」在数据上不可区分，误标一批评活的代价更大。

use std::collections::{BTreeMap, BTreeSet};

use atlas_domain::{ContainerRuntime, ContainerRuntimeCandidate, RuntimeType};
use atlas_storage::{AsyncRuntimeStore, Page, RuntimeStore, StorageResult};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::stable_uuid;
use crate::pipeline::InMemoryCatalog;

/// 一次容器物化的覆盖面，喂给快照替换清扫。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ContainerSweep {
    pub seen: BTreeSet<Uuid>,
    pub hosts: BTreeSet<Uuid>,
    /// 宿主解析不到的候选数。大于 0 就不敢清扫。
    pub unresolved: usize,
}

pub(crate) fn materialize_containers<S>(
    store: &S,
    catalog: &InMemoryCatalog,
    containers: Vec<ContainerRuntimeCandidate>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<ContainerSweep>
where
    S: RuntimeStore,
{
    let mut sweep = ContainerSweep::default();
    for candidate in containers {
        let Some(host_id) = resolve_host_id(catalog, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        let container = build_container(&candidate, host_id, now, scope);
        store.upsert_container_runtime(&container)?;
        sweep.seen.insert(container.container_id);
        sweep.hosts.insert(container.host_id);
    }
    Ok(sweep)
}

pub(crate) async fn materialize_containers_async<S>(
    store: &S,
    catalog: &InMemoryCatalog,
    containers: Vec<ContainerRuntimeCandidate>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<ContainerSweep>
where
    S: AsyncRuntimeStore,
{
    let mut sweep = ContainerSweep::default();
    for candidate in containers {
        let Some(host_id) = resolve_host_id(catalog, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        let container = build_container(&candidate, host_id, now, scope);
        AsyncRuntimeStore::upsert_container_runtime(store, &container).await?;
        sweep.seen.insert(container.container_id);
        sweep.hosts.insert(container.host_id);
    }
    Ok(sweep)
}

/// 把本次快照没出现的容器标成 stale（只标，不删；只标首次消失那次）。
pub(crate) fn mark_stale_containers<S>(
    store: &S,
    sweep: &ContainerSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> StorageResult<u64>
where
    S: RuntimeStore,
{
    if sweep.unresolved > 0 || sweep.seen.is_empty() {
        return Ok(0);
    }

    let mut marked = 0u64;
    for host_id in &sweep.hosts {
        for mut container in list_containers_paged(store, *host_id)? {
            if container.ingest_scope.as_deref() != Some(scope) {
                continue;
            }
            if sweep.seen.contains(&container.container_id) {
                continue;
            }
            container.stale_at = Some(at);
            store.upsert_container_runtime(&container)?;
            marked += 1;
        }
    }
    Ok(marked)
}

pub(crate) async fn mark_stale_containers_async<S>(
    store: &S,
    sweep: &ContainerSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> StorageResult<u64>
where
    S: AsyncRuntimeStore,
{
    if sweep.unresolved > 0 || sweep.seen.is_empty() {
        return Ok(0);
    }

    let mut marked = 0u64;
    for host_id in &sweep.hosts {
        for mut container in list_containers_paged_async(store, *host_id).await? {
            if container.ingest_scope.as_deref() != Some(scope) {
                continue;
            }
            if sweep.seen.contains(&container.container_id) {
                continue;
            }
            container.stale_at = Some(at);
            AsyncRuntimeStore::upsert_container_runtime(store, &container).await?;
            marked += 1;
        }
    }
    Ok(marked)
}

fn build_container(
    candidate: &ContainerRuntimeCandidate,
    host_id: Uuid,
    now: DateTime<Utc>,
    scope: &str,
) -> ContainerRuntime {
    ContainerRuntime {
        // 身份由「租户 + 运行时自己的容器 id」派生：容器重建会拿到新 id，
        // 资产也就跟着变成新对象 —— 这正是我们想要的（旧容器是被替换掉的）。
        container_id: stable_uuid(
            "container_runtime",
            &format!("{}:{}", candidate.tenant_id.0, candidate.container_key),
        ),
        tenant_id: candidate.tenant_id,
        // pod 目录还没有，先不写悬空外键；K8s 线索留在下面的文本列里。
        pod_id: None,
        host_id,
        runtime_type: RuntimeType::from_reported(&candidate.runtime_type),
        runtime_namespace: candidate.runtime_namespace.clone(),
        container_name: candidate.container_name.clone(),
        image_ref: candidate.image_ref.clone(),
        external_ref: Some(candidate.container_key.clone()),
        started_at: candidate.started_at.map(|item| item.0),
        last_seen_at: candidate.observed_at.map(|item| item.0).unwrap_or(now),
        // 看到的对象一律「活的」：upsert 会把旧的 stale_at 覆盖成 NULL，自然复活。
        stale_at: None,
        ingest_scope: Some(scope.to_string()),
        pod_uid: candidate.k8s_pod_uid.clone(),
        pod_name: candidate.k8s_pod_name.clone(),
        k8s_namespace: candidate.k8s_namespace.clone(),
    }
}

/// 容器挂在哪台主机：先按 `machine_id` 精确匹配，再退回主机名。
fn resolve_host_id(
    catalog: &InMemoryCatalog,
    candidate: &ContainerRuntimeCandidate,
) -> Option<Uuid> {
    if let Some(machine_id) = candidate.machine_id.as_ref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_ref() == Some(machine_id))
        {
            return Some(host.host_id);
        }
    }
    let host_name = candidate.host_name.as_ref()?;
    catalog
        .hosts
        .iter()
        .find(|host| &host.host_name == host_name)
        .map(|host| host.host_id)
}

const SWEEP_PAGE_LIMIT: u32 = 500;

fn list_containers_paged<S>(store: &S, host_id: Uuid) -> StorageResult<Vec<ContainerRuntime>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_container_runtimes_for_host(
            host_id,
            Page {
                limit: SWEEP_PAGE_LIMIT,
                offset,
            },
        )?;
        let len = batch.len();
        items.extend(batch);
        if len < SWEEP_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

async fn list_containers_paged_async<S>(
    store: &S,
    host_id: Uuid,
) -> StorageResult<Vec<ContainerRuntime>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_container_runtimes_for_host(
            store,
            host_id,
            Page {
                limit: SWEEP_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < SWEEP_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

/// 容器按运行时分组计数（主机卡片上的一行小字）。
pub(crate) fn count_by_runtime(containers: &[ContainerRuntime]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for container in containers {
        let key = match container.runtime_type {
            RuntimeType::Containerd => "containerd",
            RuntimeType::Docker => "docker",
            RuntimeType::CriO => "cri-o",
            RuntimeType::Process => "process",
            RuntimeType::Unknown => "unknown",
        };
        *counts.entry(key.to_string()).or_insert(0) += 1;
    }
    counts
}
