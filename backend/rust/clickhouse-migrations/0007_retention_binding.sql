CREATE TABLE IF NOT EXISTS retention_binding
(
    singleton UInt8,
    installation_id UUID,
    raw_retention_days UInt32,
    aggregate_retention_days UInt32,
    bound_at_unix Int64,
    CONSTRAINT chk_single_retention_binding CHECK singleton = 1,
    CONSTRAINT chk_raw_retention_positive CHECK raw_retention_days > 0,
    CONSTRAINT chk_aggregate_retention_order CHECK aggregate_retention_days >= raw_retention_days
)
ENGINE = MergeTree
ORDER BY (singleton, installation_id)
SETTINGS
    index_granularity = 8192,
    non_replicated_deduplication_window = 10000
