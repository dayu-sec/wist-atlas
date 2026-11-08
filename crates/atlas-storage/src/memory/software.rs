use atlas_domain::{
    AnchorKey, SoftwareArtifact, SoftwareArtifactPath, SoftwareFingerprint, SoftwareInstance,
    SoftwareInstanceObservation,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{Page, SoftwareStore, StorageResult};

use super::InMemoryTopologyStore;

impl SoftwareStore for InMemoryTopologyStore {
    fn upsert_artifact(&self, artifact: &SoftwareArtifact) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .software_artifacts
                .iter_mut()
                .find(|item| item.artifact_id == artifact.artifact_id)
            {
                *existing = artifact.clone();
            } else {
                state.software_artifacts.push(artifact.clone());
            }
        })
    }

    fn get_artifact(&self, artifact_id: &str) -> StorageResult<Option<SoftwareArtifact>> {
        self.with_state(|state| {
            state
                .software_artifacts
                .iter()
                .find(|item| item.artifact_id == artifact_id)
                .cloned()
        })
    }

    fn upsert_instance(&self, instance: &SoftwareInstance) -> StorageResult<()> {
        self.with_state(|state| {
            // 位置寻址：同一个部署位跨轮次是同一行。按 anchor 归并而不是按
            // instance_id 插入，否则升级后每轮都会多出一条「新软件」。
            let anchor = instance.anchor();
            if let Some(existing) = state
                .software_instances
                .iter_mut()
                .find(|item| item.host_id == instance.host_id && item.anchor() == anchor)
            {
                // 归并时**保留原 instance_id**：观测时间线与路径明细都挂在它上面，
                // 换成新 id 等于把历史孤儿化。这与 SQL 的 ON CONFLICT 不写 instance_id 一致。
                let instance_id = existing.instance_id;
                *existing = instance.clone();
                existing.instance_id = instance_id;
            } else {
                state.software_instances.push(instance.clone());
            }
        })
    }

    fn get_instance(&self, instance_id: Uuid) -> StorageResult<Option<SoftwareInstance>> {
        self.with_state(|state| {
            state
                .software_instances
                .iter()
                .find(|item| item.instance_id == instance_id)
                .cloned()
        })
    }

    fn get_instance_by_anchor(
        &self,
        host_id: Uuid,
        anchor: &AnchorKey,
    ) -> StorageResult<Option<SoftwareInstance>> {
        self.with_state(|state| {
            state
                .software_instances
                .iter()
                .find(|item| item.host_id == host_id && &item.anchor() == anchor)
                .cloned()
        })
    }

    fn list_instances_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<SoftwareInstance>> {
        self.with_state(|state| {
            state
                .software_instances
                .iter()
                // 退役行留在表里供回溯，但不进「现在有哪几个软件」。
                .filter(|item| item.host_id == host_id && item.is_live())
                .skip(page.offset as usize)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn mark_instances_stale(
        &self,
        host_id: Uuid,
        seen: &[AnchorKey],
        at: DateTime<Utc>,
    ) -> StorageResult<u64> {
        self.with_state(|state| {
            let mut marked = 0;
            for instance in state
                .software_instances
                .iter_mut()
                .filter(|item| item.host_id == host_id && item.stale_at.is_none())
            {
                if !seen.contains(&instance.anchor()) {
                    instance.stale_at = Some(at);
                    marked += 1;
                }
            }
            marked
        })
    }

    fn upsert_fingerprint(&self, fingerprint: &SoftwareFingerprint) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state.software_fingerprints.iter_mut().find(|item| {
                item.tenant_id == fingerprint.tenant_id
                    && item.host_id == fingerprint.host_id
                    && item.artifact_id == fingerprint.artifact_id
            }) {
                // `first_seen_at` 保留第一次：它是一条「这台机什么时候有的」的事实。
                existing.last_seen_at = existing.last_seen_at.max(fingerprint.last_seen_at);
                existing.size_bytes = fingerprint.size_bytes;
                existing.build_id = fingerprint.build_id.clone();
            } else {
                state.software_fingerprints.push(fingerprint.clone());
            }
        })
    }

    fn list_fingerprints_for_host(
        &self,
        host_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<SoftwareFingerprint>> {
        self.with_state(|state| {
            state
                .software_fingerprints
                .iter()
                .filter(|item| item.host_id == host_id)
                .skip(page.offset as usize)
                .take(page.limit as usize)
                .cloned()
                .collect()
        })
    }

    fn record_observation(&self, observation: &SoftwareInstanceObservation) -> StorageResult<()> {
        self.with_state(|state| {
            let exists = state.software_instance_observations.iter().any(|item| {
                item.instance_id == observation.instance_id
                    && item.artifact_id == observation.artifact_id
                    && item.observed_at == observation.observed_at
            });
            if !exists {
                state
                    .software_instance_observations
                    .push(observation.clone());
            }
        })
    }

    fn list_observations_for_instance(
        &self,
        instance_id: Uuid,
        page: Page,
    ) -> StorageResult<Vec<SoftwareInstanceObservation>> {
        self.with_state(|state| {
            let mut rows: Vec<_> = state
                .software_instance_observations
                .iter()
                .filter(|item| item.instance_id == instance_id)
                .cloned()
                .collect();
            // 时间线：最新的在前，页面要读的是「最近发生了什么」。
            rows.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
            rows.into_iter()
                .skip(page.offset as usize)
                .take(page.limit as usize)
                .collect()
        })
    }

    fn upsert_artifact_path(&self, path: &SoftwareArtifactPath) -> StorageResult<()> {
        self.with_state(|state| {
            if let Some(existing) = state
                .software_artifact_paths
                .iter_mut()
                .find(|item| item.instance_id == path.instance_id && item.path == path.path)
            {
                *existing = path.clone();
            } else {
                state.software_artifact_paths.push(path.clone());
            }
        })
    }

    fn list_artifact_paths_for_instance(
        &self,
        instance_id: Uuid,
    ) -> StorageResult<Vec<SoftwareArtifactPath>> {
        self.with_state(|state| {
            let mut rows: Vec<_> = state
                .software_artifact_paths
                .iter()
                .filter(|item| item.instance_id == instance_id)
                .cloned()
                .collect();
            // `primary` 在前：页面先给主二进制。
            rows.sort_by(|left, right| left.role.cmp(&right.role).then(left.path.cmp(&right.path)));
            rows
        })
    }
}
