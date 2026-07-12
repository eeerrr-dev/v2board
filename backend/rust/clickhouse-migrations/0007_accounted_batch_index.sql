ALTER TABLE v2_traffic_accounted_v1
ADD INDEX IF NOT EXISTS idx_ingest_batch_id ingest_batch_id
TYPE bloom_filter(0.001) GRANULARITY 1
