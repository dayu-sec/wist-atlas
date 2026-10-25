#!/usr/bin/env python3
"""
replay_pg_jsonl.py

Reset the local wist-atlas PostgreSQL schema, replay one or more JSONL files
through atlas-app postgres-live mode, then query core row counts for quick
verification.

Usage:
    python3 scripts/replay_pg_jsonl.py \
      ../asset-twins-demo/warp-parse/data/out_dat/dayu-edge.jsonl \
      ../asset-twins-demo/warp-parse/data/out_dat/dayu-telemetry.jsonl
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB_CONTAINER = "wist-atlas-postgres"
DEFAULT_DB_NAME = "wist_atlas"
DEFAULT_DB_USER = "atlas"


def run(cmd: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(cmd))
    return subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        text=True,
        capture_output=True,
        check=False,
    )


def must_run(cmd: list[str], *, cwd: Path | None = None) -> str:
    result = run(cmd, cwd=cwd)
    if result.stdout:
        print(result.stdout, end="")
    if result.returncode != 0:
        if result.stderr:
            print(result.stderr, file=sys.stderr, end="")
        raise SystemExit(result.returncode)
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    return result.stdout


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("jsonl", nargs="+", help="JSONL files to replay")
    parser.add_argument(
        "--container",
        default=os.environ.get("WIST_ATLAS_PG_CONTAINER")
        or os.environ.get("DAYU_PG_CONTAINER", DEFAULT_DB_CONTAINER),
        help=f"PostgreSQL container name, default: {DEFAULT_DB_CONTAINER}",
    )
    parser.add_argument(
        "--db",
        default=os.environ.get("WIST_ATLAS_PG_DB")
        or os.environ.get("DAYU_PG_DB", DEFAULT_DB_NAME),
        help=f"PostgreSQL database name, default: {DEFAULT_DB_NAME}",
    )
    parser.add_argument(
        "--user",
        default=os.environ.get("WIST_ATLAS_PG_USER")
        or os.environ.get("DAYU_PG_USER", DEFAULT_DB_USER),
        help=f"PostgreSQL user, default: {DEFAULT_DB_USER}",
    )
    parser.add_argument(
        "--skip-reset",
        action="store_true",
        help="Do not drop and recreate public schema before replay",
    )
    return parser.parse_args()


def resolve_inputs(paths: list[str]) -> list[str]:
    resolved = []
    for raw in paths:
        path = Path(raw)
        if not path.is_absolute():
            path = (REPO_ROOT / raw).resolve()
        if not path.exists():
            raise SystemExit(f"input file not found: {path}")
        resolved.append(str(path))
    return resolved


def reset_schema(container: str, user: str, db: str) -> None:
    os.environ.setdefault(
        "DATABASE_URL",
        f"postgres://{user}:{user}@127.0.0.1:55432/{db}",
    )
    must_run(
        ["cargo", "run", "-q", "-p", "atlas-app", "--", "postgres-live", "reset-public"],
        cwd=REPO_ROOT,
    )


def replay_jsonl(files: list[str]) -> None:
    must_run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "atlas-app",
            "--",
            "postgres-live",
            "import-jsonl",
            *files,
        ],
        cwd=REPO_ROOT,
    )


def query_counts(container: str, user: str, db: str) -> None:
    sql = """
select 'host_inventory=' || count(*) from host_inventory
union all
select 'host_runtime_state=' || count(*) from host_runtime_state
union all
select 'process_runtime_state=' || count(*) from process_runtime_state
union all
select 'process_enriched=' || count(*) from process_runtime_state
  where process_state is not null or memory_rss_kib is not null
union all
select 'ingest_job=' || count(*) from ingest_job;
""".strip()
    must_run(
        [
            "docker",
            "exec",
            container,
            "psql",
            "-U",
            user,
            "-d",
            db,
            "-At",
            "-c",
            sql,
        ]
    )


def main() -> None:
    args = parse_args()
    files = resolve_inputs(args.jsonl)

    if not args.skip_reset:
        reset_schema(args.container, args.user, args.db)

    replay_jsonl(files)
    query_counts(args.container, args.user, args.db)


if __name__ == "__main__":
    main()
