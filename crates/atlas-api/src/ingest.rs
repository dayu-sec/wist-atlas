use atlas_domain::{
    BusinessCatalogCandidate, HostCandidate, HostTelemetryCandidate, IngestEnvelope,
    NetworkSegmentCandidate, ProcessRuntimeCandidate, ProcessTelemetryCandidate,
    ResponsibilityAssignmentCandidate, SubjectType,
};
use serde_json::Value;

use crate::error::{ApiResult, invalid_field_type, invalid_field_value, missing_field};

mod record;
pub use record::{
    InMemoryIngestJobRecorder, IngestJobRecord, IngestJobRecorder, IngestJobStatus, IngestService,
};

mod extracted;
pub use extracted::*;

pub fn extract_business_catalog_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedBusinessCatalog> {
    let payload = object_payload(envelope)?;
    let rows = first_array(payload, "business_catalog", "items")?;

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(invalid_field_type("business_catalog[]"))?;
        let business_name = required_string(row, "business_name")?;
        let service_name = optional_string(row, "service_name")?;

        candidates.push(BusinessCatalogCandidate {
            tenant_id: envelope.tenant_id,
            source_kind: envelope.source_kind,
            external_ref: optional_string(row, "external_ref")?,
            business_name,
            system_name: optional_string(row, "system_name")?,
            subsystem_name: optional_string(row, "subsystem_name")?,
            service_name,
            service_type: optional_service_type(row, "service_type")?,
            boundary: optional_service_boundary(row, "boundary")?,
        });
    }

    Ok(ExtractedBusinessCatalog { candidates })
}

pub fn extract_host_candidates(envelope: &IngestEnvelope) -> ApiResult<ExtractedHosts> {
    let payload = object_payload(envelope)?;
    let mut candidates = Vec::new();
    if looks_like_edge_host_fact(payload)? {
        candidates.push(host_candidate_from_row(envelope, payload)?);
    }

    let rows = if payload.contains_key("hosts") || payload.contains_key("items") {
        first_array(payload, "hosts", "items")?
    } else {
        Vec::new()
    };

    candidates.reserve(rows.len());
    for row in rows {
        let row = row.as_object().ok_or(invalid_field_type("hosts[]"))?;
        candidates.push(host_candidate_from_row(envelope, row)?);
    }

    Ok(ExtractedHosts { candidates })
}

fn looks_like_edge_host_fact(object: &serde_json::Map<String, Value>) -> ApiResult<bool> {
    let target_kind = optional_string(object, "target_kind")?;
    if let Some(target_kind) = target_kind.as_deref() {
        return Ok(target_kind == "host");
    }

    Ok(optional_string(object, "host_name")?.is_some()
        || optional_string(object, "hostname")?.is_some())
}

fn host_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &serde_json::Map<String, Value>,
) -> ApiResult<HostCandidate> {
    let host_name = required_string_any(row, &["host_name", "hostname"])?;
    let os = row.get("os").and_then(Value::as_object);

    Ok(HostCandidate {
        tenant_id: envelope.tenant_id,
        environment_id: envelope.environment_id,
        source_kind: envelope.source_kind,
        external_ref: optional_string(row, "external_ref")?,
        host_name,
        machine_id: optional_string(row, "machine_id")?,
        os_name: optional_string(row, "os_name")?
            .or_else(|| os.and_then(|os| optional_string_lossy(os, "name"))),
        os_version: optional_string(row, "os_version")?
            .or_else(|| os.and_then(|os| optional_string_lossy(os, "version"))),
    })
}

pub fn extract_network_segment_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedNetworkSegments> {
    let payload = object_payload(envelope)?;
    if let Some(interface_rows) = optional_array(payload, "interfaces")? {
        return extract_network_segments_from_interfaces(envelope, interface_rows);
    }

    if looks_like_edge_network_fact(payload)? {
        return Ok(ExtractedNetworkSegments {
            candidates: vec![network_candidate_from_row(envelope, payload)?],
        });
    }

    let rows = if payload.contains_key("network_segments") {
        first_array(payload, "network_segments", "items")?
    } else {
        first_array(payload, "ip_addresses", "ips")?
    };

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(invalid_field_type("network_segments[]"))?;

        let cidr = optional_string(row, "cidr")?;
        let ip_addr = match optional_string(row, "ip_addr")? {
            Some(value) => Some(value),
            None => optional_string(row, "ip")?,
        };

        if cidr.is_none() && ip_addr.is_none() {
            return Err(missing_field("cidr/ip_addr"));
        }

        let segment_name = match optional_string(row, "segment_name")? {
            Some(value) => Some(value),
            None => optional_string(row, "name")?,
        };

        candidates.push(NetworkSegmentCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            segment_name,
            cidr,
            gateway_ip: optional_string(row, "gateway_ip")?,
            ip_addr,
            host_name: optional_string(row, "host_name")?,
            machine_id: optional_string(row, "machine_id")?,
            iface_name: optional_string(row, "iface_name")?,
        });
    }

    Ok(ExtractedNetworkSegments { candidates })
}

pub fn extract_process_runtime_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedProcessRuntimes> {
    let payload = object_payload(envelope)?;
    if !looks_like_edge_process_fact(payload)? {
        return Ok(ExtractedProcessRuntimes {
            candidates: Vec::new(),
        });
    }

    Ok(ExtractedProcessRuntimes {
        candidates: vec![process_candidate_from_row(envelope, payload)?],
    })
}

fn looks_like_edge_network_fact(object: &serde_json::Map<String, Value>) -> ApiResult<bool> {
    let target_kind = optional_string(object, "target_kind")?;
    if let Some(target_kind) = target_kind.as_deref() {
        if matches!(target_kind, "host" | "process" | "container") {
            return Ok(false);
        }
        if matches!(
            target_kind,
            "network_interface" | "host_network" | "network" | "ip" | "ip_address"
        ) {
            return Ok(true);
        }
    }

    Ok(optional_string(object, "ip")?.is_some()
        || optional_string(object, "ip_addr")?.is_some()
        || optional_string(object, "cidr")?.is_some())
}

fn network_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &serde_json::Map<String, Value>,
) -> ApiResult<NetworkSegmentCandidate> {
    let ip_addr = match optional_string(row, "ip_addr")? {
        Some(value) => Some(value),
        None => optional_string(row, "ip")?,
    };
    let prefix = optional_u64(row, "prefix")?;
    let cidr = optional_string(row, "cidr")?.or_else(|| {
        ip_addr
            .as_ref()
            .zip(prefix)
            .map(|(ip, prefix)| network_cidr(ip, prefix))
    });

    if cidr.is_none() && ip_addr.is_none() {
        return Err(missing_field("cidr/ip_addr"));
    }

    let segment_name = match optional_string(row, "segment_name")? {
        Some(value) => Some(value),
        None => optional_string(row, "name")?,
    }
    .or_else(|| cidr.clone())
    .or_else(|| ip_addr.clone());

    Ok(NetworkSegmentCandidate {
        tenant_id: envelope.tenant_id,
        environment_id: envelope.environment_id,
        source_kind: envelope.source_kind,
        segment_name,
        cidr,
        gateway_ip: optional_string(row, "gateway_ip")?
            .or_else(|| optional_string_lossy(row, "gateway")),
        ip_addr,
        host_name: optional_string(row, "host_name")?
            .or_else(|| optional_string_lossy(row, "hostname")),
        machine_id: optional_string(row, "machine_id")?,
        iface_name: optional_string(row, "iface_name")?
            .or_else(|| optional_string_lossy(row, "interface_name"))
            .or_else(|| optional_string_lossy(row, "name")),
    })
}

fn looks_like_edge_process_fact(object: &serde_json::Map<String, Value>) -> ApiResult<bool> {
    let target_kind = optional_string(object, "target_kind")?;
    if let Some(target_kind) = target_kind.as_deref() {
        return Ok(target_kind == "process");
    }

    Ok(optional_string(object, "pid")?.is_some()
        && optional_string(object, "process_key")?.is_some())
}

fn process_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &serde_json::Map<String, Value>,
) -> ApiResult<ProcessRuntimeCandidate> {
    let pid_raw = required_string(row, "pid")?;
    let pid = pid_raw
        .parse::<i32>()
        .map_err(|_| invalid_field_value("pid", pid_raw.clone()))?;
    let executable = optional_string(row, "executable")?
        .or_else(|| optional_string_lossy(row, "executable_name"))
        .or_else(|| optional_string_lossy(row, "process_key"))
        .ok_or_else(|| missing_field("executable_name"))?;
    let observed_at = optional_timestamp(row, "observed_at")?;
    let host_name = optional_string(row, "host_name")?
        .or_else(|| optional_string_lossy(row, "hostname"))
        .or_else(|| process_host_locator(row).and_then(|locator| locator.1));
    let machine_id = optional_string(row, "machine_id")?
        .or_else(|| process_host_locator(row).and_then(|locator| Some(locator.0)));

    Ok(ProcessRuntimeCandidate {
        tenant_id: envelope.tenant_id,
        environment_id: envelope.environment_id,
        source_kind: envelope.source_kind,
        host_name,
        machine_id,
        pid,
        executable,
        command_line: optional_string(row, "command_line")?,
        identity: optional_string(row, "identity")?,
        service_ref: optional_string(row, "service_ref")?,
        instance_key: optional_string(row, "instance_key")?,
        observed_at,
    })
}

fn process_host_locator(row: &serde_json::Map<String, Value>) -> Option<(String, Option<String>)> {
    for key in ["machine_id", "target_ref", "process_key", "external_ref"] {
        let Some(value) = optional_string_lossy(row, key) else {
            continue;
        };
        if let Some(machine_id) = extract_machine_id_prefix(&value) {
            let host_name = machine_id.strip_prefix("hostname:").map(str::to_string);
            return Some((machine_id, host_name));
        }
    }

    None
}

fn extract_machine_id_prefix(value: &str) -> Option<String> {
    if let Some(prefix) = value.strip_suffix(":host") {
        if prefix.starts_with("hostname:") && prefix.len() > "hostname:".len() {
            return Some(prefix.to_string());
        }
    }

    if let Some(prefix) = value.strip_suffix(":process") {
        if prefix.starts_with("hostname:") && prefix.len() > "hostname:".len() {
            return Some(prefix.to_string());
        }
    }

    let prefix = value.split(":pid:").next()?;
    if prefix.starts_with("hostname:") && prefix.len() > "hostname:".len() {
        return Some(prefix.to_string());
    }
    None
}

pub fn extract_subject_candidates(envelope: &IngestEnvelope) -> ApiResult<ExtractedSubjects> {
    let payload = object_payload(envelope)?;
    let mut candidates = Vec::new();

    for row in first_array(payload, "subjects", "items")? {
        candidates.push(subject_candidate_from_row(
            envelope,
            row,
            "subjects[]",
            SubjectType::User,
        )?);
    }

    if let Some(rows) = optional_array(payload, "users")? {
        for row in rows {
            candidates.push(subject_candidate_from_row(
                envelope,
                row,
                "users[]",
                SubjectType::User,
            )?);
        }
    }

    if let Some(rows) = optional_array(payload, "groups")? {
        for row in rows {
            candidates.push(subject_candidate_from_row(
                envelope,
                row,
                "groups[]",
                SubjectType::Team,
            )?);
        }
    }

    Ok(ExtractedSubjects { candidates })
}

pub fn extract_host_telemetry_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedHostTelemetry> {
    let payload = object_payload(envelope)?;
    let metric_name = optional_string(payload, "metric_name")?;
    let collection_kind = optional_string(payload, "collection_kind")?;
    let target_ref = optional_string(payload, "resource_ref")?
        .or_else(|| optional_string_lossy(payload, "target_ref"));

    if collection_kind.as_deref() != Some("host_metrics") || metric_name.is_none() {
        return Ok(ExtractedHostTelemetry {
            candidates: Vec::new(),
        });
    }

    let observed_at = envelope
        .observed_at
        .ok_or_else(|| missing_field("collect.observed_at"))?;
    let metric_name = metric_name.expect("checked above");
    let locator = target_ref.as_deref().and_then(extract_machine_id_prefix);
    let machine_id = optional_string(payload, "machine_id")?.or(locator.clone());
    let host_name = optional_string(payload, "host_name")?
        .or_else(|| optional_string_lossy(payload, "hostname"))
        .or_else(|| locator.and_then(|item| item.strip_prefix("hostname:").map(str::to_string)));

    Ok(ExtractedHostTelemetry {
        candidates: vec![HostTelemetryCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            host_name,
            machine_id,
            observed_at,
            metric_name,
            value_i64: metric_value_i64(payload),
            value_f64: metric_value_f64(payload),
        }],
    })
}

pub fn extract_process_telemetry_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedProcessTelemetry> {
    let payload = object_payload(envelope)?;
    let metric_name = optional_string(payload, "metric_name")?;
    let collection_kind = optional_string(payload, "collection_kind")?;
    let process_ref = optional_string(payload, "resource_ref")?
        .or_else(|| optional_string_lossy(payload, "target_ref"));

    if collection_kind.as_deref() != Some("process_metrics")
        || metric_name.is_none()
        || process_ref.is_none()
    {
        return Ok(ExtractedProcessTelemetry {
            candidates: Vec::new(),
        });
    }

    let observed_at = envelope
        .observed_at
        .ok_or_else(|| missing_field("collect.observed_at"))?;
    let process_ref = process_ref.expect("checked above");
    let process_locator = extract_process_ref_parts(&process_ref)
        .ok_or_else(|| invalid_field_value("payload.resource_ref", process_ref.clone()))?;

    Ok(ExtractedProcessTelemetry {
        candidates: vec![ProcessTelemetryCandidate {
            tenant_id: envelope.tenant_id,
            environment_id: envelope.environment_id,
            source_kind: envelope.source_kind,
            host_name: process_locator
                .machine_id
                .strip_prefix("hostname:")
                .map(str::to_string),
            machine_id: Some(process_locator.machine_id),
            process_ref,
            pid: process_locator.pid,
            observed_at,
            metric_name: metric_name.expect("checked above"),
            value_i64: metric_value_i64(payload),
            value_string: metric_value_string(payload),
        }],
    })
}

pub fn extract_responsibility_assignment_candidates(
    envelope: &IngestEnvelope,
) -> ApiResult<ExtractedResponsibilityAssignments> {
    let payload = object_payload(envelope)?;
    let rows = first_array(payload, "responsibility_assignments", "items")?;

    let mut candidates = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or(invalid_field_type("responsibility_assignments[]"))?;

        candidates.push(ResponsibilityAssignmentCandidate {
            tenant_id: envelope.tenant_id,
            source_kind: envelope.source_kind,
            subject_display_name: optional_string(row, "subject_display_name")?,
            subject_external_ref: optional_string(row, "subject_external_ref")?,
            subject_email: optional_string(row, "subject_email")?,
            target_kind: optional_object_kind(row, "target_kind")?
                .unwrap_or(atlas_domain::ObjectKind::Host),
            target_external_ref: optional_string(row, "target_external_ref")?,
            role: optional_responsibility_role(row, "role")?
                .unwrap_or(atlas_domain::ResponsibilityRole::Owner),
            validity: atlas_domain::ValidityWindow {
                valid_from: envelope.received_at,
                valid_to: None,
            },
        });
    }

    Ok(ExtractedResponsibilityAssignments { candidates })
}

mod json_fields;
use json_fields::*;

#[cfg(test)]
mod tests;
