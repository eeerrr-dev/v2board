//! Subscription-gated node visibility and traffic-history use cases.

use v2board_domain_model::SubscriptionAvailability;

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceAccess {
    pub banned: bool,
    pub transfer_enable: i64,
    pub expiry: Option<i64>,
    pub group_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServiceServer {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub group_ids: Vec<i32>,
    pub route_ids: Option<Vec<i32>>,
    pub name: String,
    pub rate: f64,
    pub kind: String,
    pub host: String,
    pub port: i64,
    pub cache_key: String,
    pub last_check_at: Option<i64>,
    pub online: bool,
    pub tags: Option<Vec<String>>,
    pub sort: Option<i32>,
    /// Canonical JSON supplied by the persistence adapter. It is opaque to the
    /// use case and parsed into the transport-owned recursive value at egress.
    pub extra_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrafficRecord {
    pub upload: i64,
    pub download: i64,
    pub recorded_at: i64,
    pub user_id: i64,
    pub server_rate: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerPresenceKey {
    pub kind: String,
    pub check_id: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceUsageError {
    #[error("user is not registered")]
    UserNotRegistered,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait ServiceUsageRepository: Send + Sync {
    async fn find_access(&self, user_id: i64) -> RepositoryResult<Option<ServiceAccess>>;
    async fn available_servers(
        &self,
        group_id: Option<i32>,
    ) -> RepositoryResult<Vec<ServiceServer>>;
    async fn traffic_records(
        &self,
        user_id: i64,
        from_recorded_at: i64,
    ) -> RepositoryResult<Vec<TrafficRecord>>;
}

#[allow(async_fn_in_trait)]
pub trait ServerPresence: Send + Sync {
    async fn last_checks(
        &self,
        servers: &[ServerPresenceKey],
    ) -> RepositoryResult<Vec<Option<i64>>>;
}

#[derive(Clone, Debug)]
pub struct ServiceUsageService<R, P> {
    repository: R,
    presence: P,
}

impl<R, P> ServiceUsageService<R, P>
where
    R: ServiceUsageRepository,
    P: ServerPresence,
{
    pub const fn new(repository: R, presence: P) -> Self {
        Self {
            repository,
            presence,
        }
    }

    pub async fn servers(
        &self,
        user_id: i64,
        now: i64,
    ) -> Result<Vec<ServiceServer>, ServiceUsageError> {
        let access = self
            .repository
            .find_access(user_id)
            .await?
            .ok_or(ServiceUsageError::UserNotRegistered)?;
        if !(SubscriptionAvailability {
            banned: access.banned,
            transfer_enable: access.transfer_enable,
            expiry: access.expiry,
        })
        .is_available(now)
        {
            return Ok(Vec::new());
        }

        let mut servers = self.repository.available_servers(access.group_id).await?;
        let keys = servers
            .iter()
            .map(|server| ServerPresenceKey {
                kind: server.kind.clone(),
                check_id: server.parent_id.unwrap_or(server.id),
            })
            .collect::<Vec<_>>();
        let last_checks = self.presence.last_checks(&keys).await?;
        if last_checks.len() != servers.len() {
            return Err(RepositoryError::new(
                "load server presence",
                "presence adapter returned a different number of rows",
            )
            .into());
        }
        for (server, last_check_at) in servers.iter_mut().zip(last_checks) {
            server.last_check_at = last_check_at;
            server.online = server_is_online(now, last_check_at);
            if let Some((prefix, _)) = server.cache_key.rsplit_once('-') {
                server.cache_key = format!("{prefix}-{}", i16::from(server.online));
            }
        }
        Ok(servers)
    }

    pub async fn traffic(
        &self,
        user_id: i64,
        from_recorded_at: i64,
    ) -> Result<Vec<TrafficRecord>, ServiceUsageError> {
        Ok(self
            .repository
            .traffic_records(user_id, from_recorded_at)
            .await?)
    }
}

pub fn server_is_online(now: i64, last_check_at: Option<i64>) -> bool {
    i128::from(now) - 300 <= i128::from(last_check_at.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Clone)]
    struct FakeRepository {
        access: Option<ServiceAccess>,
        servers: Vec<ServiceServer>,
    }

    impl ServiceUsageRepository for FakeRepository {
        async fn find_access(&self, _user_id: i64) -> RepositoryResult<Option<ServiceAccess>> {
            Ok(self.access)
        }

        async fn available_servers(
            &self,
            _group_id: Option<i32>,
        ) -> RepositoryResult<Vec<ServiceServer>> {
            Ok(self.servers.clone())
        }

        async fn traffic_records(
            &self,
            _user_id: i64,
            _from_recorded_at: i64,
        ) -> RepositoryResult<Vec<TrafficRecord>> {
            Ok(Vec::new())
        }
    }

    #[derive(Clone, Default)]
    struct FakePresence(Arc<AtomicUsize>);

    impl ServerPresence for FakePresence {
        async fn last_checks(
            &self,
            servers: &[ServerPresenceKey],
        ) -> RepositoryResult<Vec<Option<i64>>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(vec![Some(1_000); servers.len()])
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
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
    fn online_boundary_is_exact_and_overflow_safe() {
        assert!(server_is_online(1_000, Some(700)));
        assert!(!server_is_online(1_001, Some(700)));
        assert!(server_is_online(i64::MIN, None));
    }

    #[test]
    fn unavailable_subscription_never_loads_or_exposes_nodes() {
        let presence = FakePresence::default();
        let service = ServiceUsageService::new(
            FakeRepository {
                access: Some(ServiceAccess {
                    banned: false,
                    transfer_enable: 0,
                    expiry: Some(2_000),
                    group_id: Some(1),
                }),
                servers: vec![ServiceServer {
                    id: 1,
                    parent_id: None,
                    group_ids: vec![1],
                    route_ids: None,
                    name: "secret node".to_string(),
                    rate: 1.0,
                    kind: "shadowsocks".to_string(),
                    host: "node.test".to_string(),
                    port: 443,
                    cache_key: "shadowsocks-1-1-0".to_string(),
                    last_check_at: None,
                    online: false,
                    tags: None,
                    sort: None,
                    extra_json: None,
                }],
            },
            presence.clone(),
        );

        let servers = block_on(service.servers(7, 1_000)).expect("gated server list");
        assert!(servers.is_empty());
        assert_eq!(presence.0.load(Ordering::Relaxed), 0);
    }
}
