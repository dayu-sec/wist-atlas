pub const RESET_PUBLIC_SCHEMA: &str = r#"
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
"#;

pub const UPSERT_HOST: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, ''), NULLIF($11, '')::timestamptz, NULLIF($12, ''),
    COALESCE(NULLIF($13, '')::bigint, 0)
)
ON CONFLICT (machine_id) WHERE machine_id IS NOT NULL DO UPDATE SET
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
    ingest_scope = COALESCE(EXCLUDED.ingest_scope, host_inventory.ingest_scope)
"#;

pub const UPSERT_HOST_WITHOUT_MACHINE_ID: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz,
    NULLIF($10, ''), NULLIF($11, '')::timestamptz, NULLIF($12, ''),
    COALESCE(NULLIF($13, '')::bigint, 0)
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
    stale_at = EXCLUDED.stale_at,
    miss_count = EXCLUDED.miss_count,
    ingest_scope = COALESCE(EXCLUDED.ingest_scope, host_inventory.ingest_scope)
"#;

pub const LIST_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count
FROM host_inventory
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const LIST_ALL_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count
FROM host_inventory
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($1, '')::int4 OFFSET NULLIF($2, '')::int4
"#;

pub const GET_HOST: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at, arch, stale_at, ingest_scope, miss_count
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
    network_domain_id, tenant_id, environment_id, name, kind, description, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4, $5,
    NULLIF($6, ''), NULLIF($7, '')::timestamptz, NULLIF($8, '')::timestamptz
)
ON CONFLICT (network_domain_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    kind = EXCLUDED.kind,
    description = EXCLUDED.description,
    updated_at = EXCLUDED.updated_at
"#;

pub const UPSERT_NETWORK_SEGMENT: &str = r#"
INSERT INTO network_segment (
    network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
    address_family, created_at, updated_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, $5, NULLIF($6, ''), NULLIF($7, ''), $8,
    NULLIF($9, '')::timestamptz, NULLIF($10, '')::timestamptz
)
ON CONFLICT (network_segment_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    network_domain_id = EXCLUDED.network_domain_id,
    environment_id = EXCLUDED.environment_id,
    name = EXCLUDED.name,
    cidr = EXCLUDED.cidr,
    gateway_ip = EXCLUDED.gateway_ip,
    address_family = EXCLUDED.address_family,
    updated_at = EXCLUDED.updated_at
"#;

pub const GET_NETWORK_SEGMENT: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at
FROM network_segment
WHERE network_segment_id = NULLIF($1, '')::uuid
"#;

pub const LIST_NETWORK_SEGMENTS: &str = r#"
SELECT network_segment_id, tenant_id, network_domain_id, environment_id, name, cidr, gateway_ip,
       address_family, created_at, updated_at
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
