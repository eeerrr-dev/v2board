use v2board_application::auth::{
    AuthCache, EmailCodeScope, LimitedEmailCodeResult, RegistrationReservation, SessionIdentity,
    SessionMetadata,
};
use v2board_auth_adapters::RedisAuthCache;

#[tokio::test]
async fn redis_adapter_preserves_atomic_limits_and_opaque_session_lifecycle() {
    let Ok(redis_url) = std::env::var("RUST_INTEGRATION_REDIS_URL") else {
        return;
    };
    let client = redis::Client::open(redis_url).expect("parse integration Redis URL");
    let connection = redis::aio::ConnectionManager::new(client)
        .await
        .expect("connect to integration Redis");
    let cache = RedisAuthCache::new(connection, uuid::Uuid::new_v4());
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_secs(),
    )
    .expect("current epoch fits i64");

    assert!(
        cache
            .reserve_login_attempt("User@Example.Test", Some("203.0.113.7"), 1, 10, 60)
            .await
            .expect("reserve first login attempt")
    );
    assert!(
        !cache
            .reserve_login_attempt("user@example.test", Some("203.0.113.7"), 1, 10, 60)
            .await
            .expect("enforce normalized account limit")
    );
    cache
        .release_login_attempt("user@example.test", Some("203.0.113.7"))
        .await;
    assert!(
        cache
            .reserve_login_attempt("user@example.test", Some("203.0.113.7"), 1, 10, 60)
            .await
            .expect("released login reservation is reusable")
    );

    let registration = RegistrationReservation {
        client_ip: "203.0.113.8".to_string(),
        token: "registration-1".to_string(),
    };
    assert!(
        cache
            .reserve_registration_slot(&registration, now, now + 60, 1)
            .await
            .expect("reserve registration slot")
    );
    let blocked = RegistrationReservation {
        client_ip: registration.client_ip.clone(),
        token: "registration-2".to_string(),
    };
    assert!(
        !cache
            .reserve_registration_slot(&blocked, now, now + 60, 1)
            .await
            .expect("enforce registration slot limit")
    );
    cache.release_registration_slot(&registration).await;
    assert!(
        cache
            .reserve_registration_slot(&blocked, now, now + 60, 1)
            .await
            .expect("released registration slot is reusable")
    );

    assert!(
        cache
            .reserve_email_code("mail@example.test", "123456", now)
            .await
            .expect("reserve email code")
    );
    assert_eq!(
        cache
            .consume_email_code(
                "mail@example.test",
                "000000",
                EmailCodeScope::Registration,
                1,
                60,
            )
            .await
            .expect("record invalid code"),
        LimitedEmailCodeResult::Incorrect
    );
    assert_eq!(
        cache
            .consume_email_code(
                "mail@example.test",
                "123456",
                EmailCodeScope::Registration,
                1,
                60,
            )
            .await
            .expect("enforce code failure ceiling"),
        LimitedEmailCodeResult::Limited
    );
    assert!(
        cache
            .reserve_email_code("fresh@example.test", "654321", now)
            .await
            .expect("reserve fresh email code")
    );
    assert_eq!(
        cache
            .consume_email_code(
                "fresh@example.test",
                "654321",
                EmailCodeScope::PasswordReset,
                3,
                60,
            )
            .await
            .expect("consume email code once"),
        LimitedEmailCodeResult::Consumed
    );
    assert_eq!(
        cache
            .consume_email_code(
                "fresh@example.test",
                "654321",
                EmailCodeScope::PasswordReset,
                3,
                60,
            )
            .await
            .expect("consumed code is absent"),
        LimitedEmailCodeResult::Incorrect
    );

    cache
        .put_temporary_token("temporary", 7, 3, 60)
        .await
        .expect("store temporary token");
    let temporary = cache
        .take_temporary_token("temporary")
        .await
        .expect("consume temporary token")
        .expect("temporary identity exists");
    assert_eq!((temporary.user_id, temporary.session_epoch), (7, 3));
    assert!(
        cache
            .take_temporary_token("temporary")
            .await
            .expect("second temporary-token read")
            .is_none()
    );

    let first_identity = SessionIdentity {
        user_id: 7,
        session_id: "session-1".to_string(),
        session_epoch: 3,
    };
    let second_identity = SessionIdentity {
        user_id: 7,
        session_id: "session-2".to_string(),
        session_epoch: 3,
    };
    let metadata = SessionMetadata {
        ip: Some("203.0.113.9".to_string()),
        login_at: now,
        user_agent: Some("browser".to_string()),
        expires_at: Some(now + 60),
        password_authenticated: true,
    };
    assert!(
        cache
            .add_session(&first_identity, &metadata, "bearer-1", 60, 1, now)
            .await
            .expect("insert first opaque session")
    );
    assert!(
        cache
            .add_session(&second_identity, &metadata, "bearer-2", 60, 1, now + 1)
            .await
            .expect("insert replacement opaque session")
    );
    assert!(
        cache
            .session_identity("bearer-1")
            .await
            .expect("load evicted bearer")
            .is_none(),
        "session cardinality eviction removes the reverse mapping"
    );
    assert_eq!(
        cache
            .session_identity("bearer-2")
            .await
            .expect("load active bearer"),
        Some(second_identity.clone())
    );
    assert_eq!(
        cache
            .session_metadata(7, "session-2")
            .await
            .expect("load active metadata"),
        Some(metadata)
    );
    cache
        .remove_session(7, "session-2")
        .await
        .expect("remove active session");
    assert!(
        cache
            .session_identity("bearer-2")
            .await
            .expect("load removed bearer")
            .is_none()
    );

    assert!(
        cache
            .put_step_up("step-up", 7, "session-2", 60)
            .await
            .expect("store step-up token")
    );
    assert_eq!(
        cache
            .step_up_identity("step-up")
            .await
            .expect("load step-up token"),
        Some((7, "session-2".to_string()))
    );
}
