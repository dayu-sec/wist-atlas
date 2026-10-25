pub mod error;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use atlas_storage::PostgresTopologyStore;
use atlas_storage::{InMemoryTopologyStore, LivePostgresExecutor, MemoryPostgresExecutor};
use atlas_sync::JsonlImportSummary;
use orion_error::conversion::{ConvErr, ToStructError};
use serde::Serialize;
use uuid::Uuid;

pub use error::{AppError, AppReason, AppResult};
use error::{invalid_args, materialization_missing};

pub enum TopologyMonolith {
    Memory(InMemoryTopologyStore),
    PostgresMock(PostgresTopologyStore<MemoryPostgresExecutor>),
    PostgresLive(PostgresTopologyStore<LivePostgresExecutor>),
}

pub struct TopologyAppBuilder {
    mode: MonolithMode,
}

pub struct MonolithRunSummary {
    pub ingest_id: String,
    pub host_name: String,
    pub network_name: Option<String>,
    pub assoc_ip: Option<String>,
    pub responsibilities: Vec<String>,
}

pub type JsonlReplaySummary = JsonlImportSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonolithMode {
    Memory,
    PostgresMock,
    PostgresLive,
}

pub enum MonolithInput {
    Demo,
    File(PathBuf),
    JsonlFiles(Vec<PathBuf>),
    ServeJsonlFiles { listen: String, paths: Vec<PathBuf> },
    ResetPublic,
    ReplaceJsonlFiles(Vec<PathBuf>),
    ExportVisualization(PathBuf),
    PrintHostProcessTopology(Uuid),
    PrintFirstHostProcessTopology,
    Serve { listen: String },
}

pub enum MonolithRunResult {
    Single(MonolithRunSummary),
    Replay(JsonlReplaySummary),
    Reset(String),
    ExportVisualization(VisualizationExportSummary),
    PrintJson(String),
    Serve { listen: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VisualizationExportSummary {
    pub output_path: PathBuf,
    pub host_count: usize,
    pub process_count: usize,
}

mod http;
use http::{HttpAppState, build_http_router};

mod graph;
mod run;
use run::{run_with_postgres_store, run_with_postgres_store_async, run_with_store};

impl TopologyMonolith {
    pub fn new_in_memory() -> Self {
        Self::Memory(InMemoryTopologyStore::default())
    }

    pub fn new_postgres_mock() -> AppResult<Self> {
        let store = PostgresTopologyStore::new(MemoryPostgresExecutor::default());
        store.run_migrations().conv_err()?;
        Ok(Self::PostgresMock(store))
    }

    pub async fn new_postgres_live(database_url: impl Into<String>) -> AppResult<Self> {
        let executor = LivePostgresExecutor::new(database_url).await.conv_err()?;
        let store = PostgresTopologyStore::new(executor);
        store.run_migrations_async().await.conv_err()?;
        Ok(Self::PostgresLive(store))
    }

    pub fn run(&self, input: MonolithInput) -> AppResult<MonolithRunResult> {
        match self {
            Self::Memory(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http(listen),
                MonolithInput::ServeJsonlFiles { listen, paths } => {
                    let tenant_id = atlas_domain::TenantId(Uuid::new_v4());
                    run::run_with_jsonl_files(store.clone(), tenant_id, paths)?;
                    self.serve_http(listen)
                }
                other => run_with_store(store.clone(), other),
            },
            Self::PostgresMock(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http(listen),
                MonolithInput::ServeJsonlFiles { listen, paths } => {
                    let tenant_id = atlas_domain::TenantId(Uuid::new_v4());
                    run::run_with_jsonl_files(store.clone(), tenant_id, paths)?;
                    self.serve_http(listen)
                }
                other => run_with_postgres_store(store.clone(), other),
            },
            Self::PostgresLive(_) => Err(materialization_missing(
                "postgres-live requires async run entrypoint",
            )),
        }
    }

    pub fn serve_http(&self, listen: impl Into<String>) -> AppResult<MonolithRunResult> {
        let listen = listen.into();
        let addr: SocketAddr = listen.parse().map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("parse listen address {listen}: {err}"))
        })?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                AppReason::InputLoadFailed
                    .to_err()
                    .with_detail(format!("build tokio runtime: {err}"))
            })?;

        let state = HttpAppState::new(self.clone());
        runtime.block_on(async move {
            let router = build_http_router(state);

            let listener = tokio::net::TcpListener::bind(addr).await.map_err(|err| {
                AppReason::InputLoadFailed
                    .to_err()
                    .with_detail(format!("bind {addr}: {err}"))
            })?;
            axum::serve(listener, router).await.map_err(|err| {
                AppReason::InputLoadFailed
                    .to_err()
                    .with_detail(format!("serve http: {err}"))
            })?;
            Ok::<(), AppError>(())
        })?;

        Ok(MonolithRunResult::Serve { listen })
    }

    pub fn run_demo(&self) -> AppResult<MonolithRunSummary> {
        match self.run(MonolithInput::Demo)? {
            MonolithRunResult::Single(summary) => Ok(summary),
            MonolithRunResult::Replay(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Reset(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected single run summary"))
            }
        }
    }

    pub fn run_file(&self, path: impl AsRef<Path>) -> AppResult<MonolithRunSummary> {
        match self.run(MonolithInput::File(path.as_ref().to_path_buf()))? {
            MonolithRunResult::Single(summary) => Ok(summary),
            MonolithRunResult::Replay(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Reset(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected single run summary"))
            }
        }
    }

    pub fn replay_jsonl(&self, path: impl AsRef<Path>) -> AppResult<JsonlReplaySummary> {
        match self.run(MonolithInput::JsonlFiles(vec![path.as_ref().to_path_buf()]))? {
            MonolithRunResult::Replay(summary) => Ok(summary),
            MonolithRunResult::Single(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::Reset(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected replay summary"))
            }
        }
    }

    pub fn replay_jsonl_files(&self, paths: Vec<PathBuf>) -> AppResult<JsonlReplaySummary> {
        match self.run(MonolithInput::JsonlFiles(paths))? {
            MonolithRunResult::Replay(summary) => Ok(summary),
            MonolithRunResult::Single(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::Reset(_) => Err(materialization_missing("expected replay summary")),
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected replay summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected replay summary"))
            }
        }
    }

    pub async fn run_async(&self, input: MonolithInput) -> AppResult<MonolithRunResult> {
        match self {
            Self::Memory(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http_async(listen).await,
                MonolithInput::ServeJsonlFiles { listen, paths } => {
                    let tenant_id = atlas_domain::TenantId(Uuid::new_v4());
                    run::run_with_jsonl_files(store.clone(), tenant_id, paths)?;
                    self.serve_http_async(listen).await
                }
                other => run_with_store(store.clone(), other),
            },
            Self::PostgresMock(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http_async(listen).await,
                MonolithInput::ServeJsonlFiles { listen, paths } => {
                    let tenant_id = atlas_domain::TenantId(Uuid::new_v4());
                    run::run_with_jsonl_files(store.clone(), tenant_id, paths)?;
                    self.serve_http_async(listen).await
                }
                other => run_with_postgres_store(store.clone(), other),
            },
            Self::PostgresLive(store) => match input {
                MonolithInput::Serve { listen } => self.serve_http_async(listen).await,
                MonolithInput::ServeJsonlFiles { listen, paths } => {
                    let tenant_id = atlas_domain::TenantId(Uuid::new_v4());
                    atlas_sync::JsonlImportService::new(store.clone())
                        .import_files_async(tenant_id, &paths)
                        .await
                        .conv_err()?;
                    self.serve_http_async(listen).await
                }
                other => run_with_postgres_store_async(store.clone(), other).await,
            },
        }
    }

    pub async fn serve_http_async(
        &self,
        listen: impl Into<String>,
    ) -> AppResult<MonolithRunResult> {
        let listen = listen.into();
        let addr: SocketAddr = listen.parse().map_err(|err| {
            AppReason::InvalidArgs
                .to_err()
                .with_detail(format!("parse listen address {listen}: {err}"))
        })?;

        let state = HttpAppState::new(self.clone());
        let router = build_http_router(state);

        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|err| {
            AppReason::InputLoadFailed
                .to_err()
                .with_detail(format!("bind {addr}: {err}"))
        })?;
        axum::serve(listener, router).await.map_err(|err| {
            AppReason::InputLoadFailed
                .to_err()
                .with_detail(format!("serve http: {err}"))
        })?;

        Ok(MonolithRunResult::Serve { listen })
    }

    pub async fn run_demo_async(&self) -> AppResult<MonolithRunSummary> {
        match self.run_async(MonolithInput::Demo).await? {
            MonolithRunResult::Single(summary) => Ok(summary),
            MonolithRunResult::Replay(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Reset(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::ExportVisualization(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::PrintJson(_) => {
                Err(materialization_missing("expected single run summary"))
            }
            MonolithRunResult::Serve { .. } => {
                Err(materialization_missing("expected single run summary"))
            }
        }
    }

    pub fn store(&self) -> Option<&InMemoryTopologyStore> {
        match self {
            Self::Memory(store) => Some(store),
            Self::PostgresMock(_) | Self::PostgresLive(_) => None,
        }
    }
}

impl Clone for TopologyMonolith {
    fn clone(&self) -> Self {
        match self {
            Self::Memory(store) => Self::Memory(store.clone()),
            Self::PostgresMock(store) => Self::PostgresMock(store.clone()),
            Self::PostgresLive(store) => Self::PostgresLive(store.clone()),
        }
    }
}

impl TopologyAppBuilder {
    pub fn new() -> Self {
        Self {
            mode: MonolithMode::Memory,
        }
    }

    pub fn with_mode(mut self, mode: MonolithMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn build(self) -> AppResult<TopologyMonolith> {
        match self.mode {
            MonolithMode::Memory => Ok(TopologyMonolith::new_in_memory()),
            MonolithMode::PostgresMock => TopologyMonolith::new_postgres_mock(),
            MonolithMode::PostgresLive => Err(materialization_missing(
                "postgres-live requires async build entrypoint",
            )),
        }
    }

    pub async fn build_async(self) -> AppResult<TopologyMonolith> {
        match self.mode {
            MonolithMode::Memory => Ok(TopologyMonolith::new_in_memory()),
            MonolithMode::PostgresMock => TopologyMonolith::new_postgres_mock(),
            MonolithMode::PostgresLive => {
                TopologyMonolith::new_postgres_live(resolve_database_url()).await
            }
        }
    }
}

impl Default for TopologyAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

mod cli;
pub use cli::parse_monolith_input;
use cli::resolve_database_url;

mod payload_io;

#[cfg(test)]
mod tests;
