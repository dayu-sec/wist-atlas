use atlas_domain::{ResponsibilityAssignment, Subject, SubjectCandidate};
use atlas_storage::{AsyncCatalogStore, AsyncGovernanceStore, CatalogStore, StorageResult};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub(crate) fn materialize_subjects_and_assignments<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
    subjects: Vec<SubjectCandidate>,
    assignments: Vec<atlas_domain::ResponsibilityAssignmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: CatalogStore + atlas_storage::GovernanceStore,
{
    let mut persisted_subjects = Vec::new();
    for candidate in subjects {
        let subject = Subject {
            subject_id: Uuid::new_v4(),
            tenant_id,
            subject_type: candidate.subject_type,
            external_ref: candidate.external_ref,
            display_name: candidate.display_name,
            email: candidate.email,
            is_active: candidate.is_active,
            created_at: now,
            updated_at: now,
        };
        store.upsert_subject(&subject)?;
        persisted_subjects.push(subject);
    }

    // 责任人分配要按名字匹配主机/网段：只取第一页会让第二页之后的主机永远挂不上责任人。
    let hosts = super::list_all_hosts(store, tenant_id)?;
    let segments = super::list_all_network_segments(store, tenant_id)?;

    for candidate in assignments {
        let subject = persisted_subjects.iter().find(|subject| {
            candidate
                .subject_email
                .as_ref()
                .is_some_and(|email| subject.email.as_ref() == Some(email))
                || candidate
                    .subject_display_name
                    .as_ref()
                    .is_some_and(|name| &subject.display_name == name)
        });

        let Some(subject) = subject else {
            continue;
        };

        let target_id = match candidate.target_kind {
            atlas_domain::ObjectKind::Host => hosts.iter().find_map(|host| {
                candidate
                    .target_external_ref
                    .as_ref()
                    .filter(|target| *target == &host.host_name)
                    .map(|_| host.host_id)
            }),
            atlas_domain::ObjectKind::NetworkSegment => segments.iter().find_map(|segment| {
                candidate
                    .target_external_ref
                    .as_ref()
                    .filter(|target| *target == &segment.name)
                    .map(|_| segment.network_segment_id)
            }),
            _ => None,
        };

        if let Some(target_id) = target_id {
            let assignment = ResponsibilityAssignment {
                assignment_id: Uuid::new_v4(),
                tenant_id,
                subject_id: subject.subject_id,
                target_kind: candidate.target_kind,
                target_id,
                role: candidate.role,
                source: format!("{:?}", candidate.source_kind).to_lowercase(),
                validity: candidate.validity,
                created_at: now,
                updated_at: now,
            };
            store.upsert_responsibility_assignment(&assignment)?;
        }
    }

    Ok(())
}

pub(crate) async fn materialize_subjects_and_assignments_async<S>(
    store: &S,
    tenant_id: atlas_domain::TenantId,
    subjects: Vec<SubjectCandidate>,
    assignments: Vec<atlas_domain::ResponsibilityAssignmentCandidate>,
    now: DateTime<Utc>,
) -> StorageResult<()>
where
    S: AsyncCatalogStore + AsyncGovernanceStore,
{
    let mut persisted_subjects = Vec::new();
    for candidate in subjects {
        let subject = Subject {
            subject_id: Uuid::new_v4(),
            tenant_id,
            subject_type: candidate.subject_type,
            external_ref: candidate.external_ref,
            display_name: candidate.display_name,
            email: candidate.email,
            is_active: candidate.is_active,
            created_at: now,
            updated_at: now,
        };
        atlas_storage::AsyncCatalogStore::upsert_subject(store, &subject).await?;
        persisted_subjects.push(subject);
    }

    let hosts = super::list_all_hosts_async(store, tenant_id).await?;
    let segments = super::list_all_network_segments_async(store, tenant_id).await?;

    for candidate in assignments {
        let subject = persisted_subjects.iter().find(|subject| {
            candidate
                .subject_email
                .as_ref()
                .is_some_and(|email| subject.email.as_ref() == Some(email))
                || candidate
                    .subject_display_name
                    .as_ref()
                    .is_some_and(|name| &subject.display_name == name)
        });

        let Some(subject) = subject else {
            continue;
        };

        let target_id = match candidate.target_kind {
            atlas_domain::ObjectKind::Host => hosts.iter().find_map(|host| {
                candidate
                    .target_external_ref
                    .as_ref()
                    .filter(|target| *target == &host.host_name)
                    .map(|_| host.host_id)
            }),
            atlas_domain::ObjectKind::NetworkSegment => segments.iter().find_map(|segment| {
                candidate
                    .target_external_ref
                    .as_ref()
                    .filter(|target| *target == &segment.name)
                    .map(|_| segment.network_segment_id)
            }),
            _ => None,
        };

        if let Some(target_id) = target_id {
            let assignment = ResponsibilityAssignment {
                assignment_id: Uuid::new_v4(),
                tenant_id,
                subject_id: subject.subject_id,
                target_kind: candidate.target_kind,
                target_id,
                role: candidate.role,
                source: format!("{:?}", candidate.source_kind).to_lowercase(),
                validity: candidate.validity,
                created_at: now,
                updated_at: now,
            };
            atlas_storage::AsyncGovernanceStore::upsert_responsibility_assignment(
                store,
                &assignment,
            )
            .await?;
        }
    }

    Ok(())
}
