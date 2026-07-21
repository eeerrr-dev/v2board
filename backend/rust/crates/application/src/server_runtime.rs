//! Use cases and outbound ports for the byte-frozen node-agent APIs.

use crate::RepositoryError;
use v2board_domain_model::ServerKind;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeServerNode {
    pub id: i32,
    pub group_ids: Vec<i32>,
    pub route_ids: Vec<i32>,
    pub rate: String,
    pub host: String,
    pub server_port: i32,
    pub created_at: i64,
    pub listen_ip: Option<String>,
    pub protocol: Option<String>,
    pub version: Option<i32>,
    pub tls: Option<i16>,
    pub tls_settings_json: Option<String>,
    pub flow: Option<String>,
    pub network: Option<String>,
    pub network_settings_json: Option<String>,
    pub encryption: Option<String>,
    pub encryption_settings_json: Option<String>,
    pub zero_rtt_handshake: Option<i16>,
    pub congestion_control: Option<String>,
    pub cipher: Option<String>,
    pub obfs: Option<String>,
    pub obfs_settings_json: Option<String>,
    pub obfs_password: Option<String>,
    pub padding_scheme_json: Option<String>,
    pub server_name: Option<String>,
    pub up_mbps: Option<i32>,
    pub down_mbps: Option<i32>,
    pub dns_settings_json: Option<String>,
    pub rule_settings_json: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeServerUser {
    pub id: i64,
    pub uuid: String,
    pub speed_limit: Option<i32>,
    pub device_limit: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeServerRoute {
    pub id: i32,
    pub match_json: String,
    pub action: String,
    pub action_value_json: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTrafficEntry {
    pub user_id: i64,
    pub upload: i64,
    pub download: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistTrafficReport {
    pub installation_id: String,
    pub report_key: String,
    pub payload_hash: String,
    pub node_id: i32,
    pub node_kind: ServerKind,
    pub group_ids: Vec<i32>,
    pub rate: String,
    pub entries: Vec<RuntimeTrafficEntry>,
    pub accepted_at: i64,
    pub accounting_date: String,
    pub accounting_record_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerMetric {
    OnlineUser,
    LastCheckAt,
    LastPushAt,
}

impl ServerMetric {
    pub const fn key_suffix(self) -> &'static str {
        match self {
            Self::OnlineUser => "ONLINE_USER",
            Self::LastCheckAt => "LAST_CHECK_AT",
            Self::LastPushAt => "LAST_PUSH_AT",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliveUpdate {
    pub user_id: i64,
    pub ips_json: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistTrafficError {
    #[error("traffic report idempotency key was reused with a different payload")]
    IdempotencyConflict,
    #[error("traffic report contains an unauthorized user")]
    UnauthorizedUser,
    #[error("server traffic rate is outside the supported range")]
    RateOutOfRange,
    #[error("server traffic charge is outside the supported range")]
    ChargeOutOfRange,
    #[error("server traffic total is outside the supported range")]
    TotalOutOfRange,
    #[error("traffic analytics admission is soft rate limited")]
    AnalyticsRateLimited,
    #[error("traffic analytics admission is unavailable")]
    AnalyticsUnavailable,
    #[error("traffic analytics event is invalid")]
    AnalyticsEventInvalid,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait ServerRuntimeRepository: Send + Sync {
    async fn credential_epoch(
        &self,
        kind: ServerKind,
        node_id: i32,
    ) -> RepositoryResult<Option<i64>>;
    async fn node(
        &self,
        kind: ServerKind,
        node_id: i32,
    ) -> RepositoryResult<Option<RuntimeServerNode>>;
    async fn available_users(
        &self,
        group_ids: &[i32],
        now: i64,
    ) -> RepositoryResult<Vec<RuntimeServerUser>>;
    async fn routes(&self, route_ids: &[i32]) -> RepositoryResult<Vec<RuntimeServerRoute>>;
    async fn alive_user_ids(&self, now: i64) -> RepositoryResult<Vec<i64>>;
    async fn persist_traffic(
        &self,
        report: PersistTrafficReport,
    ) -> Result<(), PersistTrafficError>;
}

#[allow(async_fn_in_trait)]
pub trait ServerRuntimeCache: Send + Sync {
    async fn write_metric(
        &self,
        kind: ServerKind,
        node_id: i32,
        metric: ServerMetric,
        value: i64,
    ) -> RepositoryResult<()>;
    async fn alive_counts(&self, user_ids: &[i64]) -> RepositoryResult<Vec<Option<i64>>>;
    async fn merge_alive(
        &self,
        node_bucket: &str,
        now: i64,
        device_limit_mode: i32,
        updates: &[AliveUpdate],
    ) -> RepositoryResult<()>;
}

pub trait NodeCredentialVerifier: Send + Sync {
    fn verify(&self, kind: ServerKind, node_id: i32, epoch: i64, candidate: &str) -> bool;
}

#[derive(Clone, Debug)]
pub struct ServerRuntimeService<R, C, V> {
    repository: R,
    cache: C,
    credentials: V,
}

impl<R, C, V> ServerRuntimeService<R, C, V>
where
    R: ServerRuntimeRepository,
    C: ServerRuntimeCache,
    V: NodeCredentialVerifier,
{
    pub const fn new(repository: R, cache: C, credentials: V) -> Self {
        Self {
            repository,
            cache,
            credentials,
        }
    }

    pub async fn authenticate(
        &self,
        kind: ServerKind,
        node_id: i32,
        candidate: &str,
    ) -> RepositoryResult<bool> {
        let Some(epoch) = self.repository.credential_epoch(kind, node_id).await? else {
            return Ok(false);
        };
        Ok(self.credentials.verify(kind, node_id, epoch, candidate))
    }

    pub async fn node(
        &self,
        kind: ServerKind,
        node_id: i32,
    ) -> RepositoryResult<Option<RuntimeServerNode>> {
        self.repository.node(kind, node_id).await
    }

    pub async fn users(
        &self,
        group_ids: &[i32],
        now: i64,
    ) -> RepositoryResult<Vec<RuntimeServerUser>> {
        self.repository.available_users(group_ids, now).await
    }

    pub async fn routes(&self, route_ids: &[i32]) -> RepositoryResult<Vec<RuntimeServerRoute>> {
        self.repository.routes(route_ids).await
    }

    pub async fn write_metric(
        &self,
        kind: ServerKind,
        node_id: i32,
        metric: ServerMetric,
        value: i64,
    ) -> RepositoryResult<()> {
        self.cache.write_metric(kind, node_id, metric, value).await
    }

    pub async fn alive_counts(&self, now: i64) -> RepositoryResult<Vec<(i64, i64)>> {
        let user_ids = self.repository.alive_user_ids(now).await?;
        let counts = self.cache.alive_counts(&user_ids).await?;
        if counts.len() != user_ids.len() {
            return Err(RepositoryError::new(
                "load alive user counts",
                "cache returned a different number of rows",
            ));
        }
        Ok(user_ids
            .into_iter()
            .zip(counts)
            .filter_map(|(user_id, count)| count.map(|count| (user_id, count)))
            .collect())
    }

    pub async fn merge_alive(
        &self,
        node_bucket: &str,
        now: i64,
        device_limit_mode: i32,
        updates: &[AliveUpdate],
    ) -> RepositoryResult<()> {
        self.cache
            .merge_alive(node_bucket, now, device_limit_mode, updates)
            .await
    }

    pub async fn persist_traffic(
        &self,
        report: PersistTrafficReport,
    ) -> Result<(), PersistTrafficError> {
        self.repository.persist_traffic(report).await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Clone, Copy)]
    struct FakeRepository;

    impl ServerRuntimeRepository for FakeRepository {
        async fn credential_epoch(&self, _: ServerKind, _: i32) -> RepositoryResult<Option<i64>> {
            Ok(Some(4))
        }
        async fn node(&self, _: ServerKind, _: i32) -> RepositoryResult<Option<RuntimeServerNode>> {
            Ok(None)
        }
        async fn available_users(
            &self,
            _: &[i32],
            _: i64,
        ) -> RepositoryResult<Vec<RuntimeServerUser>> {
            Ok(Vec::new())
        }
        async fn routes(&self, _: &[i32]) -> RepositoryResult<Vec<RuntimeServerRoute>> {
            Ok(Vec::new())
        }
        async fn alive_user_ids(&self, _: i64) -> RepositoryResult<Vec<i64>> {
            Ok(vec![7, 9])
        }
        async fn persist_traffic(
            &self,
            _: PersistTrafficReport,
        ) -> Result<(), PersistTrafficError> {
            Ok(())
        }
    }

    #[derive(Clone, Copy)]
    struct FakeCache;

    impl ServerRuntimeCache for FakeCache {
        async fn write_metric(
            &self,
            _: ServerKind,
            _: i32,
            _: ServerMetric,
            _: i64,
        ) -> RepositoryResult<()> {
            Ok(())
        }
        async fn alive_counts(&self, _: &[i64]) -> RepositoryResult<Vec<Option<i64>>> {
            Ok(vec![Some(2), None])
        }
        async fn merge_alive(
            &self,
            _: &str,
            _: i64,
            _: i32,
            _: &[AliveUpdate],
        ) -> RepositoryResult<()> {
            Ok(())
        }
    }

    #[derive(Clone, Copy)]
    struct FakeCredentials;

    impl NodeCredentialVerifier for FakeCredentials {
        fn verify(&self, kind: ServerKind, node_id: i32, epoch: i64, candidate: &str) -> bool {
            (kind, node_id, epoch, candidate) == (ServerKind::V2node, 7, 4, "ok")
        }
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn authentication_is_bound_to_repository_epoch_and_identity() {
        let service = ServerRuntimeService::new(FakeRepository, FakeCache, FakeCredentials);
        assert!(run(service.authenticate(ServerKind::V2node, 7, "ok")).unwrap());
        assert!(!run(service.authenticate(ServerKind::V2node, 8, "ok")).unwrap());
    }

    #[test]
    fn alive_counts_preserve_user_alignment_and_drop_cache_misses() {
        let service = ServerRuntimeService::new(FakeRepository, FakeCache, FakeCredentials);
        assert_eq!(run(service.alive_counts(100)).unwrap(), vec![(7, 2)]);
    }
}
