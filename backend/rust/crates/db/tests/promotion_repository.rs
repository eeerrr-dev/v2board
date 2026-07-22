use sqlx::PgPool;
use tokio::task::JoinSet;
use v2board_application::promotion::{
    CouponCreateInput, CouponPatchInput, GenerateCodeOutcome, GiftCardCreateInput,
    GiftCardPatchInput, PromotionError, PromotionService,
};
use v2board_db::coupon::PostgresPromotionRepository;
use v2board_db::coupon::{decrement_coupon_use, find_coupon_for_update};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so tests are safe to run in
// parallel and no longer need hand-written DELETE cleanup.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn coupon_consumption_lock_and_atomic_decrement_allow_only_one_last_use(pool: PgPool) {
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let code = format!("LAST-{marker}");
    let coupon_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO coupon
            (code, name, type, value, show, limit_use, started_at, ended_at, created_at, updated_at)
        VALUES ($1, $1, 1, 100, 1, 1, 0, 200, 1, 1)
        RETURNING id
        "#,
    )
    .bind(&code)
    .fetch_one(&pool)
    .await
    .expect("insert last-use coupon fixture");

    let mut attempts = JoinSet::new();
    for _ in 0..2 {
        let pool = pool.clone();
        let code = code.clone();
        attempts.spawn(async move {
            let mut tx = pool.begin().await.expect("begin coupon attempt");
            let coupon = find_coupon_for_update(&mut tx, &code)
                .await
                .expect("lock coupon")
                .expect("coupon exists");
            let consumed = coupon.remaining_uses.is_some_and(|remaining| remaining > 0)
                && decrement_coupon_use(&mut tx, coupon.id)
                    .await
                    .expect("atomically decrement coupon");
            tx.commit().await.expect("commit coupon attempt");
            consumed
        });
    }
    let mut consumed = 0;
    while let Some(attempt) = attempts.join_next().await {
        consumed += usize::from(attempt.expect("coupon consumption task"));
    }
    assert_eq!(consumed, 1);
    let remaining: i32 = sqlx::query_scalar("SELECT limit_use FROM coupon WHERE id = $1")
        .bind(coupon_id)
        .fetch_one(&pool)
        .await
        .expect("load coupon after concurrent use");
    assert_eq!(remaining, 0);
}

#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn promotion_use_cases_round_trip_through_the_postgres_port(pool: PgPool) {
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let service = PromotionService::new(PostgresPromotionRepository::new(pool.clone()));

    let coupon_name = format!("promotion-coupon-{marker}");
    let coupon_code = format!("SAVE-{marker}");
    let coupon_id = match service
        .generate_coupon(coupon_input(&coupon_name, Some(coupon_code.clone())), 100)
        .await
        .expect("create a coupon through the application port")
    {
        GenerateCodeOutcome::Created(id) => id,
        GenerateCodeOutcome::Batch(_) => panic!("single coupon create returned a batch"),
    };
    let checked = service
        .check_coupon(99, &coupon_code.to_lowercase(), Some(7), 100)
        .await
        .expect("coupon lookup preserves case-insensitive identity");
    assert_eq!(
        (checked.id, checked.code.as_str()),
        (coupon_id, coupon_code.as_str())
    );
    assert!(matches!(
        service
            .generate_coupon(
                coupon_input(&coupon_name, Some(coupon_code.to_lowercase())),
                100,
            )
            .await,
        Err(PromotionError::DuplicateCouponCode)
    ));

    service
        .patch_coupon(
            i64::from(coupon_id),
            CouponPatchInput {
                name: Some(format!("{coupon_name}-updated")),
                remaining_uses: Some(None),
                visible: Some(false),
                ..CouponPatchInput::default()
            },
            101,
        )
        .await
        .expect("patch coupon including a nullable clear");
    let page = service
        .coupons(100, 0, Some("created_at"), Some("asc"))
        .await
        .expect("list coupons through the PostgreSQL adapter");
    let patched = page
        .items
        .into_iter()
        .find(|coupon| coupon.id == coupon_id)
        .expect("created coupon appears in the admin projection");
    assert_eq!(patched.name, format!("{coupon_name}-updated"));
    assert_eq!(patched.remaining_uses, None);
    assert!(!patched.visible);

    let duplicate_code = format!("DUP-{marker}");
    let duplicate_id = match service
        .generate_coupon(
            coupon_input(&coupon_name, Some(duplicate_code.clone())),
            102,
        )
        .await
        .expect("create duplicate-code fixture")
    {
        GenerateCodeOutcome::Created(id) => id,
        GenerateCodeOutcome::Batch(_) => panic!("single coupon create returned a batch"),
    };
    assert!(matches!(
        service
            .patch_coupon(
                i64::from(coupon_id),
                CouponPatchInput {
                    code: Some(duplicate_code),
                    ..CouponPatchInput::default()
                },
                103,
            )
            .await,
        Err(PromotionError::DuplicateCouponCode)
    ));

    let mut batch_coupon = coupon_input(&coupon_name, None);
    batch_coupon.generate_count = Some(3);
    let batch_coupon = service.generate_coupon(batch_coupon, 104).await;
    assert!(
        matches!(
            &batch_coupon,
            Ok(GenerateCodeOutcome::Batch(codes)) if codes.len() == 3
        ),
        "unexpected coupon batch outcome: {batch_coupon:?}"
    );

    let gift_name = format!("promotion-gift-{marker}");
    let gift_code = format!("GIFT-{marker}");
    let gift_id = match service
        .generate_gift_card(giftcard_input(&gift_name, Some(gift_code.clone())), 110)
        .await
        .expect("create a gift card through the application port")
    {
        GenerateCodeOutcome::Created(id) => id,
        GenerateCodeOutcome::Batch(_) => panic!("single gift-card create returned a batch"),
    };
    service
        .patch_gift_card(
            i64::from(gift_id),
            GiftCardPatchInput {
                kind_code: Some(1),
                value: Some(Some(500)),
                remaining_uses: Some(Some(2)),
                ..GiftCardPatchInput::default()
            },
            111,
        )
        .await
        .expect("patch gift card through the PostgreSQL adapter");
    let page = service
        .gift_cards(100, 0, None, None)
        .await
        .expect("list gift cards through the PostgreSQL adapter");
    let patched = page
        .items
        .into_iter()
        .find(|card| card.id == gift_id)
        .expect("created gift card appears in the admin projection");
    assert_eq!((patched.kind_code, patched.value), (1, Some(500)));
    assert_eq!(patched.remaining_uses, Some(2));

    let mut batch_gift = giftcard_input(&gift_name, None);
    batch_gift.generate_count = Some(3);
    let batch_gift = service.generate_gift_card(batch_gift, 112).await;
    assert!(
        matches!(
            &batch_gift,
            Ok(GenerateCodeOutcome::Batch(codes)) if codes.len() == 3
        ),
        "unexpected gift-card batch outcome: {batch_gift:?}"
    );

    service
        .delete_coupon(i64::from(coupon_id))
        .await
        .expect("delete coupon");
    assert!(matches!(
        service.delete_coupon(i64::from(coupon_id)).await,
        Err(PromotionError::CouponNotFound)
    ));
    service
        .delete_gift_card(i64::from(gift_id))
        .await
        .expect("delete gift card");
    assert!(matches!(
        service.delete_gift_card(i64::from(gift_id)).await,
        Err(PromotionError::GiftCardNotFound)
    ));

    assert_ne!(duplicate_id, coupon_id);
}

fn coupon_input(name: &str, code: Option<String>) -> CouponCreateInput {
    CouponCreateInput {
        name: name.to_string(),
        kind_code: 2,
        value: 25,
        starts_at: 0,
        ends_at: 200,
        remaining_uses: Some(5),
        per_user_limit: None,
        plan_ids: Some(vec![7]),
        periods: Some(vec!["month_price".to_string()]),
        code,
        generate_count: None,
    }
}

fn giftcard_input(name: &str, code: Option<String>) -> GiftCardCreateInput {
    GiftCardCreateInput {
        name: name.to_string(),
        kind_code: 4,
        value: None,
        plan_id: None,
        starts_at: 0,
        ends_at: 200,
        remaining_uses: None,
        code,
        generate_count: None,
    }
}
