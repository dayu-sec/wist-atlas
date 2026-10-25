use std::sync::Arc;

use tokio_postgres::{NoTls, types::ToSql};

use crate::{StorageResult, lock_failed, operation_failed};

use super::{PostgresExecutor, row_decode::row_to_strings, sql};

#[derive(Debug, Clone, Default)]
pub struct RecordingExecutor {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>,
}

impl RecordingExecutor {
    pub fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls
            .lock()
            .expect("recording executor poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryPostgresExecutor {
    state: std::sync::Arc<std::sync::Mutex<MemoryPostgresState>>,
}

#[derive(Clone)]
pub struct LivePostgresExecutor {
    client: Arc<tokio_postgres::Client>,
}

#[derive(Debug, Default)]
struct MemoryPostgresState {
    hosts: std::collections::BTreeMap<String, Vec<String>>,
    services: std::collections::BTreeMap<String, Vec<String>>,
    network_domains: std::collections::BTreeMap<String, Vec<String>>,
    network_segments: std::collections::BTreeMap<String, Vec<String>>,
    host_net_assocs: std::collections::BTreeMap<String, Vec<String>>,
    host_runtime_states: std::collections::BTreeMap<String, Vec<String>>,
    process_runtime_states: std::collections::BTreeMap<String, Vec<String>>,
    service_instances: std::collections::BTreeMap<String, Vec<String>>,
    runtime_bindings: std::collections::BTreeMap<String, Vec<String>>,
    subjects: std::collections::BTreeMap<String, Vec<String>>,
    responsibility_assignments: std::collections::BTreeMap<String, Vec<String>>,
    ingest_jobs: std::collections::BTreeMap<String, Vec<String>>,
}

impl PostgresExecutor for RecordingExecutor {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        self.calls
            .lock()
            .map_err(|_| lock_failed())?
            .push((sql.to_string(), params.to_vec()));
        Ok(1)
    }

    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>> {
        self.calls
            .lock()
            .map_err(|_| lock_failed())?
            .push((sql.to_string(), params.to_vec()));
        Ok(Vec::new())
    }

    fn exec_batch(&self, sql: &str) -> StorageResult<()> {
        self.calls
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?
            .push((sql.to_string(), Vec::new()));
        Ok(())
    }

    fn reset_public_schema(&self) -> StorageResult<()> {
        self.calls
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?
            .push((sql::RESET_PUBLIC_SCHEMA.to_string(), Vec::new()));
        Ok(())
    }
}

impl PostgresExecutor for MemoryPostgresExecutor {
    fn exec(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        let mut state = self.state.lock().map_err(|_| lock_failed())?;

        match sql {
            value if value == sql::UPSERT_HOST => {
                let machine_id = params[4].clone();
                if !machine_id.is_empty() {
                    let existing_key = state
                        .hosts
                        .iter()
                        .find_map(|(key, row)| (row[4] == machine_id).then(|| key.clone()));
                    if let Some(existing_key) = existing_key {
                        state.hosts.insert(existing_key, params.to_vec());
                    } else {
                        state.hosts.insert(params[0].clone(), params.to_vec());
                    }
                } else {
                    state.hosts.insert(params[0].clone(), params.to_vec());
                }
            }
            value if value == sql::UPSERT_HOST_WITHOUT_MACHINE_ID => {
                state.hosts.insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_SERVICE => {
                state.services.insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_NETWORK_DOMAIN => {
                state
                    .network_domains
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_NETWORK_SEGMENT => {
                state
                    .network_segments
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_HOST_NET_ASSOC => {
                state
                    .host_net_assocs
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_HOST_RUNTIME_STATE => {
                state
                    .host_runtime_states
                    .insert(format!("{}:{}", params[0], params[1]), params.to_vec());
            }
            value if value == sql::UPSERT_PROCESS_RUNTIME_STATE => {
                state
                    .process_runtime_states
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_SERVICE_INSTANCE => {
                state
                    .service_instances
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_RUNTIME_BINDING => {
                state
                    .runtime_bindings
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_SUBJECT => {
                state.subjects.insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_RESPONSIBILITY_ASSIGNMENT => {
                state
                    .responsibility_assignments
                    .insert(params[0].clone(), params.to_vec());
            }
            value if value == sql::UPSERT_INGEST_JOB => {
                state.ingest_jobs.insert(params[0].clone(), params.to_vec());
            }
            _ => {}
        }

        Ok(1)
    }

    fn query_rows(&self, sql: &str, params: &[String]) -> StorageResult<Vec<Vec<String>>> {
        let state = self.state.lock().map_err(|_| lock_failed())?;

        let rows = match sql {
            value if value == sql::GET_HOST => {
                state.hosts.get(&params[0]).into_iter().cloned().collect()
            }
            value if value == sql::GET_SERVICE => state
                .services
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_HOSTS => state
                .hosts
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_SERVICES => state
                .services
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_ALL_HOSTS => state.hosts.values().cloned().collect(),
            value if value == sql::GET_NETWORK_SEGMENT => state
                .network_segments
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_NETWORK_SEGMENTS => state
                .network_segments
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_HOST_NET_ASSOCS => state
                .host_net_assocs
                .values()
                .filter(|row| row[2] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_HOST_RUNTIME_STATES => state
                .host_runtime_states
                .values()
                .filter(|row| row[0] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_PROCESS_RUNTIME_STATES => state
                .process_runtime_states
                .values()
                .filter(|row| row[2] == params[0])
                .cloned()
                .collect(),
            value if value == sql::GET_SERVICE_INSTANCE => state
                .service_instances
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_SERVICE_INSTANCES => state
                .service_instances
                .values()
                .filter(|row| row[2] == params[0])
                .cloned()
                .collect(),
            value if value == sql::GET_RUNTIME_BINDING => state
                .runtime_bindings
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_RUNTIME_BINDINGS_FOR_INSTANCE => state
                .runtime_bindings
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value == sql::LIST_RUNTIME_BINDINGS_FOR_OBJECT => state
                .runtime_bindings
                .values()
                .filter(|row| row[2] == params[0] && row[3] == params[1])
                .cloned()
                .collect(),
            value if value == sql::GET_SUBJECT => state
                .subjects
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::GET_INGEST_JOB => state
                .ingest_jobs
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value == sql::LIST_RESPONSIBILITY_ASSIGNMENTS_FOR_TARGET => state
                .responsibility_assignments
                .values()
                .filter(|row| row[3] == params[0] && row[4] == params[1])
                .cloned()
                .collect(),
            value if value.contains("FROM network_domain WHERE network_domain_id = $1") => state
                .network_domains
                .get(&params[0])
                .into_iter()
                .cloned()
                .collect(),
            value if value.contains("FROM subject WHERE tenant_id = $1") => state
                .subjects
                .values()
                .filter(|row| row[1] == params[0])
                .cloned()
                .collect(),
            value if value.contains("FROM responsibility_assignment WHERE assignment_id = $1") => {
                state
                    .responsibility_assignments
                    .get(&params[0])
                    .into_iter()
                    .cloned()
                    .collect()
            }
            _ => Vec::new(),
        };

        Ok(rows)
    }

    fn exec_batch(&self, _sql: &str) -> StorageResult<()> {
        Ok(())
    }

    fn reset_public_schema(&self) -> StorageResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|err| operation_failed(err.to_string()))?;
        *state = MemoryPostgresState::default();
        Ok(())
    }
}

impl LivePostgresExecutor {
    pub async fn new(connection_string: impl Into<String>) -> StorageResult<Self> {
        let connection_string = connection_string.into();
        let (client, connection) = tokio_postgres::connect(connection_string.as_str(), NoTls)
            .await
            .map_err(|err| operation_failed(format!("connect postgres: {err}")))?;

        tokio::spawn(async move {
            let _ = connection.await;
        });

        Ok(Self {
            client: Arc::new(client),
        })
    }

    pub(super) async fn query_rows_async(
        &self,
        sql: &str,
        params: &[String],
    ) -> StorageResult<Vec<Vec<String>>> {
        let bind_params: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|value| value as &(dyn ToSql + Sync))
            .collect();
        let rows = self
            .client
            .query(sql, &bind_params)
            .await
            .map_err(|err| operation_failed(format!("query postgres sql: {err}")))?;
        rows.into_iter().map(row_to_strings).collect()
    }

    pub(super) async fn exec_async(&self, sql: &str, params: &[String]) -> StorageResult<u64> {
        let bind_params: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|value| value as &(dyn ToSql + Sync))
            .collect();
        self.client
            .execute(sql, &bind_params)
            .await
            .map_err(|err| operation_failed(format!("execute postgres sql: {err}")))
    }

    pub(super) async fn exec_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.client
            .batch_execute(sql)
            .await
            .map_err(|err| operation_failed(format!("execute postgres batch sql: {err}")))
    }
}
