use std::path::PathBuf;

use orion_error::conversion::ToStructError;
use uuid::Uuid;

use crate::{AppReason, AppResult, MonolithInput, MonolithMode, invalid_args};

pub fn parse_monolith_input(args: &[String]) -> AppResult<(MonolithMode, MonolithInput)> {
    match args {
        [] => Ok((MonolithMode::Memory, MonolithInput::Demo)),
        [cmd] if cmd == "demo" => Ok((MonolithMode::Memory, MonolithInput::Demo)),
        [cmd, path] if cmd == "file" => Ok((
            MonolithMode::Memory,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [cmd] if cmd == "postgres-mock" => Ok((MonolithMode::PostgresMock, MonolithInput::Demo)),
        [mode, cmd, path] if mode == "postgres-mock" && cmd == "file" => Ok((
            MonolithMode::PostgresMock,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [cmd] if cmd == "postgres-live" => Ok((MonolithMode::PostgresLive, MonolithInput::Demo)),
        [mode, cmd, path] if mode == "postgres-live" && cmd == "file" => Ok((
            MonolithMode::PostgresLive,
            MonolithInput::File(PathBuf::from(path)),
        )),
        [mode, cmd] if mode == "postgres-mock" && cmd == "reset-public" => {
            Ok((MonolithMode::PostgresMock, MonolithInput::ResetPublic))
        }
        [mode, cmd] if mode == "postgres-live" && cmd == "reset-public" => {
            Ok((MonolithMode::PostgresLive, MonolithInput::ResetPublic))
        }
        [mode, cmd, path] if mode == "postgres-mock" && cmd == "export-visualization" => Ok((
            MonolithMode::PostgresMock,
            MonolithInput::ExportVisualization(PathBuf::from(path)),
        )),
        [mode, cmd, path] if mode == "postgres-live" && cmd == "export-visualization" => Ok((
            MonolithMode::PostgresLive,
            MonolithInput::ExportVisualization(PathBuf::from(path)),
        )),
        [mode, cmd] if mode == "postgres-live" && cmd == "print-first-host-process-topology" => {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::PrintFirstHostProcessTopology,
            ))
        }
        [mode, cmd, host_id] if mode == "postgres-live" && cmd == "print-host-process-topology" => {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::PrintHostProcessTopology(parse_uuid_arg(host_id)?),
            ))
        }
        [mode, cmd] if mode == "postgres-mock" && cmd == "print-first-host-process-topology" => {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::PrintFirstHostProcessTopology,
            ))
        }
        [mode, cmd, host_id] if mode == "postgres-mock" && cmd == "print-host-process-topology" => {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::PrintHostProcessTopology(parse_uuid_arg(host_id)?),
            ))
        }
        [mode, cmd, flag, listen]
            if (mode == "postgres-live" || mode == "postgres-mock" || mode == "memory")
                && cmd == "serve"
                && flag == "--listen" =>
        {
            let mode = match mode.as_str() {
                "postgres-live" => MonolithMode::PostgresLive,
                "postgres-mock" => MonolithMode::PostgresMock,
                "memory" => MonolithMode::Memory,
                _ => return Err(invalid_args()),
            };
            Ok((
                mode,
                MonolithInput::Serve {
                    listen: listen.clone(),
                },
            ))
        }
        [mode, cmd, paths @ ..]
            if (mode == "postgres-live" || mode == "postgres-mock" || mode == "memory")
                && cmd == "serve-jsonl" =>
        {
            if paths.len() < 3 {
                return Err(invalid_args());
            }
            let listen_flag_index = paths.len() - 2;
            if paths[listen_flag_index] != "--listen" {
                return Err(invalid_args());
            }
            let mode = match mode.as_str() {
                "postgres-live" => MonolithMode::PostgresLive,
                "postgres-mock" => MonolithMode::PostgresMock,
                "memory" => MonolithMode::Memory,
                _ => return Err(invalid_args()),
            };
            Ok((
                mode,
                MonolithInput::ServeJsonlFiles {
                    listen: paths[listen_flag_index + 1].clone(),
                    paths: paths[..listen_flag_index]
                        .iter()
                        .map(PathBuf::from)
                        .collect(),
                },
            ))
        }
        [cmd, paths @ ..] if cmd == "replay-jsonl" && !paths.is_empty() => Ok((
            MonolithMode::Memory,
            MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
        )),
        [cmd, paths @ ..] if cmd == "import-jsonl" && !paths.is_empty() => Ok((
            MonolithMode::Memory,
            MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
        )),
        [mode, cmd, paths @ ..]
            if mode == "postgres-mock" && cmd == "replay-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-mock" && cmd == "import-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-mock" && cmd == "replace-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresMock,
                MonolithInput::ReplaceJsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-live" && cmd == "replay-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-live" && cmd == "import-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::JsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        [mode, cmd, paths @ ..]
            if mode == "postgres-live" && cmd == "replace-jsonl" && !paths.is_empty() =>
        {
            Ok((
                MonolithMode::PostgresLive,
                MonolithInput::ReplaceJsonlFiles(paths.iter().map(PathBuf::from).collect()),
            ))
        }
        _ => Err(invalid_args()),
    }
}

pub(crate) fn resolve_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("WIST_ATLAS_DATABASE_URL"))
        // 兼容旧名一个版本，便于已有开发环境无痛切换。
        .or_else(|_| std::env::var("DAYU_TOPOLOGY_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://atlas:atlas@127.0.0.1:55432/wist_atlas".to_string())
}

pub(crate) fn parse_uuid_arg(value: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        AppReason::InvalidArgs
            .to_err()
            .with_detail(format!("parse uuid argument {value}: {err}"))
    })
}
