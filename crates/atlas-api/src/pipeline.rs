use atlas_domain::{
    Confidence, HostCandidate, HostInventory, HostNetAssoc, IdentifierMatch, NetworkDomain,
    NetworkDomainKind, NetworkSegment, NetworkSegmentCandidate, ObjectKind, ResolutionResult,
    ResolutionStatus, TenantId, ValidityWindow,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct InMemoryCatalog {
    pub hosts: Vec<HostInventory>,
    pub network_domains: Vec<NetworkDomain>,
    pub network_segments: Vec<NetworkSegment>,
    pub host_net_assocs: Vec<HostNetAssoc>,
}

#[derive(Debug, Clone)]
pub struct HostResolution {
    pub host: HostInventory,
    pub resolution: ResolutionResult,
}

#[derive(Debug, Clone)]
pub struct NetworkResolution {
    pub segment: NetworkSegment,
    pub resolution: ResolutionResult,
}

#[derive(Debug, Clone)]
pub struct HostNetworkMaterialization {
    pub host: HostInventory,
    pub segment: NetworkSegment,
    pub assoc: Option<HostNetAssoc>,
    pub host_resolution: ResolutionResult,
    pub segment_resolution: ResolutionResult,
}

pub fn resolve_host_candidate(
    catalog: &InMemoryCatalog,
    candidate: &HostCandidate,
    now: DateTime<Utc>,
) -> HostResolution {
    if let Some(host) = catalog.hosts.iter().find(|host| {
        candidate
            .machine_id
            .as_ref()
            .zip(host.machine_id.as_ref())
            .is_some_and(|(candidate_id, host_id)| candidate_id == host_id)
    }) {
        return HostResolution {
            host: host.clone(),
            resolution: ResolutionResult {
                object_kind: ObjectKind::Host,
                status: ResolutionStatus::Matched,
                matched_id: Some(host.host_id),
                confidence: Confidence::High,
                rule_name: "host.machine_id".to_string(),
                matched_identifiers: vec![IdentifierMatch {
                    key: "machine_id".to_string(),
                    value: host.machine_id.clone().unwrap_or_default(),
                }],
                conflicting_ids: Vec::new(),
            },
        };
    }

    if let Some(host) = catalog
        .hosts
        .iter()
        .find(|host| host.tenant_id == candidate.tenant_id && host.host_name == candidate.host_name)
    {
        return HostResolution {
            host: host.clone(),
            resolution: ResolutionResult {
                object_kind: ObjectKind::Host,
                status: ResolutionStatus::Matched,
                matched_id: Some(host.host_id),
                confidence: Confidence::Medium,
                rule_name: "host.host_name".to_string(),
                matched_identifiers: vec![IdentifierMatch {
                    key: "host_name".to_string(),
                    value: host.host_name.clone(),
                }],
                conflicting_ids: Vec::new(),
            },
        };
    }

    let host_key = candidate
        .machine_id
        .clone()
        .unwrap_or_else(|| candidate.host_name.clone());
    let host = HostInventory {
        host_id: stable_uuid("host", &host_key),
        tenant_id: candidate.tenant_id,
        environment_id: candidate.environment_id,
        host_name: candidate.host_name.clone(),
        machine_id: candidate.machine_id.clone(),
        os_name: candidate.os_name.clone(),
        os_version: candidate.os_version.clone(),
        arch: candidate.arch.clone(),
        created_at: now,
        last_inventory_at: now,
        // 这里只是「新建一个候选」，存活/作用域由 `materialize_candidates` 在落地前统一填。
        stale_at: None,
        miss_count: 0,
        ingest_scope: None,
    };

    HostResolution {
        host: host.clone(),
        resolution: ResolutionResult {
            object_kind: ObjectKind::Host,
            status: ResolutionStatus::Created,
            matched_id: Some(host.host_id),
            confidence: Confidence::High,
            rule_name: "host.create".to_string(),
            matched_identifiers: collect_identifiers([
                ("host_name", Some(host.host_name.clone())),
                ("machine_id", host.machine_id.clone()),
            ]),
            conflicting_ids: Vec::new(),
        },
    }
}

pub fn resolve_network_candidate(
    catalog: &mut InMemoryCatalog,
    candidate: &NetworkSegmentCandidate,
    now: DateTime<Utc>,
) -> NetworkResolution {
    if let Some(segment) = catalog.network_segments.iter().find(|segment| {
        candidate
            .cidr
            .as_ref()
            .zip(segment.cidr.as_ref())
            .is_some_and(|(candidate_cidr, segment_cidr)| candidate_cidr == segment_cidr)
    }) {
        return NetworkResolution {
            segment: segment.clone(),
            resolution: ResolutionResult {
                object_kind: ObjectKind::NetworkSegment,
                status: ResolutionStatus::Matched,
                matched_id: Some(segment.network_segment_id),
                confidence: Confidence::High,
                rule_name: "network_segment.cidr".to_string(),
                matched_identifiers: vec![IdentifierMatch {
                    key: "cidr".to_string(),
                    value: segment.cidr.clone().unwrap_or_default(),
                }],
                conflicting_ids: Vec::new(),
            },
        };
    }

    let domain_id = ensure_default_network_domain(catalog, candidate.tenant_id, now);
    let segment_name = candidate
        .segment_name
        .clone()
        .or_else(|| candidate.cidr.clone())
        .or_else(|| {
            candidate
                .ip_addr
                .as_ref()
                .map(|ip| format!("unresolved:{ip}"))
        })
        .unwrap_or_else(|| "unresolved".to_string());

    let segment = NetworkSegment {
        network_segment_id: stable_uuid(
            "network_segment",
            &format!(
                "{}:{}:{}",
                candidate.tenant_id.0,
                candidate
                    .cidr
                    .clone()
                    .or_else(|| candidate.segment_name.clone())
                    .or_else(|| candidate.ip_addr.clone())
                    .unwrap_or_else(|| "unresolved".to_string()),
                segment_name
            ),
        ),
        tenant_id: candidate.tenant_id,
        network_domain_id: Some(domain_id),
        environment_id: candidate.environment_id,
        name: segment_name,
        cidr: candidate.cidr.clone(),
        gateway_ip: candidate.gateway_ip.clone(),
        address_family: if candidate
            .cidr
            .as_deref()
            .or(candidate.ip_addr.as_deref())
            .is_some_and(|value| value.contains(':'))
        {
            atlas_domain::AddressFamily::Ipv6
        } else {
            atlas_domain::AddressFamily::Ipv4
        },
        created_at: now,
        updated_at: now,
    };

    NetworkResolution {
        segment: segment.clone(),
        resolution: ResolutionResult {
            object_kind: ObjectKind::NetworkSegment,
            status: ResolutionStatus::Created,
            matched_id: Some(segment.network_segment_id),
            confidence: Confidence::Medium,
            rule_name: "network_segment.create".to_string(),
            matched_identifiers: collect_identifiers([
                ("segment_name", Some(segment.name.clone())),
                ("cidr", segment.cidr.clone()),
                ("ip_addr", candidate.ip_addr.clone()),
            ]),
            conflicting_ids: Vec::new(),
        },
    }
}

pub fn materialize_host_network(
    catalog: &mut InMemoryCatalog,
    host_candidate: &HostCandidate,
    network_candidate: &NetworkSegmentCandidate,
    now: DateTime<Utc>,
) -> HostNetworkMaterialization {
    let host_resolution = resolve_host_candidate(catalog, host_candidate, now);
    let network_resolution = resolve_network_candidate(catalog, network_candidate, now);

    upsert_host(catalog, host_resolution.host.clone());
    upsert_segment(catalog, network_resolution.segment.clone());

    let assoc = network_candidate.ip_addr.as_ref().map(|ip_addr| {
        if let Some(existing) = catalog.host_net_assocs.iter().find(|assoc| {
            assoc.host_id == host_resolution.host.host_id
                && assoc.network_segment_id == network_resolution.segment.network_segment_id
                && assoc.ip_addr == *ip_addr
                && assoc.validity.valid_to.is_none()
        }) {
            existing.clone()
        } else {
            let assoc = HostNetAssoc {
                assoc_id: stable_uuid(
                    "host_net_assoc",
                    &format!(
                        "{}:{}:{}",
                        host_resolution.host.host_id,
                        network_resolution.segment.network_segment_id,
                        ip_addr
                    ),
                ),
                tenant_id: host_candidate.tenant_id,
                host_id: host_resolution.host.host_id,
                network_segment_id: network_resolution.segment.network_segment_id,
                ip_addr: ip_addr.clone(),
                iface_name: network_candidate.iface_name.clone(),
                validity: ValidityWindow {
                    valid_from: now,
                    valid_to: None,
                },
                created_at: now,
                updated_at: now,
            };
            catalog.host_net_assocs.push(assoc.clone());
            assoc
        }
    });

    HostNetworkMaterialization {
        host: host_resolution.host,
        segment: network_resolution.segment,
        assoc,
        host_resolution: host_resolution.resolution,
        segment_resolution: network_resolution.resolution,
    }
}

fn ensure_default_network_domain(
    catalog: &mut InMemoryCatalog,
    tenant_id: TenantId,
    now: DateTime<Utc>,
) -> Uuid {
    if let Some(domain) = catalog
        .network_domains
        .iter()
        .find(|domain| domain.tenant_id == tenant_id && domain.name == "default")
    {
        return domain.network_domain_id;
    }

    let domain = NetworkDomain {
        network_domain_id: stable_uuid("network_domain", &format!("{}:default", tenant_id.0)),
        tenant_id,
        environment_id: None,
        name: "default".to_string(),
        kind: NetworkDomainKind::Unknown,
        description: Some("auto-created default network domain".to_string()),
        created_at: now,
        updated_at: now,
    };
    let id = domain.network_domain_id;
    catalog.network_domains.push(domain);
    id
}

fn upsert_host(catalog: &mut InMemoryCatalog, host: HostInventory) {
    if let Some(existing) = catalog
        .hosts
        .iter_mut()
        .find(|existing| existing.host_id == host.host_id)
    {
        *existing = host;
    } else {
        catalog.hosts.push(host);
    }
}

fn upsert_segment(catalog: &mut InMemoryCatalog, segment: NetworkSegment) {
    if let Some(existing) = catalog
        .network_segments
        .iter_mut()
        .find(|existing| existing.network_segment_id == segment.network_segment_id)
    {
        *existing = segment;
    } else {
        catalog.network_segments.push(segment);
    }
}

fn collect_identifiers<const N: usize>(
    values: [(&'static str, Option<String>); N],
) -> Vec<IdentifierMatch> {
    values
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .map(|(key, value)| IdentifierMatch {
            key: key.to_string(),
            value,
        })
        .collect()
}

fn stable_uuid(namespace: &str, key: &str) -> Uuid {
    let mut bytes = [0u8; 16];
    let left = fxhash(namespace, key, "a").to_be_bytes();
    let right = fxhash(namespace, key, "b").to_be_bytes();
    bytes[..8].copy_from_slice(&left);
    bytes[8..].copy_from_slice(&right);
    Uuid::from_bytes(bytes)
}

fn fxhash(namespace: &str, key: &str, salt: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    ("wist-atlas", namespace, key, salt).hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use atlas_domain::{EnvironmentId, IngestEnvelope, IngestMode, SourceKind};
    use chrono::Utc;

    use super::*;

    fn tenant_id() -> TenantId {
        TenantId(Uuid::new_v4())
    }

    fn host_candidate(tenant_id: TenantId) -> HostCandidate {
        HostCandidate {
            tenant_id,
            environment_id: Some(EnvironmentId(Uuid::new_v4())),
            source_kind: SourceKind::BatchImport,
            external_ref: None,
            host_name: "laptop-01".to_string(),
            machine_id: Some("machine-01".to_string()),
            os_name: Some("linux".to_string()),
            os_version: Some("6.8".to_string()),
            arch: Some("x86_64".to_string()),
        }
    }

    fn network_candidate(tenant_id: TenantId) -> NetworkSegmentCandidate {
        NetworkSegmentCandidate {
            tenant_id,
            environment_id: None,
            source_kind: SourceKind::BatchImport,
            segment_name: None,
            cidr: Some("192.168.0.0/24".to_string()),
            gateway_ip: Some("192.168.0.1".to_string()),
            ip_addr: Some("192.168.0.10".to_string()),
            host_name: Some("laptop-01".to_string()),
            machine_id: Some("machine-01".to_string()),
            iface_name: Some("wlan0".to_string()),
        }
    }

    #[test]
    fn materialize_host_and_network_creates_all_core_objects() {
        let tenant_id = tenant_id();
        let mut catalog = InMemoryCatalog::default();
        let now = Utc::now();

        let materialized = materialize_host_network(
            &mut catalog,
            &host_candidate(tenant_id),
            &network_candidate(tenant_id),
            now,
        );

        assert_eq!(catalog.hosts.len(), 1);
        assert_eq!(catalog.network_domains.len(), 1);
        assert_eq!(catalog.network_segments.len(), 1);
        assert_eq!(catalog.host_net_assocs.len(), 1);
        assert!(matches!(
            materialized.host_resolution.status,
            ResolutionStatus::Created
        ));
        assert!(matches!(
            materialized.segment_resolution.status,
            ResolutionStatus::Created
        ));
        assert_eq!(
            materialized
                .assoc
                .as_ref()
                .map(|assoc| assoc.ip_addr.as_str()),
            Some("192.168.0.10")
        );
    }

    #[test]
    fn materialize_reuses_existing_host_and_segment() {
        let tenant_id = tenant_id();
        let mut catalog = InMemoryCatalog::default();
        let now = Utc::now();

        let first = materialize_host_network(
            &mut catalog,
            &host_candidate(tenant_id),
            &network_candidate(tenant_id),
            now,
        );
        let second = materialize_host_network(
            &mut catalog,
            &host_candidate(tenant_id),
            &network_candidate(tenant_id),
            now,
        );

        assert_eq!(catalog.hosts.len(), 1);
        assert_eq!(catalog.network_segments.len(), 1);
        assert_eq!(catalog.host_net_assocs.len(), 1);
        assert!(matches!(
            second.host_resolution.status,
            ResolutionStatus::Matched
        ));
        assert!(matches!(
            second.segment_resolution.status,
            ResolutionStatus::Matched
        ));
        assert_eq!(first.host.host_id, second.host.host_id);
        assert_eq!(
            first.segment.network_segment_id,
            second.segment.network_segment_id
        );
    }

    #[test]
    fn ingest_envelope_mode_is_available_to_pipeline_callers() {
        let envelope = IngestEnvelope {
            ingest_id: "ing-1".to_string(),
            source_kind: SourceKind::BatchImport,
            source_name: "fixture".to_string(),
            ingest_mode: IngestMode::BatchUpsert,
            tenant_id: tenant_id(),
            environment_id: None,
            observed_at: None,
            received_at: Utc::now(),
            payload_ref: None,
            payload_inline: None,
            metadata: BTreeMap::new(),
        };

        assert_eq!(envelope.ingest_mode, IngestMode::BatchUpsert);
    }
}
