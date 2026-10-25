pub mod memory;
pub mod migrations;
pub mod postgres;
pub mod repositories;

pub use memory::*;
pub use migrations::*;
pub use postgres::*;
pub use repositories::*;

use orion_error::{conversion::ToStructError, prelude::*, runtime::OperationContext};

pub type StorageError = StructError<StorageReason>;
pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, PartialEq, OrionError)]
pub enum StorageReason {
    #[orion_error(identity = "conf.atlas.storage.not_configured")]
    NotConfigured,
    #[orion_error(identity = "biz.atlas.storage.not_found")]
    NotFound,
    #[orion_error(identity = "sys.atlas.storage.operation_failed")]
    OperationFailed,
    #[orion_error(identity = "logic.atlas.storage.decode_failed")]
    DecodeFailed,
    #[orion_error(transparent)]
    General(UnifiedReason),
}

pub fn not_configured() -> StorageError {
    StorageReason::NotConfigured.to_err()
}

pub fn not_found() -> StorageError {
    StorageReason::NotFound.to_err()
}

pub fn operation_failed(detail: impl Into<String>) -> StorageError {
    StorageReason::OperationFailed
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("storage operation")
                .with_meta("component.name", "atlas-storage"),
        )
}

pub fn lock_failed() -> StorageError {
    operation_failed("storage state lock is poisoned")
}

pub fn decode_failed(detail: impl Into<String>) -> StorageError {
    StorageReason::DecodeFailed
        .to_err()
        .with_detail(detail)
        .with_context(
            OperationContext::doing("decode storage row")
                .with_meta("component.name", "atlas-storage"),
        )
}
