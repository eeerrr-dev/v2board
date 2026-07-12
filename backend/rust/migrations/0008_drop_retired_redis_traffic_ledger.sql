-- Traffic reports are now committed to v2_server_traffic_report before the API
-- acknowledges a node. The former Redis accumulator has no producer in the
-- native runtime, so its SQL replay marker is dead state rather than a fallback.
DROP TABLE `v2_traffic_batch`;
