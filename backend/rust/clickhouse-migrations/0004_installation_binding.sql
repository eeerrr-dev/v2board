CREATE TABLE IF NOT EXISTS installation_binding
(
    singleton UInt8,
    installation_id UUID,
    bound_at_unix Int64,
    CONSTRAINT chk_single_installation_binding CHECK singleton = 1
)
ENGINE = MergeTree
ORDER BY (singleton, installation_id)
SETTINGS
    index_granularity = 8192,
    non_replicated_deduplication_window = 10000
