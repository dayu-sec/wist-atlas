#!/usr/bin/env python3
"""`wist-snapshot-to-atlas.py` 的映射规则测试。

只用标准库（`python3 -m unittest discover -s scripts -p 'test_*.py'`），
不引入 pytest —— 这个脚本是 P0/P1 联调工具，不该为它加依赖。

覆盖的是**适配器自己的判断**（身份回退、地址过滤、identity 来源、去重排序），
不是 `atlas.in.edge.v1` 的 schema —— 后者由 `atlas-api` 的 Rust 测试守。
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

_SCRIPT = Path(__file__).with_name("wist-snapshot-to-atlas.py")
_spec = importlib.util.spec_from_file_location("wist_snapshot_to_atlas", _SCRIPT)
assert _spec and _spec.loader
adapter = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = adapter
_spec.loader.exec_module(adapter)

OBSERVED_AT = "2026-10-25T07:56:34.061342Z"


def resource(resource_id: str, kind: str, **attributes: str) -> dict:
    return {
        "resource_id": resource_id,
        "kind": kind,
        "origin_idx": 0,
        "attributes": attributes,
        "discovered_at": OBSERVED_AT,
        "last_seen_at": OBSERVED_AT,
        "health": "healthy",
        "source": kind,
    }


def target(resource_id: str, kind: str, **hints: str) -> dict:
    return {
        "target_id": f"target:{resource_id}",
        "kind": kind,
        "origin_idx": 0,
        "resource_ref": resource_id,
        "execution_hints": hints,
        "state": "ready",
    }


def host(**attributes: str) -> dict:
    attrs = {"host.id": "machine-abc", "host.name": "node-1", **attributes}
    return resource(attrs["host.id"], "host", **attrs)


def envelope(resources: list[dict], targets: list[dict] | None = None) -> dict:
    return adapter.build_envelope(
        resources,
        targets or [],
        "agent-1",
        "tenant-demo",
        "dev",
        "snap-1",
        7,
        OBSERVED_AT,
        OBSERVED_AT,
    )


class HostIdentityTest(unittest.TestCase):
    def test_real_machine_id_is_used_as_both_ref_and_machine_id(self):
        payload = envelope([host()])["payload"]
        self.assertEqual(payload["hosts"][0]["external_ref"], "machine-abc")
        self.assertEqual(payload["hosts"][0]["machine_id"], "machine-abc")
        self.assertEqual(payload["hosts"][0]["hostname"], "node-1")

    def test_hostname_fallback_is_not_promoted_to_machine_id(self):
        # agentd 没有 /etc/machine-id 时退化成 `hostname:<name>`：那不是机器强标识，
        # 当 machine_id 用会让「同名不同机」在中心侧合并成一台。
        payload = envelope(
            [resource("hostname:mac-01", "host", **{"host.name": "mac-01"})]
        )["payload"]
        self.assertEqual(payload["hosts"][0]["external_ref"], "hostname:mac-01")
        self.assertNotIn("machine_id", payload["hosts"][0])

    def test_host_name_falls_back_to_the_resource_id(self):
        payload = envelope([resource("hostname:mac-01", "host")])["payload"]
        self.assertEqual(payload["hosts"][0]["hostname"], "mac-01")

    def test_a_snapshot_without_a_host_resource_is_rejected(self):
        with self.assertRaises(SystemExit):
            adapter.build_envelope(
                [resource("if-1", "network_interface", **{"net.if.name": "en0"})],
                [],
                "agent-1",
                "tenant-demo",
                None,
                "snap-1",
                1,
                OBSERVED_AT,
                OBSERVED_AT,
            )


class InterfaceTest(unittest.TestCase):
    def test_link_local_and_loopback_addresses_are_dropped(self):
        resources = [
            host(),
            resource(
                "if-0",
                "network_interface",
                **{"net.if.name": "lo0", "net.if.mac": "00:00:00:00:00:00"},
            ),
            resource("addr-0", "ip_address", **{"net.if.name": "lo0", "net.if.addr": "127.0.0.1"}),
            resource(
                "addr-1",
                "ip_address",
                **{"net.if.name": "lo0", "net.if.addr": "::1"},
            ),
            resource(
                "addr-2",
                "ip_address",
                **{"net.if.name": "lo0", "net.if.addr": "fe80::1", "net.if.addr.scope": "link_local"},
            ),
            resource(
                "addr-3",
                "ip_address",
                **{"net.if.name": "lo0", "net.if.addr": "169.254.1.1"},
            ),
        ]
        # 只剩回环/链路本地 → 该网卡整体不进 payload（没有地址的网卡对拓扑无意义）。
        self.assertEqual(envelope(resources)["payload"]["interfaces"], [])

    def test_routable_address_keeps_the_interface_with_full_address_detail(self):
        resources = [
            host(),
            resource("if-1", "network_interface", **{"net.if.name": "en0", "net.if.mac": "aa:bb:cc:dd:ee:ff"}),
            resource(
                "addr-1",
                "ip_address",
                **{
                    "net.if.name": "en0",
                    "net.if.addr": "192.168.3.181",
                    "net.if.prefix": "24",
                    "net.if.cidr": "192.168.3.181/24",
                    "net.if.gateway": "192.168.3.1",
                },
            ),
        ]
        interfaces = envelope(resources)["payload"]["interfaces"]
        self.assertEqual(len(interfaces), 1)
        iface = interfaces[0]
        self.assertEqual(iface["name"], "en0")
        self.assertEqual(iface["mac"], "aa:bb:cc:dd:ee:ff")
        self.assertEqual(iface["machine_id"], "machine-abc")
        self.assertEqual(
            iface["addresses"],
            [
                {
                    "family": "ipv4",
                    "ip": "192.168.3.181",
                    "prefix": 24,
                    "cidr": "192.168.3.181/24",
                    "gateway": "192.168.3.1",
                }
            ],
        )

    def test_ipv6_address_is_typed_as_ipv6(self):
        resources = [
            host(),
            resource("addr-1", "ip_address", **{"net.if.name": "en0", "net.if.addr": "2001:db8::1"}),
        ]
        addresses = envelope(resources)["payload"]["interfaces"][0]["addresses"]
        self.assertEqual(addresses[0]["family"], "ipv6")

    def test_address_without_a_network_interface_resource_still_creates_one(self):
        # 探针可能只产出 ip_address（没有单独的 network_interface 资源）。
        resources = [
            host(),
            resource("addr-1", "ip_address", **{"net.if.name": "en1", "net.if.addr": "10.0.0.5"}),
        ]
        interfaces = envelope(resources)["payload"]["interfaces"]
        self.assertEqual([i["name"] for i in interfaces], ["en1"])
        self.assertNotIn("mac", interfaces[0])


class ProcessTest(unittest.TestCase):
    def test_identity_is_read_from_the_target_not_from_resource_attributes(self):
        # 契约里 `process.identity` 在 `targets[].execution_hints`，resource attributes 里没有。
        # 只读 resource 会让所有进程退化成「同一台机上的同 pid」，去重时互相踩。
        resources = [
            host(),
            resource(
                "proc-1",
                "process",
                **{"process.pid": "210", "process.executable.name": "/usr/sbin/sshd"},
            ),
        ]
        targets = [target("proc-1", "process", **{"process.identity": "ps_lstart:9f2c"})]
        processes = envelope(resources, targets)["payload"]["processes"]
        self.assertEqual(processes[0]["identity"], "ps_lstart:9f2c")

    def test_process_without_identity_still_produces_a_row(self):
        resources = [
            host(),
            resource(
                "proc-1",
                "process",
                **{"process.pid": "210", "process.executable.name": "/usr/sbin/sshd"},
            ),
        ]
        processes = envelope(resources)["payload"]["processes"]
        self.assertNotIn("identity", processes[0])
        self.assertEqual(processes[0]["observed_at"], OBSERVED_AT)

    def test_rows_without_pid_or_executable_are_skipped(self):
        resources = [
            host(),
            resource("proc-1", "process", **{"process.pid": "210"}),
            resource("proc-2", "process", **{"process.executable.name": "/bin/ls"}),
        ]
        self.assertEqual(envelope(resources)["payload"]["processes"], [])

    def test_processes_are_deduplicated_by_pid_and_identity_then_sorted(self):
        resources = [
            host(),
            resource("proc-b", "process", **{"process.pid": "300", "process.executable.name": "/bin/b"}),
            resource("proc-a", "process", **{"process.pid": "100", "process.executable.name": "/bin/a"}),
            # 同 pid + 同 identity（pid 复用后的同一个观测量）→ 只留一条。
            resource("proc-a", "process", **{"process.pid": "100", "process.executable.name": "/bin/a"}),
        ]
        processes = envelope(resources)["payload"]["processes"]
        self.assertEqual([p["pid"] for p in processes], [100, 300])

    def test_same_pid_with_different_identity_is_kept_twice(self):
        # pid 复用：同一 pid、不同启动时刻，是两个真实进程。
        resources = [
            host(),
            resource("proc-1", "process", **{"process.pid": "210", "process.executable.name": "/bin/a"}),
            resource("proc-2", "process", **{"process.pid": "210", "process.executable.name": "/bin/a"}),
        ]
        targets = [
            target("proc-1", "process", **{"process.identity": "ps_lstart:aaa"}),
            target("proc-2", "process", **{"process.identity": "ps_lstart:bbb"}),
        ]
        processes = envelope(resources, targets)["payload"]["processes"]
        self.assertEqual(len(processes), 2)


class EnvelopeTest(unittest.TestCase):
    def test_envelope_carries_source_collect_and_payload(self):
        built = envelope([host()])
        self.assertEqual(built["schema"], "atlas.in.edge.v1")
        self.assertEqual(
            built["source"],
            {"system": "wist", "producer": "agent-1", "tenant": "tenant-demo", "env": "dev"},
        )
        self.assertEqual(built["collect"]["mode"], "snapshot")
        self.assertEqual(built["collect"]["snap_id"], "snap-1")
        self.assertEqual(built["collect"]["res_ver"], "7")
        self.assertEqual(built["collect"]["observed_at"], OBSERVED_AT)

    def test_env_is_omitted_when_not_configured(self):
        built = adapter.build_envelope(
            [host()], [], "agent-1", "tenant-demo", None, "snap-1", 1, OBSERVED_AT, OBSERVED_AT
        )
        self.assertNotIn("env", built["source"])


class ReportModeTest(unittest.TestCase):
    """`--report` 读的是 DSNAP 帧体（`ReportDiscoverySnapshot`），不是 state 目录。"""

    def _report(self) -> dict:
        return {
            "api_version": "v1",
            "kind": "report_discovery_snapshot",
            "report_id": "disc_agent-1_1",
            "agent_id": "agent-from-report",
            "instance_id": "inst-1",
            "snapshot_id": "discovery:5:2026-10-25T07:56:36Z",
            "revision": 5,
            "generated_at": OBSERVED_AT,
            "report_attempt": 1,
            "report_mode": "full_snapshot",
            "reported_at": "2026-10-25T07:56:38.736786Z",
            "snapshot": {
                "schema_version": "v1",
                "snapshot_id": "discovery:5:2026-10-25T07:56:36Z",
                "revision": 5,
                "generated_at": OBSERVED_AT,
                "origins": [],
                "resources": [host()],
                "targets": [],
            },
        }

    def test_snapshot_fields_drive_the_collect_block(self):
        built = adapter.build_envelope_from_report(self._report(), None, "tenant-demo", "dev")
        self.assertEqual(built["source"]["producer"], "agent-from-report")
        self.assertEqual(built["collect"]["snap_id"], "discovery:5:2026-10-25T07:56:36Z")
        self.assertEqual(built["collect"]["res_ver"], "5")
        self.assertEqual(built["collect"]["observed_at"], OBSERVED_AT)
        # collected_at 是「我们收到它的时刻」，与 snapshot 自带的 generated_at 不同。
        self.assertEqual(built["collect"]["collected_at"], "2026-10-25T07:56:38.736786Z")

    def test_agent_id_override_wins(self):
        built = adapter.build_envelope_from_report(self._report(), "override-1", "t", None)
        self.assertEqual(built["source"]["producer"], "override-1")

    def test_a_report_without_snapshot_is_rejected(self):
        broken = self._report()
        del broken["snapshot"]
        with self.assertRaises(SystemExit):
            adapter.build_envelope_from_report(broken, None, "t", None)

    def test_a_report_without_any_agent_id_is_rejected(self):
        broken = self._report()
        del broken["agent_id"]
        with self.assertRaises(SystemExit):
            adapter.build_envelope_from_report(broken, None, "t", None)


if __name__ == "__main__":
    unittest.main()
