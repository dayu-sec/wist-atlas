use std::collections::BTreeSet;

use atlas_domain::{
    AgentHealth, BindingScope, BusinessCatalogCandidate, Confidence, HostCandidate,
    HostRuntimeState, HostTelemetryCandidate, NetworkSegmentCandidate, ProcessRuntimeCandidate,
    ProcessRuntimeState, ProcessTelemetryCandidate, ResponsibilityAssignment, RuntimeBinding,
    RuntimeObjectType, ServiceEntity, ServiceInstance, Subject, SubjectCandidate, ValidityWindow,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore,
    InMemoryTopologyStore, Page, RuntimeStore, StorageResult,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::business::{materialize_process_binding, materialize_process_binding_async};
use super::host::resolve_host_candidate_for_process;
use super::stable_uuid;
use crate::pipeline::InMemoryCatalog;

/// 一次进程物化的“覆盖面”：快照里看到了什么，以及有没有看不懂的条目。
///
/// 它是快照替换清扫的**输入**：只有知道“这次到底看清楚了哪些”，才能说“其余的没了”。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ProcessSweep {
    /// 本次快照看到的进程（`process_id`）。
    pub seen: BTreeSet<Uuid>,
    /// 本次快照覆盖到的主机。清扫只在这些主机上比对 —— 整台主机消失是另一回事。
    pub hosts: BTreeSet<Uuid>,
    /// 宿主解析不到、因而没能物化的候选数。**大于 0 就不敢清扫**：
    /// 看不懂的条目和“不存在的进程”在数据上不可区分。
    pub unresolved: usize,
}

/// 一次快照替换清扫的结果。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaleSweep {
    /// 新标记为 stale 的进程数（已经是 stale 的不重算、也不推时间）。
    pub marked: u64,
    /// 候选里有看不懂的条目 → 放弃清扫。
    pub skipped_incomplete: bool,
    /// 本次快照里进程为空 → 放弃清扫。
    pub skipped_empty: bool,
}

/// 把进程候选物化成 runtime 状态。
///
/// `scope` 是本次 ingest 的快照作用域（`schema:system:producer:tenant:env`），写到每行上，
/// 供后续「本次快照没出现的对象」比对用。
pub(crate) fn materialize_processes<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    processes: Vec<ProcessRuntimeCandidate>,
    known_containers: &BTreeSet<Uuid>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<ProcessSweep>
where
    S: CatalogStore + RuntimeStore,
{
    let mut sweep = ProcessSweep::default();
    for candidate in processes {
        let Some(host_candidate) = resolve_host_candidate_for_process(catalog, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            sweep.unresolved += 1;
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
            // 采集侧直接给了容器 id 就用；没采到的容器不写悬空引用。
            container_id: candidate
                .container_ref
                .as_deref()
                .map(|key| super::container_uuid(candidate.tenant_id, key))
                .filter(|container_id| known_containers.contains(container_id)),
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
            // 新物化 / 被看到的对象一律“活的”：upsert 会把旧值覆盖成 NULL，自然复活。
            stale_at: None,
            ingest_scope: Some(scope.to_string()),
        };
        store.upsert_process_runtime_state(&process)?;
        materialize_process_binding(store, &candidate, &process, now)?;
        sweep.seen.insert(process.process_id);
        sweep.hosts.insert(process.host_id);
    }

    Ok(sweep)
}

pub(crate) async fn materialize_processes_async<S>(
    store: &S,
    catalog: &mut InMemoryCatalog,
    processes: Vec<ProcessRuntimeCandidate>,
    known_containers: &BTreeSet<Uuid>,
    now: DateTime<Utc>,
    scope: &str,
) -> StorageResult<ProcessSweep>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let mut sweep = ProcessSweep::default();
    for candidate in processes {
        let Some(host_candidate) = resolve_host_candidate_for_process(catalog, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        let Some(host) = catalog.hosts.iter().find(|host| {
            host.host_name == host_candidate.host_name
                && host.machine_id == host_candidate.machine_id
        }) else {
            sweep.unresolved += 1;
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
            container_id: candidate
                .container_ref
                .as_deref()
                .map(|key| super::container_uuid(candidate.tenant_id, key))
                .filter(|container_id| known_containers.contains(container_id)),
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
            stale_at: None,
            ingest_scope: Some(scope.to_string()),
        };
        atlas_storage::AsyncRuntimeStore::upsert_process_runtime_state(store, &process).await?;
        materialize_process_binding_async(store, &candidate, &process, now).await?;
        sweep.seen.insert(process.process_id);
        sweep.hosts.insert(process.host_id);
    }

    Ok(sweep)
}

/// 快照替换语义：把**本次快照没出现**的存活进程标成 stale。
///
/// 不删行 —— 资产目录要能回答「上周这台机有哪些进程」；`stale_at` 保留**首次**消失时间，
/// 后续快照不会把它往后推（否则「什么时候没的」永远答不了）。
///
/// 两条自我限制（都不是可选项）：
///
/// 1. **只比同作用域的行**。一台主机可能被多个生产者看着（如容器采集器），
///    不按 `ingest_scope` 过滤就会把别人的进程一起清掉。
/// 2. **快照不完整就不清扫**。`sweep.unresolved > 0` 或进程列表为空时直接返回，
///    因为「看不懂」与「不存在」在数据上不可区分 —— 误标一批活进程的代价远大于晚一轮清理。
///
/// 已知边界：清扫只在本次快照**覆盖到的主机**上做。整台主机从快照里消失时，它的进程
/// 不会被清（主机退役需要单独的策略：一轮探针失败不等于机器没了）。
pub(crate) fn mark_stale_processes<S>(
    store: &S,
    sweep: &ProcessSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> StorageResult<StaleSweep>
where
    S: RuntimeStore,
{
    if sweep.unresolved > 0 {
        return Ok(StaleSweep {
            skipped_incomplete: true,
            ..StaleSweep::default()
        });
    }
    if sweep.seen.is_empty() {
        return Ok(StaleSweep {
            skipped_empty: true,
            ..StaleSweep::default()
        });
    }

    let mut result = StaleSweep::default();
    for host_id in &sweep.hosts {
        for mut process in list_process_runtime_states_paged(store, *host_id)? {
            if process.stale_at.is_some() {
                // 已经是 stale：保留**首次**消失时间，不推后。
                continue;
            }
            if process.ingest_scope.as_deref() != Some(scope) {
                continue;
            }
            if sweep.seen.contains(&process.process_id) {
                continue;
            }
            process.stale_at = Some(at);
            store.upsert_process_runtime_state(&process)?;
            result.marked += 1;
        }
    }

    Ok(result)
}

/// 同 [`mark_stale_processes`]。
pub(crate) async fn mark_stale_processes_async<S>(
    store: &S,
    sweep: &ProcessSweep,
    scope: &str,
    at: DateTime<Utc>,
) -> StorageResult<StaleSweep>
where
    S: AsyncRuntimeStore,
{
    if sweep.unresolved > 0 {
        return Ok(StaleSweep {
            skipped_incomplete: true,
            ..StaleSweep::default()
        });
    }
    if sweep.seen.is_empty() {
        return Ok(StaleSweep {
            skipped_empty: true,
            ..StaleSweep::default()
        });
    }

    let mut result = StaleSweep::default();
    for host_id in &sweep.hosts {
        for mut process in list_process_runtime_states_paged_async(store, *host_id).await? {
            if process.stale_at.is_some() {
                continue;
            }
            if process.ingest_scope.as_deref() != Some(scope) {
                continue;
            }
            if sweep.seen.contains(&process.process_id) {
                continue;
            }
            process.stale_at = Some(at);
            AsyncRuntimeStore::upsert_process_runtime_state(store, &process).await?;
            result.marked += 1;
        }
    }

    Ok(result)
}

const SWEEP_PAGE_LIMIT: u32 = 500;

pub(super) fn list_process_runtime_states_paged<S>(
    store: &S,
    host_id: Uuid,
) -> StorageResult<Vec<ProcessRuntimeState>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_process_runtime_states(
            host_id,
            Page {
                limit: SWEEP_PAGE_LIMIT,
                offset,
            },
        )?;
        let batch_len = batch.len();
        items.extend(batch);
        if batch_len < SWEEP_PAGE_LIMIT as usize {
            break;
        }
        offset += batch_len as u32;
    }
    Ok(items)
}

pub(super) async fn list_process_runtime_states_paged_async<S>(
    store: &S,
    host_id: Uuid,
) -> StorageResult<Vec<ProcessRuntimeState>>
where
    S: AsyncRuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_process_runtime_states(
            store,
            host_id,
            Page {
                limit: SWEEP_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let batch_len = batch.len();
        items.extend(batch);
        if batch_len < SWEEP_PAGE_LIMIT as usize {
            break;
        }
        offset += batch_len as u32;
    }
    Ok(items)
}
