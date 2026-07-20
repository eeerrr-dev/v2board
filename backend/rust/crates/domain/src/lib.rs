//! Application services and use-case orchestration.
//!
//! Despite the historical package path, this crate is intentionally not the
//! pure domain model: it coordinates PostgreSQL, Redis, mail, and external
//! providers. Infrastructure-free business concepts belong in
//! `v2board-domain-model`; HTTP DTOs belong in `v2board-api-contract`.

pub mod admin;
pub mod auth;
pub mod http_response;
pub mod mail;
pub mod operator_config;
pub mod order;
pub mod payment_provider;
pub mod payment_secrets;
pub mod redis_runtime;
pub mod server_credentials;
pub mod smtp;
pub mod subscribe_link;
