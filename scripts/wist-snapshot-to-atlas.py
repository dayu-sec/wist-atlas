#!/usr/bin/env python3
"""Convert a wist-agentd discovery snapshot into an atlas.in.edge.v1 envelope.

Two input modes (P0 / P1):

1. `--state-dir`: read `state/discovery/{resources,targets,meta}.json` directly.
   This is the offline P0 path; no agentd/DSNAP changes needed.
2. `--report`: read a `ReportDiscoverySnapshot` JSON (the `DSNAP:` frame body).
   Use this once agentd starts sending the raw snapshot frame.

Usage:
    python3 scripts/wist-snapshot-to-atlas.py \
      --state-dir /tmp/wist-atlas-p0/state \
      --agent-id mac-local-01 \
      --tenant tenant-demo --env dev \
      --out /tmp/atlas-edge.json

    python3 scripts/wist-snapshot-to-atlas.py \
      --report /tmp/dsnap-body.json \
      --tenant tenant-demo --env dev \
      --out /tmp/atlas-edge.json --jsonl
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

HOST_KIND = "host"
IFACE_KIND = "network_interface"
ADDR_KIND = "ip_address"
PROCESS_KIND = "process"


def now_rfc3339() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="microseconds").replace("+00:00", "Z")


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def is_real_machine_id(host_id: str) -> bool:
    # agentd 在没有 /etc/machine-id 时退化成 `hostname:<name>`；那不是机器强标识。
    return bool(host_id) and not host_id.startswith("hostname:")


def meaningful_address(attrs: dict[str, str]) -> bool:
    ip = attrs.get("net.if.addr", "")
    scope = attrs.get("net.if.addr.scope", "")
    if not ip:
        return False
    if scope in {"link_local", "host"}:
        return False
    if ip.startswith("127.") or ip == "::1" or ip.startswith("fe80:") or ip.startswith("169.254."):
        return False
    return True


def build_host(resources: list[dict[str, Any]]) -> tuple[dict[str, Any], str, str | None]:
    for resource in resources:
        if resource.get("kind") != HOST_KIND:
            continue
        attrs = resource.get("attributes", {})
        host_id = attrs.get("host.id") or resource.get("resource_id", "")
        host_name = attrs.get("host.name") or host_id.removeprefix("hostname:")
        machine_id = host_id if is_real_machine_id(host_id) else None
        host = {
            "hostname": host_name,
            "external_ref": host_id,
        }
        if machine_id:
            host["machine_id"] = machine_id
        return host, host_name, machine_id
    raise SystemExit("snapshot has no host resource")


def build_interfaces(
    resources: list[dict[str, Any]], host_name: str, machine_id: str | None
) -> list[dict[str, Any]]:
    interfaces: dict[str, dict[str, Any]] = {}
    for resource in resources:
        if resource.get("kind") != IFACE_KIND:
            continue
        attrs = resource.get("attributes", {})
        name = attrs.get("net.if.name")
        if not name:
            continue
        iface = {
            "host_ref": host_name,
            "name": name,
            "addresses": [],
        }
        if machine_id:
            iface["machine_id"] = machine_id
        if mac := attrs.get("net.if.mac"):
            iface["mac"] = mac
        interfaces[name] = iface

    for resource in resources:
        if resource.get("kind") != ADDR_KIND:
            continue
        attrs = resource.get("attributes", {})
        name = attrs.get("net.if.name")
        if not name or not meaningful_address(attrs):
            continue
        iface = interfaces.setdefault(
            name,
            {
                "host_ref": host_name,
                "name": name,
                "addresses": [],
                **({"machine_id": machine_id} if machine_id else {}),
            },
        )
        ip = attrs["net.if.addr"]
        address = {
            "family": "ipv6" if ":" in ip else "ipv4",
            "ip": ip,
        }
        if prefix := attrs.get("net.if.prefix"):
            address["prefix"] = int(prefix)
        if cidr := attrs.get("net.if.cidr"):
            address["cidr"] = cidr
        if gateway := attrs.get("net.if.gateway"):
            address["gateway"] = gateway
        iface["addresses"].append(address)

    return [iface for iface in interfaces.values() if iface["addresses"]]


def build_processes(
    resources: list[dict[str, Any]],
    targets: list[dict[str, Any]],
    host_name: str,
    machine_id: str | None,
    observed_at: str,
) -> list[dict[str, Any]]:
    target_by_ref = {
        target.get("resource_ref"): target
        for target in targets
        if target.get("kind") == PROCESS_KIND
    }

    processes: dict[tuple[str, str], dict[str, Any]] = {}
    for resource in resources:
        if resource.get("kind") != PROCESS_KIND:
            continue
        attrs = resource.get("attributes", {})
        pid = attrs.get("process.pid")
        executable = attrs.get("process.executable.name")
        if not pid or not executable:
            continue

        target = target_by_ref.get(resource.get("resource_id"), {})
        hints = target.get("execution_hints", {})
        identity = hints.get("process.identity", "")

        process = {
            "pid": int(pid),
            "executable_name": executable,
            "host_ref": host_name,
            "observed_at": resource.get("discovered_at") or observed_at,
        }
        if machine_id:
            process["machine_id"] = machine_id
        if identity:
            process["identity"] = identity
        processes[(pid, identity)] = process

    return sorted(processes.values(), key=lambda item: (item["pid"], item["executable_name"]))


def build_envelope(
    resources: list[dict[str, Any]],
    targets: list[dict[str, Any]],
    agent_id: str,
    tenant: str,
    env: str | None,
    snapshot_id: str,
    revision: int | str,
    generated_at: str,
    collected_at: str,
) -> dict[str, Any]:
    host, host_name, machine_id = build_host(resources)
    interfaces = build_interfaces(resources, host_name, machine_id)
    processes = build_processes(resources, targets, host_name, machine_id, generated_at)

    source: dict[str, Any] = {
        "system": "wist",
        "producer": agent_id,
        "tenant": tenant,
    }
    if env:
        source["env"] = env

    return {
        "schema": "atlas.in.edge.v1",
        "source": source,
        "collect": {
            "mode": "snapshot",
            "snap_id": snapshot_id,
            "observed_at": generated_at,
            "collected_at": collected_at,
            "res_ver": str(revision),
        },
        "payload": {
            "hosts": [host],
            "interfaces": interfaces,
            "processes": processes,
        },
    }


def build_envelope_from_state(
    state_dir: Path, agent_id: str, tenant: str, env: str | None
) -> dict[str, Any]:
    discovery_dir = state_dir / "discovery"
    resources = load_json(discovery_dir / "resources.json")
    targets = load_json(discovery_dir / "targets.json")
    meta = load_json(discovery_dir / "meta.json")

    return build_envelope(
        resources,
        targets,
        agent_id,
        tenant,
        env,
        meta.get("snapshot_id", ""),
        meta.get("revision", 0),
        meta.get("generated_at", now_rfc3339()),
        now_rfc3339(),
    )


def build_envelope_from_report(
    report: dict[str, Any], agent_id: str | None, tenant: str, env: str | None
) -> dict[str, Any]:
    snapshot = report.get("snapshot")
    if not isinstance(snapshot, dict):
        raise SystemExit("report has no snapshot object")

    producer = agent_id or report.get("agent_id")
    if not producer:
        raise SystemExit("report has no agent_id; pass --agent-id")

    return build_envelope(
        snapshot.get("resources", []),
        snapshot.get("targets", []),
        producer,
        tenant,
        env,
        snapshot.get("snapshot_id") or report.get("snapshot_id", ""),
        snapshot.get("revision") or report.get("revision", 0),
        snapshot.get("generated_at") or report.get("generated_at", now_rfc3339()),
        report.get("reported_at") or now_rfc3339(),
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--state-dir", help="agentd config dir / state root")
    source.add_argument("--report", help="ReportDiscoverySnapshot JSON (DSNAP body)")
    parser.add_argument("--agent-id", default=None, help="override source.producer")
    parser.add_argument("--tenant", required=True, help="atlas source.tenant")
    parser.add_argument("--env", default=None, help="atlas source.env (optional)")
    parser.add_argument("--out", required=True, help="output path")
    parser.add_argument("--jsonl", action="store_true", help="emit one-line JSONL")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.state_dir:
        if not args.agent_id:
            raise SystemExit("--agent-id is required with --state-dir")
        envelope = build_envelope_from_state(
            Path(args.state_dir), args.agent_id, args.tenant, args.env
        )
    else:
        envelope = build_envelope_from_report(
            load_json(Path(args.report)), args.agent_id, args.tenant, args.env
        )

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    if args.jsonl:
        out.write_text(
            json.dumps(envelope, ensure_ascii=False, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )
    else:
        out.write_text(json.dumps(envelope, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    payload = envelope["payload"]
    print(
        f"wrote {out}: hosts={len(payload['hosts'])} "
        f"interfaces={len(payload['interfaces'])} processes={len(payload['processes'])}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
