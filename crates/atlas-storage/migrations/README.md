# atlas-storage migrations

This directory contains SQL migrations for the PostgreSQL source-of-truth schema.

Current convention:

- Files are ordered by a numeric prefix, for example `0001_initial_schema.sql`.
- Migrations are intended to be run once per database in lexical order.
- `schema_migrations` records applied versions.
- Rollback files are intentionally not added yet; early schema changes should be handled by forward migrations while the model is still stabilizing.

The storage crate currently exposes migration SQL as embedded constants. A concrete database runner can execute these scripts once the PostgreSQL client choice is finalized.
