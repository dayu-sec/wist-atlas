use atlas_domain::{
    HostInventory, HostNetAssoc, HostRuntimeState, NetworkDomain, NetworkSegment, ObjectKind,
    ProcessRuntimeState, ResponsibilityAssignment, RuntimeBinding, RuntimeObjectType,
    ServiceEntity, ServiceInstance, Subject, TenantId,
};
use chrono::{DateTime, Utc};
use orion_error::prelude::SourceRawErr;
use uuid::Uuid;

use crate::memory::IngestJobEntry;
use crate::{StorageReason, StorageResult, decode_failed, operation_failed};

pub(super) fn decode_host(row: &[String]) -> StorageResult<HostInventory> {
    Ok(HostInventory {
        host_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        environment_id: parse_optional_uuid(&row[2])?.map(atlas_domain::EnvironmentId),
        host_name: row[3].clone(),
        machine_id: empty_to_none(&row[4]),
        os_name: empty_to_none(&row[5]),
        os_version: empty_to_none(&row[6]),
        created_at: parse_datetime(&row[7])?,
        last_inventory_at: parse_datetime(&row[8])?,
    })
}

pub(super) fn decode_network_domain(row: &[String]) -> StorageResult<NetworkDomain> {
    Ok(NetworkDomain {
        network_domain_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        environment_id: parse_optional_uuid(&row[2])?.map(atlas_domain::EnvironmentId),
        name: row[3].clone(),
        kind: parse_network_domain_kind(&row[4])?,
        description: empty_to_none(&row[5]),
        created_at: parse_datetime(&row[6])?,
        updated_at: parse_datetime(&row[7])?,
    })
}

pub(super) fn decode_network_segment(row: &[String]) -> StorageResult<NetworkSegment> {
    Ok(NetworkSegment {
        network_segment_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        network_domain_id: parse_optional_uuid(&row[2])?,
        environment_id: parse_optional_uuid(&row[3])?.map(atlas_domain::EnvironmentId),
        name: row[4].clone(),
        cidr: empty_to_none(&row[5]),
        gateway_ip: empty_to_none(&row[6]),
        address_family: parse_address_family(&row[7])?,
        created_at: parse_datetime(&row[8])?,
        updated_at: parse_datetime(&row[9])?,
    })
}

pub(super) fn decode_host_net_assoc(row: &[String]) -> StorageResult<HostNetAssoc> {
    Ok(HostNetAssoc {
        assoc_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        host_id: parse_uuid(&row[2])?,
        network_segment_id: parse_uuid(&row[3])?,
        ip_addr: row[4].clone(),
        iface_name: empty_to_none(&row[5]),
        validity: atlas_domain::ValidityWindow {
            valid_from: parse_datetime(&row[6])?,
            valid_to: empty_to_none(&row[7])
                .map(|value| parse_datetime(&value))
                .transpose()?,
        },
        created_at: parse_datetime(&row[8])?,
        updated_at: parse_datetime(&row[9])?,
    })
}

pub(super) fn decode_host_runtime_state(row: &[String]) -> StorageResult<HostRuntimeState> {
    Ok(HostRuntimeState {
        host_id: parse_uuid(&row[0])?,
        observed_at: atlas_domain::ObservedAt(parse_datetime(&row[1])?),
        boot_id: empty_to_none(&row[2]),
        uptime_seconds: parse_optional_i64(&row[3])?,
        loadavg_1m: parse_optional_f64(&row[4])?,
        loadavg_5m: parse_optional_f64(&row[5])?,
        loadavg_15m: parse_optional_f64(&row[6])?,
        cpu_usage_pct: parse_optional_f64(&row[7])?,
        memory_used_bytes: parse_optional_i64(&row[8])?,
        memory_available_bytes: parse_optional_i64(&row[9])?,
        process_count: parse_optional_i64(&row[10])?,
        container_count: parse_optional_i64(&row[11])?,
        agent_health: parse_agent_health(&row[12])?,
    })
}

pub(super) fn decode_process_runtime_state(row: &[String]) -> StorageResult<ProcessRuntimeState> {
    Ok(ProcessRuntimeState {
        process_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        host_id: parse_uuid(&row[2])?,
        container_id: parse_optional_uuid(&row[3])?,
        external_ref: empty_to_none(&row[4]),
        pid: row[5]
            .parse::<i32>()
            .map_err(|err| operation_failed(err.to_string()))?,
        executable: row[6].clone(),
        command_line: empty_to_none(&row[7]),
        process_state: empty_to_none(&row[8]),
        memory_rss_kib: parse_optional_i64(&row[9])?,
        started_at: parse_datetime(&row[10])?,
        observed_at: atlas_domain::ObservedAt(parse_datetime(&row[11])?),
    })
}

pub(super) fn decode_service(row: &[String]) -> StorageResult<ServiceEntity> {
    Ok(ServiceEntity {
        service_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        business_id: parse_optional_uuid(&row[2])?,
        system_id: parse_optional_uuid(&row[3])?,
        subsystem_id: parse_optional_uuid(&row[4])?,
        name: row[5].clone(),
        namespace: empty_to_none(&row[6]),
        service_type: parse_service_type(&row[7])?,
        boundary: parse_service_boundary(&row[8])?,
        provider: empty_to_none(&row[9]),
        external_ref: empty_to_none(&row[10]),
        created_at: parse_datetime(&row[11])?,
        updated_at: parse_datetime(&row[12])?,
    })
}

pub(super) fn decode_service_instance(row: &[String]) -> StorageResult<ServiceInstance> {
    Ok(ServiceInstance {
        instance_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        service_id: parse_uuid(&row[2])?,
        workload_id: parse_optional_uuid(&row[3])?,
        started_at: parse_datetime(&row[4])?,
        ended_at: empty_to_none(&row[5])
            .map(|value| parse_datetime(&value))
            .transpose()?,
        last_seen_at: parse_datetime(&row[6])?,
    })
}

pub(super) fn decode_runtime_binding(row: &[String]) -> StorageResult<RuntimeBinding> {
    Ok(RuntimeBinding {
        binding_id: parse_uuid(&row[0])?,
        instance_id: parse_uuid(&row[1])?,
        object_type: parse_runtime_object_type(&row[2])?,
        object_id: parse_uuid(&row[3])?,
        scope: parse_binding_scope(&row[4])?,
        confidence: parse_confidence(&row[5])?,
        source: row[6].clone(),
        validity: atlas_domain::ValidityWindow {
            valid_from: parse_datetime(&row[7])?,
            valid_to: empty_to_none(&row[8])
                .map(|value| parse_datetime(&value))
                .transpose()?,
        },
        created_at: parse_datetime(&row[9])?,
        updated_at: parse_datetime(&row[10])?,
    })
}

pub(super) fn decode_subject(row: &[String]) -> StorageResult<Subject> {
    Ok(Subject {
        subject_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        subject_type: parse_subject_type(&row[2])?,
        display_name: row[3].clone(),
        external_ref: empty_to_none(&row[4]),
        email: empty_to_none(&row[5]),
        is_active: row[6] == "true",
        created_at: parse_datetime(&row[7])?,
        updated_at: parse_datetime(&row[8])?,
    })
}

pub(super) fn decode_responsibility_assignment(
    row: &[String],
) -> StorageResult<ResponsibilityAssignment> {
    Ok(ResponsibilityAssignment {
        assignment_id: parse_uuid(&row[0])?,
        tenant_id: TenantId(parse_uuid(&row[1])?),
        subject_id: parse_uuid(&row[2])?,
        target_kind: parse_object_kind(&row[3])?,
        target_id: parse_uuid(&row[4])?,
        role: parse_responsibility_role(&row[5])?,
        source: row[6].clone(),
        validity: atlas_domain::ValidityWindow {
            valid_from: parse_datetime(&row[7])?,
            valid_to: empty_to_none(&row[8])
                .map(|value| parse_datetime(&value))
                .transpose()?,
        },
        created_at: parse_datetime(&row[9])?,
        updated_at: parse_datetime(&row[10])?,
    })
}

pub(super) fn decode_ingest_job(row: &[String]) -> StorageResult<IngestJobEntry> {
    Ok(IngestJobEntry {
        ingest_id: row[0].clone(),
        tenant_id: TenantId(parse_uuid(&row[1])?),
        source_kind: row[2].clone(),
        source_name: row[3].clone(),
        received_at: parse_datetime(&row[4])?,
        status: row[5].clone(),
        payload_ref: empty_to_none(&row[6]),
        error: empty_to_none(&row[7]),
    })
}

pub(super) fn parse_uuid(value: &str) -> StorageResult<Uuid> {
    Uuid::parse_str(value).source_raw_err(StorageReason::DecodeFailed, "parse uuid")
}

pub(super) fn parse_optional_uuid(value: &str) -> StorageResult<Option<Uuid>> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_uuid(value).map(Some)
    }
}

pub(super) fn parse_datetime(value: &str) -> StorageResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .source_raw_err(StorageReason::DecodeFailed, "parse rfc3339 datetime")
}

pub(super) fn parse_optional_i64(value: &str) -> StorageResult<Option<i64>> {
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse::<i64>()
            .map(Some)
            .map_err(|err| operation_failed(err.to_string()))
    }
}

pub(super) fn parse_optional_f64(value: &str) -> StorageResult<Option<f64>> {
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse::<f64>()
            .map(Some)
            .map_err(|err| operation_failed(err.to_string()))
    }
}

pub(super) fn empty_to_none(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub(super) fn parse_network_domain_kind(
    value: &str,
) -> StorageResult<atlas_domain::NetworkDomainKind> {
    match value {
        "Lan" => Ok(atlas_domain::NetworkDomainKind::Lan),
        "Wan" => Ok(atlas_domain::NetworkDomainKind::Wan),
        "Vpc" => Ok(atlas_domain::NetworkDomainKind::Vpc),
        "Vnet" => Ok(atlas_domain::NetworkDomainKind::Vnet),
        "Vlan" => Ok(atlas_domain::NetworkDomainKind::Vlan),
        "Overlay" => Ok(atlas_domain::NetworkDomainKind::Overlay),
        "Unknown" => Ok(atlas_domain::NetworkDomainKind::Unknown),
        other => Err(decode_failed(format!(
            "unsupported network domain kind: {other}"
        ))),
    }
}

pub(super) fn parse_address_family(value: &str) -> StorageResult<atlas_domain::AddressFamily> {
    match value {
        "Ipv4" => Ok(atlas_domain::AddressFamily::Ipv4),
        "Ipv6" => Ok(atlas_domain::AddressFamily::Ipv6),
        other => Err(decode_failed(format!(
            "unsupported address family: {other}"
        ))),
    }
}

pub(super) fn parse_subject_type(value: &str) -> StorageResult<atlas_domain::SubjectType> {
    match value {
        "User" => Ok(atlas_domain::SubjectType::User),
        "Team" => Ok(atlas_domain::SubjectType::Team),
        "Rotation" => Ok(atlas_domain::SubjectType::Rotation),
        "ServiceAccount" => Ok(atlas_domain::SubjectType::ServiceAccount),
        other => Err(decode_failed(format!("unsupported subject type: {other}"))),
    }
}

pub(super) fn parse_service_type(value: &str) -> StorageResult<atlas_domain::ServiceType> {
    match value {
        "Application" => Ok(atlas_domain::ServiceType::Application),
        "Data" => Ok(atlas_domain::ServiceType::Data),
        "Platform" => Ok(atlas_domain::ServiceType::Platform),
        "Shared" => Ok(atlas_domain::ServiceType::Shared),
        other => Err(operation_failed(format!(
            "unsupported service type: {other}"
        ))),
    }
}

pub(super) fn parse_service_boundary(value: &str) -> StorageResult<atlas_domain::ServiceBoundary> {
    match value {
        "Internal" => Ok(atlas_domain::ServiceBoundary::Internal),
        "External" => Ok(atlas_domain::ServiceBoundary::External),
        "Partner" => Ok(atlas_domain::ServiceBoundary::Partner),
        "Saas" => Ok(atlas_domain::ServiceBoundary::Saas),
        other => Err(operation_failed(format!(
            "unsupported service boundary: {other}"
        ))),
    }
}

pub(super) fn parse_agent_health(value: &str) -> StorageResult<atlas_domain::AgentHealth> {
    match value {
        "Healthy" => Ok(atlas_domain::AgentHealth::Healthy),
        "Degraded" => Ok(atlas_domain::AgentHealth::Degraded),
        "Protect" => Ok(atlas_domain::AgentHealth::Protect),
        "Unavailable" => Ok(atlas_domain::AgentHealth::Unavailable),
        other => Err(operation_failed(format!(
            "unsupported agent health: {other}"
        ))),
    }
}

pub(super) fn parse_runtime_object_type(value: &str) -> StorageResult<RuntimeObjectType> {
    match value {
        "Process" => Ok(RuntimeObjectType::Process),
        "Container" => Ok(RuntimeObjectType::Container),
        "Pod" => Ok(RuntimeObjectType::Pod),
        other => Err(operation_failed(format!(
            "unsupported runtime object type: {other}"
        ))),
    }
}

pub(super) fn parse_binding_scope(value: &str) -> StorageResult<atlas_domain::BindingScope> {
    match value {
        "Declared" => Ok(atlas_domain::BindingScope::Declared),
        "Observed" => Ok(atlas_domain::BindingScope::Observed),
        "Inferred" => Ok(atlas_domain::BindingScope::Inferred),
        other => Err(operation_failed(format!(
            "unsupported binding scope: {other}"
        ))),
    }
}

pub(super) fn parse_confidence(value: &str) -> StorageResult<atlas_domain::Confidence> {
    match value {
        "Low" => Ok(atlas_domain::Confidence::Low),
        "Medium" => Ok(atlas_domain::Confidence::Medium),
        "High" => Ok(atlas_domain::Confidence::High),
        other => Err(operation_failed(format!("unsupported confidence: {other}"))),
    }
}

pub(super) fn parse_object_kind(value: &str) -> StorageResult<ObjectKind> {
    match value {
        "Host" => Ok(ObjectKind::Host),
        "NetworkSegment" => Ok(ObjectKind::NetworkSegment),
        "Subject" => Ok(ObjectKind::Subject),
        other => Err(decode_failed(format!("unsupported object kind: {other}"))),
    }
}

pub(super) fn parse_responsibility_role(
    value: &str,
) -> StorageResult<atlas_domain::ResponsibilityRole> {
    match value {
        "Owner" => Ok(atlas_domain::ResponsibilityRole::Owner),
        "Maintainer" => Ok(atlas_domain::ResponsibilityRole::Maintainer),
        "Oncall" => Ok(atlas_domain::ResponsibilityRole::Oncall),
        "Security" => Ok(atlas_domain::ResponsibilityRole::Security),
        other => Err(decode_failed(format!(
            "unsupported responsibility role: {other}"
        ))),
    }
}

pub(super) fn row_to_strings(row: tokio_postgres::Row) -> StorageResult<Vec<String>> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let type_name = column.type_().name();
            let value = match type_name {
                "uuid" => row
                    .try_get::<usize, Option<Uuid>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "text" | "varchar" => row
                    .try_get::<usize, Option<String>>(index)
                    .map(|value| value.unwrap_or_default()),
                "timestamptz" => row
                    .try_get::<usize, Option<DateTime<Utc>>>(index)
                    .map(|value| value.map(|item| item.to_rfc3339()).unwrap_or_default()),
                "bool" => row
                    .try_get::<usize, Option<bool>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "int4" => row
                    .try_get::<usize, Option<i32>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "int8" => row
                    .try_get::<usize, Option<i64>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                "float8" => row
                    .try_get::<usize, Option<f64>>(index)
                    .map(|value| value.map(|item| item.to_string()).unwrap_or_default()),
                other => {
                    return Err(operation_failed(format!(
                        "unsupported postgres column type: {other}"
                    )));
                }
            }
            .map_err(|err| operation_failed(err.to_string()))?;
            Ok(value)
        })
        .collect()
}
