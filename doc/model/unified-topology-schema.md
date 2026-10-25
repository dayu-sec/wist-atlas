# wist-atlas 统一 Schema 草案

## 1. 文档目的

本文档给出 `wist-atlas` 第一版统一 schema 草案，目标是把当前分散在多份模型文档中的核心对象收敛到一套中心存储视图中。

本文重点回答：

- 哪些表是核心主表
- 哪些表是关系表
- 哪些表是运行态表
- 哪些主键、外键和唯一约束应优先固定

相关文档：

- [`glossary.md`](../glossary.md)
- [`host-inventory-and-runtime-state-schema.md`](./host-inventory-and-runtime-state-schema.md)
- [`host-responsibility-and-maintainer-model.md`](./host-responsibility-and-maintainer-model.md)
- [`host-pod-network-topology-model.md`](./host-pod-network-topology-model.md)
- [`business-system-service-topology-model.md`](./business-system-service-topology-model.md)
- [`cluster-namespace-workload-topology-model.md`](./cluster-namespace-workload-topology-model.md)
- [`runtime-binding-model.md`](./runtime-binding-model.md)
- [`endpoint-and-dependency-observation-model.md`](./endpoint-and-dependency-observation-model.md)
- [`software-normalization-and-vuln-enrichment.md`](./software-normalization-and-vuln-enrichment.md)

---

## 2. 核心结论

第一版统一 schema 建议按四类表组织：

- 目录主表
- 关系边表
- 运行态表
- 治理与同步表

底层统一建议：

- PostgreSQL 作为主库
- `uuid` 作为中心主键
- `valid_from / valid_to` 作为关系与地址的生效时间语义
- `observed_at` 作为运行态快照时间语义

---

## 3. 目录主表

### 3.1 `business_domain`

主键：

- `business_id`

建议唯一约束：

- `(tenant_id, name)`

### 3.2 `system_boundary`

主键：

- `system_id`

建议索引：

- `(business_id)`
- `(tenant_id, name)`

### 3.3 `subsystem`

主键：

- `subsystem_id`

建议索引：

- `(system_id)`

### 3.4 `service_entity`

主键：

- `service_id`

建议字段：

- `service_id`
- `tenant_id`
- `business_id`
- `system_id`
- `subsystem_id`
- `namespace`
- `name`
- `service_type`
- `boundary`
- `provider`
- `external_ref`
- `state`
- `created_at`
- `updated_at`

建议索引：

- `(business_id)`
- `(system_id)`
- `(subsystem_id)`
- `(tenant_id, namespace, name)`
- `(tenant_id, boundary, provider)`
- `(tenant_id, external_ref)`

说明：

- 外部服务复用 `service_entity`
- `boundary` 区分 internal / external / partner / saas
- 外部服务可参与 `dep_edge`，但通常没有内部 `service_instance`

### 3.5 `cluster_inventory`

主键：

- `cluster_id`

建议唯一约束：

- `(tenant_id, name)`

### 3.6 `namespace_inventory`

主键：

- `namespace_id`

建议唯一约束：

- `(cluster_id, name)`

建议索引：

- `(tenant_id, name)`
- `(cluster_id)`

### 3.7 `workload_entity`

主键：

- `workload_id`

建议唯一约束：

- `(namespace_id, workload_kind, name)`

建议索引：

- `(service_id)`
- `(cluster_id)`
- `(namespace_id)`

### 3.8 `host_inventory`

主键：

- `host_id`

建议字段：

- `tenant_id`
- `host_name`
- `machine_id`
- `os_name`
- `os_version`
- `created_at`
- `last_inventory_at`

建议索引：

- `(tenant_id, host_name)`
- `(machine_id)`

### 3.9 `pod_inventory`

主键：

- `pod_id`

建议唯一约束：

- `(tenant_id, cluster_id, pod_uid)`

建议索引：

- `(namespace, pod_name)`
- `(node_id)`

### 3.10 `network_domain`

主键：

- `net_domain_id`

### 3.11 `network_segment`

主键：

- `net_seg_id`

建议索引：

- `(net_domain_id)`
- `(cidr)`

### 3.12 `software_product`

主键：

- `product_id`

建议索引：

- `(canonical_name)`
- `(vendor)`

### 3.13 `software_version`

主键：

- `version_id`

建议唯一约束：

- `(product_id, normalized_version, edition, release_channel)`

建议索引：

- `(product_id)`
- `(normalized_version)`

### 3.14 `software_artifact`

主键：

- `artifact_id`

建议索引：

- `(version_id)`
- `(artifact_kind, sha256)`
- `(package_manager, package_name)`
- `(purl)`

说明：

- `artifact_kind = executable` 时，`sha256` 表示可执行文件内容 hash
- `artifact_kind = script` 时，`sha256` 表示脚本内容 hash
- 脚本解释器可通过 `interpreter_artifact_id` 指向另一个 `software_artifact`

### 3.15 `subject`

主键：

- `subject_id`

建议索引：

- `(tenant_id, subject_type, name)`
- `(tenant_id, external_ref)`

### 3.16 `host_group`

主键：

- `host_group_id`

建议唯一约束：

- `(tenant_id, name)`

---

## 4. 关系边表

### 4.1 `dep_edge`

主键：

- `dependency_id`

建议索引：

- `(up_svc_id, valid_to)`
- `(down_svc_id, valid_to)`
- `(scope, source)`

### 4.2 `service_instance`

主键：

- `inst_id`

建议字段：

- `inst_id`
- `service_id`
- `runtime_kind`
- `inst_key`
- `version`
- `state`
- `started_at`
- `ended_at`
- `last_seen_at`

建议索引：

- `(service_id, last_seen_at desc)`
- `(service_id, runtime_kind, inst_key)`
- `(state, last_seen_at desc)`

说明：

- `service_instance` 表示一次服务运行副本会话
- 不直接把 `pod_id / process_id / host_id` 固化为主字段
- 底层运行对象通过 `runtime_binding(inst_id, obj_type, obj_id)` 关联
- PID 变化不必然新建 `service_instance`
- `started_at` 是会话开始时间，`last_seen_at` 是最近观测时间，`ended_at` 是确认结束时间
- `ended_at` 可为空，表示仍在运行或尚未确认结束
- 失联超时后可把 `state` 置为 `lost`，确认停止后置为 `stopped`

### 4.3 `service_workload_binding`

主键：

- `binding_id`

建议索引：

- `(service_id, valid_to)`
- `(workload_id, valid_to)`

### 4.4 `workload_pod_membership`

主键：

- `(workload_id, pod_id, valid_from)`

建议索引：

- `(workload_id, valid_to)`
- `(pod_id, valid_to)`

### 4.5 `resource_scope_membership`

主键：

- `membership_id`

建议索引：

- `(tenant_id, target_type, target_id, valid_to)`
- `(tenant_id, scope_type, scope_id, valid_to)`

### 4.6 `service_resource_set_binding`

主键：

- `binding_id`

建议索引：

- `(service_id, valid_to)`
- `(resource_set_id, valid_to)`

### 4.7 `svc_ep`

主键：

- `endpoint_id`

建议索引：

- `(service_id, valid_to)`
- `(endpoint_type, address, port)`

### 4.8 `inst_ep`

主键：

- `endpoint_id`

建议索引：

- `(inst_id, valid_to)`
- `(address, port, valid_to)`

### 4.9 `pod_placement`

主键：

- `placement_id`

建议索引：

- `(pod_id, valid_to)`
- `(host_id, valid_to)`

### 4.10 `pod_placement_evidence`

主键：

- `evidence_id`

建议索引：

- `(observed_at)`
- `(pod_uid)`
- `(pod_name)`
- `(host_name)`
- `(node_name)`

说明：

- 表达未解析或待归属的 Pod 调度事实
- 可与 `ingest_id` 或原始 payload 关联

### 4.11 `pod_placement_candidate`

主键：

- `candidate_id`

建议索引：

- `(evidence_id)`
- `(candidate_pod_id)`
- `(candidate_host_id)`
- `(status, observed_at)`

说明：

- 表达从调度 evidence 到最终 `pod_placement` 之间的候选层
- 第一版可选持久化

### 4.12 `pod_net_assoc`

主键：

- `assoc_id`

建议索引：

- `(pod_id, valid_to)`
- `(net_seg_id, valid_to)`
- `(ip_addr, valid_to)`

### 4.13 `host_net_assoc`

主键：

- `assoc_id`

建议索引：

- `(host_id, valid_to)`
- `(net_seg_id, valid_to)`
- `(ip_addr, valid_to)`

### 4.14 `pod_network_evidence`

主键：

- `evidence_id`

建议索引：

- `(observed_at)`
- `(ip_addr)`
- `(mac_addr)`
- `(net_seg_id)`

说明：

- 表达未解析或待归属的 Pod 网络事实
- 可与 `ingest_id` 或原始 payload 关联

### 4.15 `pod_net_assoc_candidate`

主键：

- `candidate_id`

建议索引：

- `(evidence_id)`
- `(candidate_pod_id)`
- `(status, observed_at)`

说明：

- 表达从 evidence 到最终 relation 之间的 Pod 候选层
- 第一版可选持久化

### 4.16 `host_network_evidence`

主键：

- `evidence_id`

建议索引：

- `(observed_at)`
- `(ip_addr)`
- `(mac_addr)`
- `(net_seg_id)`

说明：

- 表达未解析或待归属的主机网络事实
- 可与 `ingest_id` 或原始 payload 关联

### 4.17 `host_net_assoc_candidate`

主键：

- `candidate_id`

建议索引：

- `(evidence_id)`
- `(candidate_host_id)`
- `(status, observed_at)`

说明：

- 表达从 evidence 到最终 relation 之间的候选层
- 第一版可选持久化

### 4.18 `runtime_binding`

主键：

- `binding_id`

建议索引：

- `(inst_id, valid_to)`
- `(obj_type, obj_id, valid_to)`
- `(scope, confidence)`

### 4.19 `runtime_binding_evidence`

主键：

- `evidence_id`

建议索引：

- `(binding_id)`
- `(observed_at)`

### 4.20 `business_health_factor`

主键：

- `factor_id`

建议索引：

- `(business_id, factor_type, observed_at desc)`
- `(tenant_id, factor_type, status)`
- `(target_type, target_id, observed_at desc)`
- `(severity, observed_at desc)`

说明：

- 表达业务稳定性的五类健康因子摘要
- `factor_type` 固定为 `resource_sufficiency / bug_reduction / vuln_reduction / dependency_stability / threat_reduction`
- 原始日志、漏洞、BUG、依赖观测或安全告警通过 `evidence_ref` 回指，不直接塞入本表

### 4.21 `ep_res`

主键：

- `resolution_id`

建议索引：

- `(endpoint_kind, address, port, valid_to)`
- `(svc_id, valid_to)`
- `(inst_id, valid_to)`

### 4.22 `dep_obs`

主键：

- `observation_id`

建议索引：

- `(up_svc_id, down_svc_id, source, last_observed_at)`
- `(up_inst_id, down_inst_id, last_observed_at)`
- `(observation_type, confidence)`

### 4.23 `dep_ev`

主键：

- `evidence_id`

建议索引：

- `(observation_id, observed_at)`

### 4.24 `software_evidence`

主键：

- `evidence_id`

建议索引：

- `(artifact_id)`
- `(version_id)`
- `(host_id)`
- `(pod_id)`
- `(process_id)`

### 4.25 `artifact_verification`

主键：

- `verification_id`

建议索引：

- `(artifact_id, observed_at desc)`
- `(host_id, observed_at desc)`
- `(process_id, observed_at desc)`
- `(result, verification_level)`
- `(observed_sha256)`

### 4.26 `host_group_membership`

主键：

- `(host_id, host_group_id, valid_from)`

建议索引：

- `(host_id, valid_to)`
- `(host_group_id, valid_to)`

### 4.27 `responsibility_assignment`

主键：

- `assignment_id`

建议索引：

- `(tenant_id, target_type, target_id, role, valid_to)`
- `(subject_id, role, valid_to)`

---

## 5. 运行态表

### 5.1 `host_runtime_state`

建议字段：

- `host_id`
- `observed_at`
- `boot_id`
- `uptime_seconds`
- `loadavg_1m`
- `loadavg_5m`
- `loadavg_15m`
- `cpu_usage_pct`
- `memory_used_bytes`
- `memory_available_bytes`
- `disk_used_bytes`
- `disk_available_bytes`
- `network_rx_bytes`
- `network_tx_bytes`
- `process_count`
- `container_count`
- `agent_health`
- `protection_state`
- `degraded_reason`
- `last_error`
- `runtime_blob_ref`

建议唯一约束：

- `(host_id, observed_at)`

建议索引：

- `(host_id, observed_at desc)`
- `(observed_at desc)`

说明：

- 主机当前 IP 不在本表建模，统一通过 `host_net_assoc` 表达
- `disk_blob_ref` / `nic_blob_ref` 等大块 inventory 明细不进入运行态表

### 5.2 `process_runtime_state`

建议唯一约束：

- `(host_id, process_key, observed_at)`

建议索引：

- `(host_id, observed_at desc)`
- `(process_key, observed_at desc)`

说明：

- `process_key` 可采用中心定义的稳定进程 identity

### 5.3 `container_runtime`

主键：

- `container_id`

建议索引：

- `(pod_id, last_seen_at desc)`
- `(host_id, last_seen_at desc)`
- `(runtime_type, runtime_namespace)`

### 5.4 `software_vulnerability_finding`

主键：

- `finding_id`

建议索引：

- `(version_id, status)`
- `(artifact_id, status)`
- `(severity, published_at desc)`
- `(source, external_id)`

### 5.5 `software_bug`

主键：

- `bug_id`

建议索引：

- `(product_id, status)`
- `(version_id, status)`
- `(artifact_id, status)`
- `(source, external_ref)`
- `(bug_type, severity)`

### 5.6 `software_bug_finding`

主键：

- `finding_id`

建议索引：

- `(bug_id, status)`
- `(version_id, status)`
- `(artifact_id, status)`
- `(host_id, last_seen_at desc)`
- `(process_id, last_seen_at desc)`

### 5.7 `software_bug_vuln_link`

主键：

- `(bug_id, finding_id, relation_type)`

建议索引：

- `(bug_id)`
- `(finding_id)`

---

## 6. 同步与治理表

### 6.1 `external_identity_link`

主键：

- `link_id`

建议唯一约束：

- `(tenant_id, system_type, object_type, external_id)`

### 6.2 `external_sync_cursor`

主键：

- `cursor_id`

建议唯一约束：

- `(tenant_id, system_type, scope_key)`

---

## 7. 统一时间语义

第一版建议固定三类时间字段：

- `created_at / updated_at`
  - 目录对象维护时间
- `valid_from / valid_to`
  - 关系、地址、归属、生效段时间
- `observed_at`
  - 运行态快照时间

这三类时间不要混用。

---

## 8. 第一版必须固定的外键主线

```text
system_boundary.business_id -> business_domain.business_id
subsystem.system_id -> system_boundary.system_id
service_entity.system_id -> system_boundary.system_id
service_entity.subsystem_id -> subsystem.subsystem_id
namespace_inventory.cluster_id -> cluster_inventory.cluster_id
workload_entity.cluster_id -> cluster_inventory.cluster_id
workload_entity.namespace_id -> namespace_inventory.namespace_id
workload_entity.service_id -> service_entity.service_id
service_instance.service_id -> service_entity.service_id
service_workload_binding.service_id -> service_entity.service_id
service_workload_binding.workload_id -> workload_entity.workload_id
workload_pod_membership.workload_id -> workload_entity.workload_id
workload_pod_membership.pod_id -> pod_inventory.pod_id
svc_ep.service_id -> service_entity.service_id
inst_ep.inst_id -> service_instance.inst_id
runtime_binding.inst_id -> service_instance.inst_id
runtime_binding_evidence.binding_id -> runtime_binding.binding_id
business_health_factor.business_id -> business_domain.business_id
pod_placement.pod_id -> pod_inventory.pod_id
pod_placement.host_id -> host_inventory.host_id
pod_net_assoc.pod_id -> pod_inventory.pod_id
pod_net_assoc.net_seg_id -> network_segment.net_seg_id
host_net_assoc.host_id -> host_inventory.host_id
host_net_assoc.net_seg_id -> network_segment.net_seg_id
ep_res.svc_id -> service_entity.service_id
ep_res.inst_id -> service_instance.inst_id
dep_obs.up_svc_id -> service_entity.service_id
dep_obs.down_svc_id -> service_entity.service_id
dep_obs.up_inst_id -> service_instance.inst_id
dep_obs.down_inst_id -> service_instance.inst_id
dep_ev.observation_id -> dep_obs.observation_id
software_version.product_id -> software_product.product_id
software_artifact.version_id -> software_version.version_id
software_evidence.artifact_id -> software_artifact.artifact_id
software_evidence.version_id -> software_version.version_id
artifact_verification.artifact_id -> software_artifact.artifact_id
responsibility_assignment.subject_id -> subject.subject_id
host_group_membership.host_group_id -> host_group.host_group_id
software_vulnerability_finding.version_id -> software_version.version_id
software_vulnerability_finding.artifact_id -> software_artifact.artifact_id
software_bug.product_id -> software_product.product_id
software_bug.version_id -> software_version.version_id
software_bug.artifact_id -> software_artifact.artifact_id
software_bug_finding.bug_id -> software_bug.bug_id
software_bug_finding.version_id -> software_version.version_id
software_bug_finding.artifact_id -> software_artifact.artifact_id
software_bug_vuln_link.bug_id -> software_bug.bug_id
software_bug_vuln_link.finding_id -> software_vulnerability_finding.finding_id
```

---

## 9. 第一版不建议过度设计的部分

第一版先不要一开始就做得过重：

- 不必先做图数据库主存储
- 不必先做全对象事件溯源
- 不必先做复杂多租户跨域共享模型
- 不必先做所有对象的通用 EAV 大表

优先固定：

- 主表是谁
- 关系边是谁
- 时间语义是什么
- 唯一键和核心索引是什么

---

## 10. 第一阶段落地优先级

建议按以下批次落表：

### Phase A

- `business_domain`
- `system_boundary`
- `service_entity`
- `host_inventory`
- `host_runtime_state`
- `subject`
- `responsibility_assignment`

### Phase B

- `cluster_inventory`
- `namespace_inventory`
- `workload_entity`
- `pod_inventory`
- `service_workload_binding`
- `workload_pod_membership`
- `pod_placement`
- `pod_net_assoc`
- `host_net_assoc`

### Phase C

- `service_instance`
- `svc_ep`
- `inst_ep`
- `container_runtime`
- `runtime_binding`
- `runtime_binding_evidence`
- `business_health_factor`

### Phase D

- `software_product`
- `software_version`
- `software_artifact`
- `software_evidence`
- `artifact_verification`
- `software_vulnerability_finding`
- `software_bug`
- `software_bug_finding`
- `software_bug_vuln_link`
- `dep_obs`
- `dep_ev`
- `ep_res`
- `external_identity_link`
- `external_sync_cursor`

## 11. 当前建议

当前建议固定为：

- 先用 PostgreSQL 把统一 schema 主干定住
- 后续 ingest、sync、query 都围绕这套 schema 展开
- 先从主线对象和主线外键开始，不追求第一版一次覆盖所有细节
