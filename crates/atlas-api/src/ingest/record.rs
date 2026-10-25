use std::sync::{Arc, Mutex};

use atlas_domain::{AtlasInputEnvelope, IngestEnvelope, IngestMode, SourceKind};
use chrono::{DateTime, Utc};
use orion_error::conversion::ConvErr;
use serde::{Deserialize, Serialize};

use crate::error::{ApiResult, missing_payload, recorder_unavailable, unsupported_ingest_mode};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestJobStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestJobRecord {
    pub ingest_id: String,
    pub tenant_id: atlas_domain::TenantId,
    pub source_kind: SourceKind,
    pub source_name: String,
    pub received_at: DateTime<Utc>,
    pub status: IngestJobStatus,
    pub payload_ref: Option<String>,
    pub error: Option<String>,
}

impl IngestJobRecord {
    pub fn accepted(envelope: &IngestEnvelope) -> Self {
        Self {
            ingest_id: envelope.ingest_id.clone(),
            tenant_id: envelope.tenant_id,
            source_kind: envelope.source_kind,
            source_name: envelope.source_name.clone(),
            received_at: envelope.received_at,
            status: IngestJobStatus::Accepted,
            payload_ref: envelope.payload_ref.clone(),
            error: None,
        }
    }
}

pub trait IngestJobRecorder {
    fn record_ingest_job(&self, record: IngestJobRecord) -> ApiResult<()>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryIngestJobRecorder {
    records: Arc<Mutex<Vec<IngestJobRecord>>>,
}

impl InMemoryIngestJobRecorder {
    pub fn records(&self) -> ApiResult<Vec<IngestJobRecord>> {
        self.records
            .lock()
            .map(|records| records.clone())
            .map_err(|_| recorder_unavailable())
    }
}

impl IngestJobRecorder for InMemoryIngestJobRecorder {
    fn record_ingest_job(&self, record: IngestJobRecord) -> ApiResult<()> {
        self.records
            .lock()
            .map_err(|_| recorder_unavailable())?
            .push(record);
        Ok(())
    }
}

pub struct IngestService<R> {
    recorder: R,
}

impl<R> IngestService<R>
where
    R: IngestJobRecorder,
{
    pub fn new(recorder: R) -> Self {
        Self { recorder }
    }

    pub fn submit(&self, envelope: IngestEnvelope) -> ApiResult<IngestJobRecord> {
        if envelope.payload_inline.is_none() && envelope.payload_ref.is_none() {
            return Err(missing_payload());
        }

        if envelope.ingest_mode == IngestMode::Delta {
            return Err(unsupported_ingest_mode());
        }

        let record = IngestJobRecord::accepted(&envelope);
        self.recorder.record_ingest_job(record.clone())?;
        Ok(record)
    }

    pub fn submit_atlas_input(
        &self,
        input: AtlasInputEnvelope,
        tenant_id: atlas_domain::TenantId,
        environment_id: Option<atlas_domain::EnvironmentId>,
    ) -> ApiResult<IngestJobRecord> {
        input.validate().conv_err()?;
        self.submit(input.into_ingest_envelope(tenant_id, environment_id, Utc::now()))
    }
}
