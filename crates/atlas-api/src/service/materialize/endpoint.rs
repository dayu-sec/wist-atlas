//! 监听地址 → 服务实例端点。
//!
//! 这一层做的是**推断**，不是投影：agentd 只报「这个地址在听、socket 属于 pid N」，
//! 而中心侧的模型要求 `InstEp → ServiceInstance → ServiceEntity` 一串都在。
//! 所以这里按可执行文件名造一个逻辑服务、按 (服务, 主机) 造一个实例、再按 pid 绑到进程上。
//!
//! 两条刻意的取舍：
//!
//! - **服务身份按可执行文件名跨主机归并**（`proc:nginx`）。逻辑服务本就该是一对多——
//!   「nginx 跑在 8 台机器上」比「8 个叫 nginx 的服务」有用。代价是两个不同的 `node`
//!   应用会被并成一个，这一版接受，等声明式目录（`business_catalog`）进来再纠正。
//! - **找不到宿主进程的端口直接跳过**。模型的 `InstEp` 必须挂在实例上，而实例必须有
//!   服务；给无名端口编一个 `tcp/8080` 服务只会把目录弄脏。Linux 上 inode→pid 反查
//!   在 root 下几乎是全的，这条路径很少走到。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use atlas_domain::{
    BindingScope, Confidence, InstanceEndpoint, ProcessRuntimeState, RuntimeBinding,
    RuntimeObjectType, ServiceBoundary, ServiceEndpointCandidate, ServiceEntity, ServiceInstance,
    ServiceType, ValidityWindow,
};
use atlas_storage::{CatalogStore, Page, RuntimeStore, StorageResult};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::stable_uuid;
use crate::pipeline::InMemoryCatalog;

/// 一个端口最终要写下去的四类行。同步与异步路径共用这份「计划」，
/// 免得两边的 uuid 推导规则漂移。
struct EndpointPlan {
    service: ServiceEntity,
    instance: ServiceInstance,
    binding: RuntimeBinding,
    endpoint: InstanceEndpoint,
}

/// 本次物化的结果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct EndpointSweep {
    /// 写进去的监听端口数。
    pub materialized: usize,
    /// 因为找不到宿主或宿主进程而跳过的候选数。
    pub unresolved: usize,
    /// 关闭（`valid_to` 置位）的过期端点数。
    pub closed: u64,
}

/// 一次取多少行。服务实例的端点通常个位数，但清扫要翻页——
/// 只取第一页会把第 101 个以后的端点永远留着。
const ENDPOINT_PAGE_LIMIT: u32 = 200;

pub(crate) fn materialize_service_endpoints<S>(
    store: &S,
    catalog: &InMemoryCatalog,
    candidates: Vec<ServiceEndpointCandidate>,
    reported: bool,
    sweep_hosts: &BTreeSet<Uuid>,
    now: DateTime<Utc>,
) -> StorageResult<EndpointSweep>
where
    S: CatalogStore + RuntimeStore,
{
    let mut processes_by_host: BTreeMap<Uuid, Vec<ProcessRuntimeState>> = BTreeMap::new();
    let mut sweep = EndpointSweep::default();
    let mut seen_by_instance: BTreeMap<Uuid, BTreeSet<Uuid>> = BTreeMap::new();
    // 同一个服务的多个端口会产生多份完全相同的“计划”；服务与实例只需写一次。
    let mut written_instances: BTreeSet<Uuid> = BTreeSet::new();

    for candidate in candidates {
        let Some(host_id) = resolve_host_id(catalog, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        let processes = match processes_by_host.get(&host_id) {
            Some(processes) => processes,
            None => {
                let processes = super::process::list_process_runtime_states_paged(store, host_id)?;
                processes_by_host.insert(host_id, processes);
                processes_by_host.get(&host_id).expect("just inserted")
            }
        };
        let Some(process) = pick_process(processes, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        // 可执行名是服务身份的唯一来源；空了就归不了类，宁可跳过也别写一条无名服务。
        if process.executable.trim().is_empty() {
            sweep.unresolved += 1;
            continue;
        }

        let plan = plan_endpoint(&candidate, host_id, process, now);
        if written_instances.insert(plan.instance.instance_id) {
            store.upsert_service(&plan.service)?;
            store.upsert_service_instance(&plan.instance)?;
            store.upsert_runtime_binding(&plan.binding)?;
            // 同一个实例换了宿主进程（pid 被复用、进程重启后接管端口）时，
            // 旧绑定不能一直悬着，否则“这个实例由谁在跑”同时有两个答案。
            close_previous_bindings(
                store,
                plan.instance.instance_id,
                process.process_id,
                plan.endpoint.valid_from,
                now,
            )?;
        }
        store.upsert_instance_endpoint(&plan.endpoint)?;

        sweep.materialized += 1;
        seen_by_instance
            .entry(plan.instance.instance_id)
            .or_default()
            .insert(plan.endpoint.endpoint_id);
    }

    sweep.closed = close_stale_endpoints(store, reported, sweep_hosts, &seen_by_instance, now)?;
    Ok(sweep)
}

pub(crate) async fn materialize_service_endpoints_async<S>(
    store: &S,
    catalog: &InMemoryCatalog,
    candidates: Vec<ServiceEndpointCandidate>,
    reported: bool,
    sweep_hosts: &BTreeSet<Uuid>,
    now: DateTime<Utc>,
) -> StorageResult<EndpointSweep>
where
    S: atlas_storage::AsyncRuntimeStore + atlas_storage::AsyncCatalogStore,
{
    use atlas_storage::{AsyncCatalogStore, AsyncRuntimeStore};

    let mut processes_by_host: BTreeMap<Uuid, Vec<ProcessRuntimeState>> = BTreeMap::new();
    let mut sweep = EndpointSweep::default();
    let mut seen_by_instance: BTreeMap<Uuid, BTreeSet<Uuid>> = BTreeMap::new();
    let mut written_instances: BTreeSet<Uuid> = BTreeSet::new();

    for candidate in candidates {
        let Some(host_id) = resolve_host_id(catalog, &candidate) else {
            sweep.unresolved += 1;
            continue;
        };
        if !processes_by_host.contains_key(&host_id) {
            let processes =
                super::process::list_process_runtime_states_paged_async(store, host_id).await?;
            processes_by_host.insert(host_id, processes);
        }
        let Some(process) = pick_process(
            processes_by_host.get(&host_id).expect("inserted above"),
            &candidate,
        ) else {
            sweep.unresolved += 1;
            continue;
        };
        if process.executable.trim().is_empty() {
            sweep.unresolved += 1;
            continue;
        }

        let plan = plan_endpoint(&candidate, host_id, process, now);
        if written_instances.insert(plan.instance.instance_id) {
            AsyncCatalogStore::upsert_service(store, &plan.service).await?;
            AsyncRuntimeStore::upsert_service_instance(store, &plan.instance).await?;
            AsyncRuntimeStore::upsert_runtime_binding(store, &plan.binding).await?;
            close_previous_bindings_async(
                store,
                plan.instance.instance_id,
                process.process_id,
                plan.endpoint.valid_from,
                now,
            )
            .await?;
        }
        AsyncRuntimeStore::upsert_instance_endpoint(store, &plan.endpoint).await?;

        sweep.materialized += 1;
        seen_by_instance
            .entry(plan.instance.instance_id)
            .or_default()
            .insert(plan.endpoint.endpoint_id);
    }

    sweep.closed =
        close_stale_endpoints_async(store, reported, sweep_hosts, &seen_by_instance, now).await?;
    Ok(sweep)
}

/// 把候选推导成四行。纯函数，方便单测。
fn plan_endpoint(
    candidate: &ServiceEndpointCandidate,
    host_id: Uuid,
    process: &ProcessRuntimeState,
    now: DateTime<Utc>,
) -> EndpointPlan {
    let tenant_id = candidate.tenant_id;
    let observed_at = candidate.observed_at.map(|item| item.0).unwrap_or(now);

    // 服务名取可执行文件的 basename：`/usr/sbin/sshd` → `sshd`。
    // 进程候选里带的是 agentd 的 `process.executable.name`，Linux 上是全路径。
    let service_name = Path::new(&process.executable)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&process.executable)
        .to_string();
    let service_ref = format!("proc:{service_name}");
    let service_id = stable_uuid("service", &format!("{}:{}", tenant_id.0, service_ref));

    let service = ServiceEntity {
        service_id,
        tenant_id,
        business_id: None,
        system_id: None,
        subsystem_id: None,
        name: service_name,
        namespace: None,
        // 从进程推断不出业务语义，给一个最不误导的默认值：
        // 「本机平台组件」，边界按内部资源处理。声明式目录进来后会覆盖。
        service_type: ServiceType::Platform,
        boundary: ServiceBoundary::Internal,
        provider: Some("wist:discovery".to_string()),
        external_ref: Some(service_ref),
        created_at: now,
        updated_at: now,
    };

    // 一台主机上一个服务只该有一个实例（容器化以后再细分）。
    let instance_id = stable_uuid(
        "service_instance",
        &format!("{}:{}:{}", tenant_id.0, service_id, host_id),
    );
    let instance = ServiceInstance {
        instance_id,
        tenant_id,
        service_id,
        workload_id: None,
        started_at: process.started_at,
        ended_at: None,
        last_seen_at: observed_at,
    };

    let binding = RuntimeBinding {
        binding_id: stable_uuid(
            "runtime_binding",
            &format!("{}:{}:process", instance_id, process.process_id),
        ),
        instance_id,
        object_type: RuntimeObjectType::Process,
        object_id: process.process_id,
        scope: BindingScope::Observed,
        // socket owner 是强证据：这个进程确实持有这个监听 socket。
        confidence: Confidence::High,
        source: "runtime_probe".to_string(),
        validity: ValidityWindow {
            valid_from: observed_at,
            valid_to: None,
        },
        created_at: now,
        updated_at: now,
    };

    let endpoint_id = stable_uuid(
        "instance_endpoint",
        &format!(
            "{}:{}:{}:{}",
            instance_id, candidate.protocol, candidate.address, candidate.port
        ),
    );
    let endpoint = InstanceEndpoint {
        endpoint_id,
        tenant_id,
        instance_id,
        address: candidate.address.clone(),
        port: candidate.port,
        protocol: candidate.protocol.clone(),
        source: "runtime_probe".to_string(),
        valid_from: observed_at,
        valid_to: None,
        created_at: now,
        updated_at: now,
    };

    EndpointPlan {
        service,
        instance,
        binding,
        endpoint,
    }
}

/// 在主机进程里挑出持有该 socket 的那一个。
///
/// pid 会被复用，`list_process_runtime_states` 又会把已 stale 的行一起返回，
/// 所以不能 `find(pid == ...)` 了事：
///
/// 1. 有 `process_identity` 时先按身份精确匹配（只有它能区分同 pid 的不同进程）；
/// 2. 否则在同 pid 里优先**没 stale** 的，再比 `observed_at` 取最新。
fn pick_process<'a>(
    processes: &'a [ProcessRuntimeState],
    candidate: &ServiceEndpointCandidate,
) -> Option<&'a ProcessRuntimeState> {
    let pid = candidate.pid?;
    let mut same_pid = processes.iter().filter(|process| process.pid == pid);
    let first = same_pid.next()?;
    let mut candidates = vec![first];
    candidates.extend(same_pid);

    if let Some(identity) = candidate.process_identity.as_deref() {
        let matched = candidates
            .iter()
            .copied()
            .filter(|process| {
                process
                    .external_ref
                    .as_deref()
                    .map(|reference| reference.ends_with(identity))
                    .unwrap_or(false)
            })
            .max_by_key(|process| (process.stale_at.is_none(), process.observed_at.0));
        if let Some(process) = matched {
            return Some(process);
        }
    }

    candidates
        .into_iter()
        .max_by_key(|process| (process.stale_at.is_none(), process.observed_at.0))
}

fn resolve_host_id(
    catalog: &InMemoryCatalog,
    candidate: &ServiceEndpointCandidate,
) -> Option<Uuid> {
    if let Some(machine_id) = candidate.machine_id.as_deref() {
        if let Some(host) = catalog
            .hosts
            .iter()
            .find(|host| host.machine_id.as_deref() == Some(machine_id))
        {
            return Some(host.host_id);
        }
    }

    let host_name = candidate.host_name.as_deref()?;
    catalog
        .hosts
        .iter()
        .find(|host| host.host_name == host_name)
        .map(|host| host.host_id)
}

/// 本次快照里没再出现的端点：置 `valid_to`，不删行。
///
/// 两层差异：
///
/// - **本轮的实例**：跟本轮的端口集比，没出现的关掉（端口集变了，进程还活着）；
/// - **本主机上本轮根本没碰到的实例**（进程还在、端口全关了）：按 host 把实例拉出来一起比。
///   不这么做的话，关掉的端口会永远显示在暴露面上 —— 那比缺数据更糟。
///
/// `reported == false`（输入里没 `service_endpoints` 字段）时一律不清扫：
/// 采集端不支持端点的情况不能当成「一个端口都没开」。
fn close_stale_endpoints<S>(
    store: &S,
    reported: bool,
    sweep_hosts: &BTreeSet<Uuid>,
    seen_by_instance: &BTreeMap<Uuid, BTreeSet<Uuid>>,
    now: DateTime<Utc>,
) -> StorageResult<u64>
where
    S: RuntimeStore,
{
    if !reported {
        return Ok(0);
    }

    let mut instance_ids: BTreeSet<Uuid> = seen_by_instance.keys().copied().collect();
    for host_id in sweep_hosts {
        instance_ids.extend(store.list_instance_ids_for_host(*host_id)?);
    }

    let empty: BTreeSet<Uuid> = BTreeSet::new();
    let mut closed = 0;
    for instance_id in instance_ids {
        let seen = seen_by_instance.get(&instance_id).unwrap_or(&empty);
        for endpoint in list_live_endpoints(store, instance_id)? {
            if seen.contains(&endpoint.endpoint_id) {
                continue;
            }
            store.close_instance_endpoint(endpoint.endpoint_id, now)?;
            closed += 1;
        }
    }
    Ok(closed)
}

/// 把该实例指向**其它进程**的未关闭绑定收尾。
///
/// `runtime_binding` 的有效期就是「这段时间这个实例由这个进程在跑」。
/// 进程重启后接管同一个端口时，实例 id 不变（服务 + 主机决定），
/// 但宿主进程变了 —— 旧绑定到此为止。
fn close_previous_bindings<S>(
    store: &S,
    instance_id: Uuid,
    keep_process: Uuid,
    at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: RuntimeStore,
{
    for binding in store.list_runtime_bindings_for_instance(instance_id, Page::default())? {
        if binding.object_type != RuntimeObjectType::Process
            || binding.object_id == keep_process
            || binding.validity.valid_to.is_some()
        {
            continue;
        }
        let closed = RuntimeBinding {
            validity: ValidityWindow {
                valid_from: binding.validity.valid_from,
                valid_to: Some(at),
            },
            updated_at: now,
            ..binding
        };
        store.upsert_runtime_binding(&closed)?;
    }
    Ok(())
}

async fn close_previous_bindings_async<S>(
    store: &S,
    instance_id: Uuid,
    keep_process: Uuid,
    at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: atlas_storage::AsyncRuntimeStore,
{
    use atlas_storage::AsyncRuntimeStore;

    for binding in
        AsyncRuntimeStore::list_runtime_bindings_for_instance(store, instance_id, Page::default())
            .await?
    {
        if binding.object_type != RuntimeObjectType::Process
            || binding.object_id == keep_process
            || binding.validity.valid_to.is_some()
        {
            continue;
        }
        let closed = RuntimeBinding {
            validity: ValidityWindow {
                valid_from: binding.validity.valid_from,
                valid_to: Some(at),
            },
            updated_at: now,
            ..binding
        };
        AsyncRuntimeStore::upsert_runtime_binding(store, &closed).await?;
    }
    Ok(())
}

/// 翻页取完一个实例的活端点：只取第一页会把第 101 个之后的端点永远留着。
fn list_live_endpoints<S>(store: &S, instance_id: Uuid) -> StorageResult<Vec<InstanceEndpoint>>
where
    S: RuntimeStore,
{
    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = store.list_instance_endpoints(
            instance_id,
            Page {
                limit: ENDPOINT_PAGE_LIMIT,
                offset,
            },
        )?;
        let len = batch.len();
        items.extend(batch);
        if len < ENDPOINT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

async fn list_live_endpoints_async<S>(
    store: &S,
    instance_id: Uuid,
) -> StorageResult<Vec<InstanceEndpoint>>
where
    S: atlas_storage::AsyncRuntimeStore,
{
    use atlas_storage::AsyncRuntimeStore;

    let mut items = Vec::new();
    let mut offset = 0;
    loop {
        let batch = AsyncRuntimeStore::list_instance_endpoints(
            store,
            instance_id,
            Page {
                limit: ENDPOINT_PAGE_LIMIT,
                offset,
            },
        )
        .await?;
        let len = batch.len();
        items.extend(batch);
        if len < ENDPOINT_PAGE_LIMIT as usize {
            break;
        }
        offset += len as u32;
    }
    Ok(items)
}

async fn close_stale_endpoints_async<S>(
    store: &S,
    reported: bool,
    sweep_hosts: &BTreeSet<Uuid>,
    seen_by_instance: &BTreeMap<Uuid, BTreeSet<Uuid>>,
    now: DateTime<Utc>,
) -> StorageResult<u64>
where
    S: atlas_storage::AsyncRuntimeStore,
{
    use atlas_storage::AsyncRuntimeStore;

    if !reported {
        return Ok(0);
    }

    let mut instance_ids: BTreeSet<Uuid> = seen_by_instance.keys().copied().collect();
    for host_id in sweep_hosts {
        instance_ids.extend(AsyncRuntimeStore::list_instance_ids_for_host(store, *host_id).await?);
    }

    let empty: BTreeSet<Uuid> = BTreeSet::new();
    let mut closed = 0;
    for instance_id in instance_ids {
        let seen = seen_by_instance.get(&instance_id).unwrap_or(&empty);
        for endpoint in list_live_endpoints_async(store, instance_id).await? {
            if seen.contains(&endpoint.endpoint_id) {
                continue;
            }
            AsyncRuntimeStore::close_instance_endpoint(store, endpoint.endpoint_id, now).await?;
            closed += 1;
        }
    }
    Ok(closed)
}
