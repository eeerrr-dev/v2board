ALTER TABLE v2_traffic_accounted_v1
MATERIALIZE INDEX idx_ingest_batch_id
SETTINGS mutations_sync = 2
