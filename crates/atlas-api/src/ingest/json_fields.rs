use std::collections::BTreeMap;

use atlas_domain::{
    IngestEnvelope, NetworkSegmentCandidate, ServiceBoundary, ServiceType, SubjectCandidate,
    SubjectType,
};
use chrono::Utc;
use serde_json::Value;

use super::{ExtractedNetworkSegments, extract_machine_id_prefix};
use crate::error::{
    ApiResult, invalid_field_type, invalid_field_value, missing_field, missing_payload,
    payload_must_be_object,
};

pub(super) fn object_payload(
    envelope: &IngestEnvelope,
) -> ApiResult<&serde_json::Map<String, Value>> {
    envelope
        .payload_inline
        .as_ref()
        .ok_or(missing_payload())?
        .as_object()
        .ok_or(payload_must_be_object())
}

pub(super) fn optional_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<Vec<&'a Value>>> {
    match object.get(field) {
        Some(Value::Array(values)) => Ok(Some(values.iter().collect())),
        Some(_) => Err(invalid_field_type(field)),
        None => Ok(None),
    }
}

pub(super) fn first_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    preferred: &'static str,
    fallback: &'static str,
) -> ApiResult<Vec<&'a Value>> {
    if let Some(values) = optional_array(object, preferred)? {
        return Ok(values);
    }

    Ok(optional_array(object, fallback)?.unwrap_or_default())
}

pub(super) fn required_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<String> {
    optional_string(object, field)?.ok_or(missing_field(field))
}

pub(super) fn required_string_any(
    object: &serde_json::Map<String, Value>,
    fields: &'static [&'static str],
) -> ApiResult<String> {
    for field in fields {
        if let Some(value) = optional_string(object, field)? {
            return Ok(value);
        }
    }

    Err(missing_field(fields[0]))
}

pub(super) fn optional_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<String>> {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        Some(Value::String(_)) => Ok(None),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

pub(super) fn optional_string_lossy(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Option<String> {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        _ => None,
    }
}

pub(super) fn metric_value_i64(object: &serde_json::Map<String, Value>) -> Option<i64> {
    match object.get("value") {
        Some(Value::Number(value)) => value.as_i64(),
        _ => None,
    }
}

pub(super) fn metric_value_f64(object: &serde_json::Map<String, Value>) -> Option<f64> {
    match object.get("value") {
        Some(Value::Number(value)) => value.as_f64(),
        _ => None,
    }
}

pub(super) fn metric_value_string(object: &serde_json::Map<String, Value>) -> Option<String> {
    match object.get("value") {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        _ => None,
    }
}

pub(super) struct ProcessRefParts {
    pub(super) machine_id: String,
    pub(super) pid: i32,
}

pub(super) fn extract_process_ref_parts(value: &str) -> Option<ProcessRefParts> {
    let machine_id = extract_machine_id_prefix(value)?;
    let pid_part = value.split(":pid:").nth(1)?;
    let pid_raw = pid_part.split(':').next()?;
    let pid = pid_raw.parse::<i32>().ok()?;

    Some(ProcessRefParts { machine_id, pid })
}

pub(super) fn extract_network_segments_from_interfaces(
    envelope: &IngestEnvelope,
    interface_rows: Vec<&Value>,
) -> ApiResult<ExtractedNetworkSegments> {
    let payload = object_payload(envelope)?;
    let host = payload.get("host").and_then(Value::as_object);
    let host_from_hosts = payload
        .get("hosts")
        .and_then(Value::as_array)
        .and_then(|hosts| hosts.first())
        .and_then(Value::as_object);
    let host_name = host
        .and_then(|host| optional_string_lossy(host, "host_name"))
        .or_else(|| host.and_then(|host| optional_string_lossy(host, "hostname")))
        .or_else(|| host_from_hosts.and_then(|host| optional_string_lossy(host, "host_name")))
        .or_else(|| host_from_hosts.and_then(|host| optional_string_lossy(host, "hostname")));
    let machine_id = host
        .and_then(|host| optional_string_lossy(host, "machine_id"))
        .or_else(|| host_from_hosts.and_then(|host| optional_string_lossy(host, "machine_id")));

    let mut candidates = Vec::new();
    for row in interface_rows {
        let row = row.as_object().ok_or(invalid_field_type("interfaces[]"))?;
        let iface_name = optional_string(row, "name")?;
        let row_host_ref = optional_string(row, "host_ref")?;
        let row_machine_id = optional_string(row, "machine_id")?;
        let addresses = optional_array(row, "addresses")?.unwrap_or_default();
        for address in addresses {
            let address = address
                .as_object()
                .ok_or(invalid_field_type("interfaces[].addresses[]"))?;
            let ip_addr = optional_string(address, "ip")?;
            let prefix = optional_u64(address, "prefix")?;
            let cidr = optional_string(address, "cidr")?.or_else(|| {
                ip_addr
                    .as_ref()
                    .zip(prefix)
                    .map(|(ip, prefix)| network_cidr(ip, prefix))
            });
            candidates.push(NetworkSegmentCandidate {
                tenant_id: envelope.tenant_id,
                environment_id: envelope.environment_id,
                source_kind: envelope.source_kind,
                segment_name: cidr.clone().or_else(|| ip_addr.clone()),
                cidr,
                gateway_ip: optional_string(address, "gateway")?,
                ip_addr,
                host_name: row_host_ref.clone().or_else(|| host_name.clone()),
                machine_id: row_machine_id.clone().or_else(|| machine_id.clone()),
                iface_name: iface_name.clone(),
            });
        }
    }

    Ok(ExtractedNetworkSegments { candidates })
}

pub(super) fn network_cidr(ip: &str, prefix: u64) -> String {
    if prefix <= 32 {
        if let Ok(ipv4) = ip.parse::<std::net::Ipv4Addr>() {
            let ip_num = u32::from(ipv4);
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            return format!("{}/{}", std::net::Ipv4Addr::from(ip_num & mask), prefix);
        }
    }

    format!("{ip}/{prefix}")
}

pub(super) fn subject_candidate_from_row(
    envelope: &IngestEnvelope,
    row: &Value,
    field: &'static str,
    default_type: SubjectType,
) -> ApiResult<SubjectCandidate> {
    let row = row.as_object().ok_or(invalid_field_type(field))?;
    let display_name = required_string_any(row, &["display_name", "name"])?;
    let subject_type = match optional_subject_type(row, "subject_type")? {
        Some(subject_type) => Some(subject_type),
        None => optional_subject_type(row, "group_type")?,
    }
    .unwrap_or(default_type);

    Ok(SubjectCandidate {
        tenant_id: envelope.tenant_id,
        source_kind: envelope.source_kind,
        subject_type,
        external_ref: optional_string(row, "external_ref")?
            .or_else(|| optional_string_lossy(row, "external_id")),
        display_name,
        email: optional_string(row, "email")?,
        is_active: optional_bool(row, "is_active")?.unwrap_or(true),
    })
}

pub(super) fn optional_u64(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<u64>> {
    match object.get(field) {
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid_field_type(field)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

pub(super) fn optional_timestamp(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<atlas_domain::ObservedAt>> {
    match object.get(field) {
        Some(Value::String(value)) => {
            let parsed = chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| invalid_field_value(field, value.clone()))?
                .with_timezone(&Utc);
            Ok(Some(atlas_domain::ObservedAt(parsed)))
        }
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

pub(super) fn optional_bool(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<bool>> {
    match object.get(field) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(invalid_field_type(field)),
    }
}

pub(super) fn optional_service_type(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<ServiceType>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "application" => Ok(ServiceType::Application),
            "data" => Ok(ServiceType::Data),
            "platform" => Ok(ServiceType::Platform),
            "shared" => Ok(ServiceType::Shared),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

pub(super) fn optional_service_boundary(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<ServiceBoundary>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "internal" => Ok(ServiceBoundary::Internal),
            "external" => Ok(ServiceBoundary::External),
            "partner" => Ok(ServiceBoundary::Partner),
            "saas" => Ok(ServiceBoundary::Saas),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

pub(super) fn optional_subject_type(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<SubjectType>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "user" => Ok(SubjectType::User),
            "team" => Ok(SubjectType::Team),
            "rotation" => Ok(SubjectType::Rotation),
            "service_account" => Ok(SubjectType::ServiceAccount),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

pub(super) fn optional_responsibility_role(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<atlas_domain::ResponsibilityRole>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "owner" => Ok(atlas_domain::ResponsibilityRole::Owner),
            "maintainer" => Ok(atlas_domain::ResponsibilityRole::Maintainer),
            "oncall" => Ok(atlas_domain::ResponsibilityRole::Oncall),
            "security" => Ok(atlas_domain::ResponsibilityRole::Security),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

pub(super) fn optional_object_kind(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> ApiResult<Option<atlas_domain::ObjectKind>> {
    optional_string(object, field)?
        .map(|value| match value.as_str() {
            "host" => Ok(atlas_domain::ObjectKind::Host),
            "network_segment" => Ok(atlas_domain::ObjectKind::NetworkSegment),
            "subject" => Ok(atlas_domain::ObjectKind::Subject),
            _ => Err(invalid_field_value(field, value)),
        })
        .transpose()
}

#[allow(dead_code)]
pub(super) fn empty_metadata() -> BTreeMap<String, String> {
    BTreeMap::new()
}
