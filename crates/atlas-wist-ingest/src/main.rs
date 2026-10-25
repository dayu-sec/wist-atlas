//! `atlas-wist-ingest` —— 消费网关 `agent-discovery.jsonl`，增量灌进 wist-atlas。
//!
//! ```text
//! atlas-wist-ingest --input <FILE> --checkpoint <FILE> --tenant <UUID> \
//!                   [--env <E>] [--interval <SECS>] [--once] [--store memory|postgres]
//! ```
//!
//! 与「开一个写端点让网关 POST 过来」的区别（选型记录）：
//!
//! - **不丢帧**：网关的 `http_sink` 只有 `max_retries = 3`，之后帧就没了；文件是追加写的，
//!   atlas 停机期间帧还在长，恢复后自动补。
//! - **不需要新端口/新鉴权**：信任边界就是文件权限；写 atlas 的只有 atlas 自己。
//! - 代价是要处理「读到哪了」：见 [`atlas_wist_ingest`] 里的偏移与轮转语义。
//!
//! 失败策略（决定检查点推不推进）：
//!
//! | 情况 | 处理 | 推进 |
//! |---|---|---|
//! | 行不是合法 JSON / 没有 `body` / 快照缺 host | 记一条错误，跳过 | **推进**（永远修不好） |
//! | 转换成功、写库失败 | 记一条错误，本轮停 | **不推进**（下一轮重试） |

use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use atlas_api::TopologyIngestService;
use atlas_domain::TenantId;
use atlas_wist_ingest::{DiscoveryLogTail, LineAt, convert_sink_line};

const USAGE: &str = "\
usage:
  atlas-wist-ingest --input <FILE> --checkpoint <FILE> --tenant <UUID>
                    [--env <E>] [--interval <SECS>] [--once] [--store memory|postgres]

notes:
  --input       网关的 agent-discovery.jsonl（wparse 的 agent-discovery sink）
  --checkpoint  偏移检查点（必须显式给：worker 的状态不该跟着 CWD 走）
  --tenant      租户 UUID（文件里没有租户，只能外部给）
  --env         覆盖行里的 env（不给就用行上的部署环境）
  --interval    轮询间隔秒，默认 30
  --once        只跑一轮就退出（排查 / cron）
  --store       memory（默认）| postgres（读 DATABASE_URL）";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("atlas-wist-ingest failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    let args = Args::parse(std::env::args().skip(1))?.validate()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| CliError::Message(format!("build runtime: {err}")))?;
    runtime.block_on(async move { run_loop(args).await })
}

async fn run_loop(args: Invocation) -> Result<(), CliError> {
    let mut tail = DiscoveryLogTail::open(&args.input, &args.checkpoint)
        .map_err(|err| CliError::Message(format!("open {}: {err}", args.input.display())))?;

    match args.store {
        StoreKind::Memory => {
            let service = TopologyIngestService::new_in_memory();
            loop {
                let report = poll_once(&mut tail, &service, &args).await?;
                log_report(&report, tail.offset());
                if args.once || !report.should_continue() {
                    break;
                }
                tokio::time::sleep(args.interval).await;
            }
        }
        StoreKind::Postgres => serve_postgres(tail, args).await?,
    }
    Ok(())
}

async fn serve_postgres(tail: DiscoveryLogTail, args: Invocation) -> Result<(), CliError> {
    use atlas_storage::PostgresTopologyStore;

    let url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("WIST_ATLAS_DATABASE_URL"))
        .map_err(|_| CliError::Message("--store postgres needs DATABASE_URL".into()))?;
    let executor = atlas_storage::LivePostgresExecutor::new(&url)
        .await
        .map_err(|err| CliError::Message(format!("connect postgres: {err}")))?;
    let store = PostgresTopologyStore::new(executor);
    store
        .run_migrations_async()
        .await
        .map_err(|err| CliError::Message(format!("run migrations: {err}")))?;

    let service = TopologyIngestService::new(store);
    let mut tail = tail;
    loop {
        let report = poll_once_async(&mut tail, &service, &args).await?;
        log_report(&report, tail.offset());
        if args.once || !report.should_continue() {
            break;
        }
        tokio::time::sleep(args.interval).await;
    }
    Ok(())
}

/// 本轮摘要。
#[derive(Debug, Default)]
struct Report {
    lines: usize,
    ingested: usize,
    skipped: usize,
    /// 撞上写库错误而提前结束。
    stopped: bool,
    processes_marked_stale: u64,
}

impl Report {
    fn should_continue(&self) -> bool {
        // 写库失败通常是数据面/数据库暂时不可用：继续轮询等它恢复。
        // 但如果整轮没有任何进展且一直失败，日志会持续提示，运维能看见。
        true
    }
}

fn log_report(report: &Report, offset: u64) {
    println!(
        "poll done: lines={} ingested={} skipped={} stale_marked={} offset={offset}{}",
        report.lines,
        report.ingested,
        report.skipped,
        report.processes_marked_stale,
        if report.stopped {
            " (stopped early)"
        } else {
            ""
        }
    );
}

async fn poll_once(
    tail: &mut DiscoveryLogTail,
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    args: &Invocation,
) -> Result<Report, CliError> {
    let lines = tail
        .poll()
        .map_err(|err| CliError::Message(format!("poll: {err}")))?;
    let mut report = Report::default();

    for line in &lines {
        report.lines += 1;
        match ingest_line(service, line, args) {
            Ok(outcome) => {
                report.ingested += 1;
                report.processes_marked_stale += outcome;
                tail.commit(line.next_offset)
                    .map_err(|err| CliError::Message(format!("commit: {err}")))?;
            }
            Err(LineError::Permanent(reason)) => {
                eprintln!("skip {}@{}: {reason}", args.input.display(), line.offset);
                report.skipped += 1;
                // 永久性错误（不是快照、没有 body）：留着只会每轮重试同一条。
                tail.commit(line.next_offset)
                    .map_err(|err| CliError::Message(format!("commit: {err}")))?;
            }
            Err(LineError::Transient(reason)) => {
                eprintln!(
                    "stop at {}@{}: {reason}（下一轮重试）",
                    args.input.display(),
                    line.offset
                );
                report.stopped = true;
                return Ok(report);
            }
        }
    }

    Ok(report)
}

async fn poll_once_async<S>(
    tail: &mut DiscoveryLogTail,
    service: &TopologyIngestService<S>,
    args: &Invocation,
) -> Result<Report, CliError>
where
    S: atlas_storage::AsyncCatalogStore
        + atlas_storage::AsyncRuntimeStore
        + atlas_storage::AsyncIngestStore
        + atlas_storage::AsyncGovernanceStore,
{
    let lines = tail
        .poll()
        .map_err(|err| CliError::Message(format!("poll: {err}")))?;
    let mut report = Report::default();

    for line in &lines {
        report.lines += 1;
        let input = match convert_sink_line(&line.text, &args.tenant, args.env.as_deref()) {
            Ok(input) => input,
            Err(reason) => {
                eprintln!("skip {}@{}: {reason}", args.input.display(), line.offset);
                report.skipped += 1;
                tail.commit(line.next_offset)
                    .map_err(|err| CliError::Message(format!("commit: {err}")))?;
                continue;
            }
        };

        match service
            .submit_atlas_input_and_materialize_async(input, TenantId(args.tenant_id), None)
            .await
        {
            Ok((_record, summary)) => {
                report.ingested += 1;
                report.processes_marked_stale += summary.stale_process_count;
                tail.commit(line.next_offset)
                    .map_err(|err| CliError::Message(format!("commit: {err}")))?;
            }
            Err(err) => {
                eprintln!(
                    "stop at {}@{}: {err}（下一轮重试）",
                    args.input.display(),
                    line.offset
                );
                report.stopped = true;
                return Ok(report);
            }
        }
    }

    Ok(report)
}

enum LineError {
    /// 永远修不好（格式问题）：跳过并推进。
    Permanent(String),
    /// 可能是暂时的（写库失败）：不推进，下一轮重试。
    Transient(String),
}

fn ingest_line(
    service: &TopologyIngestService<atlas_storage::InMemoryTopologyStore>,
    line: &LineAt,
    args: &Invocation,
) -> Result<u64, LineError> {
    let input = convert_sink_line(&line.text, &args.tenant, args.env.as_deref())
        .map_err(LineError::Permanent)?;

    service
        .submit_atlas_input_and_materialize(input, TenantId(args.tenant_id), None)
        .map(|(_record, summary)| summary.stale_process_count)
        .map_err(|err| LineError::Transient(err.to_string()))
}

// ── 参数 ────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Args {
    input: Option<PathBuf>,
    checkpoint: Option<PathBuf>,
    tenant: Option<String>,
    env: Option<String>,
    interval: Option<u64>,
    once: bool,
    store: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreKind {
    Memory,
    Postgres,
}

#[derive(Debug)]
struct Invocation {
    input: PathBuf,
    checkpoint: PathBuf,
    tenant: String,
    tenant_id: uuid::Uuid,
    env: Option<String>,
    interval: Duration,
    once: bool,
    store: StoreKind,
}

impl Args {
    fn parse(argv: impl Iterator<Item = String>) -> Result<Self, CliError> {
        let mut args = Args::default();
        let mut argv = argv.peekable();
        while let Some(flag) = argv.next() {
            match flag.as_str() {
                "--input" => args.input = Some(PathBuf::from(value_of(&flag, &mut argv)?)),
                "--checkpoint" => {
                    args.checkpoint = Some(PathBuf::from(value_of(&flag, &mut argv)?))
                }
                "--tenant" => args.tenant = Some(value_of(&flag, &mut argv)?),
                "--env" => args.env = Some(value_of(&flag, &mut argv)?),
                "--interval" => {
                    let raw = value_of(&flag, &mut argv)?;
                    args.interval = Some(raw.parse::<u64>().map_err(|_| {
                        CliError::Message(format!("--interval must be a number, got `{raw}`"))
                    })?);
                }
                "--store" => args.store = Some(value_of(&flag, &mut argv)?),
                "--once" => args.once = true,
                "--help" | "-h" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                other => {
                    return Err(CliError::Message(format!(
                        "unrecognized argument `{other}`\n\n{USAGE}"
                    )));
                }
            }
        }
        Ok(args)
    }

    fn validate(self) -> Result<Invocation, CliError> {
        let tenant = self
            .tenant
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CliError::Message(format!("--tenant is required\n\n{USAGE}")))?;
        let tenant_id = uuid::Uuid::parse_str(&tenant)
            .map_err(|_| CliError::Message(format!("--tenant must be a UUID, got `{tenant}`")))?;

        let store = match self.store.as_deref() {
            None | Some("memory") => StoreKind::Memory,
            Some("postgres") => StoreKind::Postgres,
            Some(other) => {
                return Err(CliError::Message(format!(
                    "--store must be memory or postgres, got `{other}`"
                )));
            }
        };

        Ok(Invocation {
            input: require_path("--input", self.input)?,
            checkpoint: require_path("--checkpoint", self.checkpoint)?,
            tenant,
            tenant_id,
            env: self.env.filter(|value| !value.trim().is_empty()),
            interval: Duration::from_secs(self.interval.unwrap_or(30).max(1)),
            once: self.once,
            store,
        })
    }
}

fn value_of(flag: &str, argv: &mut impl Iterator<Item = String>) -> Result<String, CliError> {
    argv.next()
        .ok_or_else(|| CliError::Message(format!("`{flag}` needs a value")))
}

fn require_path(flag: &str, value: Option<PathBuf>) -> Result<PathBuf, CliError> {
    value
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| CliError::Message(format!("{flag} is required\n\n{USAGE}")))
}

#[derive(Debug)]
enum CliError {
    Message(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}
