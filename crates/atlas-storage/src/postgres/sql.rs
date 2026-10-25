pub const RESET_PUBLIC_SCHEMA: &str = r#"
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
"#;

pub const UPSERT_HOST: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz
)
ON CONFLICT (machine_id) WHERE machine_id IS NOT NULL DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    host_name = EXCLUDED.host_name,
    machine_id = EXCLUDED.machine_id,
    os_name = EXCLUDED.os_name,
    os_version = EXCLUDED.os_version,
    last_inventory_at = EXCLUDED.last_inventory_at
"#;

pub const UPSERT_HOST_WITHOUT_MACHINE_ID: &str = r#"
INSERT INTO host_inventory (
    host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
    created_at, last_inventory_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid, $4,
    NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
    NULLIF($8, '')::timestamptz, NULLIF($9, '')::timestamptz
)
ON CONFLICT (host_id) DO UPDATE SET
    tenant_id = EXCLUDED.tenant_id,
    environment_id = EXCLUDED.environment_id,
    host_name = EXCLUDED.host_name,
    machine_id = EXCLUDED.machine_id,
    os_name = EXCLUDED.os_name,
    os_version = EXCLUDED.os_version,
    last_inventory_at = EXCLUDED.last_inventory_at
"#;

pub const LIST_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
WHERE tenant_id = NULLIF($1, '')::uuid
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($2, '')::int4 OFFSET NULLIF($3, '')::int4
"#;

pub const LIST_ALL_HOSTS: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
FROM host_inventory
ORDER BY host_name ASC, host_id ASC
LIMIT NULLIF($1, '')::int4 OFFSET NULLIF($2, '')::int4
"#;

pub const GET_HOST: &str = r#"
SELECT host_id, tenant_id, environment_id, host_name, machine_id, os_name, os_version,
       created_at, last_inventory_at
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
    process_state, memory_rss_kib, started_at, observed_at
) VALUES (
    NULLIF($1, '')::uuid, NULLIF($2, '')::uuid, NULLIF($3, '')::uuid,
    NULLIF($4, '')::uuid, NULLIF($5, ''), NULLIF($6, '')::integer, $7,
    NULLIF($8, ''), NULLIF($9, ''), NULLIF($10, '')::bigint,
    NULLIF($11, '')::timestamptz, NULLIF($12, '')::timestamptz
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
    observed_at = EXCLUDED.observed_at
"#;

pub const LIST_PROCESS_RUNTIME_STATES: &str = r#"
SELECT process_id, tenant_id, host_id, container_id, external_ref, pid, executable, command_line,
       process_state, memory_rss_kib, started_at, observed_at
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
