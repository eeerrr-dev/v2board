use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const REPORTED_EVENT_NAME: &str = "traffic.reported.v1";
pub const ACCOUNTED_EVENT_NAME: &str = "traffic.accounted.v1";
const EVENT_SCHEMA_MAJOR: i16 = 1;
const EVENT_ID_DOMAIN: &[u8] = b"v2board.analytics.event-id.v2\0";
const PAYLOAD_HASH_DOMAIN: &[u8] = b"v2board.analytics.payload.v1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Explicit,
    Implicit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountedOutcome {
    Applied,
    StaleEpoch,
    MissingUser,
}

/// Common immutable traffic facts. Integer-like fields intentionally use
/// decimal strings so event JSON survives JavaScript and heterogeneous
/// analytics consumers without precision loss.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrafficEventCore {
    pub installation_id: String,
    pub report_key: String,
    pub payload_hash: String,
    pub identity_kind: IdentityKind,
    pub user_id: String,
    pub traffic_epoch: String,
    pub server_id: String,
    pub server_type: String,
    pub rate_text: String,
    pub rate_decimal_10_2: String,
    pub raw_u: String,
    pub raw_d: String,
    pub charged_u: String,
    pub charged_d: String,
    pub accepted_at: i64,
    pub accounting_date: String,
    pub accounting_timezone: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportedTrafficEvent {
    pub event_id: String,
    pub event_name: String,
    pub schema_major: i16,
    #[serde(flatten)]
    pub core: TrafficEventCore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccountedTrafficEvent {
    pub event_id: String,
    pub event_name: String,
    pub schema_major: i16,
    #[serde(flatten)]
    pub core: TrafficEventCore,
    pub accounted_at: i64,
    pub outcome: AccountedOutcome,
    pub u_after: Option<String>,
    pub d_after: Option<String>,
}

/// The exact row persisted in PostgreSQL's typed outbox.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnalyticsEvent {
    pub event_id: String,
    pub event_name: String,
    pub schema_major: i16,
    pub report_key: String,
    pub partition_month: String,
    pub occurred_at: i64,
    pub payload: Value,
    pub payload_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EventValidationError {
    #[error("{0} must not be empty")]
    Empty(&'static str),
    #[error("{0} is not a lower-case SHA-256 hex digest")]
    Digest(&'static str),
    #[error("{0} is not a supported non-negative decimal integer")]
    Integer(&'static str),
    #[error("{0} is not a supported positive decimal integer")]
    PositiveInteger(&'static str),
    #[error("rate field {0} is not a finite decimal")]
    Decimal(&'static str),
    #[error("accounting timezone must be Asia/Shanghai")]
    Timezone,
    #[error("accounting_date is invalid")]
    AccountingDate,
    #[error("installation_id must be a canonical UUID")]
    InstallationId,
    #[error("accounted_at must be at or after accepted_at")]
    AccountedBeforeAccepted,
    #[error("event identity does not match its immutable fields")]
    EventIdentity,
    #[error("applied accounted events require both authoritative post-update counters")]
    MissingPostUpdateCounters,
    #[error("non-applied accounted events must not claim authoritative post-update counters")]
    UnexpectedPostUpdateCounters,
    #[error("event payload could not be serialized: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl ReportedTrafficEvent {
    pub fn new(core: TrafficEventCore) -> Result<Self, EventValidationError> {
        core.validate()?;
        Ok(Self {
            event_id: deterministic_event_id(
                REPORTED_EVENT_NAME,
                &core.installation_id,
                &core.report_key,
                &core.user_id,
            ),
            event_name: REPORTED_EVENT_NAME.to_string(),
            schema_major: EVENT_SCHEMA_MAJOR,
            core,
        })
    }

    pub fn into_outbox(self) -> Result<AnalyticsEvent, EventValidationError> {
        self.core.validate()?;
        validate_envelope(
            &self.event_id,
            &self.event_name,
            self.schema_major,
            &self.core,
            REPORTED_EVENT_NAME,
        )?;
        AnalyticsEvent::from_payload(
            self.event_id.clone(),
            self.event_name.clone(),
            self.schema_major,
            self.core.report_key.clone(),
            self.core.accounting_date.clone(),
            self.core.accepted_at,
            &self,
        )
    }
}

impl AccountedTrafficEvent {
    pub fn new(
        core: TrafficEventCore,
        accounted_at: i64,
        outcome: AccountedOutcome,
        u_after: Option<String>,
        d_after: Option<String>,
    ) -> Result<Self, EventValidationError> {
        core.validate()?;
        validate_accounted_counters(outcome, u_after.as_deref(), d_after.as_deref())?;
        if accounted_at < core.accepted_at {
            return Err(EventValidationError::AccountedBeforeAccepted);
        }
        Ok(Self {
            event_id: deterministic_event_id(
                ACCOUNTED_EVENT_NAME,
                &core.installation_id,
                &core.report_key,
                &core.user_id,
            ),
            event_name: ACCOUNTED_EVENT_NAME.to_string(),
            schema_major: EVENT_SCHEMA_MAJOR,
            core,
            accounted_at,
            outcome,
            u_after,
            d_after,
        })
    }

    pub fn into_outbox(self) -> Result<AnalyticsEvent, EventValidationError> {
        self.core.validate()?;
        validate_envelope(
            &self.event_id,
            &self.event_name,
            self.schema_major,
            &self.core,
            ACCOUNTED_EVENT_NAME,
        )?;
        validate_accounted_counters(
            self.outcome,
            self.u_after.as_deref(),
            self.d_after.as_deref(),
        )?;
        if self.accounted_at < self.core.accepted_at {
            return Err(EventValidationError::AccountedBeforeAccepted);
        }
        AnalyticsEvent::from_payload(
            self.event_id.clone(),
            self.event_name.clone(),
            self.schema_major,
            self.core.report_key.clone(),
            self.core.accounting_date.clone(),
            self.accounted_at,
            &self,
        )
    }
}

impl TrafficEventCore {
    fn validate(&self) -> Result<(), EventValidationError> {
        for (name, value) in [
            ("report_key", self.report_key.as_str()),
            ("server_type", self.server_type.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(EventValidationError::Empty(name));
            }
        }
        let installation_id = Uuid::parse_str(&self.installation_id)
            .map_err(|_| EventValidationError::InstallationId)?;
        if installation_id.to_string() != self.installation_id {
            return Err(EventValidationError::InstallationId);
        }
        validate_digest("payload_hash", &self.payload_hash)?;
        validate_positive_integer("user_id", &self.user_id)?;
        validate_non_negative_integer("traffic_epoch", &self.traffic_epoch)?;
        validate_positive_integer("server_id", &self.server_id)?;
        validate_non_negative_decimal("rate_text", &self.rate_text)?;
        validate_decimal_10_2("rate_decimal_10_2", &self.rate_decimal_10_2)?;
        for (name, value) in [
            ("raw_u", self.raw_u.as_str()),
            ("raw_d", self.raw_d.as_str()),
            ("charged_u", self.charged_u.as_str()),
            ("charged_d", self.charged_d.as_str()),
        ] {
            validate_non_negative_integer(name, value)?;
        }
        if self.accepted_at < 0 {
            return Err(EventValidationError::Integer("accepted_at"));
        }
        if self.accounting_timezone != "Asia/Shanghai" {
            return Err(EventValidationError::Timezone);
        }
        NaiveDate::parse_from_str(&self.accounting_date, "%Y-%m-%d")
            .map_err(|_| EventValidationError::AccountingDate)?;
        Ok(())
    }
}

impl AnalyticsEvent {
    fn from_payload<T: Serialize>(
        event_id: String,
        event_name: String,
        schema_major: i16,
        report_key: String,
        accounting_date: String,
        occurred_at: i64,
        event: &T,
    ) -> Result<Self, EventValidationError> {
        let payload = canonical_value(serde_json::to_value(event)?);
        let canonical_bytes = serde_json::to_vec(&payload)?;
        let payload_sha256 = sha256_domain(PAYLOAD_HASH_DOMAIN, &canonical_bytes);
        let date = NaiveDate::parse_from_str(&accounting_date, "%Y-%m-%d")
            .map_err(|_| EventValidationError::AccountingDate)?;
        Ok(Self {
            event_id,
            event_name,
            schema_major,
            report_key,
            partition_month: date.format("%Y-%m-01").to_string(),
            occurred_at,
            payload,
            payload_sha256,
        })
    }
}

pub fn deterministic_event_id(
    event_name: &str,
    installation_id: &str,
    report_key: &str,
    user_id: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(EVENT_ID_DOMAIN);
    for field in [
        event_name.as_bytes(),
        installation_id.as_bytes(),
        report_key.as_bytes(),
        user_id.as_bytes(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    hex::encode(digest.finalize())
}

fn validate_envelope(
    event_id: &str,
    event_name: &str,
    schema_major: i16,
    core: &TrafficEventCore,
    expected_name: &str,
) -> Result<(), EventValidationError> {
    let expected_id = deterministic_event_id(
        expected_name,
        &core.installation_id,
        &core.report_key,
        &core.user_id,
    );
    if event_name != expected_name || schema_major != EVENT_SCHEMA_MAJOR || event_id != expected_id
    {
        return Err(EventValidationError::EventIdentity);
    }
    Ok(())
}

fn validate_accounted_counters(
    outcome: AccountedOutcome,
    u_after: Option<&str>,
    d_after: Option<&str>,
) -> Result<(), EventValidationError> {
    match (outcome, u_after, d_after) {
        (AccountedOutcome::Applied, Some(u), Some(d)) => {
            validate_non_negative_integer("u_after", u)?;
            validate_non_negative_integer("d_after", d)
        }
        (AccountedOutcome::Applied, _, _) => Err(EventValidationError::MissingPostUpdateCounters),
        (_, None, None) => Ok(()),
        _ => Err(EventValidationError::UnexpectedPostUpdateCounters),
    }
}

fn validate_digest(name: &'static str, value: &str) -> Result<(), EventValidationError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(EventValidationError::Digest(name))
    }
}

fn validate_non_negative_integer(
    name: &'static str,
    value: &str,
) -> Result<(), EventValidationError> {
    value
        .parse::<u128>()
        .map(|_| ())
        .map_err(|_| EventValidationError::Integer(name))
}

fn validate_positive_integer(name: &'static str, value: &str) -> Result<(), EventValidationError> {
    value
        .parse::<u128>()
        .ok()
        .filter(|value| *value > 0)
        .map(|_| ())
        .ok_or(EventValidationError::PositiveInteger(name))
}

fn validate_non_negative_decimal(
    name: &'static str,
    value: &str,
) -> Result<(), EventValidationError> {
    let decimal = value
        .parse::<Decimal>()
        .map_err(|_| EventValidationError::Decimal(name))?;
    if decimal.is_sign_negative() {
        return Err(EventValidationError::Decimal(name));
    }
    Ok(())
}

fn validate_decimal_10_2(name: &'static str, value: &str) -> Result<(), EventValidationError> {
    let decimal = value
        .parse::<Decimal>()
        .map_err(|_| EventValidationError::Decimal(name))?;
    if decimal.is_sign_negative() || decimal.scale() > 2 || decimal > Decimal::new(9_999_999_999, 2)
    {
        return Err(EventValidationError::Decimal(name));
    }
    Ok(())
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonical_value(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        value => value,
    }
}

fn sha256_domain(domain: &[u8], payload: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((payload.len() as u64).to_be_bytes());
    digest.update(payload);
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> TrafficEventCore {
        TrafficEventCore {
            installation_id: "40aa4a80-eb4b-4b25-9c3b-e17ed047873d".into(),
            report_key: "abc".into(),
            payload_hash: "a".repeat(64),
            identity_kind: IdentityKind::Explicit,
            user_id: "9007199254740993".into(),
            traffic_epoch: "7".into(),
            server_id: "42".into(),
            server_type: "vmess".into(),
            rate_text: "1.255".into(),
            rate_decimal_10_2: "1.26".into(),
            raw_u: "10".into(),
            raw_d: "20".into(),
            charged_u: "13".into(),
            charged_d: "25".into(),
            accepted_at: 1_700_000_000,
            accounting_date: "2026-07-12".into(),
            accounting_timezone: "Asia/Shanghai".into(),
        }
    }

    #[test]
    fn event_ids_are_stable_and_separate_event_meanings() {
        let reported = ReportedTrafficEvent::new(core()).unwrap();
        let accounted = AccountedTrafficEvent::new(
            core(),
            1_700_000_001,
            AccountedOutcome::Applied,
            Some("100".into()),
            Some("200".into()),
        )
        .unwrap();
        assert_eq!(
            reported.event_id,
            deterministic_event_id(
                REPORTED_EVENT_NAME,
                "40aa4a80-eb4b-4b25-9c3b-e17ed047873d",
                "abc",
                "9007199254740993"
            )
        );
        assert_ne!(reported.event_id, accounted.event_id);
    }

    #[test]
    fn payload_hash_and_month_are_deterministic() {
        let first = ReportedTrafficEvent::new(core())
            .unwrap()
            .into_outbox()
            .unwrap();
        let second = ReportedTrafficEvent::new(core())
            .unwrap()
            .into_outbox()
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.partition_month, "2026-07-01");
        assert_eq!(first.payload_sha256.len(), 64);
    }

    #[test]
    fn accounted_outcomes_pin_counter_semantics() {
        assert!(
            AccountedTrafficEvent::new(core(), 1, AccountedOutcome::Applied, None, None).is_err()
        );
        assert!(
            AccountedTrafficEvent::new(
                core(),
                1,
                AccountedOutcome::StaleEpoch,
                Some("1".into()),
                Some("2".into())
            )
            .is_err()
        );
    }

    #[test]
    fn clickhouse_decimal_and_accounting_time_are_validated_before_delivery() {
        let mut invalid = core();
        invalid.rate_decimal_10_2 = "100000000.00".into();
        assert!(ReportedTrafficEvent::new(invalid).is_err());

        let mut invalid = core();
        invalid.rate_decimal_10_2 = "-0.01".into();
        assert!(ReportedTrafficEvent::new(invalid).is_err());

        assert!(
            AccountedTrafficEvent::new(
                core(),
                1,
                AccountedOutcome::Applied,
                Some("1".into()),
                Some("2".into()),
            )
            .is_err()
        );
    }
}
