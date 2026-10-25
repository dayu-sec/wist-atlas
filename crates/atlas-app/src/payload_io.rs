use std::fs;
use std::path::{Path, PathBuf};

use orion_error::prelude::SourceErr;
use serde_json::Value;

use crate::{AppReason, AppResult};

pub(crate) fn load_demo_payload() -> AppResult<Value> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/p0_monolith_demo.json");
    load_payload_from_file(path)
}

pub(crate) fn load_payload_from_file(path: impl AsRef<Path>) -> AppResult<Value> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).source_err(
        AppReason::InputLoadFailed,
        format!("read {}", path.display()),
    )?;
    serde_json::from_str(&raw).source_err(
        AppReason::InputLoadFailed,
        format!("parse {} as json", path.display()),
    )
}
