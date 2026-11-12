pub const RESET_PUBLIC_SCHEMA: &str = r#"
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
"#;

/// 主机 upsert。
///
/// **仲裁列是主键 `host_id`，不是 `machine_id`。**
///
/// 为什么：`host_id` 是每一行都有的身份（`stable_uuid` 依次用 machine_id / cloud_instance_id /
/// host_name 派生），所以任何一次写入都能对上已有的行。反过来，用 `machine_id` 当仲裁列会
/// 在两个真实场景里**撞主键**——云侧先建行（`machine_id` 为空）、端侧后补机器 ID，
/// 或者主机先按名字入库、之后才报机器 ID：插入时主键冲突不在仲裁范围内，直接报错。
///
/// `machine_id` 的重复仍然由 `idx_host_inventory_machine_id` 拦住 —— 宁可让这一帧失败，
/// 也不要像旧写法那样把一台机器静默改挂到另一个 `host_id` 上。
///
/// `ingest_scope` / `cloud_instance_id` / `cloud_account` 用 COALESCE：本次没带值就保留旧值，
/// 别让「端侧不知道云账号」变成一次数据丢失。
pub const UPSERT_HOST: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count,
    cloud_instance_id, cloud_account
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, ''), NULLIF($11, '')::timestamptz, NULLIF($12, ''),
    COALESCE(NULLIF($13, '')::bigint, 0),
    NULLIF($14, ''),
    NULLIF($15, '')
)
ON CONFLICT (host_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    host_name = EXCLUDED.host_name,
    machine_id = EXCLUDED.machine_id,
    os_name = EXCLUDED.os_name,
    os_version = EXCLUDED.os_version,
    arch = EXCLUDED.arch,
    last_inventory_at = EXCLUDED.last_inventory_at,
    -- 看到了就复活，计数归零（与 process 的 stale_at 同一语义）。
    stale_at = EXCLUDED.stale_at,
    miss_count = EXCLUDED.miss_count,
    ingest_scope = COALESCE(EXCLUDED.ingest_scope, host_inventory.ingest_scope),
    cloud_instance_id = COALESCE(EXCLUDED.cloud_instance_id, host_inventory.cloud_instance_id),
    cloud_account = COALESCE(EXCLUDED.cloud_account, host_inventory.cloud_account)
"#;

pub const LIST_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count,
       cloud_instance_id, cloud_account
FROM host_inventory
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const LIST_ALL_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count,
       cloud_instance_id, cloud_account
FROM host_inventory
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($1, '')::int4 OFFSET NULLIF($2, '')::int4
"#;

pub const GET_HOST: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count,
       cloud_instance_id, cloud_account
FROM host_inventory
WHERE host_id = NULLIF($1, '')::uuid
"#;

pub const UPSERT_SERVICE: &str = r#"
INSERT INTO service_entity (
    service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
    service_type, boundary, provider, external_ref, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, '')::uuid, $6, NULLIF($7, ''), $8, $9,
    NULLIF($10, ''), NULLIF($11, ''), NULLIF($12, '')::timestamptz,
    NULLIF($13, '')::timestamptz
)
ON CONFLICT (service_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    business_id = EXCLUDED.business_id,
    system_id = EXCLUDED.system_id,
    subsystem_id = EXCLUDED.subsystem_id,
    name = EXCLUDED.name,
    namespace = EXCLUDED.namespace,
    service_type = EXCLUDED.service_type,
    boundary = EXCLUDED.boundary,
    provider = EXCLUDED.provider,
    external_ref = EXCLUDED.external_ref,
    updated_at = EXCLUDED.updated_at
"#;

pub const GET_SERVICE: &str = r#"
SELECT service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
       service_type, boundary, provider, external_ref, created_at, updated_at
FROM service_entity
WHERE service_id = NULLIF($1, '')::uuid
"#;

pub const LIST_SERVICES: &str = r#"
SELECT service_id, tenant_id, business_id, system_id, subsystem_id, name, namespace,
       service_type, boundary, provider, external_ref, created_at, updated_at
FROM service_entity
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY name ASC, service_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_NETWORK_DOMAIN: &str = r#"
INSERT INTO network_domain (
    network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at,
    external_ref, cidr, cloud_account, stale_at, miss_count, ingest_scope
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4, $5,
    NULLIF($6, ''), NULLIF($7, '')::timestamptz, NULLIF($8, '')::timestamptz,
    NULLIF($9, ''), NULLIF($10, ''), NULLIF($11, ''),
    NULLIF($12, '')::timestamptz, COALESCE(NULLIF($13, '')::bigint, 0), NULLIF($14, '')
)
ON CONFLICT (network_domain_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    kind = EXCLUDED.kind,
    description = EXCLUDED.description,
    updated_at = EXCLUDED.updated_at,
    external_ref = COALESCE(EXCLUDED.external_ref, network_domain.external_ref),
    cidr = COALESCE(EXCLUDED.cidr, network_domain.cidr),
    cloud_account = COALESCE(EXCLUDED.cloud_account, network_domain.cloud_account),
    stale_at = EXCLUDED.stale_at,
    miss_count = EXCLUDED.miss_count,
    ingest_scope = COALESCE(EXCLUDED.ingest_scope, network_domain.ingest_scope)
"#;

pub const LIST_NETWORK_DOMAINS: &str = r#"
SELECT network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at,
       external_ref, cidr, cloud_account, stale_at, miss_count, ingest_scope
FROM network_domain
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY name ASC, network_domain_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

/// 退役权持有者。参数全用 `NULLIF($n,'')::type`：调用方统一传字符串，
/// 直接写 `$1::uuid` 会让 tokio-postgres 按列类型推出 `uuid` 然后拒掉 `&str`。
pub const UPSERT_RETIREMENT_OWNER: &str = r#"
INSERT INTO retirement_owner (tenant_id, cloud_account, producer, claimed_at)
VALUES (NULLIF($1, '')::uuid, $2, $3, NULLIF($4, '')::timestamptz)
ON CONFLICT (tenant_id, cloud_account) DO UPDATE SET
    producer = EXCLUDED.producer,
    claimed_at = EXCLUDED.claimed_at
"#;

pub const GET_RETIREMENT_OWNER: &str = r#"
SELECT tenant_id, cloud_account, producer, claimed_at
FROM retirement_owner
WHERE tenant_id = NULLIF($1, '')::uuid AND cloud_account = $2
"#;

pub const UPSERT_NETWORK_SEGMENT: &str = r#"
INSERT INTO network_segment (
    network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
    address_family, created_at, updated_at, external_ref,
    cloud_account, stale_at, miss_count, ingest_scope
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, $5, NULLIF($6, ''), NULLIF($7, ''), $8,
    NULLIF($9, '')::timestamptz, NULLIF($10, '')::timestamptz,
    NULLIF($11, ''), NULLIF($12, ''),
    NULLIF($13, '')::timestamptz, COALESCE(NULLIF($14, '')::bigint, 0), NULLIF($15, '')
)
ON CONFLICT (network_segment_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    network_domain_id = EXCLUDED.network_domain_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    cidr = EXCLUDED.cidr,
    gateway_ip = EXCLUDED.gateway_ip,
    address_family = EXCLUDED.address_family,
    updated_at = EXCLUDED.updated_at,
    external_ref = COALESCE(EXCLUDED.external_ref, network_segment.external_ref),
    cloud_account = COALESCE(EXCLUDED.cloud_account, network_segment.cloud_account),
    stale_at = EXCLUDED.stale_at,
    miss_count = EXCLUDED.miss_count,
    ingest_scope = COALESCE(EXCLUDED.ingest_scope, network_segment.ingest_scope)
"#;

pub const GET_NETWORK_SEGMENT: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at, external_ref,
       cloud_account, stale_at, miss_count, ingest_scope
FROM network_segment
WHERE network_segment_id = NULLIF($1, '')::uuid
"#;

pub const LIST_NETWORK_SEGMENTS: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at, external_ref,
       cloud_account, stale_at, miss_count, ingest_scope
FROM network_segment
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY name ASC, network_segment_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_HOST_NET_ASSOC: &str = r#"
INSERT INTO host_net_assoc (
    assoc_id, tenant_id, host_id, network_segment_id, ip_addr, iface_name,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, $5, NULLIF($6, ''), NULLIF($7, '')::timestamptz,
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, '')::timestamptz
)
ON CONFLICT (assoc_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    host_id = EXCLUDED.host_id,
    network_segment_id = EXCLUDED.network_segment_id,
    ip_addr = EXCLUDED.ip_addr,
    iface_name = EXCLUDED.iface_name,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

pub const LIST_HOST_NET_ASSOCS: &str = r#"
SELECT assoc_id, tenant_id, host_id, network_segment_id, ip_addr, iface_name,
       valid_from, valid_to, created_at, updated_at
FROM host_net_assoc
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY created_at ASC, assoc_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_ELASTIC_IP: &str = r#"
INSERT INTO elastic_ip (
    elastic_ip_id, tenant_id, environment_id, external_ref, address, status,
    charge_type, bandwidth_mbps, cloud_account,
    bound_target_kind, bound_host_id, bound_external_target_ref, bound_since,
    created_at, last_inventory_at, stale_at, miss_count, ingest_scope
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    $4, $5, $6, NULLIF($7, ''), NULLIF($8, '')::bigint, NULLIF($9, ''),
    NULLIF($10, ''), NULLIF($11, '')::uuid, NULLIF($12, ''),
    NULLIF($13, '')::timestamptz,
    NULLIF($14, '')::timestamptz, NULLIF($15, '')::timestamptz,
    NULLIF($16, '')::timestamptz, COALESCE(NULLIF($17, '')::bigint, 0),
    NULLIF($18, '')
)
ON CONFLICT (elastic_ip_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    external_ref = EXCLUDED.external_ref,
    address = EXCLUDED.address,
    status = EXCLUDED.status,
    charge_type = COALESCE(EXCLUDED.charge_type, elastic_ip.charge_type),
    bandwidth_mbps = COALESCE(EXCLUDED.bandwidth_mbps, elastic_ip.bandwidth_mbps),
    cloud_account = COALESCE(EXCLUDED.cloud_account, elastic_ip.cloud_account),
    -- 解绑时必须能把绑定字段清空，所以这里不能用 COALESCE。
    bound_target_kind = EXCLUDED.bound_target_kind,
    bound_host_id = EXCLUDED.bound_host_id,
    bound_external_target_ref = EXCLUDED.bound_external_target_ref,
    bound_since = EXCLUDED.bound_since,
    last_inventory_at = EXCLUDED.last_inventory_at,
    -- 看到了就复活（与主机的 stale_at 同一语义）。
    stale_at = EXCLUDED.stale_at,
    miss_count = EXCLUDED.miss_count,
    ingest_scope = COALESCE(EXCLUDED.ingest_scope, elastic_ip.ingest_scope)
"#;

pub const LIST_ELASTIC_IPS: &str = r#"
SELECT elastic_ip_id, tenant_id, environment_id, external_ref, address, status,
       charge_type, bandwidth_mbps, cloud_account,
       bound_target_kind, bound_host_id, bound_external_target_ref, bound_since,
       created_at, last_inventory_at, stale_at, miss_count, ingest_scope
FROM elastic_ip
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY address ASC, elastic_ip_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

/// 单库部署（一个库一个租户）的读取，与 `LIST_ALL_HOSTS` 同一条纪律：
/// 租户由库边界给，不在 SQL 里再筛一次。
pub const LIST_ALL_ELASTIC_IPS: &str = r#"
SELECT elastic_ip_id, tenant_id, environment_id, external_ref, address, status,
       charge_type, bandwidth_mbps, cloud_account,
       bound_target_kind, bound_host_id, bound_external_target_ref, bound_since,
       created_at, last_inventory_at, stale_at, miss_count, ingest_scope
FROM elastic_ip
ORDER BY address ASC, elastic_ip_id ASC
LIMIT NULLIF($1, '')::int4 OFFSET NULLIF($2, '')::int4
"#;

pub const UPSERT_SUBJECT: &str = r#"
INSERT INTO subject (
    subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active,
    created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, $3, $4, NULLIF($5, ''),
    NULLIF($6, ''), NULLIF($7, '')::boolean, NULLIF($8, '')::timestamptz,
    NULLIF($9, '')::timestamptz
)
ON CONFLICT (subject_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    subject_type = EXCLUDED.subject_type,
    display_name = EXCLUDED.display_name,
    external_ref = EXCLUDED.external_ref,
    email = EXCLUDED.email,
    is_active = EXCLUDED.is_active,
    updated_at = EXCLUDED.updated_at
"#;

pub const GET_SUBJECT: &str = r#"
SELECT subject_id, tenant_id, subject_type, display_name, external_ref, email, is_active,
       created_at, updated_at
FROM subject
WHERE subject_id = NULLIF($1, '')::uuid
"#;

pub const UPSERT_RESPONSIBILITY_ASSIGNMENT: &str = r#"
INSERT INTO responsibility_assignment (
    assignment_id, tenant_id, subject_id, target_kind, target_id, role, source,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, '')::uuid, $6, $7, NULLIF($8, '')::timestamptz,
    NULLIF($9, '')::timestamptz, NULLIF($10, '')::timestamptz,
    NULLIF($11, '')::timestamptz
)
ON CONFLICT (assignment_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    subject_id = EXCLUDED.subject_id,
    target_kind = EXCLUDED.target_kind,
    target_id = EXCLUDED.target_id,
    role = EXCLUDED.role,
    source = EXCLUDED.source,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

pub const LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET: &str = r#"
SELECT assignment_id, tenant_id, subject_id, target_kind, target_id, role, source,
       valid_from, valid_to, created_at, updated_at
FROM responsibility_assignment
WHERE target_kind = $1 AND target_id = NULLIF($2, '')::uuid
ORDER BY created_at ASC, assignment_id ASC
LIMIT NULLIF($3, '')::int4 OFFSET NULLIF($4, '')::int4
"#;

pub const UPSERT_INGEST_JOB: &str = r#"
INSERT INTO ingest_job (
    ingest_id, tenant_id, source_kind, source_name, received_at, status, payload_ref, error
) VALUES (
    $1, NULLIF($2, '')::uuid, $3, $4, NULLIF($5, '')::timestamptz, $6,
    NULLIF($7, ''), NULLIF($8, '')
)
ON CONFLICT (ingest_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    source_kind = EXCLUDED.source_kind,
    source_name = EXCLUDED.source_name,
    received_at = EXCLUDED.received_at,
    status = EXCLUDED.status,
    payload_ref = EXCLUDED.payload_ref,
    error = EXCLUDED.error
"#;

pub const GET_INGEST_JOB: &str = r#"
SELECT ingest_id, tenant_id, source_kind, source_name, received_at, status, payload_ref, error
FROM ingest_job
WHERE ingest_id = $1
"#;

pub const UPSERT_HOST_RUNTIME_STATE: &str = r#"
INSERT INTO host_runtime_state (
    host_id, observed_at, boot_id, uptime_seconds, loadavg_1m, loadavg_5m, loadavg_15m,
    cpu_usage_pct, memory_used_bytes, memory_available_bytes, process_count, container_count,
    agent_health
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::timestamptz, NULLIF($3, ''),
    NULLIF($4, '')::bigint, NULLIF($5, '')::double precision,
    NULLIF($6, '')::double precision, NULLIF($7, '')::double precision,
    NULLIF($8, '')::double precision, NULLIF($9, '')::bigint,
    NULLIF($10, '')::bigint, NULLIF($11, '')::bigint,
    NULLIF($12, '')::bigint, $13
)
ON CONFLICT (host_id, observed_at) DO UPDATE SET
    boot_id = EXCLUDED.boot_id,
    uptime_seconds = EXCLUDED.uptime_seconds,
    loadavg_1m = EXCLUDED.loadavg_1m,
    loadavg_5m = EXCLUDED.loadavg_5m,
    loadavg_15m = EXCLUDED.loadavg_15m,
    cpu_usage_pct = EXCLUDED.cpu_usage_pct,
    memory_used_bytes = EXCLUDED.memory_used_bytes,
    memory_available_bytes = EXCLUDED.memory_available_bytes,
    process_count = EXCLUDED.process_count,
    container_count = EXCLUDED.container_count,
    agent_health = EXCLUDED.agent_health
"#;

pub const LIST_HOST_RUNTIME_STATES: &str = r#"
SELECT host_id, observed_at, boot_id, uptime_seconds, loadavg_1m, loadavg_5m, loadavg_15m,
       cpu_usage_pct, memory_used_bytes, memory_available_bytes, process_count, container_count,
       agent_health
FROM host_runtime_state
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY observed_at DESC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_PROCESS_RUNTIME_STATE: &str = r#"
INSERT INTO process_runtime_state (
    process_id, tenant_id, host_id, container_id, external_ref, pid, executable, command_line,
    process_state, memory_rss_kib, started_at, observed_at, stale_at, ingest_scope
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, ''), NULLIF($6, '')::integer, $7,
    NULLIF($8, ''), NULLIF($9, ''), NULLIF($10, '')::bigint,
    NULLIF($11, '')::timestamptz, NULLIF($12, '')::timestamptz,
    NULLIF($13, '')::timestamptz, NULLIF($14, '')
)
ON CONFLICT (process_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    host_id = EXCLUDED.host_id,
    container_id = EXCLUDED.container_id,
    external_ref = EXCLUDED.external_ref,
    pid = EXCLUDED.pid,
    executable = EXCLUDED.executable,
    command_line = EXCLUDED.command_line,
    process_state = EXCLUDED.process_state,
    memory_rss_kib = EXCLUDED.memory_rss_kib,
    started_at = EXCLUDED.started_at,
    observed_at = EXCLUDED.observed_at,
    -- `stale_at` 跟着入参走，不单独保留：活的对象传 NULL ⇒ 复活时自动清掉标记。
    stale_at = EXCLUDED.stale_at,
    -- 作用域只“首次归属”：老行没有 scope 时补上，已有就不覆盖（同一对象可被多个生产者看见）。
    ingest_scope = COALESCE(process_runtime_state.ingest_scope, EXCLUDED.ingest_scope)
"#;

pub const LIST_PROCESS_RUNTIME_STATES: &str = r#"
SELECT process_id, tenant_id, host_id, container_id, external_ref, pid, executable, command_line,
       process_state, memory_rss_kib, started_at, observed_at, stale_at, ingest_scope
FROM process_runtime_state
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY observed_at DESC, process_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

// 主机列表页的规模列。只数存活进程：退役的行仍留在表里（presence 语义），
// 不过滤 `stale_at` 会把「历史上跑过多少进程」当成「现在有多少」。
pub const LIST_HOST_PROCESS_COUNTS: &str = r#"
SELECT host_id, COUNT(*) AS process_count
FROM process_runtime_state
WHERE stale_at IS NULL
GROUP BY host_id
ORDER BY host_id
LIMIT NULLIF($1, '')::int4 OFFSET NULLIF($2, '')::int4
"#;

pub const UPSERT_CONTAINER_RUNTIME: &str = r#"
INSERT INTO container_runtime (
    container_id, tenant_id, pod_id, host_id, runtime_type, runtime_namespace,
    container_name, image_ref, external_ref, started_at, last_seen_at, stale_at, ingest_scope,
    pod_uid, pod_name, k8s_namespace
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, $5, NULLIF($6, ''), NULLIF($7, ''), NULLIF($8, ''),
    NULLIF($9, ''), NULLIF($10, '')::timestamptz, NULLIF($11, '')::timestamptz,
    NULLIF($12, '')::timestamptz, NULLIF($13, ''),
    NULLIF($14, ''), NULLIF($15, ''), NULLIF($16, '')
)
ON CONFLICT (container_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    pod_id = EXCLUDED.pod_id,
    host_id = EXCLUDED.host_id,
    runtime_type = EXCLUDED.runtime_type,
    runtime_namespace = EXCLUDED.runtime_namespace,
    container_name = EXCLUDED.container_name,
    image_ref = EXCLUDED.image_ref,
    external_ref = EXCLUDED.external_ref,
    started_at = EXCLUDED.started_at,
    last_seen_at = EXCLUDED.last_seen_at,
    stale_at = EXCLUDED.stale_at,
    ingest_scope = EXCLUDED.ingest_scope,
    pod_uid = EXCLUDED.pod_uid,
    pod_name = EXCLUDED.pod_name,
    k8s_namespace = EXCLUDED.k8s_namespace
"#;

// 主机的容器列表：只给活着的（`stale_at IS NULL`）。退役行留着是为了回溯「上周跑过
// 哪些容器」，不该再出现在现役视图里 —— 与进程、端点的读侧口径一致。
pub const LIST_CONTAINER_RUNTIMES_FOR_HOST: &str = r#"
SELECT container_id, tenant_id, pod_id, host_id, runtime_type, runtime_namespace,
       container_name, image_ref, external_ref, started_at, last_seen_at, stale_at, ingest_scope,
       pod_uid, pod_name, k8s_namespace
FROM container_runtime
WHERE host_id = NULLIF($1, '')::uuid AND stale_at IS NULL
ORDER BY container_name ASC NULLS LAST, container_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_SERVICE_INSTANCE: &str = r#"
INSERT INTO service_instance (
    instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, '')::timestamptz,
    NULLIF($6, '')::timestamptz, NULLIF($7, '')::timestamptz
)
ON CONFLICT (instance_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    service_id = EXCLUDED.service_id,
    workload_id = EXCLUDED.workload_id,
    started_at = EXCLUDED.started_at,
    ended_at = EXCLUDED.ended_at,
    last_seen_at = EXCLUDED.last_seen_at
"#;

pub const GET_SERVICE_INSTANCE: &str = r#"
SELECT instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
FROM service_instance
WHERE instance_id = NULLIF($1, '')::uuid
"#;

pub const LIST_SERVICE_INSTANCES: &str = r#"
SELECT instance_id, tenant_id, service_id, workload_id, started_at, ended_at, last_seen_at
FROM service_instance
WHERE service_id = NULLIF($1, '')::uuid
ORDER BY started_at DESC, instance_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_INSTANCE_ENDPOINT: &str = r#"
INSERT INTO instance_endpoint (
    endpoint_id, tenant_id, inst_id, address, port, protocol, source,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, '')::int4, $6, $7, NULLIF($8, '')::timestamptz,
    NULLIF($9, '')::timestamptz, NULLIF($10, '')::timestamptz, NULLIF($11, '')::timestamptz
)
ON CONFLICT (endpoint_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    inst_id = EXCLUDED.inst_id,
    address = EXCLUDED.address,
    port = EXCLUDED.port,
    protocol = EXCLUDED.protocol,
    source = EXCLUDED.source,
    -- 一段暴露窗口的起点不该被每份快照推后（那样 valid_from 就退化成「上次观测」）。
    -- 但如果这行已经被关过（valid_to 非空），这次是**重新开监听**，起点重新计。
    valid_from = CASE
        WHEN instance_endpoint.valid_to IS NOT NULL THEN EXCLUDED.valid_from
        ELSE LEAST(instance_endpoint.valid_from, EXCLUDED.valid_from)
    END,
    -- 显式跟随写入值：同一条写入既可能是新开一段，也可能是纠正后重新满上 valid_to。
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

pub const LIST_INSTANCE_ENDPOINTS: &str = r#"
SELECT endpoint_id, tenant_id, inst_id, address, port, protocol, source,
       valid_from, valid_to, created_at, updated_at
FROM instance_endpoint
WHERE inst_id = NULLIF($1, '')::uuid AND valid_to IS NULL
ORDER BY protocol, address, port
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const CLOSE_INSTANCE_ENDPOINT: &str = r#"
UPDATE instance_endpoint
SET valid_to = NULLIF($2, '')::timestamptz,
    updated_at = NULLIF($2, '')::timestamptz
WHERE endpoint_id = NULLIF($1, '')::uuid AND valid_to IS NULL
"#;

// 「这台主机上还活着的实例」是关端点时的批量入口：不 JOIN service_instance，
// 因为实例是否存活只由它的进程（stale_at）决定，再绕一层反而是多余的条件。
pub const LIST_INSTANCE_IDS_FOR_HOST: &str = r#"
SELECT DISTINCT rb.instance_id
FROM runtime_binding rb
JOIN process_runtime_state p ON p.process_id = rb.object_id
WHERE rb.object_type = 'Process'
  AND p.host_id = NULLIF($1, '')::uuid
  AND p.stale_at IS NULL
ORDER BY rb.instance_id
"#;

pub const UPSERT_RUNTIME_BINDING: &str = r#"
INSERT INTO runtime_binding (
    binding_id, instance_id, object_type, object_id, scope, confidence, source,
    valid_from, valid_to, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, $3, NULLIF($4, '')::uuid, $5,
    $6, $7, NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, '')::timestamptz, NULLIF($11, '')::timestamptz
)
ON CONFLICT (binding_id) DO UPDATE SET
    instance_id = EXCLUDED.instance_id,
    object_type = EXCLUDED.object_type,
    object_id = EXCLUDED.object_id,
    scope = EXCLUDED.scope,
    confidence = EXCLUDED.confidence,
    source = EXCLUDED.source,
    valid_from = EXCLUDED.valid_from,
    valid_to = EXCLUDED.valid_to,
    updated_at = EXCLUDED.updated_at
"#;

pub const GET_RUNTIME_BINDING: &str = r#"
SELECT binding_id, instance_id, object_type, object_id, scope, confidence, source,
       valid_from, valid_to, created_at, updated_at
FROM runtime_binding
WHERE binding_id = NULLIF($1, '')::uuid
"#;

pub const LIST_RUNTIME_BINDINGS_FOR_INSTANCE: &str = r#"
SELECT binding_id, instance_id, object_type, object_id, scope, confidence, source,
       valid_from, valid_to, created_at, updated_at
FROM runtime_binding
WHERE instance_id = NULLIF($1, '')::uuid
ORDER BY created_at ASC, binding_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const LIST_RUNTIME_BINDINGS_FOR_OBJECT: &str = r#"
SELECT binding_id, instance_id, object_type, object_id, scope, confidence, source,
       valid_from, valid_to, created_at, updated_at
FROM runtime_binding
WHERE object_type = $1 AND object_id = NULLIF($2, '')::uuid
ORDER BY created_at ASC, binding_id ASC
LIMIT NULLIF($3, '')::int4 OFFSET NULLIF($4, '')::int4
"#;

// ── 软件指纹与实例（0006） ──────────────────────────────────

pub const UPSERT_SOFTWARE_ARTIFACT: &str = r#"
INSERT INTO software_artifact (
    artifact_id, version_id, artifact_kind, sha256, size_bytes, build_id,
    package_manager, package_name, purl, interpreter_artifact_id, coordinate,
    first_seen_at, last_seen_at
) VALUES (
    $1, NULLIF($2, ''), COALESCE(NULLIF($3, ''), 'executable'), $4,
    NULLIF($5, '')::bigint, NULLIF($6, ''), NULLIF($7, ''), NULLIF($8, ''),
    NULLIF($9, ''), NULLIF($10, ''), NULLIF($11, '')::jsonb,
    NULLIF($12, '')::timestamptz, NULLIF($13, '')::timestamptz
)
ON CONFLICT (artifact_id) DO UPDATE SET
    version_id = COALESCE(EXCLUDED.version_id, software_artifact.version_id),
    artifact_kind = EXCLUDED.artifact_kind,
    sha256 = EXCLUDED.sha256,
    size_bytes = COALESCE(EXCLUDED.size_bytes, software_artifact.size_bytes),
    build_id = COALESCE(EXCLUDED.build_id, software_artifact.build_id),
    package_manager = COALESCE(EXCLUDED.package_manager, software_artifact.package_manager),
    package_name = COALESCE(EXCLUDED.package_name, software_artifact.package_name),
    purl = COALESCE(EXCLUDED.purl, software_artifact.purl),
    interpreter_artifact_id =
        COALESCE(EXCLUDED.interpreter_artifact_id, software_artifact.interpreter_artifact_id),
    coordinate = COALESCE(EXCLUDED.coordinate, software_artifact.coordinate),
    -- 首次见到保留最早；最后见到取更晚的（乱序重投不会把时间倒退）。
    first_seen_at = LEAST(software_artifact.first_seen_at, EXCLUDED.first_seen_at),
    last_seen_at = GREATEST(software_artifact.last_seen_at, EXCLUDED.last_seen_at)
"#;

pub const GET_SOFTWARE_ARTIFACT: &str = r#"
SELECT artifact_id, version_id, artifact_kind, sha256, size_bytes, build_id,
       package_manager, package_name, purl, interpreter_artifact_id,
       coordinate::text, first_seen_at, last_seen_at
FROM software_artifact
WHERE artifact_id = $1
"#;

pub const UPSERT_SOFTWARE_INSTANCE: &str = r#"
INSERT INTO software_instance (
    instance_id, tenant_id, host_id, anchor_kind, anchor_value,
    current_artifact_id, identity_missing_reason,
    claimed_package_name, claimed_product_id, claimed_version, claimed_source,
    role, reason, status, first_seen_at, last_seen_at, stale_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4, $5,
    NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, ''), NULLIF($9, ''), NULLIF($10, ''), NULLIF($11, ''),
    COALESCE(NULLIF($12, ''), '[]'), COALESCE(NULLIF($13, ''), '[]'),
    $14, NULLIF($15, '')::timestamptz, NULLIF($16, '')::timestamptz,
    NULLIF($17, '')::timestamptz
)
ON CONFLICT (host_id, anchor_kind, anchor_value) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    current_artifact_id = EXCLUDED.current_artifact_id,
    identity_missing_reason = EXCLUDED.identity_missing_reason,
    claimed_package_name = EXCLUDED.claimed_package_name,
    claimed_product_id = EXCLUDED.claimed_product_id,
    claimed_version = EXCLUDED.claimed_version,
    claimed_source = EXCLUDED.claimed_source,
    role = EXCLUDED.role,
    reason = EXCLUDED.reason,
    status = EXCLUDED.status,
    last_seen_at = EXCLUDED.last_seen_at,
    -- 看到了就复活；计数归零与进程的 stale 同一语义。
    stale_at = EXCLUDED.stale_at
"#;

pub const GET_SOFTWARE_INSTANCE: &str = r#"
SELECT instance_id, tenant_id, host_id, anchor_kind, anchor_value,
       current_artifact_id, identity_missing_reason,
       claimed_package_name, claimed_product_id, claimed_version, claimed_source,
       role, reason, status, first_seen_at, last_seen_at, stale_at
FROM software_instance
WHERE instance_id = NULLIF($1, '')::uuid
"#;

pub const GET_SOFTWARE_INSTANCE_BY_ANCHOR: &str = r#"
SELECT instance_id, tenant_id, host_id, anchor_kind, anchor_value,
       current_artifact_id, identity_missing_reason,
       claimed_package_name, claimed_product_id, claimed_version, claimed_source,
       role, reason, status, first_seen_at, last_seen_at, stale_at
FROM software_instance
WHERE host_id = NULLIF($1, '')::uuid AND anchor_kind = $2 AND anchor_value = $3
"#;

pub const LIST_SOFTWARE_INSTANCES_FOR_HOST: &str = r#"
SELECT instance_id, tenant_id, host_id, anchor_kind, anchor_value,
       current_artifact_id, identity_missing_reason,
       claimed_package_name, claimed_product_id, claimed_version, claimed_source,
       role, reason, status, first_seen_at, last_seen_at, stale_at
FROM software_instance
WHERE host_id = NULLIF($1, '')::uuid AND stale_at IS NULL
ORDER BY anchor_kind ASC, anchor_value ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const LIST_LIVE_SOFTWARE_INSTANCE_ANCHORS: &str = r#"
SELECT instance_id, anchor_kind, anchor_value
FROM software_instance
WHERE host_id = NULLIF($1, '')::uuid AND stale_at IS NULL
"#;

pub const MARK_SOFTWARE_INSTANCE_STALE: &str = r#"
UPDATE software_instance
SET stale_at = NULLIF($2, '')::timestamptz
WHERE instance_id = NULLIF($1, '')::uuid
"#;

pub const UPSERT_SOFTWARE_FINGERPRINT: &str = r#"
INSERT INTO software_fingerprint (
    tenant_id, host_id, artifact_id, size_bytes, build_id, first_seen_at, last_seen_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, $3,
    COALESCE(NULLIF($4, '')::bigint, 0), NULLIF($5, ''),
    NULLIF($6, '')::timestamptz, NULLIF($7, '')::timestamptz
)
ON CONFLICT (tenant_id, host_id, artifact_id) DO UPDATE SET
    size_bytes = EXCLUDED.size_bytes,
    build_id = COALESCE(EXCLUDED.build_id, software_fingerprint.build_id),
    first_seen_at = LEAST(software_fingerprint.first_seen_at, EXCLUDED.first_seen_at),
    last_seen_at = GREATEST(software_fingerprint.last_seen_at, EXCLUDED.last_seen_at)
"#;

pub const LIST_SOFTWARE_FINGERPRINTS_FOR_HOST: &str = r#"
SELECT tenant_id, host_id, artifact_id, size_bytes, build_id, first_seen_at, last_seen_at
FROM software_fingerprint
WHERE host_id = NULLIF($1, '')::uuid
ORDER BY artifact_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_SOFTWARE_INSTANCE_OBSERVATION: &str = r#"
INSERT INTO software_instance_observation (
    instance_id, artifact_id, observed_at, version_running, version_installed
) VALUES (
    NULLIF($1, '')::uuid, $2, NULLIF($3, '')::timestamptz,
    NULLIF($4, ''), NULLIF($5, '')
)
ON CONFLICT (instance_id, artifact_id, observed_at) DO UPDATE SET
    version_running = EXCLUDED.version_running,
    version_installed = EXCLUDED.version_installed
"#;

pub const LIST_SOFTWARE_INSTANCE_OBSERVATIONS: &str = r#"
SELECT instance_id, artifact_id, observed_at, version_running, version_installed
FROM software_instance_observation
WHERE instance_id = NULLIF($1, '')::uuid
ORDER BY observed_at DESC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const UPSERT_SOFTWARE_ARTIFACT_PATH: &str = r#"
INSERT INTO software_artifact_path (
    instance_id, path, role, size_bytes, mtime, inode
) VALUES (
    NULLIF($1, '')::uuid, $2, $3,
    COALESCE(NULLIF($4, '')::bigint, 0), NULLIF($5, '')::timestamptz,
    COALESCE(NULLIF($6, '')::bigint, 0)
)
ON CONFLICT (instance_id, path) DO UPDATE SET
    role = EXCLUDED.role,
    size_bytes = EXCLUDED.size_bytes,
    mtime = EXCLUDED.mtime,
    inode = EXCLUDED.inode
"#;

pub const LIST_SOFTWARE_ARTIFACT_PATHS: &str = r#"
SELECT instance_id, path, role, size_bytes, mtime, inode
FROM software_artifact_path
WHERE instance_id = NULLIF($1, '')::uuid
ORDER BY role ASC, path ASC
"#;
