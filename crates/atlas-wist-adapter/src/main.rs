//! `atlas-wist-adapter` —— 把 wist-agentd 的发现快照转成 `atlas.in.edge.v1`。
//!
//! ```text
//! atlas-wist-adapter --state-dir <DIR> --agent-id <ID> --tenant <T> --out <PATH> [--env <E>] [--jsonl]
//! atlas-wist-adapter --report <JSON>  [--agent-id <ID>] --tenant <T> --out <PATH> [--env <E>] [--jsonl]
//! ```
//!
//! 两个输入口互斥：
//!
//! - `--state-dir`：读 agentd 的 **state 根**（其下有 `discovery/{resources,targets,meta}.json`）。
//!   离线路径，不需要 agentd 发包，也不需要网关。`--agent-id` 必填（state 里没有身份）。
//! - `--report`：读一帧 `DSNAP:` 的正文（`ReportDiscoverySnapshot`）。
//!   `--agent-id` 可省 —— 快照自带 `agent_id`。
//!
//! 不加 `--jsonl` 时输出缩进 JSON（人看 / 进 git）；加了是单行（喂 `replay-jsonl`）。

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use atlas_wist_adapter::{
    AdapterError, EdgeEnvelope, envelope_from_report_json, envelope_from_state_dir,
};

const USAGE: &str = "\
usage:
  atlas-wist-adapter --state-dir <DIR>  --agent-id <ID> --tenant <T> --out <PATH> [--env <E>] [--jsonl]
  atlas-wist-adapter --report <JSON>    [--agent-id <ID>] --tenant <T> --out <PATH> [--env <E>] [--jsonl]

notes:
  --state-dir  agentd 的 state 根（其下有 discovery/resources.json）
  --report     一帧 DSNAP: 的正文（ReportDiscoverySnapshot JSON）
  --jsonl      输出单行 JSON（可直接喂 wist-atlas replay-jsonl）";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("atlas-wist-adapter failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    let args = Args::parse(std::env::args().skip(1))?.validate()?;

    let envelope = match (&args.state_dir, &args.report) {
        (Some(state_dir), None) => {
            let agent_id = args.agent_id.as_deref().ok_or_else(|| {
                CliError::Message("--agent-id is required with --state-dir".into())
            })?;
            envelope_from_state_dir(state_dir, agent_id, &args.tenant, args.env.as_deref())?
        }
        (None, Some(report_path)) => {
            let raw = fs::read_to_string(report_path).map_err(|source| AdapterError::Read {
                path: report_path.clone(),
                source,
            })?;
            envelope_from_report_json(
                &raw,
                args.agent_id.as_deref(),
                &args.tenant,
                args.env.as_deref(),
            )?
        }
        _ => {
            return Err(CliError::Message(
                "exactly one of --state-dir / --report is required".into(),
            ));
        }
    };

    write_envelope(&envelope, &args.out, args.jsonl)?;

    let payload = &envelope.payload;
    println!(
        "wrote {}: hosts={} interfaces={} processes={}",
        args.out.display(),
        payload.hosts.len(),
        payload.interfaces.len(),
        payload.processes.len()
    );
    Ok(())
}

fn write_envelope(envelope: &EdgeEnvelope, out: &Path, jsonl: bool) -> Result<(), AdapterError> {
    let rendered = if jsonl {
        serde_json::to_string(envelope)
    } else {
        serde_json::to_string_pretty(envelope)
    }
    .map_err(|source| AdapterError::Parse {
        path: out.to_path_buf(),
        source,
    })?;

    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| AdapterError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(out, format!("{rendered}\n")).map_err(|source| AdapterError::Write {
        path: out.to_path_buf(),
        source,
    })
}

// ── 参数 ────────────────────────────────────────────────────

/// 原始参数：全部 `Option`，校验前不假定任何一项存在。
#[derive(Debug, Default)]
struct Args {
    state_dir: Option<PathBuf>,
    report: Option<PathBuf>,
    agent_id: Option<String>,
    tenant: Option<String>,
    env: Option<String>,
    out: Option<PathBuf>,
    jsonl: bool,
}

/// 校验后的调用：必填项已是具体类型，后面不用再 `unwrap`。
#[derive(Debug)]
struct Invocation {
    state_dir: Option<PathBuf>,
    report: Option<PathBuf>,
    agent_id: Option<String>,
    tenant: String,
    env: Option<String>,
    out: PathBuf,
    jsonl: bool,
}

impl Args {
    fn parse(argv: impl Iterator<Item = String>) -> Result<Self, CliError> {
        let mut args = Args::default();
        let mut argv = argv.peekable();

        while let Some(flag) = argv.next() {
            match flag.as_str() {
                "--state-dir" => args.state_dir = Some(PathBuf::from(value_of(&flag, &mut argv)?)),
                "--report" => args.report = Some(PathBuf::from(value_of(&flag, &mut argv)?)),
                "--agent-id" => args.agent_id = Some(value_of(&flag, &mut argv)?),
                "--tenant" => args.tenant = Some(value_of(&flag, &mut argv)?),
                "--env" => args.env = Some(value_of(&flag, &mut argv)?),
                "--out" => args.out = Some(PathBuf::from(value_of(&flag, &mut argv)?)),
                "--jsonl" => args.jsonl = true,
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
        if self.state_dir.is_some() && self.report.is_some() {
            return Err(CliError::Message(
                "--state-dir and --report are mutually exclusive".into(),
            ));
        }
        if self.state_dir.is_none() && self.report.is_none() {
            return Err(CliError::Message(format!(
                "one of --state-dir / --report is required\n\n{USAGE}"
            )));
        }

        Ok(Invocation {
            state_dir: self.state_dir,
            report: self.report,
            agent_id: self.agent_id,
            tenant: require("--tenant", self.tenant)?,
            env: self.env,
            out: require_path("--out", self.out)?,
            jsonl: self.jsonl,
        })
    }
}

fn value_of(flag: &str, argv: &mut impl Iterator<Item = String>) -> Result<String, CliError> {
    argv.next()
        .ok_or_else(|| CliError::Message(format!("`{flag}` needs a value")))
}

fn require(flag: &str, value: Option<String>) -> Result<String, CliError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CliError::Message(format!("{flag} is required\n\n{USAGE}")))
}

fn require_path(flag: &str, value: Option<PathBuf>) -> Result<PathBuf, CliError> {
    value
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| CliError::Message(format!("{flag} is required\n\n{USAGE}")))
}

#[derive(Debug)]
enum CliError {
    Adapter(AdapterError),
    Message(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adapter(err) => write!(f, "{err}"),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<AdapterError> for CliError {
    fn from(err: AdapterError) -> Self {
        Self::Adapter(err)
    }
}
