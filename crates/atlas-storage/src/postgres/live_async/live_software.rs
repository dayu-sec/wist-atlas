use atlas_domain::{
    AnchorKey, SoftwareArtifact, SoftwareArtifactPath, SoftwareFingerprint, SoftwareInstance,
    SoftwareInstanceObservation,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{AsyncSoftwareStore, Page, StorageResult, operation_failed};

use super::super::{LivePostgresExecutor, PostgresTopologyStore, row_decode::*, sql};

fn json_string<T: serde::Serialize>(value: &T) -> StorageResult<String> {
    serde_json::to_string(value)
        .map_err(|err| operation_failed(format!("encode json column: {err}")))
}

fn encode_roles(roles: &[atlas_domain::SoftwareRole]) -> StorageResult<String> {
    let raw: Vec<&str> = roles.iter().map(|role| role.as_str()).collect();
    json_string(&raw)
}

fn encode_reasons(reasons: &[atlas_domain::SoftwareReason]) -> StorageResult<String> {
    let raw: Vec<&str> = reasons.iter().map(|reason| reason.as_str()).collect();
    json_string(&raw)
}

impl AsyncSoftwareStore for PostgresTopologyStore<LivePostgresExecutor> {
    async fn upsert_artifact(&self, artifact: &SoftwareArtifact) -> StorageResult<()> {
        let coordinate = match &artifact.coordinate {
            Some(value) => json_string(value)?,
            None => String::new(),
        };
        self.executor
            .exec_async(
                sql::UPSERT_SOFTWARE_ARTIFACT,
                &[
                    artifact.artifact_id.clone(),
                    artifact.version_id.clone().unwrap_or_default(),
                    artifact.artifact_kind.as_str().to_string(),
                    artifact.sha256.clone(),
                    artifact
                        .size_bytes
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    artifact.build_id.clone().unwrap_or_default(),
                    artifact.package_manager.clone().unwrap_or_default(),
                    artifact.package_name.clone().unwrap_or_default(),
                    artifact.purl.clone().unwrap_or_default(),
                    artifact.interpreter_artifact_id.clone().unwrap_or_default(),
                    coordinate,
                    artifact
                        .first_seen_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_default(),
                    artifact
                        .last_seen_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_default(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_artifact(&self, artifact_id: &str) -> StorageResult<Option<SoftwareArtifact>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SOFTWARE_ARTIFACT, &[artifact_id.to_string()])
            .await?;
        rows.into_iter()
            .next()
            .map(|row| decode_software_artifact(&row))
            .transpose()
    }

    async fn upsert_instance(&self, instance: &SoftwareInstance) -> StorageResult<()> {
        let role = encode_roles(&instance.role)?;
        let reason = encode_reasons(&instance.reason)?;
        self.executor
            .exec_async(
                sql::UPSERT_SOFTWARE_INSTANCE,
                &[
                    instance.instance_id.to_string(),
                    instance.tenant_id.0.to_string(),
                    instance.host_id.to_string(),
                    instance.anchor_kind.as_str().to_string(),
                    instance.anchor_value.clone(),
                    instance.current_artifact_id.clone().unwrap_or_default(),
                    instance.identity_missing_reason.clone().unwrap_or_default(),
                    instance.claimed_package_name.clone().unwrap_or_default(),
                    instance.claimed_product_id.clone().unwrap_or_default(),
                    instance.claimed_version.clone().unwrap_or_default(),
                    instance
                        .claimed_source
                        .map(|value| value.as_str().to_string())
                        .unwrap_or_default(),
                    role,
                    reason,
                    instance.status.as_str().to_string(),
                    instance.first_seen_at.to_rfc3339(),
                    instance.last_seen_at.to_rfc3339(),
                    instance
                        .stale_at
                        .map(|value| value.to_rfc3339())
                        .unwrap_or_default(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_instance(&self, instance_id: Uuid) -> StorageResult<Option<SoftwareInstance>> {
        let rows = self
            .executor
            .query_rows_async(sql::GET_SOFTWARE_INSTANCE, &[instance_id.to_string()])
            .await?;
        rows.into_iter()
            .next()
            .map(|row| decode_software_instance(&row))
            .transpose()
    }

    async fn get_instance_by_anchor(
        &self,
        host_id: Uuid,
        anchor: &AnchorKey,
    ) -> StorageResult<Option<SoftwareInstance>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::GET_SOFTWARE_INSTANCE_BY_ANCHOR,
                &[
                    host_id.to_string(),
                    anchor.anchor_kind.as_str().to_string(),
                    anchor.anchor_value.clone(),
                ],
            )
            .await?;
        rows.into_iter()
            .next()
            .map(|row| decode_software_instance(&row))
            .transpose()
    }

    async fn list_instances_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<SoftwareInstance>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SOFTWARE_INSTANCES_FOR_HOST,
                &[
                    host_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_software_instance(&row))
            .collect()
    }

    async fn mark_instances_stale(
        &self,
        host_id: Uuid,
        seen: &[AnchorKey],
        at: DateTime<Utc>,
    ) -> StorageResult<u64> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_LIVE_SOFTWARE_INSTANCE_ANCHORS,
                &[host_id.to_string()],
            )
            .await?;
        let mut marked = 0;
        for row in rows {
            let anchor = AnchorKey {
                anchor_kind: parse_software_enum::<atlas_domain::AnchorKind>(&row[1])?,
                anchor_value: row[2].clone(),
            };
            if seen.contains(&anchor) {
                continue;
            }
            self.executor
                .exec_async(
                    sql::MARK_SOFTWARE_INSTANCE_STALE,
                    &[row[0].clone(), at.to_rfc3339()],
                )
                .await?;
            marked += 1;
        }
        Ok(marked)
    }

    async fn upsert_fingerprint(&self, fingerprint: &SoftwareFingerprint) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SOFTWARE_FINGERPRINT,
                &[
                    fingerprint.tenant_id.0.to_string(),
                    fingerprint.host_id.to_string(),
                    fingerprint.artifact_id.clone(),
                    fingerprint.size_bytes.to_string(),
                    fingerprint.build_id.clone().unwrap_or_default(),
                    fingerprint.first_seen_at.to_rfc3339(),
                    fingerprint.last_seen_at.to_rfc3339(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_fingerprints_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<SoftwareFingerprint>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SOFTWARE_FINGERPRINTS_FOR_HOST,
                &[
                    host_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_software_fingerprint(&row))
            .collect()
    }

    async fn record_observation(
        &self,
        observation: &SoftwareInstanceObservation,
    ) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SOFTWARE_INSTANCE_OBSERVATION,
                &[
                    observation.instance_id.to_string(),
                    observation.artifact_id.clone(),
                    observation.observed_at.to_rfc3339(),
                    observation.version_running.clone().unwrap_or_default(),
                    observation.version_installed.clone().unwrap_or_default(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_observations_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<SoftwareInstanceObservation>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SOFTWARE_INSTANCE_OBSERVATIONS,
                &[
                    instance_id.to_string(),
                    page.limit.to_string(),
                    page.offset.to_string(),
                ],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_software_observation(&row))
            .collect()
    }

    async fn upsert_artifact_path(&self, path: &SoftwareArtifactPath) -> StorageResult<()> {
        self.executor
            .exec_async(
                sql::UPSERT_SOFTWARE_ARTIFACT_PATH,
                &[
                    path.instance_id.to_string(),
                    path.path.clone(),
                    path.role.clone(),
                    path.size_bytes.to_string(),
                    path.mtime.to_rfc3339(),
                    path.inode.to_string(),
                ],
            )
            .await?;
        Ok(())
    }

    async fn list_artifact_paths_for_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Vec<SoftwareArtifactPath>> {
        let rows = self
            .executor
            .query_rows_async(
                sql::LIST_SOFTWARE_ARTIFACT_PATHS,
                &[instance_id.to_string()],
            )
            .await?;
        rows.into_iter()
            .map(|row| decode_software_artifact_path(&row))
            .collect()
    }
}
