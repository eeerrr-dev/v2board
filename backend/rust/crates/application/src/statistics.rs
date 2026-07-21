//! Operator reporting use cases and their persistence/calendar ports.

use std::collections::BTreeMap;

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticsWindow {
    Today,
    Previous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticsBucket {
    Daily,
    Monthly,
}

impl StatisticsBucket {
    pub const fn storage_code(self) -> &'static str {
        match self {
            Self::Daily => "d",
            Self::Monthly => "m",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatisticsBoundaries {
    pub now: i64,
    pub today: i64,
    pub yesterday: i64,
    pub month: i64,
    pub previous_month: i64,
}

impl StatisticsBoundaries {
    pub const fn range(self, window: StatisticsWindow) -> (i64, i64) {
        match window {
            StatisticsWindow::Today => (self.today, self.now),
            StatisticsWindow::Previous => (self.yesterday, self.today),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StatisticsSummary {
    pub online_user: i64,
    pub month_income: i64,
    pub month_register_total: i64,
    pub day_register_total: i64,
    pub ticket_pending_total: i64,
    pub commission_pending_total: i64,
    pub payment_reconciliation_pending_total: i64,
    pub payment_reconciliation_pending_amount: i64,
    pub day_income: i64,
    pub last_month_income: i64,
    pub commission_month_payout: i64,
    pub commission_last_month_payout: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerRankSource {
    pub server_id: i64,
    pub server_type: String,
    pub upload: i64,
    pub download: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerRank {
    pub server_id: i64,
    pub server_type: String,
    pub server_name: Option<String>,
    pub upload: i64,
    pub download: i64,
    pub total_gib: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserRankSource {
    pub user_id: i64,
    pub server_rate: f64,
    pub upload: i64,
    pub download: i64,
    pub email: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserRank {
    pub user_id: i64,
    pub email: String,
    pub upload: i64,
    pub download: i64,
    pub total_gib: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatSeriesSource {
    pub recorded_at: i64,
    pub register_count: i64,
    pub paid_total: i64,
    pub paid_count: i64,
    pub commission_total: i64,
    pub commission_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatSeriesPoint {
    pub series: &'static str,
    pub date: String,
    pub value: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UserTraffic {
    pub recorded_at: i64,
    pub upload: i64,
    pub download: i64,
    pub server_rate: f64,
}

#[allow(async_fn_in_trait)]
pub trait StatisticsRepository: Send + Sync {
    async fn summary(
        &self,
        boundaries: StatisticsBoundaries,
    ) -> RepositoryResult<StatisticsSummary>;
    async fn server_rank_sources(
        &self,
        start: i64,
        end: i64,
    ) -> RepositoryResult<(Vec<ServerRankSource>, BTreeMap<(String, i64), String>)>;
    async fn user_rank_sources(
        &self,
        start: i64,
        end: i64,
    ) -> RepositoryResult<Vec<UserRankSource>>;
    async fn series_sources(
        &self,
        bucket: StatisticsBucket,
    ) -> RepositoryResult<Vec<StatSeriesSource>>;
    async fn user_traffic(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepositoryResult<(Vec<UserTraffic>, i64)>;
}

pub trait StatisticsCalendar: Send + Sync {
    fn boundaries(&self) -> StatisticsBoundaries;
    fn month_day(&self, timestamp: i64) -> String;
}

#[derive(Clone, Debug)]
pub struct StatisticsService<R, C> {
    repository: R,
    calendar: C,
}

impl<R, C> StatisticsService<R, C>
where
    R: StatisticsRepository,
    C: StatisticsCalendar,
{
    pub const fn new(repository: R, calendar: C) -> Self {
        Self {
            repository,
            calendar,
        }
    }

    pub async fn summary(&self) -> RepositoryResult<StatisticsSummary> {
        self.repository.summary(self.calendar.boundaries()).await
    }

    pub async fn server_rank(&self, window: StatisticsWindow) -> RepositoryResult<Vec<ServerRank>> {
        const GIB: f64 = 1_073_741_824.0;
        let (start, end) = self.calendar.boundaries().range(window);
        let (sources, names) = self.repository.server_rank_sources(start, end).await?;
        let mut rows = sources
            .into_iter()
            .map(|source| {
                let name_key = (
                    canonical_server_type(&source.server_type).to_string(),
                    source.server_id,
                );
                ServerRank {
                    server_id: source.server_id,
                    server_type: source.server_type,
                    server_name: names.get(&name_key).cloned(),
                    upload: source.upload,
                    download: source.download,
                    total_gib: (source.upload as f64 + source.download as f64) / GIB,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| right.total_gib.total_cmp(&left.total_gib));
        Ok(rows)
    }

    pub async fn user_rank(&self, window: StatisticsWindow) -> RepositoryResult<Vec<UserRank>> {
        const GIB: f64 = 1_073_741_824.0;
        let (start, end) = self.calendar.boundaries().range(window);
        let sources = self.repository.user_rank_sources(start, end).await?;
        let mut order = Vec::new();
        let mut totals = BTreeMap::<i64, UserRank>::new();
        for source in sources {
            let weighted =
                (source.upload as f64 + source.download as f64) * source.server_rate / GIB;
            if let Some(row) = totals.get_mut(&source.user_id) {
                row.total_gib += weighted;
            } else {
                order.push(source.user_id);
                totals.insert(
                    source.user_id,
                    UserRank {
                        user_id: source.user_id,
                        email: source.email.unwrap_or_else(|| "null".to_string()),
                        upload: source.upload,
                        download: source.download,
                        total_gib: weighted,
                    },
                );
            }
        }
        let mut rows = order
            .into_iter()
            .filter_map(|user_id| totals.remove(&user_id))
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| right.total_gib.total_cmp(&left.total_gib));
        rows.truncate(15);
        Ok(rows)
    }

    pub async fn series(&self, bucket: StatisticsBucket) -> RepositoryResult<Vec<StatSeriesPoint>> {
        let sources = self.repository.series_sources(bucket).await?;
        let mut points = Vec::with_capacity(sources.len() * 5);
        for source in sources {
            let date = self.calendar.month_day(source.recorded_at);
            points.push(StatSeriesPoint {
                series: "register_count",
                date: date.clone(),
                value: source.register_count,
            });
            points.push(StatSeriesPoint {
                series: "paid_total",
                date: date.clone(),
                value: source.paid_total,
            });
            points.push(StatSeriesPoint {
                series: "paid_count",
                date: date.clone(),
                value: source.paid_count,
            });
            points.push(StatSeriesPoint {
                series: "commission_paid_total",
                date: date.clone(),
                value: source.commission_total,
            });
            points.push(StatSeriesPoint {
                series: "commission_paid_count",
                date,
                value: source.commission_count,
            });
        }
        points.reverse();
        Ok(points)
    }

    pub async fn user_traffic(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepositoryResult<(Vec<UserTraffic>, i64)> {
        self.repository.user_traffic(user_id, limit, offset).await
    }
}

fn canonical_server_type(server_type: &str) -> &str {
    if server_type == "v2ray" {
        "vmess"
    } else {
        server_type
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    #[derive(Default)]
    struct FakeState {
        server: Vec<ServerRankSource>,
        users: Vec<UserRankSource>,
        series: Vec<StatSeriesSource>,
        observed_range: Option<(i64, i64)>,
    }

    impl StatisticsRepository for FakeRepository {
        async fn summary(&self, _: StatisticsBoundaries) -> RepositoryResult<StatisticsSummary> {
            Ok(StatisticsSummary::default())
        }

        async fn server_rank_sources(
            &self,
            start: i64,
            end: i64,
        ) -> RepositoryResult<(Vec<ServerRankSource>, BTreeMap<(String, i64), String>)> {
            self.0.lock().unwrap().observed_range = Some((start, end));
            Ok((
                self.0.lock().unwrap().server.clone(),
                BTreeMap::from([(("vmess".to_string(), 1), "node".to_string())]),
            ))
        }

        async fn user_rank_sources(&self, _: i64, _: i64) -> RepositoryResult<Vec<UserRankSource>> {
            Ok(self.0.lock().unwrap().users.clone())
        }

        async fn series_sources(
            &self,
            _: StatisticsBucket,
        ) -> RepositoryResult<Vec<StatSeriesSource>> {
            Ok(self.0.lock().unwrap().series.clone())
        }

        async fn user_traffic(
            &self,
            _: i64,
            _: i64,
            _: i64,
        ) -> RepositoryResult<(Vec<UserTraffic>, i64)> {
            Ok((Vec::new(), 0))
        }
    }

    #[derive(Clone, Copy)]
    struct FakeCalendar;

    impl StatisticsCalendar for FakeCalendar {
        fn boundaries(&self) -> StatisticsBoundaries {
            StatisticsBoundaries {
                now: 300,
                today: 200,
                yesterday: 100,
                month: 50,
                previous_month: 10,
            }
        }

        fn month_day(&self, timestamp: i64) -> String {
            format!("date-{timestamp}")
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
    fn server_ranking_owns_window_type_alias_and_weighting() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().server = vec![ServerRankSource {
            server_id: 1,
            server_type: "v2ray".to_string(),
            upload: 1_073_741_824,
            download: 0,
        }];
        let rows = run(StatisticsService::new(repository.clone(), FakeCalendar)
            .server_rank(StatisticsWindow::Previous))
        .unwrap();
        assert_eq!(
            repository.0.lock().unwrap().observed_range,
            Some((100, 200))
        );
        assert_eq!(rows[0].server_name.as_deref(), Some("node"));
        assert_eq!(rows[0].total_gib, 1.0);
    }

    #[test]
    fn user_ranking_aggregates_weighted_totals_but_keeps_first_raw_row() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().users = vec![
            UserRankSource {
                user_id: 7,
                server_rate: 1.0,
                upload: 1_073_741_824,
                download: 0,
                email: Some("first@example.test".to_string()),
            },
            UserRankSource {
                user_id: 7,
                server_rate: 2.0,
                upload: 1_073_741_824,
                download: 0,
                email: Some("ignored@example.test".to_string()),
            },
        ];
        let rows = run(
            StatisticsService::new(repository, FakeCalendar).user_rank(StatisticsWindow::Today)
        )
        .unwrap();
        assert_eq!(rows[0].email, "first@example.test");
        assert_eq!(rows[0].upload, 1_073_741_824);
        assert_eq!(rows[0].total_gib, 3.0);
    }

    #[test]
    fn series_projection_is_oldest_first_with_stable_machine_slugs() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().series = vec![StatSeriesSource {
            recorded_at: 5,
            register_count: 1,
            paid_total: 2,
            paid_count: 3,
            commission_total: 4,
            commission_count: 5,
        }];
        let rows =
            run(StatisticsService::new(repository, FakeCalendar).series(StatisticsBucket::Daily))
                .unwrap();
        assert_eq!(rows[0].series, "commission_paid_count");
        assert_eq!(rows[4].series, "register_count");
        assert_eq!(rows[0].date, "date-5");
    }
}
