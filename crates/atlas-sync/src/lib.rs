pub mod error;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use atlas_api::TopologyIngestService;
use atlas_domain::{
    AtlasInputEnvelope, EnvironmentId, IngestEnvelope, IngestMode, ObservedAt, SourceKind, TenantId,
};
use atlas_storage::AsyncIngestStore;
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncRuntimeStore, CatalogStore, GovernanceStore,
    IngestStore, Page, RuntimeStore,
};
use chrono::{DateTime, Utc};
use orion_error::{conversion::ConvErr, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub use error::{SyncError, SyncReason, SyncResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalIdentityLink {
    pub link_id: Uuid,
    pub system_type: String,
    pub external_id: String,
    pub internal_id: Uuid,
    pub last_synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalSyncCursor {
    pub cursor_id: Uuid,
    pub system_type: String,
    pub scope_key: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlImportSummary {
    pub total_lines: usize,
    pub success_lines: usize,
    pub failed_lines: usize,
    pub host_count: usize,
    pub network_count: usize,
    pub process_count: usize,
    pub enriched_process_count: usize,
    pub host_runtime_count: usize,
    pub last_ingest_id: Option<String>,
    pub failures: Vec<String>,
}

pub struct JsonlImportService<S> {
    store: S,
}

impl<S> JsonlImportService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> JsonlImportService<S>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    pub fn import_files(
        &self,
        tenant_id: TenantId,
        paths: &[PathBuf],
    ) -> SyncResult<JsonlImportSummary> {
        let ingest = TopologyIngestService::new(self.store.clone());
        let mut summary = JsonlImportSummary::default();
        for path in paths {
            let lines = load_jsonl_lines(path)?;
            for input_line in lines {
                ingest_sync_line(&ingest, tenant_id, input_line, &mut summary)?;
            }
        }

        finalize_summary_sync(&self.store, tenant_id, summary)
    }
}

impl<S> JsonlImportService<S>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore + AsyncIngestStore,
{
    pub async fn import_files_async(
        &self,
        tenant_id: TenantId,
        paths: &[PathBuf],
    ) -> SyncResult<JsonlImportSummary> {
        let ingest = TopologyIngestService::new(self.store.clone());
        let mut summary = JsonlImportSummary::default();

        for path in paths {
            let lines = load_jsonl_lines(path)?;
            for input_line in lines {
                ingest_async_line(&ingest, tenant_id, input_line, &mut summary).await?;
            }
        }

        finalize_summary_async(&self.store, tenant_id, summary).await
    }
}

#[derive(Debug, Clone)]
struct JsonlInputLine {
    path: PathBuf,
    line_no: usize,
    line: String,
}

impl JsonlInputLine {
    fn location(&self) -> String {
        format!("{} line {}", self.path.display(), self.line_no)
    }
}

impl Default for JsonlImportSummary {
    fn default() -> Self {
        Self {
            total_lines: 0,
            success_lines: 0,
            failed_lines: 0,
            host_count: 0,
            network_count: 0,
            process_count: 0,
            enriched_process_count: 0,
            host_runtime_count: 0,
            last_ingest_id: None,
            failures: Vec::new(),
        }
    }
}

fn load_jsonl_lines(path: &PathBuf) -> SyncResult<Vec<JsonlInputLine>> {
    let file = fs::File::open(path).source_err(
        SyncReason::InputLoadFailed,
        format!("read {}", path.display()),
    )?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();

    for (index, line_result) in reader.lines().enumerate() {
        let line_no = index + 1;
        let line = line_result.source_err(
            SyncReason::InputLoadFailed,
            format!("read {} line {}", path.display(), line_no),
        )?;

        if line.trim().is_empty() {
            continue;
        }

        lines.push(JsonlInputLine {
            path: path.clone(),
            line_no,
            line,
        });
    }

    Ok(lines)
}

fn ingest_sync_line<S>(
    ingest: &TopologyIngestService<S>,
    tenant_id: TenantId,
    input_line: JsonlInputLine,
    summary: &mut JsonlImportSummary,
) -> SyncResult<()>
where
    S: CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    summary.total_lines += 1;

    let input = match serde_json::from_str::<Value>(&input_line.line).source_err(
        SyncReason::InputLoadFailed,
        format!("parse {} as json", input_line.location()),
    ) {
        Ok(input) => input,
        Err(err) => {
            record_failure(summary, &input_line, err.to_string());
            return Ok(());
        }
    };

    let envelope = match ingest_envelope_from_input(input, tenant_id) {
        Ok(envelope) => envelope,
        Err(err) => {
            record_failure(summary, &input_line, err.to_string());
            return Ok(());
        }
    };

    let submit_result: SyncResult<_> = ingest.submit_and_materialize(envelope).conv_err();
    match submit_result {
        Ok((record, _)) => {
            summary.success_lines += 1;
            summary.last_ingest_id = Some(record.ingest_id);
        }
        Err(err) => record_failure(summary, &input_line, err.to_string()),
    }

    Ok(())
}

async fn ingest_async_line<S>(
    ingest: &TopologyIngestService<S>,
    tenant_id: TenantId,
    input_line: JsonlInputLine,
    summary: &mut JsonlImportSummary,
) -> SyncResult<()>
where
    S: AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore + AsyncIngestStore,
{
    summary.total_lines += 1;

    let input = match serde_json::from_str::<Value>(&input_line.line).source_err(
        SyncReason::InputLoadFailed,
        format!("parse {} as json", input_line.location()),
    ) {
        Ok(input) => input,
        Err(err) => {
            record_failure(summary, &input_line, err.to_string());
            return Ok(());
        }
    };

    let envelope = match ingest_envelope_from_input(input, tenant_id) {
        Ok(envelope) => envelope,
        Err(err) => {
            record_failure(summary, &input_line, err.to_string());
            return Ok(());
        }
    };

    let submit_result: SyncResult<_> = ingest
        .submit_and_materialize_async(envelope)
        .await
        .conv_err();
    match submit_result {
        Ok((record, _)) => {
            summary.success_lines += 1;
            summary.last_ingest_id = Some(record.ingest_id);
        }
        Err(err) => record_failure(summary, &input_line, err.to_string()),
    }

    Ok(())
}

fn record_failure(summary: &mut JsonlImportSummary, input_line: &JsonlInputLine, err: String) {
    summary.failed_lines += 1;
    summary
        .failures
        .push(format!("{}: {}", input_line.location(), err));
}

fn finalize_summary_sync<S>(
    store: &S,
    tenant_id: TenantId,
    mut summary: JsonlImportSummary,
) -> SyncResult<JsonlImportSummary>
where
    S: CatalogStore + RuntimeStore,
{
    let full_page = Page {
        limit: i32::MAX as u32,
        offset: 0,
    };
    let hosts = CatalogStore::list_hosts(store, tenant_id, full_page).conv_err()?;
    summary.host_count = hosts.len();
    summary.network_count = CatalogStore::list_network_segments(store, tenant_id, full_page)
        .conv_err()?
        .len();
    summary.process_count = hosts
        .iter()
        .try_fold(0usize, |acc, host| {
            // 与现役视图同口径：已标 stale 的进程不在「现在有多少进程」里。
            // 否则导入报告的 processes=N 会和页面上的数字对不上。
            RuntimeStore::list_process_runtime_states(store, host.host_id, full_page).map(|items| {
                acc + items
                    .into_iter()
                    .filter(|item| item.stale_at.is_none())
                    .count()
            })
        })
        .conv_err()?;
    summary.enriched_process_count = hosts
        .iter()
        .try_fold(0usize, |acc, host| {
            RuntimeStore::list_process_runtime_states(store, host.host_id, full_page).map(|items| {
                acc + items
                    .into_iter()
                    .filter(|item| item.stale_at.is_none())
                    .filter(|item| item.process_state.is_some() || item.memory_rss_kib.is_some())
                    .count()
            })
        })
        .conv_err()?;
    summary.host_runtime_count = hosts
        .iter()
        .try_fold(0usize, |acc, host| {
            RuntimeStore::list_host_runtime_states(store, host.host_id, full_page)
                .map(|items| acc + items.len())
        })
        .conv_err()?;
    Ok(summary)
}

async fn finalize_summary_async<S>(
    store: &S,
    tenant_id: TenantId,
    mut summary: JsonlImportSummary,
) -> SyncResult<JsonlImportSummary>
where
    S: AsyncCatalogStore + AsyncRuntimeStore,
{
    let full_page = Page {
        limit: i32::MAX as u32,
        offset: 0,
    };
    let hosts = AsyncCatalogStore::list_hosts(store, tenant_id, full_page)
        .await
        .conv_err()?;
    summary.host_count = hosts.len();
    summary.network_count = AsyncCatalogStore::list_network_segments(store, tenant_id, full_page)
        .await
        .conv_err()?
        .len();
    summary.process_count = 0;
    summary.enriched_process_count = 0;
    summary.host_runtime_count = 0;

    for host in &hosts {
        let processes =
            AsyncRuntimeStore::list_process_runtime_states(store, host.host_id, full_page)
                .await
                .conv_err()?;
        summary.process_count += processes
            .iter()
            .filter(|item| item.stale_at.is_none())
            .count();
        summary.enriched_process_count += processes
            .iter()
            .filter(|item| item.stale_at.is_none())
            .filter(|item| item.process_state.is_some() || item.memory_rss_kib.is_some())
            .count();
        summary.host_runtime_count +=
            AsyncRuntimeStore::list_host_runtime_states(store, host.host_id, full_page)
                .await
                .conv_err()?
                .len();
    }

    Ok(summary)
}

fn ingest_envelope_from_input(input: Value, tenant_id: TenantId) -> SyncResult<IngestEnvelope> {
    if looks_like_atlas_input(&input) {
        let atlas_input: AtlasInputEnvelope = serde_json::from_value(input)
            .source_err(SyncReason::InputLoadFailed, "parse atlas input")?;
        atlas_input.validate().conv_err()?;
        return Ok(atlas_input.into_ingest_envelope(tenant_id, None, Utc::now()));
    }

    Ok(IngestEnvelope {
        ingest_id: "demo-ingest-1".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "atlas-sync".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None::<EnvironmentId>,
        observed_at: None::<ObservedAt>,
        received_at: Utc::now(),
        payload_ref: None,
        payload_inline: Some(input),
        metadata: Default::default(),
    })
}

fn looks_like_atlas_input(input: &Value) -> bool {
    let Some(object) = input.as_object() else {
        return false;
    };

    object.contains_key("schema")
        || object.contains_key("source")
        || object.contains_key("collect")
        || object.contains_key("payload")
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_storage::InMemoryTopologyStore;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../fixtures/{name}"))
    }

    #[test]
    fn import_files_replays_edge_and_telemetry_jsonl() {
        let store = InMemoryTopologyStore::default();
        let service = JsonlImportService::new(store);
        let tenant_id = TenantId(Uuid::new_v4());

        let summary = service
            .import_files(
                tenant_id,
                &[
                    fixture_path("atlas_edge_host_only.jsonl"),
                    fixture_path("atlas_telemetry_host_sample.jsonl"),
                ],
            )
            .unwrap();

        assert_eq!(summary.total_lines, 3);
        assert_eq!(summary.success_lines, 3);
        assert_eq!(summary.failed_lines, 0);
        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.host_runtime_count, 1);
    }

    #[test]
    fn import_files_enriches_process_from_telemetry() {
        let store = InMemoryTopologyStore::default();
        let service = JsonlImportService::new(store);
        let tenant_id = TenantId(Uuid::new_v4());

        let summary = service
            .import_files(
                tenant_id,
                &[fixture_path("atlas_telemetry_process_sample.jsonl")],
            )
            .unwrap();

        assert_eq!(summary.total_lines, 3);
        assert_eq!(summary.success_lines, 3);
        assert_eq!(summary.failed_lines, 0);
        assert_eq!(summary.host_count, 1);
        assert_eq!(summary.process_count, 1);
        assert_eq!(summary.enriched_process_count, 1);
    }
}
