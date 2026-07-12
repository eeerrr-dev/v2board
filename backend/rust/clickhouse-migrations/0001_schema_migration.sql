CREATE TABLE IF NOT EXISTS v2_schema_migration
(
    version UInt64,
    name String,
    checksum String,
    applied_at_unix Int64
)
ENGINE = MergeTree
ORDER BY (version, applied_at_unix, checksum)
