use orion_error::{prelude::*, reason::ErrorIdentityProvider};
use serde::{Deserialize, Serialize};

pub type DomainError = StructError<DomainReason>;
pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum DomainReason {
    #[orion_error(identity = "biz.atlas.domain.schema_invalid")]
    SchemaInvalid,
    #[orion_error(identity = "biz.atlas.domain.schema_unsupported")]
    SchemaUnsupported,
    #[orion_error(identity = "biz.atlas.domain.payload_invalid")]
    PayloadInvalid,
    #[orion_error(identity = "biz.atlas.domain.field_missing")]
    FieldMissing,
    #[orion_error(identity = "biz.atlas.domain.field_invalid")]
    FieldInvalid,
    #[orion_error(identity = "biz.atlas.domain.ref_unresolved")]
    RefUnresolved,
    #[orion_error(identity = "biz.atlas.domain.identity_conflict")]
    IdentityConflict,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    Intake,
    Validate,
    Normalize,
    Resolve,
    Persist,
    Derive,
    Query,
    Sync,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineScope {
    Request,
    Batch,
    Row,
    Candidate,
    Relation,
    Job,
    View,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineAction {
    Reject,
    RowReject,
    DeadLetter,
    Unresolved,
    Retry,
    Alert,
    ReturnApiError,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineDecision {
    pub stage: PipelineStage,
    pub scope: PipelineScope,
    pub action: PipelineAction,
    pub code: String,
    pub retryable: bool,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestJobError {
    pub code: String,
    pub stage: PipelineStage,
    pub action: PipelineAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_ref: Option<String>,
}

pub fn protocol_code(identity_code: &str) -> &'static str {
    match identity_code {
        "biz.atlas.api.payload_missing" => "input.payload_missing",
        "biz.atlas.api.schema_unsupported" => "input.schema_unsupported",
        "biz.atlas.api.ingest_rejected" => "input.ingest_rejected",
        "biz.atlas.api.query_invalid" => "query.invalid_filter",
        "biz.atlas.domain.schema_invalid" => "input.envelope_invalid",
        "biz.atlas.domain.schema_unsupported" => "input.schema_unsupported",
        "biz.atlas.domain.payload_invalid" => "input.payload_invalid",
        "biz.atlas.domain.field_missing" => "validate.field_missing",
        "biz.atlas.domain.field_invalid" => "validate.type_mismatch",
        "biz.atlas.domain.ref_unresolved" => "resolve.ref_unresolved",
        "biz.atlas.domain.identity_conflict" => "resolve.identity_conflict",
        "conf.atlas.storage.not_configured" => "system.dependency_unavailable",
        "biz.atlas.storage.not_found" => "query.not_found",
        "sys.atlas.storage.operation_failed" => "persist.transaction_failed",
        "logic.atlas.storage.decode_failed" => "persist.decode_failed",
        "biz.atlas.app.invalid_args" => "input.envelope_invalid",
        "sys.atlas.app.input_load_failed" => "input.raw_parse_failed",
        "logic.atlas.app.materialization_missing" => "system.internal",
        "sys.atlas.sync.fetch_failed" => "sync.fetch_failed",
        "sys.atlas.sync.input_load_failed" => "input.raw_parse_failed",
        "biz.atlas.sync.cursor_conflict" => "sync.cursor_conflict",
        "sys.atlas.sync.rate_limited" => "sync.rate_limited",
        _ => "system.internal",
    }
}

pub fn decision_for_identity(identity_code: &str) -> PipelineDecision {
    let code = protocol_code(identity_code).to_string();
    match code.as_str() {
        value if value.starts_with("input.") => PipelineDecision {
            stage: PipelineStage::Intake,
            scope: PipelineScope::Request,
            action: PipelineAction::Reject,
            code,
            retryable: false,
            severity: ErrorSeverity::Warning,
        },
        value if value.starts_with("validate.") => PipelineDecision {
            stage: PipelineStage::Validate,
            scope: PipelineScope::Batch,
            action: PipelineAction::Reject,
            code,
            retryable: false,
            severity: ErrorSeverity::Warning,
        },
        value if value.starts_with("resolve.") => PipelineDecision {
            stage: PipelineStage::Resolve,
            scope: PipelineScope::Candidate,
            action: PipelineAction::Unresolved,
            code,
            retryable: false,
            severity: ErrorSeverity::Warning,
        },
        value if value.starts_with("persist.") => PipelineDecision {
            stage: PipelineStage::Persist,
            scope: PipelineScope::Job,
            action: PipelineAction::Retry,
            code,
            retryable: true,
            severity: ErrorSeverity::Error,
        },
        value if value.starts_with("query.") => PipelineDecision {
            stage: PipelineStage::Query,
            scope: PipelineScope::Request,
            action: PipelineAction::ReturnApiError,
            code,
            retryable: false,
            severity: ErrorSeverity::Warning,
        },
        value if value.starts_with("sync.") => PipelineDecision {
            stage: PipelineStage::Sync,
            scope: PipelineScope::Job,
            action: PipelineAction::Retry,
            code,
            retryable: true,
            severity: ErrorSeverity::Error,
        },
        _ => PipelineDecision {
            stage: PipelineStage::Persist,
            scope: PipelineScope::System,
            action: PipelineAction::Alert,
            code,
            retryable: false,
            severity: ErrorSeverity::Error,
        },
    }
}

pub fn decision_for_error<R>(error: &StructError<R>) -> PipelineDecision
where
    R: orion_error::reason::DomainReason + ErrorIdentityProvider,
{
    decision_for_identity(&error.identity_snapshot().code)
}

pub fn protocol_error_for<R>(error: &StructError<R>) -> ProtocolErrorBody
where
    R: orion_error::reason::DomainReason + ErrorIdentityProvider,
{
    let identity = error.identity_snapshot();
    let decision = decision_for_identity(&identity.code);
    ProtocolErrorBody {
        code: decision.code,
        message: identity.reason,
        request_id: None,
        ingest_id: None,
        fields: context_fields(error),
        retryable: decision.retryable,
    }
}

pub fn ingest_job_error_for<R>(error: &StructError<R>) -> IngestJobError
where
    R: orion_error::reason::DomainReason + ErrorIdentityProvider,
{
    let identity = error.identity_snapshot();
    let decision = decision_for_identity(&identity.code);
    IngestJobError {
        code: decision.code,
        stage: decision.stage,
        action: decision.action,
        message: identity.detail.or(Some(identity.reason)),
        row_ref: None,
        field_path: context_meta(error, "field_path"),
        raw_ref: None,
    }
}

fn context_fields<R>(error: &StructError<R>) -> Vec<String>
where
    R: orion_error::reason::DomainReason + ErrorIdentityProvider,
{
    context_meta(error, "field_path").into_iter().collect()
}

fn context_meta<R>(error: &StructError<R>, key: &str) -> Option<String>
where
    R: orion_error::reason::DomainReason + ErrorIdentityProvider,
{
    error.context_metadata().get(key).map(ToString::to_string)
}
