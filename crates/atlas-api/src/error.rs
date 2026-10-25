use atlas_domain::DomainReason;
use atlas_storage::StorageReason;
use orion_error::{conversion::ToStructError, prelude::*, runtime::OperationContext};

pub type ApiError = StructError<ApiReason>;
pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum ApiReason {
    #[orion_error(identity = "biz.atlas.api.payload_missing")]
    PayloadMissing,
    #[orion_error(identity = "biz.atlas.api.ingest_mode_unsupported")]
    UnsupportedIngestMode,
    #[orion_error(identity = "biz.atlas.api.payload_invalid")]
    PayloadInvalid,
    #[orion_error(identity = "biz.atlas.api.field_missing")]
    FieldMissing,
    #[orion_error(identity = "biz.atlas.api.field_invalid")]
    FieldInvalid,
    #[orion_error(identity = "biz.atlas.api.ingest_rejected")]
    IngestRejected,
    #[orion_error(identity = "biz.atlas.api.query_invalid")]
    QueryInvalid,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

impl From<DomainReason> for ApiReason {
    fn from(value: DomainReason) -> Self {
        match value {
            DomainReason::SchemaInvalid
            | DomainReason::SchemaUnsupported
            | DomainReason::PayloadInvalid
            | DomainReason::FieldMissing
            | DomainReason::FieldInvalid => ApiReason::PayloadInvalid,
            DomainReason::RefUnresolved | DomainReason::IdentityConflict => {
                ApiReason::IngestRejected
            }
            DomainReason::General(reason) => ApiReason::General(reason),
        }
    }
}

impl From<StorageReason> for ApiReason {
    fn from(value: StorageReason) -> Self {
        match value {
            StorageReason::NotConfigured | StorageReason::OperationFailed => {
                ApiReason::IngestRejected
            }
            StorageReason::DecodeFailed => ApiReason::IngestRejected,
            StorageReason::NotFound => ApiReason::QueryInvalid,
            StorageReason::General(reason) => ApiReason::General(reason),
        }
    }
}

pub fn missing_payload() -> ApiError {
    ApiReason::PayloadMissing.to_err()
}

pub fn unsupported_ingest_mode() -> ApiError {
    ApiReason::UnsupportedIngestMode
        .to_err()
        .with_detail("ingest mode `delta` is not supported yet")
}

pub fn payload_must_be_object() -> ApiError {
    ApiReason::PayloadInvalid
        .to_err()
        .with_detail("payload_inline must be a JSON object")
}

pub fn missing_field(field: &'static str) -> ApiError {
    ApiReason::FieldMissing
        .to_err()
        .with_detail(format!("payload field `{field}` is required"))
        .with_context(
            OperationContext::doing("validate payload field")
                .with_meta("field_path", field)
                .with_meta("component.name", "atlas-api"),
        )
}

pub fn invalid_field_type(field: &'static str) -> ApiError {
    ApiReason::FieldInvalid
        .to_err()
        .with_detail(format!("payload field `{field}` has invalid type"))
        .with_context(
            OperationContext::doing("validate payload field type")
                .with_meta("field_path", field)
                .with_meta("component.name", "atlas-api"),
        )
}

pub fn invalid_field_value(field: &'static str, value: impl Into<String>) -> ApiError {
    let value = value.into();
    ApiReason::FieldInvalid
        .to_err()
        .with_detail(format!(
            "payload field `{field}` has invalid value `{}`",
            value
        ))
        .with_context(
            OperationContext::doing("validate payload field value")
                .with_meta("field_path", field)
                .with_meta("component.name", "atlas-api"),
        )
}

pub fn recorder_failed(detail: impl Into<String>) -> ApiError {
    ApiReason::IngestRejected
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("record ingest job").with_meta("component.name", "atlas-api"),
        )
}

pub fn recorder_unavailable() -> ApiError {
    recorder_failed("ingest job recorder is unavailable")
}
