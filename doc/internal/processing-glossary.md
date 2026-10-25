# 内部处理术语表

## 1. 文档目的

本文档定义 `wist-atlas` 内部 adapter / resolver / materializer pipeline 使用的中间对象、短名和字段命名规则。

本文档不是外部接口契约。外部系统无需构造本文档中的 `*Cand`、`*Ev`、lower_snake 字段集合或 atlas 内部处理链路 ID。

相关文档：

- [`../external-integration/external-glossary.md`](../external-integration/external-glossary.md)
- [`../external-integration/input-taxonomy-and-style.md`](../external-integration/input-taxonomy-and-style.md)
- [`../external-integration/warp-insight-adapter-spec.md`](../external-integration/warp-insight-adapter-spec.md)

---

## 2. 分层边界

内部处理链路：

```text
External Raw Input
  -> staging
  -> adapter
  -> candidate / evidence / observation
  -> identity resolver
  -> materializer
  -> source-of-truth / derived view
```

规则：

- adapter 输出可以使用 `Cand`、`Ev`、`Obs` 等内部短名。
- resolver / materializer 负责把内部中间对象归并为中心模型对象。
- 外部 payload 中不得出现本文档的 lower_snake 输出集合名，除非另有正式 API 文档定义。
- 一个长名只能有一个规范短名，不能同时出现多种缩写。

---

## 3. Adapter 输出短名

| atlas 规范短名 | 全称 | lower_snake 字段名 | 中文名 | 来源 | 定义 |
| --- | --- | --- | --- | --- | --- |
| `HostCand` | `HostCandidate` | `host_cands` | 主机候选 | adapter 输出 | 等待解析成 `HostInventory` 的候选 |
| `NetSegCand` | `NetworkSegmentCandidate` | `net_seg_cands` | 网络段候选 | adapter 输出 | 等待解析成 `NetworkSegment` 的候选 |
| `ProcRtCand` | `ProcessRuntimeCandidate` | `proc_rt_cands` | 进程运行候选 | adapter 输出（规划） | 等待解析成进程运行态对象的候选；当前作为内部处理术语，是否落入代码模型以后续实现为准 |
| `CtrRtCand` | `ContainerRuntimeCandidate` | `ctr_rt_cands` | 容器运行候选 | adapter 输出（规划） | 等待解析成容器运行对象的候选；当前作为内部处理术语，是否落入代码模型以后续实现为准 |
| `PodCand` | `PodCandidate` | `pod_cands` | Pod 候选 | adapter 输出 | 等待解析成 `PodInventory` 的候选 |
| `SubjectCand` | `SubjectCandidate` | `subject_cands` | 主体候选 | adapter 输出 | 等待解析成 `Subject` 的人、团队或轮值候选 |
| `SubjectMemberCand` | `SubjectMembershipCandidate` | `subject_member_cands` | 主体成员关系候选 | adapter 输出 | 等待解析成 subject membership 的候选 |
| `RespCand` | `ResponsibilityCandidate` | `resp_cands` | 责任候选 | adapter 输出 | 等待解析成责任关系的候选 |
| `RespAssignCand` | `ResponsibilityAssignmentCandidate` | `resp_assign_cands` | 责任分配候选 | adapter 输出 | 等待解析成 `ResponsibilityAssignment` 的候选 |
| `OncallCand` | `OncallCandidate` | `oncall_cands` | 值班候选 | adapter 输出 | 等待解析成 oncall 轮值或路由关系的候选 |
| `EndpointCand` | `EndpointCandidate` | `endpoint_cands` | 入口候选 | adapter 输出 | 等待解析成 `SvcEp`、`InstEp` 或 `EpRes` 的候选 |
| `EpResCand` | `EpResCandidate` | `ep_res_cands` | 入口解析候选 | adapter 输出 | 等待解析成 `EpRes` 的候选 |
| `ClusterCand` | `ClusterCandidate` | `cluster_cands` | 集群候选 | adapter 输出 | 等待解析成 `ClusterInventory` 的候选 |
| `NamespaceCand` | `NamespaceCandidate` | `namespace_cands` | 命名空间候选 | adapter 输出 | 等待解析成 `NamespaceInventory` 的候选 |
| `WorkloadCand` | `WorkloadCandidate` | `workload_cands` | 工作负载候选 | adapter 输出 | 等待解析成 `WorkloadEntity` 的候选 |
| `CatalogCand` | `CatalogCandidate` | `catalog_cands` | 目录候选 | adapter 输出 | 等待解析成业务、系统、服务目录对象的候选 |
| `DeclDepCand` | `DeclaredDependencyCandidate` | `decl_dep_cands` | 声明依赖候选 | adapter 输出 | 等待解析成声明依赖关系的候选 |
| `ArtifactVerifyCand` | `ArtifactVerificationCandidate` | `artifact_verify_cands` | 制品验真候选 | adapter 输出 | 等待解析成 `ArtifactVerification` 的候选 |
| `VulnAdvisoryRaw` | `VulnerabilityAdvisoryRaw` | `vuln_advisory_raw` | 漏洞公告原始输入 | adapter 输出 | 保留漏洞情报来源的原始公告事实 |
| `AffectedRangeCand` | `AffectedRangeCandidate` | `affected_range_cands` | 受影响范围候选 | adapter 输出 | 等待解析成漏洞影响范围的候选 |
| `FindingCand` | `FindingCandidate` | `finding_cands` | 发现项候选 | adapter 输出 | 等待解析成 vulnerability / risk finding 的候选 |
| `BugFindingCand` | `SoftwareBugFindingCandidate` | `bug_finding_cands` | 软件 bug 发现候选 | adapter 输出 | 等待解析成 `SoftwareBugFinding` 的候选 |
| `RiskCand` | `RiskCandidate` | `risk_cands` | 风险候选 | adapter 输出 | 等待解析成风险视图或风险事实的候选 |
| `CorrectionCand` | `CorrectionCandidate` | `correction_cands` | 修正候选 | adapter 输出 | 等待解析成人工修正变更的候选 |
| `ExtIdLinkCand` | `ExternalIdentityLinkCandidate` | `ext_id_link_cands` | 外部身份链接候选 | adapter 输出 | 保留外部系统对象与 atlas 候选对象之间的身份链接 |
| `SwEv` | `SoftwareEvidence` | `sw_ev` | 软件证据 | adapter 输出 | 支撑软件识别的进程、文件、包、镜像线索 |
| `RtBindEv` | `RuntimeBindingEvidence` | `rt_bind_ev` | 运行绑定证据 | adapter 输出 | 支撑运行对象归属服务实例的证据 |
| `RtBindCand` | `RuntimeBindingCandidate` | `rt_bind_cands` | 运行绑定候选 | adapter 输出 | 等待解析成 `RuntimeBinding` 的候选 |
| `HostNetEv` | `HostNetworkEvidence` | `host_net_ev` | 主机网络证据 | adapter 输出 | 支撑主机网络归属和地址解析的证据 |
| `PodNetEv` | `PodNetworkEvidence` | `pod_net_ev` | Pod 网络证据 | adapter 输出 | 支撑 Pod 网络归属和地址解析的证据 |
| `BugEv` | `BugEvidence` | `bug_ev` | bug 证据 | adapter 输出 | 支撑软件 bug 识别和归因的证据 |
| `ThreatEv` | `ThreatEvidence` | `threat_ev` | 威胁证据 | adapter 输出 | 支撑安全风险识别和归因的证据 |
| `HostNetAssocCand` | `HostNetAssocCandidate` | `host_net_assoc_cands` | 主机网络关联候选 | adapter 输出 | 等待解析成 `HostNetAssoc` 的候选 |
| `TargetEv` | `DiscoveryTargetEvidence` | `target_ev` | 发现目标证据 | adapter 输出 | atlas 对外部 target 的原始证据保留 |
| `ResFactSnap` | `ResourceFactSnapshot` | `res_fact_snap` | 资源事实快照 | adapter 输出 | 某来源在某时间形成的一组资源事实 |

---

## 4. 中心模型对象

| 中心术语 | 中文名 | 来源 | 定义 |
| --- | --- | --- | --- |
| `HostInventory` | 主机目录对象 | resolver/materializer 输出 | 中心侧稳定主机目录对象 |
| `ServiceEntity` | 逻辑服务对象 | resolver/materializer 输出 | 中心侧逻辑服务定义 |
| `SvcEp` | 服务稳定入口 | resolver/materializer 输出 | atlas 中心服务的稳定访问入口 |
| `InstEp` | 实例运行地址 | resolver/materializer 输出 | atlas 中心服务实例的运行时地址 |
| `ServiceInstance` | 服务运行实例 | resolver/materializer 输出 | atlas 中心逻辑服务的一次运行副本会话 |

---

## 5. 内部 ID 和处理链路 ID

| 术语 | 中文名 | 示例 | 规则 |
| --- | --- | --- | --- |
| `internal_id` | 内部主键 | `host_id`、`service_id` | source-of-truth 对象的内部主键，只能由 atlas 中心生成 |
| `staged_payload_id` | 暂存载荷 ID | `stage_01J...` | staging 层处理链路 ID，不是中心主键 |
| `candidate_id` | 候选对象 ID | `cand_01J...` | candidate 层处理链路 ID，不是中心主键 |
| `evidence_id` | 证据 ID | `ev_01J...` | evidence 层处理链路 ID，不是中心主键 |

规则：

- adapter 输出必须保留来源 ID，但不得把来源 ID 写入 atlas 内部主键字段。
- staging、candidate、evidence 可以有自己的处理链路 ID。
- 对外部输入去重时优先使用来源作用域幂等键，例如 `source.system + producer_id + snapshot_id`，不要依赖 atlas 中心主键。
