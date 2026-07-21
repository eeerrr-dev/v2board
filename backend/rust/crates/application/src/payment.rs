//! Administrative payment-method use cases and outbound ports.
//!
//! Provider configuration reaches this layer only after the transport has
//! decoded one of the closed provider DTOs. Encrypted JSON envelopes, crypto,
//! PostgreSQL, runtime configuration, and HTTP errors remain outer-adapter
//! concerns.

use std::collections::{BTreeMap, BTreeSet};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

pub type ProviderConfig = BTreeMap<String, String>;
pub type ProviderForm = BTreeMap<String, ProviderFormField>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFormField {
    pub label: String,
    pub description: String,
    pub kind: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaymentMethodRecord {
    pub id: i32,
    pub name: String,
    pub provider: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    /// Exact PostgreSQL NUMERIC spelling. Conversion to a JSON number is an
    /// inbound-adapter concern.
    pub handling_fee_percent: Option<String>,
    pub uuid: String,
    /// The authenticated at-rest envelope. Only the security adapter may open
    /// it; repositories and application use cases keep it opaque.
    pub sealed_config: String,
    pub notify_domain: Option<String>,
    pub enable: bool,
    pub sort: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaymentMethod {
    pub id: i32,
    pub name: String,
    pub provider: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    pub handling_fee_percent: Option<String>,
    pub uuid: String,
    pub config: ProviderConfig,
    pub notify_domain: Option<String>,
    pub notify_url: String,
    pub enable: bool,
    pub sort: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
    pub legacy_md5_signature: bool,
    pub security_warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaymentCreateInput {
    pub name: String,
    pub provider: String,
    pub config: ProviderConfig,
    pub icon: Option<String>,
    pub notify_domain: Option<String>,
    pub handling_fee_fixed: Option<i64>,
    pub handling_fee_percent: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaymentPatchInput {
    pub name: Option<String>,
    pub icon: Option<Option<String>>,
    pub notify_domain: Option<Option<String>>,
    pub handling_fee_fixed: Option<Option<i64>>,
    pub handling_fee_percent: Option<Option<String>>,
    pub enable: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewPaymentMethod {
    pub name: String,
    pub provider: String,
    pub config: String,
    pub uuid: String,
    pub icon: Option<String>,
    pub notify_domain: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    pub handling_fee_percent: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaymentChanges {
    pub name: Option<String>,
    pub icon: Option<Option<String>>,
    pub notify_domain: Option<Option<String>>,
    pub handling_fee_fixed: Option<Option<i32>>,
    pub handling_fee_percent: Option<Option<String>>,
    pub enable: Option<bool>,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangePaymentOutcome {
    Updated,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchivePaymentOutcome {
    Archived,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortPaymentsOutcome {
    Sorted,
    PaymentSetChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentInputViolation {
    EmptyName,
    EmptyProvider,
    UnknownProvider,
    EmptyConfig,
    InvalidNotifyDomain,
    FixedFeeOutOfRange,
    PercentFeeOutOfRange,
    SortIdOutOfRange,
    DuplicateSortId,
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("payment security adapter failed during {operation}: {message}")]
pub struct PaymentSecurityError {
    operation: &'static str,
    message: String,
}

impl PaymentSecurityError {
    pub fn new(operation: &'static str, message: impl ToString) -> Self {
        Self {
            operation,
            message: message.to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    #[error("application URL is not configured")]
    AppUrlNotConfigured,
    #[error("invalid payment-method input: {0:?}")]
    InvalidInput(PaymentInputViolation),
    #[error("payment method not found")]
    NotFound,
    #[error("payment-method set changed concurrently")]
    UpdateConflict,
    #[error(transparent)]
    Security(#[from] PaymentSecurityError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait PaymentRepository: Send + Sync {
    async fn list_active(&self) -> RepositoryResult<Vec<PaymentMethodRecord>>;
    async fn find_active(&self, id: i32) -> RepositoryResult<Option<PaymentMethodRecord>>;
    async fn create(&self, payment: NewPaymentMethod) -> RepositoryResult<i32>;
    async fn change(
        &self,
        id: i32,
        changes: PaymentChanges,
    ) -> RepositoryResult<ChangePaymentOutcome>;
    async fn archive(&self, id: i32, archived_at: i64) -> RepositoryResult<ArchivePaymentOutcome>;
    async fn sort_exact(
        &self,
        ids: &[i32],
        updated_at: i64,
    ) -> RepositoryResult<SortPaymentsOutcome>;
}

/// Cryptography and provider-manifest adapter. It is deliberately synchronous:
/// all operations are local CPU work over an opaque encrypted envelope.
pub trait PaymentSecurity: Send + Sync {
    fn provider_codes(&self) -> Vec<String>;
    fn new_uuid(&self) -> String;
    fn seal(
        &self,
        provider: &str,
        uuid: &str,
        config: &ProviderConfig,
    ) -> Result<String, PaymentSecurityError>;
    fn redact(
        &self,
        provider: &str,
        uuid: &str,
        sealed_config: &str,
    ) -> Result<ProviderConfig, PaymentSecurityError>;
    fn provider_form(&self, provider: &str, config: Option<&ProviderConfig>) -> ProviderForm;
    fn uses_legacy_md5(&self, provider: &str) -> bool;
    fn security_warning(&self, provider: &str) -> Option<String>;
}

pub struct PaymentService<R, S> {
    repository: R,
    security: S,
    app_url: Option<String>,
}

impl<R, S> PaymentService<R, S>
where
    R: PaymentRepository,
    S: PaymentSecurity,
{
    pub fn new(repository: R, security: S, app_url: Option<String>) -> Self {
        Self {
            repository,
            security,
            app_url,
        }
    }

    pub fn provider_codes(&self) -> Vec<String> {
        self.security.provider_codes()
    }

    pub async fn payments(&self) -> Result<Vec<PaymentMethod>, PaymentError> {
        let rows = self.repository.list_active().await?;
        rows.into_iter().map(|row| self.payment(row)).collect()
    }

    pub async fn provider_form(
        &self,
        provider: &str,
        payment_id: Option<i64>,
    ) -> Result<ProviderForm, PaymentError> {
        let config = match payment_id {
            Some(id) => {
                let id = i32::try_from(id).map_err(|_| PaymentError::NotFound)?;
                let row = self
                    .repository
                    .find_active(id)
                    .await?
                    .ok_or(PaymentError::NotFound)?;
                if row.provider == provider {
                    Some(
                        self.security
                            .redact(&row.provider, &row.uuid, &row.sealed_config)?,
                    )
                } else {
                    None
                }
            }
            None => None,
        };
        Ok(self.security.provider_form(provider, config.as_ref()))
    }

    pub async fn create(&self, input: PaymentCreateInput, now: i64) -> Result<i32, PaymentError> {
        self.require_app_url()?;
        validate_create(&input, &self.security)?;
        let handling_fee_fixed = input
            .handling_fee_fixed
            .map(|value| {
                i32::try_from(value)
                    .ok()
                    .filter(|value| *value >= 0)
                    .ok_or(PaymentInputViolation::FixedFeeOutOfRange)
            })
            .transpose()
            .map_err(PaymentError::InvalidInput)?;
        let uuid = self.security.new_uuid();
        let config = self.security.seal(&input.provider, &uuid, &input.config)?;
        Ok(self
            .repository
            .create(NewPaymentMethod {
                name: input.name,
                provider: input.provider,
                config,
                uuid,
                icon: input.icon,
                notify_domain: input.notify_domain,
                handling_fee_fixed,
                handling_fee_percent: input.handling_fee_percent,
                created_at: now,
                updated_at: now,
            })
            .await?)
    }

    pub async fn patch(
        &self,
        id: i64,
        input: PaymentPatchInput,
        now: i64,
    ) -> Result<(), PaymentError> {
        self.require_app_url()?;
        validate_patch(&input)?;
        let id = i32::try_from(id).map_err(|_| PaymentError::NotFound)?;
        let handling_fee_fixed = input
            .handling_fee_fixed
            .map(|update| {
                update
                    .map(|value| {
                        i32::try_from(value)
                            .ok()
                            .filter(|value| *value >= 0)
                            .ok_or(PaymentInputViolation::FixedFeeOutOfRange)
                    })
                    .transpose()
            })
            .transpose()
            .map_err(PaymentError::InvalidInput)?;
        match self
            .repository
            .change(
                id,
                PaymentChanges {
                    name: input.name,
                    icon: input.icon,
                    notify_domain: input.notify_domain,
                    handling_fee_fixed,
                    handling_fee_percent: input.handling_fee_percent,
                    enable: input.enable,
                    updated_at: now,
                },
            )
            .await?
        {
            ChangePaymentOutcome::Updated => Ok(()),
            ChangePaymentOutcome::NotFound => Err(PaymentError::NotFound),
        }
    }

    pub async fn archive(&self, id: i64, now: i64) -> Result<(), PaymentError> {
        let id = i32::try_from(id).map_err(|_| PaymentError::NotFound)?;
        match self.repository.archive(id, now).await? {
            ArchivePaymentOutcome::Archived => Ok(()),
            ArchivePaymentOutcome::NotFound => Err(PaymentError::NotFound),
        }
    }

    pub async fn sort(&self, ids: &[i64], now: i64) -> Result<(), PaymentError> {
        let ids = normalize_sort_ids(ids).map_err(PaymentError::InvalidInput)?;
        match self.repository.sort_exact(&ids, now).await? {
            SortPaymentsOutcome::Sorted => Ok(()),
            SortPaymentsOutcome::PaymentSetChanged => Err(PaymentError::UpdateConflict),
        }
    }

    fn payment(&self, row: PaymentMethodRecord) -> Result<PaymentMethod, PaymentError> {
        let config = self
            .security
            .redact(&row.provider, &row.uuid, &row.sealed_config)?;
        let notify_path = format!("/api/v1/guest/payment/notify/{}/{}", row.provider, row.uuid);
        let base = row
            .notify_domain
            .as_deref()
            .filter(|value| !value.is_empty())
            .or_else(|| self.app_url.as_deref().filter(|value| !value.is_empty()));
        let notify_url = base.map_or_else(
            || notify_path.clone(),
            |base| format!("{}{}", base.trim_end_matches('/'), notify_path),
        );
        let legacy_md5_signature = self.security.uses_legacy_md5(&row.provider);
        let security_warning = self.security.security_warning(&row.provider);
        Ok(PaymentMethod {
            id: row.id,
            name: row.name,
            provider: row.provider,
            icon: row.icon,
            handling_fee_fixed: row.handling_fee_fixed,
            handling_fee_percent: row.handling_fee_percent,
            uuid: row.uuid,
            config,
            notify_domain: row.notify_domain,
            notify_url,
            enable: row.enable,
            sort: row.sort,
            created_at: row.created_at,
            updated_at: row.updated_at,
            legacy_md5_signature,
            security_warning,
        })
    }

    fn require_app_url(&self) -> Result<(), PaymentError> {
        self.app_url
            .as_deref()
            .filter(|value| !value.is_empty())
            .map(|_| ())
            .ok_or(PaymentError::AppUrlNotConfigured)
    }
}

fn validate_create<S: PaymentSecurity>(
    input: &PaymentCreateInput,
    security: &S,
) -> Result<(), PaymentError> {
    if input.name.trim().is_empty() {
        return Err(PaymentError::InvalidInput(PaymentInputViolation::EmptyName));
    }
    if input.provider.trim().is_empty() {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::EmptyProvider,
        ));
    }
    if !security
        .provider_codes()
        .iter()
        .any(|provider| provider == &input.provider)
    {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::UnknownProvider,
        ));
    }
    if input.config.is_empty() {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::EmptyConfig,
        ));
    }
    if input
        .notify_domain
        .as_deref()
        .is_some_and(|domain| !valid_url_shape(domain))
    {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::InvalidNotifyDomain,
        ));
    }
    if input
        .handling_fee_fixed
        .is_some_and(|value| value < 0 || i32::try_from(value).is_err())
    {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::FixedFeeOutOfRange,
        ));
    }
    validate_percent_update(input.handling_fee_percent.as_deref())
}

fn validate_patch(input: &PaymentPatchInput) -> Result<(), PaymentError> {
    if input
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(PaymentError::InvalidInput(PaymentInputViolation::EmptyName));
    }
    if input
        .notify_domain
        .as_ref()
        .and_then(|domain| domain.as_deref())
        .is_some_and(|domain| !valid_url_shape(domain))
    {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::InvalidNotifyDomain,
        ));
    }
    if input.handling_fee_fixed.is_some_and(|update| {
        update.is_some_and(|value| value < 0 || i32::try_from(value).is_err())
    }) {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::FixedFeeOutOfRange,
        ));
    }
    match input.handling_fee_percent.as_ref() {
        Some(Some(value)) => validate_percent_update(Some(value)),
        Some(None) | None => Ok(()),
    }
}

fn validate_percent_update(value: Option<&str>) -> Result<(), PaymentError> {
    if value.is_some_and(|value| !decimal_in_closed_range(value, "0.1", "100")) {
        return Err(PaymentError::InvalidInput(
            PaymentInputViolation::PercentFeeOutOfRange,
        ));
    }
    Ok(())
}

/// Approximates the established URL rule without binding the application
/// layer to an HTTP/URL implementation: an alphabetic-led URI scheme and a
/// non-empty authority are required.
fn valid_url_shape(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    let bytes = scheme.as_bytes();
    if bytes.is_empty()
        || !bytes[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return false;
    }
    !rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .is_empty()
}

fn normalize_sort_ids(ids: &[i64]) -> Result<Vec<i32>, PaymentInputViolation> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        let id = i32::try_from(*id)
            .ok()
            .filter(|id| *id > 0)
            .ok_or(PaymentInputViolation::SortIdOutOfRange)?;
        if !seen.insert(id) {
            return Err(PaymentInputViolation::DuplicateSortId);
        }
        normalized.push(id);
    }
    Ok(normalized)
}

/// Decimal comparison that never rounds through binary floating point. Inputs
/// may use the JSON-number exponent spelling emitted by a transport adapter.
fn decimal_in_closed_range(value: &str, minimum: &str, maximum: &str) -> bool {
    let Some(value) = DecimalDigits::parse(value) else {
        return false;
    };
    let Some(minimum) = DecimalDigits::parse(minimum) else {
        return false;
    };
    let Some(maximum) = DecimalDigits::parse(maximum) else {
        return false;
    };
    value.compare(&minimum).is_ge() && value.compare(&maximum).is_le()
}

struct DecimalDigits {
    digits: String,
    scale: i32,
}

impl DecimalDigits {
    fn parse(value: &str) -> Option<Self> {
        let (mantissa, exponent) = match value.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (mantissa, exponent.parse::<i32>().ok()?),
            None => (value, 0_i32),
        };
        if mantissa.starts_with('-') || mantissa.starts_with('+') {
            return None;
        }
        let mut pieces = mantissa.split('.');
        let whole = pieces.next()?;
        let fraction = pieces.next().unwrap_or_default();
        if pieces.next().is_some()
            || whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            || (fraction.is_empty() && mantissa.ends_with('.'))
        {
            return None;
        }
        let raw = format!("{whole}{fraction}");
        let digits = raw.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits }.to_string();
        let fraction_len = i32::try_from(fraction.len()).ok()?;
        let scale = fraction_len.checked_sub(exponent)?;
        (scale.unsigned_abs() <= 1_000).then_some(Self { digits, scale })
    }

    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        let common_scale = self.scale.max(other.scale).max(0);
        let left = self.expanded(common_scale);
        let right = other.expanded(common_scale);
        left.len().cmp(&right.len()).then_with(|| left.cmp(&right))
    }

    fn expanded(&self, common_scale: i32) -> String {
        if self.digits == "0" {
            return "0".to_string();
        }
        let zeros = usize::try_from(common_scale - self.scale).unwrap_or_default();
        let mut expanded = String::with_capacity(self.digits.len() + zeros);
        expanded.push_str(&self.digits);
        expanded.extend(std::iter::repeat_n('0', zeros));
        expanded
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

    #[derive(Default)]
    struct FakeState {
        rows: Vec<PaymentMethodRecord>,
        created: Option<NewPaymentMethod>,
        changes: Option<PaymentChanges>,
        sorted: Option<Vec<i32>>,
        calls: usize,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    impl PaymentRepository for FakeRepository {
        async fn list_active(&self) -> RepositoryResult<Vec<PaymentMethodRecord>> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            Ok(state.rows.clone())
        }

        async fn find_active(&self, id: i32) -> RepositoryResult<Option<PaymentMethodRecord>> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            Ok(state.rows.iter().find(|row| row.id == id).cloned())
        }

        async fn create(&self, payment: NewPaymentMethod) -> RepositoryResult<i32> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.created = Some(payment);
            Ok(9)
        }

        async fn change(
            &self,
            _: i32,
            changes: PaymentChanges,
        ) -> RepositoryResult<ChangePaymentOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.changes = Some(changes);
            Ok(ChangePaymentOutcome::Updated)
        }

        async fn archive(&self, _: i32, _: i64) -> RepositoryResult<ArchivePaymentOutcome> {
            self.0.lock().unwrap().calls += 1;
            Ok(ArchivePaymentOutcome::Archived)
        }

        async fn sort_exact(&self, ids: &[i32], _: i64) -> RepositoryResult<SortPaymentsOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.sorted = Some(ids.to_vec());
            Ok(SortPaymentsOutcome::Sorted)
        }
    }

    #[derive(Clone, Default)]
    struct FakeSecurity;

    impl PaymentSecurity for FakeSecurity {
        fn provider_codes(&self) -> Vec<String> {
            vec!["StripeCheckout".to_string()]
        }

        fn new_uuid(&self) -> String {
            "fixed-uuid".to_string()
        }

        fn seal(
            &self,
            provider: &str,
            uuid: &str,
            _: &ProviderConfig,
        ) -> Result<String, PaymentSecurityError> {
            Ok(format!("sealed:{provider}:{uuid}"))
        }

        fn redact(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<ProviderConfig, PaymentSecurityError> {
            Ok(BTreeMap::from([(
                "stripe_sk_live".to_string(),
                "********".to_string(),
            )]))
        }

        fn provider_form(&self, _: &str, config: Option<&ProviderConfig>) -> ProviderForm {
            BTreeMap::from([(
                "stripe_sk_live".to_string(),
                ProviderFormField {
                    label: "Secret".to_string(),
                    description: String::new(),
                    kind: "input".to_string(),
                    value: config.and_then(|config| config.get("stripe_sk_live").cloned()),
                },
            )])
        }

        fn uses_legacy_md5(&self, _: &str) -> bool {
            false
        }

        fn security_warning(&self, _: &str) -> Option<String> {
            None
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

    fn create_input() -> PaymentCreateInput {
        PaymentCreateInput {
            name: "Card".to_string(),
            provider: "StripeCheckout".to_string(),
            config: BTreeMap::from([("stripe_sk_live".to_string(), "secret".to_string())]),
            icon: None,
            notify_domain: Some("https://pay.example.test".to_string()),
            handling_fee_fixed: Some(10),
            handling_fee_percent: Some("0.1".to_string()),
        }
    }

    #[test]
    fn fee_window_is_decimal_exact_and_accepts_exponent_spelling() {
        for value in ["0.1", "1e-1", "100", "1e2", "12.345"] {
            assert!(decimal_in_closed_range(value, "0.1", "100"), "{value}");
        }
        for value in ["0", "0.099999999999", "100.0001", "-1", "NaN", "1e1001"] {
            assert!(!decimal_in_closed_range(value, "0.1", "100"), "{value}");
        }
    }

    #[test]
    fn invalid_input_never_reaches_persistence_or_crypto() {
        let repository = FakeRepository::default();
        let mut input = create_input();
        input.provider = "Unknown".to_string();
        assert!(matches!(
            block_on(
                PaymentService::new(
                    repository.clone(),
                    FakeSecurity,
                    Some("https://app.example".to_string()),
                )
                .create(input, 10)
            ),
            Err(PaymentError::InvalidInput(
                PaymentInputViolation::UnknownProvider
            ))
        ));
        assert_eq!(repository.0.lock().unwrap().calls, 0);
    }

    #[test]
    fn create_seals_the_typed_config_before_the_repository() {
        let repository = FakeRepository::default();
        let service = PaymentService::new(
            repository.clone(),
            FakeSecurity,
            Some("https://app.example".to_string()),
        );
        assert_eq!(block_on(service.create(create_input(), 44)).unwrap(), 9);
        let created = repository.0.lock().unwrap().created.clone().unwrap();
        assert_eq!(created.config, "sealed:StripeCheckout:fixed-uuid");
        assert_eq!((created.created_at, created.updated_at), (44, 44));
    }

    #[test]
    fn list_opens_only_redacted_data_and_composes_the_notify_url() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().rows.push(PaymentMethodRecord {
            id: 1,
            name: "Card".to_string(),
            provider: "StripeCheckout".to_string(),
            icon: None,
            handling_fee_fixed: None,
            handling_fee_percent: Some("2.5".to_string()),
            uuid: "uuid".to_string(),
            sealed_config: "ciphertext".to_string(),
            notify_domain: None,
            enable: true,
            sort: Some(1),
            created_at: 1,
            updated_at: 2,
        });
        let payments = block_on(
            PaymentService::new(
                repository,
                FakeSecurity,
                Some("https://app.example/".to_string()),
            )
            .payments(),
        )
        .unwrap();
        assert_eq!(
            payments[0].notify_url,
            "https://app.example/api/v1/guest/payment/notify/StripeCheckout/uuid"
        );
        assert_eq!(payments[0].config["stripe_sk_live"], "********");
    }

    #[test]
    fn patch_interface_cannot_mutate_provider_or_verification_material() {
        let repository = FakeRepository::default();
        let service = PaymentService::new(
            repository.clone(),
            FakeSecurity,
            Some("https://app.example".to_string()),
        );
        block_on(service.patch(
            1,
            PaymentPatchInput {
                name: Some("Renamed".to_string()),
                enable: Some(true),
                ..PaymentPatchInput::default()
            },
            55,
        ))
        .unwrap();
        let changes = repository.0.lock().unwrap().changes.clone().unwrap();
        assert_eq!(changes.name.as_deref(), Some("Renamed"));
        assert_eq!(changes.updated_at, 55);
    }

    #[test]
    fn sort_rejects_duplicates_before_the_exact_set_repository_port() {
        let repository = FakeRepository::default();
        let service = PaymentService::new(
            repository.clone(),
            FakeSecurity,
            Some("https://app.example".to_string()),
        );
        assert!(matches!(
            block_on(service.sort(&[1, 1], 1)),
            Err(PaymentError::InvalidInput(
                PaymentInputViolation::DuplicateSortId
            ))
        ));
        assert_eq!(repository.0.lock().unwrap().calls, 0);
    }
}
