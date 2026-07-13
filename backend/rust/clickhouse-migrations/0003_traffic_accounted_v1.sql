CREATE TABLE IF NOT EXISTS traffic_accounted_v1
(
    event_id String,
    schema_major UInt16,
    installation_id UUID,
    report_key String,
    payload_hash String,
    identity_kind LowCardinality(String),
    user_id UInt64,
    traffic_epoch UInt64,
    server_id UInt64,
    server_type LowCardinality(String),
    rate_text String,
    rate_decimal_10_2 Decimal(10, 2),
    raw_u UInt64,
    raw_d UInt64,
    charged_u UInt64,
    charged_d UInt64,
    accepted_at_unix Int64,
    accounting_date Date,
    accounting_timezone LowCardinality(String),
    accounted_at_unix Int64,
    outcome LowCardinality(String),
    u_after Nullable(UInt64),
    d_after Nullable(UInt64),
    ingest_batch_id UUID,
    batch_row_number UInt32,
    outbox_payload_sha256 String,
    table_generation UInt32,
    ingested_at_unix Int64,
    INDEX idx_ingest_batch_id ingest_batch_id TYPE bloom_filter(0.001) GRANULARITY 1
)
ENGINE = MergeTree
PARTITION BY (table_generation, toYYYYMM(accounting_date))
ORDER BY
(
    installation_id,
    user_id,
    accounting_date,
    accounted_at_unix,
    event_id,
    ingest_batch_id,
    batch_row_number
)
SETTINGS
    index_granularity = 8192,
    non_replicated_deduplication_window = 10000
