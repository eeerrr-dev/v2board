use std::{
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use super::*;

#[derive(Clone)]
struct FakeRepository {
    snapshot: ConfigurationSnapshot,
    active: ActiveConfiguration,
    commits: Arc<Mutex<Vec<(i64, ConfigurationMap, String)>>>,
}

impl ConfigurationRepository for FakeRepository {
    type Activation = ConfigurationMap;

    fn current_snapshot(&self) -> Result<ConfigurationSnapshot, ConfigurationPortError> {
        Ok(self.snapshot.clone())
    }

    async fn load_active(&self) -> Result<ActiveConfiguration, ConfigurationPortError> {
        Ok(self.active.clone())
    }

    async fn materialize(
        &self,
        active: &ActiveConfiguration,
        changes: &ConfigurationMap,
    ) -> Result<MaterializedConfiguration<Self::Activation>, ConfigurationPortError> {
        let mut values = active.values.clone();
        for (key, value) in changes {
            values.insert(key.clone(), value.clone());
        }
        Ok(MaterializedConfiguration {
            activation: values.clone(),
            values,
        })
    }

    async fn commit(
        &self,
        expected_revision: i64,
        values: &ConfigurationMap,
        actor: &str,
    ) -> Result<i64, ConfigurationPortError> {
        self.commits.lock().expect("commits").push((
            expected_revision,
            values.clone(),
            actor.to_string(),
        ));
        Ok(expected_revision + 1)
    }

    fn at_revision(&self, mut activation: Self::Activation, revision: i64) -> Self::Activation {
        activation.insert(
            "revision".to_string(),
            ConfigurationValue::Integer(revision),
        );
        activation
    }
}

#[derive(Clone, Default)]
struct FakeExternal {
    mail: Arc<Mutex<Vec<String>>>,
    telegram: Arc<Mutex<Vec<Option<String>>>>,
}

impl ConfigurationExternal for FakeExternal {
    async fn send_test_mail(&self, recipient: &str) -> Result<(), ConfigurationPortError> {
        self.mail
            .lock()
            .expect("mail calls")
            .push(recipient.to_string());
        Ok(())
    }

    async fn set_telegram_webhook(
        &self,
        token: Option<&str>,
    ) -> Result<(), ConfigurationPortError> {
        self.telegram
            .lock()
            .expect("telegram calls")
            .push(token.map(str::to_string));
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeBulkMail {
    commands: Arc<Mutex<Vec<(MailAudience, String, usize)>>>,
}

type RecordedCommit = (i64, ConfigurationMap, String);
type FakeService = ConfigurationService<FakeRepository, FakeExternal, FakeBulkMail>;
type ServiceHarness = (
    FakeService,
    Arc<Mutex<Vec<RecordedCommit>>>,
    FakeExternal,
    FakeBulkMail,
);

impl BulkMailRepository for FakeBulkMail {
    async fn enqueue(&self, command: BulkMailCommand<'_>) -> Result<(), ConfigurationPortError> {
        self.commands.lock().expect("commands").push((
            command.audience,
            command.actor.to_string(),
            command.maximum_recipients,
        ));
        Ok(())
    }
}

fn service() -> ServiceHarness {
    let commits = Arc::new(Mutex::new(Vec::new()));
    let active_values = ConfigurationMap::from([
        (
            "app_name".to_string(),
            ConfigurationValue::String("Before".to_string()),
        ),
        (
            "server_token".to_string(),
            ConfigurationValue::String("a-real-secret-value".to_string()),
        ),
    ]);
    let repository = FakeRepository {
        snapshot: ConfigurationSnapshot {
            revision: 7,
            groups: ConfigurationGroups::from([
                (
                    "site".to_string(),
                    ConfigurationMap::from([(
                        "app_name".to_string(),
                        ConfigurationValue::String("Before".to_string()),
                    )]),
                ),
                (
                    "server".to_string(),
                    ConfigurationMap::from([(
                        "server_token".to_string(),
                        ConfigurationValue::String("a-real-secret-value".to_string()),
                    )]),
                ),
            ]),
        },
        active: ActiveConfiguration {
            revision: 7,
            values: active_values,
            effective_admin_path: "fallback1".to_string(),
        },
        commits: commits.clone(),
    };
    let external = FakeExternal::default();
    let bulk = FakeBulkMail::default();
    (
        ConfigurationService::new(repository, external.clone(), bulk.clone()),
        commits,
        external,
        bulk,
    )
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
fn view_redacts_secrets_and_selects_only_known_groups() {
    let (service, _, _, _) = service();
    let selected = service.view(Some("server")).expect("selected view");
    assert_eq!(selected.revision, 7);
    assert_eq!(selected.groups.len(), 1);
    assert_eq!(
        selected.groups["server"]["server_token"],
        ConfigurationValue::String(REDACTED_SECRET.to_string())
    );

    let unknown = service.view(Some("unknown")).expect("full fallback view");
    assert_eq!(unknown.groups.len(), 2);
}

#[test]
fn patch_strips_round_tripped_secrets_and_effective_path_before_noop_check() {
    let (service, commits, _, _) = service();
    let changes = ConfigurationMap::from([
        (
            "server_token".to_string(),
            ConfigurationValue::String(REDACTED_SECRET.to_string()),
        ),
        (
            "secure_path".to_string(),
            ConfigurationValue::String("/fallback1/".to_string()),
        ),
    ]);
    assert!(matches!(
        run(service.patch(changes, 7, "admin@example.test")),
        Ok(ConfigurationPatchOutcome::Unchanged)
    ));
    assert!(commits.lock().expect("commits").is_empty());
}

#[test]
fn patch_commits_normalized_values_with_actor_and_attaches_database_revision() {
    let (service, commits, _, _) = service();
    let outcome = run(service.patch(
        ConfigurationMap::from([(
            "app_name".to_string(),
            ConfigurationValue::String("After".to_string()),
        )]),
        7,
        " admin@example.test ",
    ))
    .expect("patch");
    let ConfigurationPatchOutcome::Committed {
        activation,
        revision,
    } = outcome
    else {
        panic!("expected commit");
    };
    assert_eq!(revision, 8);
    assert_eq!(activation["revision"], ConfigurationValue::Integer(8));
    let commits = commits.lock().expect("commits");
    assert_eq!(commits[0].0, 7);
    assert_eq!(commits[0].2, "admin:admin@example.test");
}

#[test]
fn stale_and_nonpositive_revisions_are_rejected_before_commit() {
    let (service, commits, _, _) = service();
    for revision in [0, -1, 6] {
        let error = run(service.patch(ConfigurationMap::new(), revision, "admin@example.test"))
            .expect_err("revision rejected");
        let expected = if revision <= 0 {
            ConfigurationCode::ConfigValidationFailed
        } else {
            ConfigurationCode::ConfigRevisionConflict
        };
        assert!(matches!(
            error,
            ConfigurationError::Business { code, .. } if code == expected
        ));
    }
    assert!(commits.lock().expect("commits").is_empty());
}

#[test]
fn patch_validation_preserves_native_types_and_security_rules() {
    for (key, value) in [
        ("force_https", ConfigurationValue::String("1".to_string())),
        ("email_port", ConfigurationValue::Integer(65_536)),
        (
            "secure_path",
            ConfigurationValue::String("short".to_string()),
        ),
        (
            "deposit_bounus",
            ConfigurationValue::StringList(vec!["invalid".to_string()]),
        ),
        (
            "server_token",
            ConfigurationValue::String("too-short".to_string()),
        ),
    ] {
        assert!(validate_patch(&ConfigurationMap::from([(key.to_string(), value)])).is_err());
    }
    validate_patch(&ConfigurationMap::from([
        (
            "deposit_bounus".to_string(),
            ConfigurationValue::StringList(vec!["100:20.5".to_string()]),
        ),
        (
            "try_out_hour".to_string(),
            ConfigurationValue::Number("1.5".to_string()),
        ),
    ]))
    .expect("valid native patch");
}

#[test]
fn patch_validation_pins_duration_and_exact_decimal_bounds() {
    for field in [
        "show_subscribe_expire",
        "register_limit_expire",
        "password_limit_expire",
    ] {
        for value in [0, -1, 525_601] {
            assert!(
                validate_patch(&ConfigurationMap::from([(
                    field.to_string(),
                    ConfigurationValue::Integer(value),
                )]))
                .is_err()
            );
        }
        validate_patch(&ConfigurationMap::from([(
            field.to_string(),
            ConfigurationValue::Integer(525_600),
        )]))
        .expect("maximum safe duration");
    }

    for (field, accepted, rejected) in [
        (
            "commission_withdraw_limit",
            ConfigurationValue::String("92233720368547758.07".to_string()),
            ConfigurationValue::String("92233720368547758.08".to_string()),
        ),
        (
            "try_out_hour",
            ConfigurationValue::Number("2562047788015215.5019444444".to_string()),
            ConfigurationValue::Number("2562047788015216".to_string()),
        ),
    ] {
        validate_patch(&ConfigurationMap::from([(field.to_string(), accepted)]))
            .expect("exact upper bound");
        assert!(validate_patch(&ConfigurationMap::from([(field.to_string(), rejected)])).is_err());
    }
}

#[test]
fn external_and_bulk_mail_use_cases_cross_only_declared_ports() {
    let (service, _, external, bulk) = service();
    run(service.test_mail("admin@example.test")).expect("test mail");
    run(service.set_telegram_webhook(Some("123:token"))).expect("telegram");
    run(service.send_bulk_mail(
        MailAudience::Staff,
        &BulkMailInput {
            subject: "Subject".to_string(),
            content: "Content".to_string(),
            filter: None,
        },
        "staff@example.test",
        "request-1",
    ))
    .expect("bulk mail");
    assert_eq!(
        external.mail.lock().expect("mail").as_slice(),
        ["admin@example.test"]
    );
    assert_eq!(
        external.telegram.lock().expect("telegram").as_slice(),
        [Some("123:token".to_string())]
    );
    assert_eq!(
        bulk.commands.lock().expect("commands").as_slice(),
        [(
            MailAudience::Staff,
            "staff:staff@example.test".to_string(),
            50_000
        )]
    );
}

#[test]
fn empty_bulk_mail_fields_are_rejected_without_touching_outbox_port() {
    let (service, _, _, bulk) = service();
    for (subject, content, field) in [("", "content", "subject"), ("subject", "", "content")] {
        let error = run(service.send_bulk_mail(
            MailAudience::Admin,
            &BulkMailInput {
                subject: subject.to_string(),
                content: content.to_string(),
                filter: None,
            },
            "admin@example.test",
            "request-1",
        ))
        .expect_err("empty field");
        assert!(
            matches!(error, ConfigurationError::Validation { field: actual, .. } if actual == field)
        );
    }
    assert!(bulk.commands.lock().expect("commands").is_empty());
}
