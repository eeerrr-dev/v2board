use std::{
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use super::*;

#[derive(Clone, Default)]
struct FakePorts(Arc<Mutex<FakeState>>);

#[derive(Default)]
struct FakeState {
    events: Vec<&'static str>,
    account_by_email: Option<AuthAccount>,
    account_by_id: Option<AuthAccount>,
    active_session_epoch: Option<i64>,
    invite: Option<InviteCodeRecord>,
    invite_consumed: bool,
    trial: Option<TrialPlanRecord>,
    insert_outcome: Option<InsertAuthAccountOutcome>,
    inserted_account: Option<NewAuthAccount>,
    mfa: Option<MfaRecord>,
    registration_reserved: bool,
    login_reserved: bool,
    email_code_result: Option<LimitedEmailCodeResult>,
    email_send_allowed: bool,
    email_code_reserved: bool,
    temporary_identity: Option<SessionIdentity>,
    session_added: bool,
    session_identity: Option<SessionIdentity>,
    session_metadata: Option<SessionMetadata>,
    sessions: Vec<StoredSession>,
    step_up_reserved: bool,
    step_up_added: bool,
    step_up_identity: Option<(i64, String)>,
    password_matches: bool,
    password_needs_rehash: bool,
    recaptcha_matches: bool,
    accepted_mfa_step: Option<i64>,
}

impl FakePorts {
    fn ready() -> Self {
        let ports = Self::default();
        {
            let mut state = ports.0.lock().unwrap();
            state.invite_consumed = true;
            state.registration_reserved = true;
            state.login_reserved = true;
            state.email_code_result = Some(LimitedEmailCodeResult::Consumed);
            state.email_send_allowed = true;
            state.email_code_reserved = true;
            state.session_added = true;
            state.step_up_reserved = true;
            state.step_up_added = true;
            state.password_matches = true;
            state.recaptcha_matches = true;
        }
        ports
    }

    fn event(&self, event: &'static str) {
        self.0.lock().unwrap().events.push(event);
    }
}

struct FakeRegistration(FakePorts);

impl RegistrationTransaction for FakeRegistration {
    async fn lock_invite_code(&mut self, _: &str) -> RepositoryResult<Option<InviteCodeRecord>> {
        self.0.event("lock_invite");
        Ok(self.0.0.lock().unwrap().invite)
    }

    async fn consume_invite_code(&mut self, _: i32, _: i64) -> RepositoryResult<bool> {
        self.0.event("consume_invite");
        Ok(self.0.0.lock().unwrap().invite_consumed)
    }

    async fn lock_trial_plan(&mut self, _: i32) -> RepositoryResult<Option<TrialPlanRecord>> {
        self.0.event("lock_trial");
        Ok(self.0.0.lock().unwrap().trial)
    }

    async fn insert_account(
        &mut self,
        account: NewAuthAccount,
    ) -> RepositoryResult<InsertAuthAccountOutcome> {
        self.0.event("insert_account");
        let mut state = self.0.0.lock().unwrap();
        state.inserted_account = Some(account);
        Ok(state
            .insert_outcome
            .unwrap_or(InsertAuthAccountOutcome::Inserted(7)))
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.0.event("commit");
        Ok(())
    }
}

impl AuthRepository for FakePorts {
    type Registration<'a> = FakeRegistration;

    async fn begin_registration(&self) -> RepositoryResult<Self::Registration<'_>> {
        self.event("begin_registration");
        Ok(FakeRegistration(self.clone()))
    }

    async fn find_account_by_email(&self, _: &str) -> RepositoryResult<Option<AuthAccount>> {
        self.event("find_by_email");
        Ok(self.0.lock().unwrap().account_by_email.clone())
    }

    async fn find_account_by_id(&self, _: i64) -> RepositoryResult<Option<AuthAccount>> {
        self.event("find_by_id");
        Ok(self.0.lock().unwrap().account_by_id.clone())
    }

    async fn active_session_epoch(&self, _: i64) -> RepositoryResult<Option<i64>> {
        self.event("active_session_epoch");
        Ok(self.0.lock().unwrap().active_session_epoch)
    }

    async fn rehash_password(&self, _: i64, _: &str, _: &str, _: i64) -> RepositoryResult<()> {
        self.event("rehash_password");
        Ok(())
    }

    async fn update_password(&self, _: i64, _: &str, _: i64) -> RepositoryResult<bool> {
        self.event("update_password");
        Ok(true)
    }

    async fn change_password_if_current(
        &self,
        _: i64,
        _: &str,
        _: i64,
        _: &str,
        _: i64,
    ) -> RepositoryResult<bool> {
        self.event("change_password");
        Ok(true)
    }

    async fn update_security(&self, _: i64, _: &str, _: &str, _: i64) -> RepositoryResult<bool> {
        self.event("update_security");
        Ok(true)
    }

    async fn increment_invite_view(&self, _: &str, _: i64) -> RepositoryResult<()> {
        self.event("increment_invite_view");
        Ok(())
    }

    async fn find_mfa(&self, _: i64) -> RepositoryResult<Option<MfaRecord>> {
        self.event("find_mfa");
        Ok(self.0.lock().unwrap().mfa.clone())
    }

    async fn upsert_pending_mfa(
        &self,
        _: i64,
        _: &SealedMfaSecret,
        _: i64,
    ) -> RepositoryResult<bool> {
        self.event("upsert_mfa");
        Ok(true)
    }

    async fn enable_mfa(&self, _: i64, _: i64, _: i64) -> RepositoryResult<bool> {
        self.event("enable_mfa");
        Ok(true)
    }

    async fn consume_mfa_step(&self, _: i64, _: i64, _: i64) -> RepositoryResult<bool> {
        self.event("consume_mfa");
        Ok(true)
    }

    async fn disable_mfa(&self, _: i64, _: i64) -> RepositoryResult<bool> {
        self.event("disable_mfa");
        Ok(true)
    }
}

impl AuthCache for FakePorts {
    async fn reserve_login_attempt(
        &self,
        _: &str,
        _: Option<&str>,
        _: i64,
        _: i64,
        _: u64,
    ) -> RepositoryResult<bool> {
        self.event("reserve_login");
        Ok(self.0.lock().unwrap().login_reserved)
    }

    async fn release_login_attempt(&self, _: &str, _: Option<&str>) {
        self.event("release_login");
    }

    async fn reserve_registration_slot(
        &self,
        _: &RegistrationReservation,
        _: i64,
        _: i64,
        _: i64,
    ) -> RepositoryResult<bool> {
        self.event("reserve_registration");
        Ok(self.0.lock().unwrap().registration_reserved)
    }

    async fn release_registration_slot(&self, _: &RegistrationReservation) {
        self.event("release_registration");
    }

    async fn consume_email_code(
        &self,
        _: &str,
        _: &str,
        _: EmailCodeScope,
        _: i64,
        _: u64,
    ) -> RepositoryResult<LimitedEmailCodeResult> {
        self.event("consume_email_code");
        Ok(self
            .0
            .lock()
            .unwrap()
            .email_code_result
            .unwrap_or(LimitedEmailCodeResult::Incorrect))
    }

    async fn increment_email_send_limit(&self, _: &str, _: i64, _: u64) -> RepositoryResult<bool> {
        self.event("email_send_limit");
        Ok(self.0.lock().unwrap().email_send_allowed)
    }

    async fn reserve_email_code(&self, _: &str, _: &str, _: i64) -> RepositoryResult<bool> {
        self.event("reserve_email_code");
        Ok(self.0.lock().unwrap().email_code_reserved)
    }

    async fn release_email_code(&self, _: &str, _: &str) {
        self.event("release_email_code");
    }

    async fn put_temporary_token(&self, _: &str, _: i64, _: i64, _: u64) -> RepositoryResult<()> {
        self.event("put_temporary_token");
        Ok(())
    }

    async fn take_temporary_token(&self, _: &str) -> RepositoryResult<Option<SessionIdentity>> {
        self.event("take_temporary_token");
        Ok(self.0.lock().unwrap().temporary_identity.clone())
    }

    async fn add_session(
        &self,
        _: &SessionIdentity,
        _: &SessionMetadata,
        _: &str,
        _: u64,
        _: i64,
        _: i64,
    ) -> RepositoryResult<bool> {
        self.event("add_session");
        Ok(self.0.lock().unwrap().session_added)
    }

    async fn session_identity(&self, _: &str) -> RepositoryResult<Option<SessionIdentity>> {
        self.event("session_identity");
        Ok(self.0.lock().unwrap().session_identity.clone())
    }

    async fn session_metadata(&self, _: i64, _: &str) -> RepositoryResult<Option<SessionMetadata>> {
        self.event("session_metadata");
        Ok(self.0.lock().unwrap().session_metadata.clone())
    }

    async fn sessions(&self, _: i64) -> RepositoryResult<Vec<StoredSession>> {
        self.event("sessions");
        Ok(self.0.lock().unwrap().sessions.clone())
    }

    async fn remove_session(&self, _: i64, _: &str) -> RepositoryResult<()> {
        self.event("remove_session");
        Ok(())
    }

    async fn remove_all_sessions(&self, _: i64) -> RepositoryResult<()> {
        self.event("remove_all_sessions");
        Ok(())
    }

    async fn reserve_step_up_attempt(
        &self,
        _: i64,
        _: Option<&str>,
        _: i64,
        _: i64,
        _: u64,
    ) -> RepositoryResult<bool> {
        self.event("reserve_step_up");
        Ok(self.0.lock().unwrap().step_up_reserved)
    }

    async fn clear_step_up_attempts(&self, _: i64, _: Option<&str>) {
        self.event("clear_step_up");
    }

    async fn put_step_up(&self, _: &str, _: i64, _: &str, _: u64) -> RepositoryResult<bool> {
        self.event("put_step_up");
        Ok(self.0.lock().unwrap().step_up_added)
    }

    async fn step_up_identity(&self, _: &str) -> RepositoryResult<Option<(i64, String)>> {
        self.event("step_up_identity");
        Ok(self.0.lock().unwrap().step_up_identity.clone())
    }
}

impl AuthExternal for FakePorts {
    fn now(&self) -> i64 {
        1_000
    }

    fn uuid(&self) -> RepositoryResult<String> {
        self.event("uuid");
        Ok("uuid".to_string())
    }

    fn compact_id(&self) -> RepositoryResult<String> {
        self.event("compact_id");
        Ok("compact".to_string())
    }

    fn opaque_token(&self) -> RepositoryResult<String> {
        self.event("opaque_token");
        Ok("opaque-session".to_string())
    }

    fn email_code(&self) -> RepositoryResult<String> {
        self.event("email_code");
        Ok("123456".to_string())
    }

    async fn hash_password(&self, _: &str) -> RepositoryResult<String> {
        self.event("hash_password");
        Ok("new-hash".to_string())
    }

    async fn verify_password(
        &self,
        _: Option<&str>,
        _: Option<&str>,
        _: &str,
        _: &str,
    ) -> RepositoryResult<bool> {
        self.event("verify_password");
        Ok(self.0.lock().unwrap().password_matches)
    }

    async fn verify_dummy_password(&self, _: &str) -> RepositoryResult<()> {
        self.event("verify_dummy_password");
        Ok(())
    }

    fn password_needs_rehash(&self, _: Option<&str>, _: &str) -> bool {
        self.event("password_needs_rehash");
        self.0.lock().unwrap().password_needs_rehash
    }

    async fn verify_recaptcha(&self, _: &str) -> RepositoryResult<bool> {
        self.event("verify_recaptcha");
        Ok(self.0.lock().unwrap().recaptcha_matches)
    }

    async fn send_verification_mail(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<(), MailDeliveryError> {
        self.event("send_mail");
        Ok(())
    }

    async fn subscribe_url(&self, user_id: i64, token: &str) -> RepositoryResult<String> {
        self.event("subscribe_url");
        Ok(format!(
            "https://example.test/subscribe/{user_id}?token={token}"
        ))
    }

    fn create_mfa_secret(&self, _: i64, _: &str, _: &str) -> RepositoryResult<SealedMfaSecret> {
        self.event("create_mfa_secret");
        Ok(SealedMfaSecret {
            secret_nonce: vec![1],
            secret_ciphertext: vec![2],
            secret_tag: vec![3],
            public_secret: "secret".to_string(),
            otpauth_url: "otpauth://totp/test".to_string(),
        })
    }

    fn accepted_mfa_step(
        &self,
        _: i64,
        _: &MfaRecord,
        _: &str,
        _: i64,
    ) -> RepositoryResult<Option<i64>> {
        self.event("accepted_mfa_step");
        Ok(self.0.lock().unwrap().accepted_mfa_step)
    }
}

fn account(id: i64) -> AuthAccount {
    AuthAccount {
        id,
        email: "user@example.test".to_string(),
        password_hash: "stored-hash".to_string(),
        password_algo: None,
        password_salt: None,
        session_epoch: 0,
        banned: false,
        is_admin: 0,
        is_staff: 0,
        admin_permissions: Vec::new(),
    }
}

fn policy() -> AuthPolicy {
    AuthPolicy {
        app_name: "V2Board".to_string(),
        app_url: Some("https://example.test".to_string()),
        password_limit_enable: true,
        password_limit_count: 5,
        password_limit_expire_minutes: 5,
        password_limit_ttl_seconds: 300,
        register_limit_by_ip_enable: true,
        register_limit_count: 3,
        register_limit_expire_minutes: 1,
        register_limit_ttl_seconds: 60,
        stop_register: false,
        invite_force: false,
        invite_never_expire: false,
        email_verify: false,
        email_whitelist_enable: false,
        email_whitelist_suffix: Vec::new(),
        email_gmail_limit_enable: false,
        recaptcha_enable: false,
        trial_plan_id: 0,
        trial_duration: TrialDuration::Seconds(3_600),
        auth_session_ttl_seconds: 2_592_000,
        privileged_auth_session_ttl_seconds: 43_200,
        auth_session_max_per_user: 10,
        privileged_step_up_max_attempts: 5,
        privileged_step_up_attempt_window_seconds: 300,
        privileged_step_up_ttl_seconds: 300,
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
fn login_policy_orders_limiter_credentials_and_session_ports() {
    let ports = FakePorts::ready();
    {
        let mut state = ports.0.lock().unwrap();
        state.account_by_email = Some(account(7));
        state.account_by_id = Some(account(7));
    }
    let result = run(
        AuthService::new(ports.clone(), ports.clone(), ports.clone(), policy()).login(
            " User@Example.Test ",
            "correct-password",
            None,
            Some("203.0.113.8".to_string()),
            Some("browser".to_string()),
        ),
    )
    .unwrap();

    assert_eq!(
        result,
        AuthData {
            is_admin: false,
            auth_data: "opaque-session".to_string()
        }
    );
    assert_eq!(
        ports.0.lock().unwrap().events,
        [
            "reserve_login",
            "find_by_email",
            "verify_password",
            "release_login",
            "password_needs_rehash",
            "find_by_id",
            "compact_id",
            "opaque_token",
            "add_session"
        ]
    );
}

#[test]
fn missing_account_runs_dummy_verification_without_touching_session_ports() {
    let ports = FakePorts::ready();
    let error = run(
        AuthService::new(ports.clone(), ports.clone(), ports.clone(), policy()).login(
            "missing@example.test",
            "correct-password",
            None,
            None,
            None,
        ),
    )
    .unwrap_err();

    assert!(error.is_code(AuthCode::InvalidCredentials));
    assert_eq!(
        ports.0.lock().unwrap().events,
        ["reserve_login", "find_by_email", "verify_dummy_password"]
    );
}

#[test]
fn registration_owns_policy_order_and_transactional_account_shape() {
    let ports = FakePorts::ready();
    {
        let mut state = ports.0.lock().unwrap();
        state.invite = Some(InviteCodeRecord { id: 11, user_id: 3 });
        state.trial = Some(TrialPlanRecord {
            id: 9,
            group_id: 4,
            transfer_gib: 2,
            device_limit: Some(3),
            speed_limit: Some(10),
        });
        state.account_by_id = Some(account(7));
    }
    let mut registration_policy = policy();
    registration_policy.invite_force = true;
    registration_policy.email_verify = true;
    registration_policy.recaptcha_enable = true;
    registration_policy.trial_plan_id = 9;
    registration_policy.trial_duration = TrialDuration::Seconds(7_200);

    let result = run(AuthService::new(
        ports.clone(),
        ports.clone(),
        ports.clone(),
        registration_policy,
    )
    .register(
        RegisterInput {
            email: " New@Example.Test ".to_string(),
            password: "correct-password".to_string(),
            invite_code: Some("invite".to_string()),
            email_code: Some("123456".to_string()),
            recaptcha_data: Some("captcha".to_string()),
        },
        Some("203.0.113.9".to_string()),
        Some("browser".to_string()),
    ))
    .unwrap();

    assert_eq!(result.auth_data, "opaque-session");
    let state = ports.0.lock().unwrap();
    let inserted = state.inserted_account.as_ref().unwrap();
    assert_eq!(inserted.invite_user_id, Some(3));
    assert_eq!(inserted.email, "New@Example.Test");
    assert_eq!(inserted.transfer_enable, 2 * 1_073_741_824);
    assert_eq!(inserted.group_id, Some(4));
    assert_eq!(inserted.plan_id, Some(9));
    assert_eq!(inserted.expired_at, Some(8_200));
    assert_eq!(
        state.events,
        [
            "compact_id",
            "reserve_registration",
            "verify_recaptcha",
            "consume_email_code",
            "hash_password",
            "uuid",
            "compact_id",
            "begin_registration",
            "lock_invite",
            "consume_invite",
            "lock_trial",
            "insert_account",
            "commit",
            "find_by_id",
            "compact_id",
            "opaque_token",
            "add_session"
        ]
    );
}

#[test]
fn rejected_registration_releases_rate_reservation_and_never_commits() {
    let ports = FakePorts::ready();
    ports.0.lock().unwrap().insert_outcome = Some(InsertAuthAccountOutcome::EmailAlreadyRegistered);
    let error = run(
        AuthService::new(ports.clone(), ports.clone(), ports.clone(), policy()).register(
            RegisterInput {
                email: "user@example.test".to_string(),
                password: "correct-password".to_string(),
                invite_code: None,
                email_code: None,
                recaptcha_data: None,
            },
            Some("203.0.113.10".to_string()),
            None,
        ),
    )
    .unwrap_err();

    assert!(error.is_code(AuthCode::EmailAlreadyRegistered));
    let events = &ports.0.lock().unwrap().events;
    assert!(events.contains(&"release_registration"));
    assert!(!events.contains(&"commit"));
    assert!(!events.contains(&"add_session"));
}

#[test]
fn invalid_registration_never_reaches_an_outbound_port() {
    let ports = FakePorts::ready();
    let error = run(
        AuthService::new(ports.clone(), ports.clone(), ports.clone(), policy()).register(
            RegisterInput {
                email: "not-an-email".to_string(),
                password: "correct-password".to_string(),
                invite_code: None,
                email_code: None,
                recaptcha_data: None,
            },
            Some("203.0.113.11".to_string()),
            None,
        ),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        AuthError::Validation { field: "email", .. }
    ));
    assert!(ports.0.lock().unwrap().events.is_empty());
}

#[test]
fn validation_preserves_field_precedence_and_character_counting() {
    assert!(validate_email("user@example.test").is_ok());
    assert!(matches!(
        validate_email("   "),
        Err(AuthError::Validation {
            field: "email",
            message: "Email can not be empty"
        })
    ));
    assert!(matches!(
        validate_email("bad"),
        Err(AuthError::Validation {
            field: "email",
            message: "Email format is incorrect"
        })
    ));
    assert_eq!(normalize_email(" User@Example.TEST "), "user@example.test");

    assert!(validate_password("password").is_ok());
    assert!(matches!(
        validate_password("七个中文密码"),
        Err(AuthError::Validation {
            field: "password",
            message: "Password must be greater than 8 digits"
        })
    ));

    assert!(validate_forget("user@example.test", "password", "123456").is_ok());
    assert!(matches!(
        validate_forget("", "short", ""),
        Err(AuthError::Validation {
            field: "email",
            message: "Email can not be empty"
        })
    ));
    assert!(matches!(
        validate_forget("user@example.test", "password", ""),
        Err(AuthError::Validation {
            field: "email_code",
            message: "Email verification code cannot be empty"
        })
    ));
    assert!(matches!(
        validate_change_password("", "short"),
        Err(AuthError::Validation {
            field: "old_password",
            message: "Old password cannot be empty"
        })
    ));
    assert!(matches!(
        validate_change_password("old-password", ""),
        Err(AuthError::Validation {
            field: "new_password",
            message: "New password cannot be empty"
        })
    ));
}

#[test]
fn registration_auxiliary_inputs_are_bounded_before_expensive_work() {
    let valid = RegisterInput {
        email: "user@example.test".to_string(),
        password: "password".to_string(),
        invite_code: Some("a".repeat(MAX_INVITE_CODE_BYTES)),
        email_code: Some("1".repeat(MAX_EMAIL_CODE_BYTES)),
        recaptcha_data: Some("r".repeat(MAX_RECAPTCHA_DATA_BYTES)),
    };
    assert!(validate_registration_auxiliary_inputs(&valid).is_ok());

    let mut oversized = valid.clone();
    oversized.invite_code = Some("a".repeat(MAX_INVITE_CODE_BYTES + 1));
    assert!(matches!(
        validate_registration_auxiliary_inputs(&oversized),
        Err(AuthError::Validation {
            field: "invite_code",
            ..
        })
    ));
    oversized = valid.clone();
    oversized.email_code = Some("1".repeat(MAX_EMAIL_CODE_BYTES + 1));
    assert!(matches!(
        validate_registration_auxiliary_inputs(&oversized),
        Err(AuthError::Validation {
            field: "email_code",
            ..
        })
    ));
    oversized = valid;
    oversized.recaptcha_data = Some("r".repeat(MAX_RECAPTCHA_DATA_BYTES + 1));
    assert!(matches!(
        validate_registration_auxiliary_inputs(&oversized),
        Err(AuthError::Validation {
            field: "recaptcha_data",
            ..
        })
    ));
}

#[test]
fn trial_math_session_policy_and_redirects_remain_application_owned() {
    assert_eq!(checked_trial_transfer_bytes(2).unwrap(), 2_147_483_648);
    assert!(checked_trial_transfer_bytes(-1).is_err());
    assert!(checked_trial_transfer_bytes(i64::MAX).is_err());
    assert_eq!(session_ttl_seconds(30, 12, 0, 0), 30);
    assert_eq!(session_ttl_seconds(30, 12, 1, 0), 12);
    assert_eq!(session_ttl_seconds(30, 12, 0, 1), 12);
    assert_eq!(truncate_utf8("ab中文".to_string(), 6), "ab中");

    let ports = FakePorts::ready();
    let service = AuthService::new(ports.clone(), ports.clone(), ports, policy());
    assert_eq!(
        service.login_redirect_url("token", None),
        "https://example.test/login?verify=token&redirect=dashboard"
    );
    assert_eq!(
        service.login_redirect_url("token", Some("orders")),
        "https://example.test/login?verify=token&redirect=orders"
    );
}
