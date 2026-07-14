CREATE TABLE IF NOT EXISTS traffic_reported_daily
(
    installation_id UUID,
    accounting_date Date,
    user_id UInt64,
    server_id UInt64,
    server_type LowCardinality(String),
    rate_text String,
    rate_decimal_10_2 Decimal(10, 2),
    table_generation UInt32,
    ingest_batch_id UUID,
    batch_aggregate_row_number UInt32,
    event_count UInt64,
    raw_u UInt64,
    raw_d UInt64,
    charged_u UInt64,
    charged_d UInt64
)
ENGINE = SummingMergeTree((event_count, raw_u, raw_d, charged_u, charged_d))
PARTITION BY (table_generation, toYYYYMM(accounting_date))
ORDER BY
(
    installation_id,
    accounting_date,
    user_id,
    server_id,
    server_type,
    rate_text,
    rate_decimal_10_2,
    table_generation,
    ingest_batch_id,
    batch_aggregate_row_number
)
SETTINGS
    index_granularity = 8192,
    non_replicated_deduplication_window = 10000
