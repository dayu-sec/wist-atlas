-- wist-atlas initial PostgreSQL schema.
-- This migration defines the first source-of-truth tables required by P0.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS business_domain (
    business_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS system_boundary (
    system_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    business_id UUID NOT NULL REFERENCES business_domain(business_id),
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, name)
);

CREATE INDEX IF NOT EXISTS idx_system_boundary_business_id
    ON system_boundary(business_id);

CREATE TABLE IF NOT EXISTS subsystem (
    subsystem_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    system_id UUID NOT NULL REFERENCES system_boundary(system_id),
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (system_id, name)
);

CREATE INDEX IF NOT EXISTS idx_subsystem_system_id
    ON subsystem(system_id);

CREATE TABLE IF NOT EXISTS service_entity (
    service_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    business_id UUID REFERENCES business_domain(business_id),
    system_id UUID REFERENCES system_boundary(system_id),
    subsystem_id UUID REFERENCES subsystem(subsystem_id),
    name TEXT NOT NULL,
    namespace TEXT,
    service_type TEXT NOT NULL,
    boundary TEXT NOT NULL,
    provider TEXT,
    external_ref TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, namespace, name)
);

CREATE INDEX IF NOT EXISTS idx_service_entity_business_id
    ON service_entity(business_id);

CREATE INDEX IF NOT EXISTS idx_service_entity_system_id
    ON service_entity(system_id);

CREATE INDEX IF NOT EXISTS idx_service_entity_subsystem_id
    ON service_entity(subsystem_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_service_entity_external_ref
    ON service_entity(tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS cluster_inventory (
    cluster_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    environment_id UUID,
    name TEXT NOT NULL,
    provider TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS namespace_inventory (
    namespace_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    cluster_id UUID NOT NULL REFERENCES cluster_inventory(cluster_id),
    name TEXT NOT NULL,
    environment_id UUID,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (cluster_id, name)
);

CREATE INDEX IF NOT EXISTS idx_namespace_inventory_tenant_name
    ON namespace_inventory(tenant_id, name);

CREATE TABLE IF NOT EXISTS workload_entity (
    workload_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    cluster_id UUID NOT NULL REFERENCES cluster_inventory(cluster_id),
    namespace_id UUID NOT NULL REFERENCES namespace_inventory(namespace_id),
    service_id UUID REFERENCES service_entity(service_id),
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (namespace_id, kind, name)
);

CREATE INDEX IF NOT EXISTS idx_workload_entity_service_id
    ON workload_entity(service_id);

CREATE INDEX IF NOT EXISTS idx_workload_entity_cluster_id
    ON workload_entity(cluster_id);

CREATE TABLE IF NOT EXISTS host_inventory (
    host_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    environment_id UUID,
    host_name TEXT NOT NULL,
    machine_id TEXT,
    os_name TEXT,
    os_version TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    last_inventory_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, host_name),
    CHECK (last_inventory_at >= created_at)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_host_inventory_machine_id
    ON host_inventory(machine_id)
    WHERE machine_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS network_domain (
    network_domain_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    environment_id UUID,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS network_segment (
    network_segment_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    network_domain_id UUID REFERENCES network_domain(network_domain_id),
    environment_id UUID,
    name TEXT NOT NULL,
    cidr TEXT,
    gateway_ip TEXT,
    address_family TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, name)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_network_segment_cidr
    ON network_segment(tenant_id, cidr)
    WHERE cidr IS NOT NULL;

CREATE TABLE IF NOT EXISTS host_net_assoc (
    assoc_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    host_id UUID NOT NULL REFERENCES host_inventory(host_id),
    network_segment_id UUID NOT NULL REFERENCES network_segment(network_segment_id),
    ip_addr TEXT NOT NULL,
    iface_name TEXT,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (valid_to IS NULL OR valid_to > valid_from)
);

CREATE INDEX IF NOT EXISTS idx_host_net_assoc_host
    ON host_net_assoc(host_id);

CREATE INDEX IF NOT EXISTS idx_host_net_assoc_segment
    ON host_net_assoc(network_segment_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_host_net_assoc_open_interval
    ON host_net_assoc(host_id, network_segment_id, ip_addr)
    WHERE valid_to IS NULL;

CREATE TABLE IF NOT EXISTS pod_inventory (
    pod_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    cluster_id UUID NOT NULL REFERENCES cluster_inventory(cluster_id),
    namespace_id UUID NOT NULL REFERENCES namespace_inventory(namespace_id),
    workload_id UUID REFERENCES workload_entity(workload_id),
    pod_uid TEXT NOT NULL,
    pod_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, cluster_id, pod_uid)
);

CREATE INDEX IF NOT EXISTS idx_pod_inventory_namespace_name
    ON pod_inventory(namespace_id, pod_name);

CREATE TABLE IF NOT EXISTS subject (
    subject_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    subject_type TEXT NOT NULL,
    display_name TEXT NOT NULL,
    external_ref TEXT,
    email TEXT,
    is_active BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_subject_external_ref
    ON subject(tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_subject_email
    ON subject(tenant_id, email)
    WHERE email IS NOT NULL;

CREATE TABLE IF NOT EXISTS host_runtime_state (
    host_id UUID NOT NULL REFERENCES host_inventory(host_id),
    observed_at TIMESTAMPTZ NOT NULL,
    boot_id TEXT,
    uptime_seconds BIGINT,
    loadavg_1m DOUBLE PRECISION,
    loadavg_5m DOUBLE PRECISION,
    loadavg_15m DOUBLE PRECISION,
    cpu_usage_pct DOUBLE PRECISION,
    memory_used_bytes BIGINT,
    memory_available_bytes BIGINT,
    process_count BIGINT,
    container_count BIGINT,
    agent_health TEXT NOT NULL,
    PRIMARY KEY (host_id, observed_at)
);

CREATE TABLE IF NOT EXISTS process_runtime_state (
    process_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    host_id UUID NOT NULL REFERENCES host_inventory(host_id),
    container_id UUID,
    external_ref TEXT,
    pid INTEGER NOT NULL,
    executable TEXT NOT NULL,
    command_line TEXT,
    process_state TEXT,
    memory_rss_kib BIGINT,
    started_at TIMESTAMPTZ NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_process_runtime_state_host_id
    ON process_runtime_state(host_id);

CREATE INDEX IF NOT EXISTS idx_process_runtime_state_external_ref
    ON process_runtime_state(tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS service_instance (
    instance_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    service_id UUID NOT NULL REFERENCES service_entity(service_id),
    workload_id UUID REFERENCES workload_entity(workload_id),
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    last_seen_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_service_instance_service_id
    ON service_instance(service_id);

CREATE TABLE IF NOT EXISTS runtime_binding (
    binding_id UUID PRIMARY KEY,
    instance_id UUID NOT NULL REFERENCES service_instance(instance_id),
    object_type TEXT NOT NULL,
    object_id UUID NOT NULL,
    scope TEXT NOT NULL,
    confidence TEXT NOT NULL,
    source TEXT NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (valid_to IS NULL OR valid_to > valid_from)
);

CREATE INDEX IF NOT EXISTS idx_runtime_binding_instance_id
    ON runtime_binding(instance_id);

CREATE INDEX IF NOT EXISTS idx_runtime_binding_object
    ON runtime_binding(object_type, object_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_runtime_binding_open_interval
    ON runtime_binding(instance_id, object_type, object_id)
    WHERE valid_to IS NULL;

CREATE TABLE IF NOT EXISTS workload_pod_membership (
    membership_id UUID PRIMARY KEY,
    workload_id UUID NOT NULL REFERENCES workload_entity(workload_id),
    pod_id UUID NOT NULL REFERENCES pod_inventory(pod_id),
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    CHECK (valid_to IS NULL OR valid_to > valid_from)
);

CREATE TABLE IF NOT EXISTS pod_placement (
    placement_id UUID PRIMARY KEY,
    pod_id UUID NOT NULL REFERENCES pod_inventory(pod_id),
    host_id UUID NOT NULL REFERENCES host_inventory(host_id),
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    CHECK (valid_to IS NULL OR valid_to > valid_from)
);

CREATE TABLE IF NOT EXISTS responsibility_assignment (
    assignment_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    subject_id UUID NOT NULL REFERENCES subject(subject_id),
    target_kind TEXT NOT NULL,
    target_id UUID NOT NULL,
    role TEXT NOT NULL,
    source TEXT NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CHECK (valid_to IS NULL OR valid_to > valid_from)
);

CREATE INDEX IF NOT EXISTS idx_responsibility_assignment_target
    ON responsibility_assignment(target_kind, target_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_responsibility_assignment_open_interval
    ON responsibility_assignment(subject_id, target_kind, target_id, role)
    WHERE valid_to IS NULL;

CREATE TABLE IF NOT EXISTS external_identity_link (
    link_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    system_type TEXT NOT NULL,
    object_type TEXT NOT NULL,
    external_id TEXT NOT NULL,
    external_key TEXT,
    internal_kind TEXT NOT NULL,
    internal_id UUID NOT NULL,
    status TEXT NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    last_synced_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, system_type, object_type, external_id)
);

CREATE INDEX IF NOT EXISTS idx_external_identity_link_internal
    ON external_identity_link(internal_kind, internal_id);

CREATE TABLE IF NOT EXISTS external_sync_cursor (
    cursor_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    system_type TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    cursor_value TEXT,
    full_sync_token TEXT,
    last_success_at TIMESTAMPTZ,
    last_attempt_at TIMESTAMPTZ,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (tenant_id, system_type, scope_key)
);

CREATE TABLE IF NOT EXISTS ingest_job (
    ingest_id TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL,
    source_kind TEXT NOT NULL,
    source_name TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    payload_ref TEXT,
    error TEXT
);

CREATE TABLE IF NOT EXISTS sync_job (
    sync_job_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    system_type TEXT NOT NULL,
    scope_key TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    finished_at TIMESTAMPTZ,
    status TEXT NOT NULL,
    staged_payload_ref TEXT,
    error TEXT
);

INSERT INTO schema_migrations(version)
VALUES ('0001_initial_schema')
ON CONFLICT (version) DO NOTHING;
