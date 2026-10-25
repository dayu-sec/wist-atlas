use atlas_api::ApiReason;
use atlas_domain::DomainReason;
use atlas_storage::StorageReason;
use orion_error::{conversion::ToStructError, prelude::*, runtime::OperationContext};

pub type SyncError = StructError<SyncReason>;
pub type SyncResult<T> = Result<T, SyncError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum SyncReason {
    #[orion_error(identity = "sys.atlas.sync.input_load_failed")]
    InputLoadFailed,
    #[orion_error(identity = "sys.atlas.sync.fetch_failed")]
    FetchFailed,
    #[orion_error(identity = "biz.atlas.sync.cursor_conflict")]
    CursorConflict,
    #[orion_error(identity = "sys.atlas.sync.rate_limited")]
    RateLimited,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

impl From<StorageReason> for SyncReason {
    fn from(value: StorageReason) -> Self {
        match value {
            StorageReason::General(reason) => SyncReason::General(reason),
            _ => SyncReason::FetchFailed,
        }
    }
}

impl From<ApiReason> for SyncReason {
    fn from(value: ApiReason) -> Self {
        match value {
            ApiReason::General(reason) => SyncReason::General(reason),
            _ => SyncReason::FetchFailed,
        }
    }
}

impl From<DomainReason> for SyncReason {
    fn from(value: DomainReason) -> Self {
        match value {
            DomainReason::SchemaInvalid
            | DomainReason::SchemaUnsupported
            | DomainReason::PayloadInvalid
            | DomainReason::FieldMissing
            | DomainReason::FieldInvalid => SyncReason::InputLoadFailed,
            DomainReason::RefUnresolved | DomainReason::IdentityConflict => SyncReason::FetchFailed,
            DomainReason::General(reason) => SyncReason::General(reason),
        }
    }
}

pub fn fetch_failed(detail: impl Into<String>) -> SyncError {
    SyncReason::FetchFailed
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("fetch external topology source")
                .with_meta("component.name", "atlas-sync"),
        )
}

pub fn cursor_conflict(detail: impl Into<String>) -> SyncError {
    SyncReason::CursorConflict
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("advance sync cursor")
                .with_meta("component.name", "atlas-sync"),
        )
}

pub fn rate_limited(detail: impl Into<String>) -> SyncError {
    SyncReason::RateLimited
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("fetch external topology source")
                .with_meta("component.name", "atlas-sync"),
        )
}
