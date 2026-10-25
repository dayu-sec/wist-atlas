use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    DomainError, DomainReason, DomainResult, EnvironmentId, ObservedAt, SourceKind, TenantId,
};
use orion_error::conversion::ToStructError;

const ATLAS_INPUT_SCHEMA_FAMILIES: &[&str] = &[
    "edge",
    "cmdb",
    "iam",
    "k8s",
    "telemetry",
    "sw",
    "artifact",
    "vuln",
    "bug",
    "security",
    "oncall",
    "manual",
    "correction",
];

mod candidates;
pub use candidates::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AtlasInputEnvelope {
    pub schema: String,
    pub source: AtlasInputSource,
    pub collect: AtlasInputCollect,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AtlasInputSource {
    pub system: String,
    pub producer: String,
    #[serde(alias = "tenant_ref")]
    pub tenant: String,
    #[serde(alias = "env_ref")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AtlasInputCollect {
    pub mode: AtlasInputMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collected_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_string_like",
        skip_serializing_if = "Option::is_none"
    )]
    pub res_ver: Option<String>,
}

impl AtlasInputCollect {
    pub fn collected_or_observed_at(&self) -> Option<DateTime<Utc>> {
        self.collected_at.or(self.observed_at)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AtlasInputMode {
    Snapshot,
    Incremental,
    Window,
    Correction,
}

impl AtlasInputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Incremental => "incremental",
            Self::Window => "window",
            Self::Correction => "correction",
        }
    }
}

impl AtlasInputEnvelope {
    pub fn schema_family(&self) -> Option<&str> {
        let mut parts = self.schema.split('.');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            // `atlas.in.*` 是当前前缀；`atlas.in.*` 作为兼容别名保留一个版本。
            (Some(prefix), Some("in"), Some(family), Some(version))
                if matches!(prefix, "atlas" | "dayu")
                    && version.starts_with('v')
                    && parts.next().is_none() =>
            {
                Some(family)
            }
            _ => None,
        }
    }

    pub fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.schema, "schema")?;
        require_non_empty(&self.source.system, "source.system")?;
        require_non_empty(&self.source.producer, "source.producer")?;
        require_non_empty(&self.source.tenant, "source.tenant")?;
        if let Some(env) = self.source.env.as_ref() {
            require_non_empty(env, "source.env")?;
        }

        let family = self.schema_family().ok_or_else(|| {
            invalid_schema("schema must match atlas.in.<family>.v<major> (legacy: atlas.in.*)")
        })?;
        if !ATLAS_INPUT_SCHEMA_FAMILIES.contains(&family) {
            return Err(DomainReason::SchemaUnsupported
                .to_err()
                .with_detail(format!("schema family `{family}` is not supported")));
        }

        if !self.payload.is_object() {
            return Err(DomainReason::PayloadInvalid
                .to_err()
                .with_detail("payload must be a JSON object"));
        }

        match self.collect.mode {
            AtlasInputMode::Snapshot => {
                require_option_non_empty(self.collect.snap_id.as_deref(), "collect.snap_id")?;
                if self.collect.observed_at.is_none() {
                    return Err(missing_field(
                        "collect.observed_at",
                        "collect.observed_at is required for snapshot mode",
                    ));
                }
            }
            AtlasInputMode::Window => {
                if self.collect.observed_at.is_none() {
                    return Err(missing_field(
                        "collect.observed_at",
                        "collect.observed_at is required for window mode",
                    ));
                }
                let window = self
                    .payload
                    .get("window")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        missing_field(
                            "payload.window",
                            "payload.window is required for window mode",
                        )
                    })?;
                require_json_string(window.get("start"), "payload.window.start")?;
                require_json_string(window.get("end"), "payload.window.end")?;
            }
            AtlasInputMode::Correction => {
                require_json_string(self.payload.get("correction_id"), "payload.correction_id")?;
            }
            AtlasInputMode::Incremental => {}
        }

        Ok(())
    }

    pub fn source_kind(&self) -> SourceKind {
        match self.schema_family() {
            Some("edge") | Some("sw") | Some("artifact") => SourceKind::EdgeDiscovery,
            Some("telemetry") | Some("bug") | Some("security") => SourceKind::TelemetrySummary,
            Some("manual") | Some("correction") => SourceKind::Manual,
            Some("cmdb") | Some("iam") | Some("k8s") | Some("vuln") | Some("oncall") => {
                SourceKind::ExternalSync
            }
            _ => SourceKind::ExternalSync,
        }
    }

    pub fn ingest_mode(&self) -> IngestMode {
        match self.collect.mode {
            AtlasInputMode::Snapshot => IngestMode::Snapshot,
            AtlasInputMode::Incremental => IngestMode::Delta,
            AtlasInputMode::Window | AtlasInputMode::Correction => IngestMode::BatchUpsert,
        }
    }

    pub fn idempotency_key(&self) -> String {
        match self.collect.mode {
            AtlasInputMode::Snapshot => format!(
                "{}:{}:{}:{}:{}:{}",
                self.schema,
                self.source.system,
                self.source.producer,
                self.source.tenant,
                self.source.env.as_deref().unwrap_or(""),
                self.collect.snap_id.as_deref().unwrap_or("")
            ),
            AtlasInputMode::Window => {
                let window = self.payload.get("window").and_then(Value::as_object);
                let start = window
                    .and_then(|window| window.get("start"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let end = window
                    .and_then(|window| window.get("end"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}",
                    self.schema,
                    self.source.system,
                    self.source.producer,
                    self.source.tenant,
                    self.source.env.as_deref().unwrap_or(""),
                    self.collect.snap_id.as_deref().unwrap_or(""),
                    start,
                    end
                )
            }
            AtlasInputMode::Incremental => format!(
                "{}:{}:{}:{}:{}:{}",
                self.schema,
                self.source.system,
                self.source.producer,
                self.source.tenant,
                self.source.env.as_deref().unwrap_or(""),
                self.collect.cursor.as_deref().unwrap_or("")
            ),
            AtlasInputMode::Correction => format!(
                "{}:{}:{}",
                self.schema,
                self.source.system,
                self.payload
                    .get("correction_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
        }
    }

    pub fn into_ingest_envelope(
        self,
        tenant_id: TenantId,
        environment_id: Option<EnvironmentId>,
        received_at: DateTime<Utc>,
    ) -> IngestEnvelope {
        let observed_at = self.collect.observed_at.map(ObservedAt);
        let received_at = self
            .collect
            .collected_or_observed_at()
            .unwrap_or(received_at);
        let ingest_id = self.idempotency_key();
        let source_kind = self.source_kind();
        let ingest_mode = self.ingest_mode();
        let mut metadata = BTreeMap::new();
        metadata.insert("schema".to_string(), self.schema.clone());
        metadata.insert("source.system".to_string(), self.source.system.clone());
        metadata.insert("source.producer".to_string(), self.source.producer.clone());
        metadata.insert("source.tenant".to_string(), self.source.tenant.clone());
        if let Some(env) = self.source.env.as_ref() {
            metadata.insert("source.env".to_string(), env.clone());
        }
        metadata.insert(
            "collect.mode".to_string(),
            self.collect.mode.as_str().to_string(),
        );
        metadata.insert("idempotency_key".to_string(), self.idempotency_key());
        if let Some(cursor) = self.collect.cursor.as_ref() {
            metadata.insert("collect.cursor".to_string(), cursor.clone());
        }
        if let Some(res_ver) = self.collect.res_ver.as_ref() {
            metadata.insert("collect.res_ver".to_string(), res_ver.clone());
        }

        IngestEnvelope {
            ingest_id,
            source_kind,
            source_name: format!("{}:{}", self.source.system, self.source.producer),
            ingest_mode,
            tenant_id,
            environment_id,
            observed_at,
            received_at,
            payload_ref: None,
            payload_inline: Some(self.payload),
            metadata,
        }
    }
}

impl IngestEnvelope {
    /// 快照作用域：`schema:system:producer:tenant:env`。
    ///
    /// 快照替换语义靠它回答「哪些对象属于同一份完整快照」。只按 `tenant_id` 划会把
    /// 同租户其它生产者的对象一起标 stale；`source_name` 已经是 `system:producer`。
    ///
    /// `env` 优先取生产方**声明**的 `source.env`，而不是解析后的 `environment_id`：
    /// CLI 导入路径目前不传 `environment_id`，用它会得到空串，两个环境就混成一个作用域。
    pub fn ingest_scope(&self) -> String {
        let schema = self
            .metadata
            .get("schema")
            .map(String::as_str)
            .unwrap_or("");
        let environment = self
            .metadata
            .get("source.env")
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| self.environment_id.map(|id| id.0.to_string()))
            .unwrap_or_default();
        format!(
            "{schema}:{}:{}:{environment}",
            self.source_name, self.tenant_id.0
        )
    }
}

fn require_non_empty(value: &str, field: &'static str) -> DomainResult<()> {
    if value.trim().is_empty() {
        Err(DomainReason::FieldInvalid
            .to_err()
            .with_detail(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

fn require_option_non_empty(value: Option<&str>, field: &'static str) -> DomainResult<()> {
    match value {
        Some(value) => require_non_empty(value, field),
        None => Err(missing_field(field, format!("{field} is required"))),
    }
}

fn require_json_string(value: Option<&Value>, field: &'static str) -> DomainResult<()> {
    match value {
        Some(Value::String(value)) => require_non_empty(value, field),
        Some(_) => Err(DomainReason::FieldInvalid
            .to_err()
            .with_detail(format!("{field} must be a string"))),
        None => Err(missing_field(field, format!("{field} is required"))),
    }
}

fn invalid_schema(detail: impl Into<String>) -> DomainError {
    DomainReason::SchemaInvalid.to_err().with_detail(detail)
}

fn missing_field(_field: &'static str, detail: impl Into<String>) -> DomainError {
    DomainReason::FieldMissing.to_err().with_detail(detail)
}

fn deserialize_optional_string_like<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringLike {
        String(String),
        Integer(i64),
        Unsigned(u64),
        Float(f64),
        Bool(bool),
    }

    let value = Option::<StringLike>::deserialize(deserializer)?;
    Ok(value.map(|value| match value {
        StringLike::String(value) => value,
        StringLike::Integer(value) => value.to_string(),
        StringLike::Unsigned(value) => value.to_string(),
        StringLike::Float(value) => value.to_string(),
        StringLike::Bool(value) => value.to_string(),
    }))
}

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestMode {
    Snapshot,
    Delta,
    BatchUpsert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngestEnvelope {
    pub ingest_id: String,
    pub source_kind: SourceKind,
    pub source_name: String,
    pub ingest_mode: IngestMode,
    pub tenant_id: TenantId,
    pub environment_id: Option<EnvironmentId>,
    pub observed_at: Option<ObservedAt>,
    pub received_at: DateTime<Utc>,
    pub payload_ref: Option<String>,
    pub payload_inline: Option<Value>,
    pub metadata: BTreeMap<String, String>,
}
