#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

pub const INITIAL_SCHEMA_VERSION: &str = "0001_initial_schema";

pub const INITIAL_SCHEMA_SQL: &str = include_str!("../migrations/0001_initial_schema.sql");

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: INITIAL_SCHEMA_VERSION,
    sql: INITIAL_SCHEMA_SQL,
}];

#[cfg(test)]
mod tests {
    use super::INITIAL_SCHEMA_SQL;

    #[test]
    fn initial_schema_contains_p0_tables() {
        for table in [
            "host_inventory",
            "network_domain",
            "network_segment",
            "host_net_assoc",
            "host_runtime_state",
            "process_runtime_state",
            "service_instance",
            "runtime_binding",
            "subject",
            "responsibility_assignment",
            "ingest_job",
        ] {
            assert!(
                INITIAL_SCHEMA_SQL.contains(table),
                "initial schema should contain {table}"
            );
        }
    }

    #[test]
    fn initial_schema_records_migration_version() {
        assert!(INITIAL_SCHEMA_SQL.contains("0001_initial_schema"));
    }
}
