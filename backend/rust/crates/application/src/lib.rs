//! Infrastructure-independent application use cases.
//!
//! This crate owns orchestration and outbound ports. PostgreSQL, Redis, HTTP,
//! runtime configuration, transport DTOs, and RFC error rendering belong to
//! outer adapters and are deliberately absent from this dependency graph.

pub mod account;
pub mod admin_order;
pub mod admin_user;
pub mod audit;
pub mod auth;
pub mod configuration;
pub mod content;
pub mod giftcard;
pub mod invite;
pub mod logs;
pub mod maintenance;
pub mod operator_access;
pub mod order;
pub mod order_jobs;
pub mod payment;
pub mod plan;
pub mod promotion;
pub mod reconciliation;
pub mod server_management;
pub mod server_runtime;
pub mod service_usage;
pub mod statistics;
pub mod subscription;
pub mod system_monitoring;
pub mod telegram;
pub mod ticket;
pub mod worker_mail;
pub mod worker_statistics;
pub mod worker_traffic;

mod error;

pub use error::{ApplicationError, RepositoryError};
