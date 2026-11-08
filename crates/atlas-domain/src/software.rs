//! 软件指纹与实例（客户域）。
//!
//! 设计见 `doc/design/center/models/software-fingerprint-verification.md`：
//!
//! - **身份由字节定**：`artifact_id = "sha256:<hex>"` 是这台主机上这个软件实例的
//!   唯一权威 ID；产品名 / 版本 / 路径 / unit 名都只是属性或位置锚点。
//! - **artifact 内容寻址，instance 位置寻址**，两个都要：只有 artifact 会在升级后
//!   多出一个「新资产」、历史断链；只有 instance 则回答不了「跑的是不是原厂那份」。
//! - 客户域**只有 `software_instance` 一张表**：早期的 `software_installation`
//!   （包 × 主机）已并入 —— 它的每个字段在这里都有归宿（`claimed_*` / `artifact_kind`），
//!   而「装了但没跑」已被范围文档划出图谱。

use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::TenantId;

/// 部署单元锚点：取第一个拿得到的。
///
/// 这一层**没有任何标准**（systemd unit / 容器名 / socket 都不是可移植标识），
/// 但它是图谱节点必须有的位置：没有它，同一个软件每轮采样可能给出不同身份。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    SystemdUnit,
    Container,
    Socket,
    Argv0,
    InstallRoot,
    Process,
}

impl AnchorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemdUnit => "systemd_unit",
            Self::Container => "container",
            Self::Socket => "socket",
            Self::Argv0 => "argv0",
            Self::InstallRoot => "install_root",
            Self::Process => "process",
        }
    }
}

impl FromStr for AnchorKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "systemd_unit" => Self::SystemdUnit,
            "container" => Self::Container,
            "socket" => Self::Socket,
            "argv0" => Self::Argv0,
            "install_root" => Self::InstallRoot,
            "process" => Self::Process,
            _ => return Err(()),
        })
    }
}

/// `(anchor_kind, anchor_value)` —— 实例在主机内的唯一位置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AnchorKey {
    pub anchor_kind: AnchorKind,
    pub anchor_value: String,
}

impl AnchorKey {
    pub fn new(anchor_kind: AnchorKind, anchor_value: impl Into<String>) -> Self {
        Self {
            anchor_kind,
            anchor_value: anchor_value.into(),
        }
    }
}

/// 声称值来源。它给的是「它说自己是什么」，不是验证结论。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimedSource {
    /// 包反查（`dpkg -S` / `rpm -qf` / `apk info --who-owns`）。
    PathPackage,
    /// 二进制自报（`version_probe` 白名单）。
    Probe,
    /// 容器镜像 tag。
    ImageTag,
}

impl ClaimedSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PathPackage => "path_package",
            Self::Probe => "probe",
            Self::ImageTag => "image_tag",
        }
    }
}

impl FromStr for ClaimedSource {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "path_package" => Self::PathPackage,
            "probe" => Self::Probe,
            "image_tag" => Self::ImageTag,
            _ => return Err(()),
        })
    }
}

/// 入库凭据：解释「这条为什么在图上」。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareRole {
    ExposedService,
    BusinessRuntime,
    Middleware,
    Runtime,
    Agent,
}

impl SoftwareRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExposedService => "exposed_service",
            Self::BusinessRuntime => "business_runtime",
            Self::Middleware => "middleware",
            Self::Runtime => "runtime",
            Self::Agent => "agent",
        }
    }
}

impl FromStr for SoftwareRole {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "exposed_service" => Self::ExposedService,
            "business_runtime" => Self::BusinessRuntime,
            "middleware" => Self::Middleware,
            "runtime" => Self::Runtime,
            "agent" => Self::Agent,
            _ => return Err(()),
        })
    }
}

/// 进入图谱的判据。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareReason {
    Listener,
    ServiceBinding,
    GovernanceClass,
    FingerprintCandidate,
}

impl SoftwareReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Listener => "listener",
            Self::ServiceBinding => "service_binding",
            Self::GovernanceClass => "governance_class",
            Self::FingerprintCandidate => "fingerprint_candidate",
        }
    }
}

impl FromStr for SoftwareReason {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "listener" => Self::Listener,
            "service_binding" => Self::ServiceBinding,
            "governance_class" => Self::GovernanceClass,
            "fingerprint_candidate" => Self::FingerprintCandidate,
            _ => return Err(()),
        })
    }
}

/// 指纹结论分级。
///
/// 「来源完整性」与「漏洞状态」是两条正交的轴：**原厂 ≠ 安全**（官方版本也会有洞），
/// **有洞 ≠ 被篡改**（只是没打补丁）。能驱动漏洞匹配的只有前三档。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintStatus {
    /// hash 命中可信源。
    Official,
    /// 有声称值但 hash 不符 —— 文件被替换。
    ClaimedMismatch,
    /// 我们自己的构建产物。
    Ours,
    /// 现场学到 + 人工确认。
    HumanConfirmed,
    /// AI 推断达阈值但未确认。
    AiSuspected,
    /// 未知（自研 / 自编译 / 私有）。**ID 仍然有效**，可去重、可事后回填。
    Observed,
    /// 算不出 hash，没有权威 ID。
    IdentityMissing,
}

impl FingerprintStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::ClaimedMismatch => "claimed_mismatch",
            Self::Ours => "ours",
            Self::HumanConfirmed => "human_confirmed",
            Self::AiSuspected => "ai_suspected",
            Self::Observed => "observed",
            Self::IdentityMissing => "identity_missing",
        }
    }

    /// 这一档能不能驱动漏洞匹配。
    pub fn drives_vulnerability_matching(self) -> bool {
        matches!(self, Self::Official | Self::Ours | Self::HumanConfirmed)
    }
}

impl FromStr for FingerprintStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "official" => Self::Official,
            "claimed_mismatch" => Self::ClaimedMismatch,
            "ours" => Self::Ours,
            "human_confirmed" => Self::HumanConfirmed,
            "ai_suspected" => Self::AiSuspected,
            "observed" => Self::Observed,
            "identity_missing" => Self::IdentityMissing,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Executable,
    Script,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::Script => "script",
        }
    }
}

impl FromStr for ArtifactKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "executable" => Self::Executable,
            "script" => Self::Script,
            _ => return Err(()),
        })
    }
}

/// 构建坐标。库的键是**构建**不是版本：同一版本在 alpine/x86_64 与 alpine/aarch64
/// 是两个 artifact。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactCoordinate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libc: Option<String>,
}

/// 一份字节（一次构建）。
///
/// `artifact_id` 是内容寻址 `sha256:<hex>`，与现场上报同值 —— 中心不另设内部 UUID，
/// 否则两个 ID 迟早会漂，而「同一份字节两个身份」正是这套设计最想防的错。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareArtifact {
    pub artifact_id: String,
    pub sha256: String,
    pub artifact_kind: ArtifactKind,
    pub version_id: Option<String>,
    pub size_bytes: Option<i64>,
    pub build_id: Option<String>,
    pub package_manager: Option<String>,
    pub package_name: Option<String>,
    pub purl: Option<String>,
    pub interpreter_artifact_id: Option<String>,
    pub coordinate: Option<ArtifactCoordinate>,
    pub first_seen_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

impl SoftwareArtifact {
    /// 从裸 sha256 生成内容寻址 id。已经是 `sha256:` 前缀时原样返回。
    pub fn content_id(sha256: &str) -> String {
        if sha256.starts_with("sha256:") {
            sha256.to_string()
        } else {
            format!("sha256:{sha256}")
        }
    }
}

/// 一台主机上的一个部署位：它跑的是哪份字节、声称是什么包、为什么进图谱。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareInstance {
    pub instance_id: Uuid,
    pub tenant_id: TenantId,
    pub host_id: Uuid,
    pub anchor_kind: AnchorKind,
    pub anchor_value: String,
    /// 权威身份。为空 ⇔ 算不出 hash，此时 `identity_missing_reason` 必须有值。
    pub current_artifact_id: Option<String>,
    pub identity_missing_reason: Option<String>,
    /// 声称的**包名**（`postgresql-16`）与**产品**（`postgresql`）不合并：
    /// 补丁与 advisory 按包发，只存产品会丢掉「装的是哪个发行版包」。
    pub claimed_package_name: Option<String>,
    pub claimed_product_id: Option<String>,
    /// 原样版本串；来自包元数据时它**就是** `version_installed`。
    pub claimed_version: Option<String>,
    pub claimed_source: Option<ClaimedSource>,
    pub role: Vec<SoftwareRole>,
    pub reason: Vec<SoftwareReason>,
    pub status: FingerprintStatus,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    /// 退役时刻；`None` = 现役。
    pub stale_at: Option<DateTime<Utc>>,
}

impl SoftwareInstance {
    pub fn anchor(&self) -> AnchorKey {
        AnchorKey {
            anchor_kind: self.anchor_kind,
            anchor_value: self.anchor_value.clone(),
        }
    }

    /// 现役视图：退役行留在表里供回溯，但不进「现在有哪几个软件」。
    pub fn is_live(&self) -> bool {
        self.stale_at.is_none()
    }

    /// 「升级了但没重启」：当前跑的字节与包记录版本不一致时由调用方判定，
    /// 这里只回答「两者是不是同一个版本口径」。
    pub fn version_drifted(&self, version_running: &str) -> bool {
        match self.claimed_version.as_deref() {
            Some(installed) => installed != version_running,
            None => false,
        }
    }
}

/// host × artifact 观测：回答「这台机上出现过哪些字节」，用于去重与跨机同源。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareFingerprint {
    pub tenant_id: TenantId,
    pub host_id: Uuid,
    pub artifact_id: String,
    pub size_bytes: i64,
    pub build_id: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// 观测时间线：回答「哪天升的、升到哪份字节」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareInstanceObservation {
    pub instance_id: Uuid,
    pub artifact_id: String,
    pub observed_at: DateTime<Utc>,
    pub version_running: Option<String>,
    pub version_installed: Option<String>,
}

/// 本机报告里的路径明细。
///
/// **不进摘要通道**：只有 `identity_missing` 与「边缘判出与包记录不符」两种情况下随帧上送。
/// 理由不是敏感，而是体积与抖动 —— 路径一旦被当作去重键，容器内每轮变化的路径会把去重打散。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoftwareArtifactPath {
    pub instance_id: Uuid,
    pub path: String,
    /// `primary` | `sub_artifact`
    pub role: String,
    pub size_bytes: i64,
    pub mtime: DateTime<Utc>,
    pub inode: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_id_is_idempotent_and_self_describing() {
        assert_eq!(SoftwareArtifact::content_id("abc123"), "sha256:abc123");
        assert_eq!(
            SoftwareArtifact::content_id("sha256:abc123"),
            "sha256:abc123"
        );
    }

    #[test]
    fn every_anchor_kind_round_trips_through_its_db_form() {
        for kind in [
            AnchorKind::SystemdUnit,
            AnchorKind::Container,
            AnchorKind::Socket,
            AnchorKind::Argv0,
            AnchorKind::InstallRoot,
            AnchorKind::Process,
        ] {
            assert_eq!(AnchorKind::from_str(kind.as_str()), Ok(kind));
        }
        assert_eq!(AnchorKind::from_str("nope"), Err(()));
    }

    #[test]
    fn every_status_round_trips_and_only_three_drive_matching() {
        let statuses = [
            FingerprintStatus::Official,
            FingerprintStatus::ClaimedMismatch,
            FingerprintStatus::Ours,
            FingerprintStatus::HumanConfirmed,
            FingerprintStatus::AiSuspected,
            FingerprintStatus::Observed,
            FingerprintStatus::IdentityMissing,
        ];
        for status in statuses {
            assert_eq!(FingerprintStatus::from_str(status.as_str()), Ok(status));
        }
        let drivers: Vec<_> = statuses
            .iter()
            .filter(|status| status.drives_vulnerability_matching())
            .collect();
        assert_eq!(drivers.len(), 3);
    }

    #[test]
    fn role_and_reason_round_trip() {
        for role in [
            SoftwareRole::ExposedService,
            SoftwareRole::BusinessRuntime,
            SoftwareRole::Middleware,
            SoftwareRole::Runtime,
            SoftwareRole::Agent,
        ] {
            assert_eq!(SoftwareRole::from_str(role.as_str()), Ok(role));
        }
        for reason in [
            SoftwareReason::Listener,
            SoftwareReason::ServiceBinding,
            SoftwareReason::GovernanceClass,
            SoftwareReason::FingerprintCandidate,
        ] {
            assert_eq!(SoftwareReason::from_str(reason.as_str()), Ok(reason));
        }
    }

    #[test]
    fn version_drift_only_reports_when_there_is_a_claim() {
        let now = Utc::now();
        let mut instance = SoftwareInstance {
            instance_id: Uuid::new_v4(),
            tenant_id: TenantId(Uuid::new_v4()),
            host_id: Uuid::new_v4(),
            anchor_kind: AnchorKind::SystemdUnit,
            anchor_value: "nginx.service".to_string(),
            current_artifact_id: Some("sha256:old".to_string()),
            identity_missing_reason: None,
            claimed_package_name: Some("nginx".to_string()),
            claimed_product_id: None,
            claimed_version: Some("1.24".to_string()),
            claimed_source: Some(ClaimedSource::PathPackage),
            role: vec![SoftwareRole::Middleware],
            reason: vec![SoftwareReason::Listener],
            status: FingerprintStatus::Observed,
            first_seen_at: now,
            last_seen_at: now,
            stale_at: None,
        };
        assert!(instance.is_live());
        // 磁盘 1.24、内存里跑 1.23 = 升级没重启。
        assert!(instance.version_drifted("1.23"));
        assert!(!instance.version_drifted("1.24"));

        instance.claimed_version = None;
        // 没有声称值就谈不上漂移 —— 不能把「不知道」报成「不一致」。
        assert!(!instance.version_drifted("1.23"));
    }
}
