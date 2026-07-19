//! Deterministic import contract for the pinned legacy MySQL source profile.
//!
//! The pre-release schema v1 reads one stopped database snapshot into one empty
//! target. The separate dump is backup evidence, not converter input. The
//! mapping and loss policy below are the complete MySQL import contract.

mod audit;
mod hashing;
mod mappings;
mod sql;
mod transform;
mod values;

#[cfg(test)]
mod tests;

pub use audit::*;
pub use hashing::*;
pub use mappings::*;
pub use sql::*;
pub use transform::*;
pub use values::*;

pub const MYSQL_IMPORT_SOURCE_PROFILE: &str =
    "wyx2685-v2board@7e77de9f4873b317157490529f7be7d6f8a62421";
/// Schema identity of only the source tables that contribute retained rows.
/// Discard-only tables are deliberately excluded so harmless legacy residue
/// cannot change or block the typed import contract.
pub const MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256: &str =
    "264b474fcd7af15fdeca1ac335a3613e166ebfc6a884745eb5f82af3107c7afb";
pub const MYSQL_SOURCE_INSTALL_SQL_SHA256: &str =
    "04b04531037b9e0b6f2a6b02194a8f1bc102789af8ee7be963fd721d51bca8e2";
pub const MYSQL_IMPORT_SCHEMA_VERSION: u32 = 1;
pub const TARGET_POSTGRES_SCHEMA_ID: &str = "migrations-postgres/mysql-import-v1";
pub const MYSQL_IMPORT_REGISTRY_VERSION: u32 = 1;
/// Native identities are positive. The source preflight rejects every
/// non-positive business primary key before any target is created, so ordered
/// source streams start at the exact lower bound of the accepted domain.
pub const SOURCE_ID_LOWER_BOUND: i64 = 0;

const TARGET_POSTGRES_MIGRATIONS: &[(i64, &str, &[u8])] = &[
    (
        1,
        "0001_initial.sql",
        include_bytes!("../../../migrations-postgres/0001_initial.sql"),
    ),
    (
        2,
        "0002_import_finalize.sql",
        include_bytes!("../../../migrations-postgres/0002_import_finalize.sql"),
    ),
];
