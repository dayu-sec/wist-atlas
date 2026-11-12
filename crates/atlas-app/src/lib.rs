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

mod cells;

/// 只给 OpenAPI 用的文档 DTO（与 domain 同形状，不参与序列化）。
mod http_schema;

mod ingest_http;

mod probe;

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

        // 多客户：按客户清单连库。**没有清单就不启动**（0.5.0 去掉了默认库 ——
        // 以前那条「没配清单就落 `DATABASE_URL` 指向的库」已经删了：它会遮住
        // 「客户没登记」这种现场问题，把客户拓扑写进一个没人看的库）。
        //
        // **必须先做这一步**：`self`（入口构造的 monolith）在 `serve` 下只是个占位，
        // 真正连库就发生在这里；先连客户库再把状态包成 `cells_only`，就不存在
        // 「以为是默认库、其实是占位 store」的中间态。
        let cells = cells::CellRouter::from_env().await?.ok_or_else(|| {
            AppReason::InvalidArgs.to_err().with_detail(format!(
                "no customer cells configured: the default database was removed in 0.5.0 — \
                 register at least one customer ({} must list them; on a wist-field \
                 stack: sys/scripts/add-customer.sh <cust_id> && gops sys localize)",
                cells::CELLS_FILE_ENV
            ))
        })?;

        // 写探针：默认关，`WIST_ATLAS_PROBE=on` 才开（与 evidence 的 `[probe].enabled` 同一条纪律）。
        // 开启后每 `WIST_ATLAS_PROBE_INTERVAL_SECS`（默认 300）**对每个客户各写一份**，
        // 写进**那个客户自己的库**（0.5.0 之后已经没有可以放“保留租户”的公共库了）。
        let probe = if std::env::var("WIST_ATLAS_PROBE").as_deref() == Ok("on") {
            let interval_ms = std::env::var("WIST_ATLAS_PROBE_INTERVAL_SECS")
                .ok()
                .and_then(|raw| raw.trim().parse::<u64>().ok())
                .unwrap_or(300);
            // 0 会让探针变成死循环狂打自己的库。
            let interval_secs = interval_ms.max(1);
            let shared = probe::new_shared();
            // targets 从刚连好的客户路由里取：探针写的库与 HTTP 写路径是同一个。
            probe::spawn(
                probe_targets(&cells),
                shared.clone(),
                std::time::Duration::from_secs(interval_secs),
            );
            Some(shared)
        } else {
            None
        };

        // `self` 不参与：serve 的库全部来自客户清单，没有“默认库”这一档。
        let mut state = HttpAppState::cells_only(cells);
        // 读接口要令牌：`serve` 是唯一的生产入口，拿不到就直接失败，不默默降级成“谁都能读”。
        // 与 `wist-field-api` 的“没有不校验这一档”同一条纪律。
        let token = std::env::var("WIST_ATLAS_TOKEN")
            .ok()
            .map(|raw| raw.trim().to_string())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                AppReason::InvalidArgs.to_err().with_detail(
                    "WIST_ATLAS_TOKEN is required: read endpoints (/api/*) need `Authorization: Bearer <token>`. \
                     /healthz and the write endpoint /internal/atlas stay open."
                        .to_string(),
                )
            })?;
        state = state.with_token(token);

        if let Some(default) = cells::default_tenant() {
            state = state.with_default_tenant(default);
        }

        if let Some(probe) = probe {
            state = state.with_probe(probe);
        }
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

    /// 按 CLI 输入构造这次运行的 monolith —— `main` 只用这一个入口。
    ///
    /// `Serve` 是唯一**不该碰默认库**的输入：客户的库由 `serve_http_async` 里的
    /// 客户清单连，进程启动时先连一个“默认库”只会在没有 `DATABASE_URL` 的栈里
    /// 无限重启（0.5.0 的现场故障）。所以 `Serve` 一律返回内存占位：
    /// 它只承载 `run_async` 的 `Serve` 分支，`HttpAppState` 在 `serve` 里由
    /// `cells_only` 构造，占位 app 永远不会被 resolve 到。
    ///
    /// 其余输入（`file` / `replay-jsonl` / `import-jsonl` / `demo` …）确实需要
    /// 一个库，模式照旧决定连哪个。
    pub async fn build_for_input(self, input: &MonolithInput) -> AppResult<TopologyMonolith> {
        match input {
            MonolithInput::Serve { .. } => Ok(TopologyMonolith::new_in_memory()),
            _ => match self.mode {
                MonolithMode::PostgresLive => self.build_async().await,
                MonolithMode::Memory | MonolithMode::PostgresMock => self.build(),
            },
        }
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

// 供 `serve_http_async` 用：把客户路由摊成探针目标（每个客户一个）。
fn probe_targets(cells: &cells::CellRouter) -> Vec<probe::Target> {
    cells
        .iter()
        .map(|(cust_id, cell)| probe::Target {
            cust_id: cust_id.to_string(),
            tenant: cell.tenant,
            app: cell.app.clone(),
        })
        .collect()
}

mod cli;
pub use cli::parse_monolith_input;
use cli::resolve_database_url;

mod payload_io;

#[cfg(test)]
mod tests;
