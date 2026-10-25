use std::fs;
use std::path::PathBuf;

use atlas_api::{TopologyIngestService, TopologyQueryService};
use atlas_domain::{
    AtlasInputEnvelope, IngestEnvelope, IngestMode, ObjectKind, SourceKind, TenantId,
};
use atlas_storage::{
    AsyncCatalogStore, AsyncGovernanceStore, AsyncIngestStore, AsyncRuntimeStore, CatalogStore,
    GovernanceStore, IngestStore, LivePostgresExecutor, Page, PostgresTopologyStore, RuntimeStore,
};
use atlas_sync::JsonlImportService;
use chrono::Utc;
use orion_error::{conversion::ConvErr, prelude::*};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{invalid_args, materialization_missing};
use crate::graph::{
    build_host_process_topology_graph, build_host_process_topology_graph_async,
    build_visualization_graph,
};
use crate::http::VisualizationEnvelope;
use crate::payload_io::{load_demo_payload, load_payload_from_file};
use crate::{
    AppReason, AppResult, MonolithInput, MonolithRunResult, MonolithRunSummary,
    VisualizationExportSummary,
};

pub(super) fn run_with_store<S>(store: S, input: MonolithInput) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let tenant_id = TenantId(Uuid::new_v4());
    match input {
        MonolithInput::Demo => run_with_payload(store, tenant_id, load_demo_payload()?),
        MonolithInput::File(path) => {
            run_with_payload(store, tenant_id, load_payload_from_file(&path)?)
        }
        MonolithInput::JsonlFiles(paths) => run_with_jsonl_files(store, tenant_id, paths),
        MonolithInput::ExportVisualization(output_path) => {
            export_visualization_for_store(store, output_path)
        }
        MonolithInput::PrintHostProcessTopology(host_id) => {
            print_host_process_topology_for_store(store, Some(host_id))
        }
        MonolithInput::PrintFirstHostProcessTopology => {
            print_host_process_topology_for_store(store, None)
        }
        MonolithInput::Serve { .. } | MonolithInput::ServeJsonlFiles { .. } => Err(invalid_args()),
        MonolithInput::ResetPublic | MonolithInput::ReplaceJsonlFiles(_) => Err(invalid_args()),
    }
}

pub(super) fn run_with_postgres_store<E>(
    store: PostgresTopologyStore<E>,
    input: MonolithInput,
) -> AppResult<MonolithRunResult>
where
    E: atlas_storage::PostgresExecutor,
    PostgresTopologyStore<E>:
        AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore + AsyncIngestStore,
{
    match input {
        MonolithInput::JsonlFiles(paths) => {
            let tenant_id = TenantId(Uuid::new_v4());
            run_with_jsonl_files(store, tenant_id, paths)
        }
        MonolithInput::ResetPublic => {
            store.reset_public_schema().conv_err()?;
            Ok(MonolithRunResult::Reset(
                "public schema reset complete".to_string(),
            ))
        }
        MonolithInput::ReplaceJsonlFiles(paths) => {
            store.reset_public_schema().conv_err()?;
            let tenant_id = TenantId(Uuid::new_v4());
            run_with_jsonl_files(store, tenant_id, paths)
        }
        other => run_with_store(store, other),
    }
}

pub(super) async fn run_with_postgres_store_async(
    store: PostgresTopologyStore<LivePostgresExecutor>,
    input: MonolithInput,
) -> AppResult<MonolithRunResult> {
    match input {
        MonolithInput::Demo => {
            run_with_payload_async(store, TenantId(Uuid::new_v4()), load_demo_payload()?).await
        }
        MonolithInput::File(path) => {
            run_with_payload_async(
                store,
                TenantId(Uuid::new_v4()),
                load_payload_from_file(&path)?,
            )
            .await
        }
        MonolithInput::JsonlFiles(paths) => {
            let tenant_id = TenantId(Uuid::new_v4());
            let summary = JsonlImportService::new(store)
                .import_files_async(tenant_id, &paths)
                .await
                .conv_err()?;
            Ok(MonolithRunResult::Replay(summary))
        }
        MonolithInput::ResetPublic => {
            store.reset_public_schema_async().await.conv_err()?;
            Ok(MonolithRunResult::Reset(
                "public schema reset complete".to_string(),
            ))
        }
        MonolithInput::ReplaceJsonlFiles(paths) => {
            store.reset_public_schema_async().await.conv_err()?;
            let tenant_id = TenantId(Uuid::new_v4());
            let summary = JsonlImportService::new(store)
                .import_files_async(tenant_id, &paths)
                .await
                .conv_err()?;
            Ok(MonolithRunResult::Replay(summary))
        }
        MonolithInput::ExportVisualization(output_path) => {
            export_visualization_for_store_async(store, output_path).await
        }
        MonolithInput::PrintHostProcessTopology(host_id) => {
            print_host_process_topology_for_store_async(store, Some(host_id)).await
        }
        MonolithInput::PrintFirstHostProcessTopology => {
            print_host_process_topology_for_store_async(store, None).await
        }
        MonolithInput::Serve { .. } | MonolithInput::ServeJsonlFiles { .. } => Err(invalid_args()),
    }
}

fn run_with_payload<S>(
    store: S,
    tenant_id: TenantId,
    payload_inline: Value,
) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let ingest = TopologyIngestService::new(store.clone());
    let query = TopologyQueryService::new(store.clone());
    let envelope = ingest_envelope_from_input(payload_inline, tenant_id)?;

    let (record, _) = ingest.submit_and_materialize(envelope).conv_err()?;
    let host = store
        .list_hosts(tenant_id, Page::default())
        .conv_err()?
        .into_iter()
        .next()
        .ok_or_else(|| materialization_missing("host was not materialized"))?;
    let host_view = query
        .host_topology_view(host.host_id)
        .conv_err()?
        .ok_or_else(|| materialization_missing("host topology view was not built"))?;
    let assoc = host_view.network_assocs.first();
    let network = host_view.network_segments.first();
    let responsibility_views = query
        .effective_responsibility_view(ObjectKind::Host, host.host_id)
        .conv_err()?;

    Ok(MonolithRunResult::Single(MonolithRunSummary {
        ingest_id: record.ingest_id,
        host_name: host_view.host.host_name,
        network_name: network.map(|item| item.name.clone()),
        assoc_ip: assoc.map(|item| item.ip_addr.clone()),
        responsibilities: responsibility_views
            .into_iter()
            .map(|view| format!("{}:{:?}", view.subject.display_name, view.assignment.role))
            .collect(),
    }))
}

pub(super) fn run_with_jsonl_files<S>(
    store: S,
    tenant_id: TenantId,
    paths: Vec<PathBuf>,
) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore + IngestStore,
{
    let summary = JsonlImportService::new(store)
        .import_files(tenant_id, &paths)
        .conv_err()?;
    Ok(MonolithRunResult::Replay(summary))
}

async fn run_with_payload_async<S>(
    store: S,
    tenant_id: TenantId,
    payload_inline: Value,
) -> AppResult<MonolithRunResult>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore + AsyncIngestStore,
{
    let ingest = TopologyIngestService::new(store.clone());
    let query = TopologyQueryService::new(store.clone());
    let envelope = ingest_envelope_from_input(payload_inline, tenant_id)?;

    let (record, _) = ingest
        .submit_and_materialize_async(envelope)
        .await
        .conv_err()?;
    let host = AsyncCatalogStore::list_hosts(&store, tenant_id, Page::default())
        .await
        .conv_err()?
        .into_iter()
        .next()
        .ok_or_else(|| materialization_missing("host was not materialized"))?;
    let host_view = query
        .host_topology_view_async(host.host_id)
        .await
        .conv_err()?
        .ok_or_else(|| materialization_missing("host topology view was not built"))?;
    let assoc = host_view.network_assocs.first();
    let network = host_view.network_segments.first();
    let responsibility_views = query
        .effective_responsibility_view_async(ObjectKind::Host, host.host_id)
        .await
        .conv_err()?;

    Ok(MonolithRunResult::Single(MonolithRunSummary {
        ingest_id: record.ingest_id,
        host_name: host_view.host.host_name,
        network_name: network.map(|item| item.name.clone()),
        assoc_ip: assoc.map(|item| item.ip_addr.clone()),
        responsibilities: responsibility_views
            .into_iter()
            .map(|view| format!("{}:{:?}", view.subject.display_name, view.assignment.role))
            .collect(),
    }))
}

fn export_visualization_for_store<S>(store: S, output_path: PathBuf) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    let graph = build_visualization_graph(store)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).source_err(
            AppReason::InputLoadFailed,
            format!("create {}", parent.display()),
        )?;
    }
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload)
        .source_err(AppReason::InputLoadFailed, "serialize visualization export")?;
    fs::write(&output_path, body).source_err(
        AppReason::InputLoadFailed,
        format!("write {}", output_path.display()),
    )?;

    Ok(MonolithRunResult::ExportVisualization(
        VisualizationExportSummary {
            output_path,
            host_count: payload.data.metadata.host_count,
            process_count: payload.data.metadata.process_count,
        },
    ))
}

async fn export_visualization_for_store_async<S>(
    store: S,
    output_path: PathBuf,
) -> AppResult<MonolithRunResult>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let graph = build_host_process_topology_graph_async(store, None).await?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).source_err(
            AppReason::InputLoadFailed,
            format!("create {}", parent.display()),
        )?;
    }
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload)
        .source_err(AppReason::InputLoadFailed, "serialize visualization export")?;
    fs::write(&output_path, body).source_err(
        AppReason::InputLoadFailed,
        format!("write {}", output_path.display()),
    )?;

    Ok(MonolithRunResult::ExportVisualization(
        VisualizationExportSummary {
            output_path,
            host_count: payload.data.metadata.host_count,
            process_count: payload.data.metadata.process_count,
        },
    ))
}

fn print_host_process_topology_for_store<S>(
    store: S,
    host_id: Option<Uuid>,
) -> AppResult<MonolithRunResult>
where
    S: Clone + CatalogStore + RuntimeStore + GovernanceStore,
{
    let graph = build_host_process_topology_graph(store, host_id)?;
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload).source_err(
        AppReason::InputLoadFailed,
        "serialize host process topology",
    )?;
    Ok(MonolithRunResult::PrintJson(body))
}

async fn print_host_process_topology_for_store_async<S>(
    store: S,
    host_id: Option<Uuid>,
) -> AppResult<MonolithRunResult>
where
    S: Clone + AsyncCatalogStore + AsyncRuntimeStore + AsyncGovernanceStore,
{
    let graph = build_host_process_topology_graph_async(store, host_id).await?;
    let payload = VisualizationEnvelope {
        status: "ok",
        data: graph,
    };
    let body = serde_json::to_string_pretty(&payload).source_err(
        AppReason::InputLoadFailed,
        "serialize host process topology",
    )?;
    Ok(MonolithRunResult::PrintJson(body))
}

fn ingest_envelope_from_input(input: Value, tenant_id: TenantId) -> AppResult<IngestEnvelope> {
    if looks_like_atlas_input(&input) {
        let atlas_input: AtlasInputEnvelope = serde_json::from_value(input)
            .source_err(AppReason::InputLoadFailed, "parse atlas input")?;
        atlas_input.validate().conv_err()?;
        return Ok(atlas_input.into_ingest_envelope(tenant_id, None, Utc::now()));
    }

    Ok(IngestEnvelope {
        ingest_id: "demo-ingest-1".to_string(),
        source_kind: SourceKind::BatchImport,
        source_name: "monolith".to_string(),
        ingest_mode: IngestMode::BatchUpsert,
        tenant_id,
        environment_id: None,
        observed_at: None,
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
