//! Durable PostgreSQL-to-ClickHouse analytics projection.
//!
//! PostgreSQL remains authoritative. This crate owns only typed immutable
//! events, the SQL outbox relay, and ClickHouse projection verification.

mod client;
mod event;
mod outbox;
mod projection;
mod schema;

pub use client::clickhouse_client;
pub use event::{
    ACCOUNTED_EVENT_NAME, AccountedOutcome, AccountedTrafficEvent, AnalyticsEvent,
    EventValidationError, IdentityKind, REPORTED_EVENT_NAME, ReportedTrafficEvent,
    TrafficEventCore, deterministic_event_id,
};
pub use outbox::{
    ClaimedBatch, DeliveryBatchState, OutboxBacklog, OutboxError, OutboxRecord, PruneResult,
    claim_delivery_batch, enqueue_event, enqueue_events, mark_batch_published, outbox_backlog,
    prune_published_outbox, quarantine_batch, release_batch_for_retry,
};
pub use projection::{BatchProjectionError, ProjectionStatus, project_or_verify_batch};
pub use schema::{
    CLICKHOUSE_MIGRATIONS, ClickHouseMigrationError, bind_clickhouse_installation,
    migrate_clickhouse, verify_clickhouse_runtime_ready,
};
