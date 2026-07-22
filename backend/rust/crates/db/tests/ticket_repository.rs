use sqlx::PgPool;
use v2board_application::ticket::{
    DurableMailDelivery, NewTicket, OperatorTicketListQuery, OperatorTicketOrder,
    OperatorTicketReply, OperatorTicketReplyOutcome, TicketCreateOutcome, TicketRepository,
    UserTicketReply, UserTicketReplyOutcome,
};
use v2board_db::ticket::PostgresTicketRepository;
use v2board_domain_model::TicketLevel;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so tests are safe to run in
// parallel.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn ticket_state_and_notification_outbox_share_the_postgres_port(pool: PgPool) {
    let repository = PostgresTicketRepository::new(pool.clone());
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let user_id = insert_user(&pool, &format!("ticket-user-{marker}@example.test")).await;
    let operator_id = insert_user(&pool, &format!("ticket-admin-{marker}@example.test")).await;

    let ticket_id = match repository
        .create(NewTicket {
            user_id,
            subject: "Need help".to_string(),
            level: TicketLevel::Medium,
            message: "Opening message".to_string(),
            created_at: 100,
            require_paid_order: false,
        })
        .await
        .expect("create ticket through repository port")
    {
        TicketCreateOutcome::Created(id) => id,
        outcome => panic!("unexpected ticket creation outcome: {outcome:?}"),
    };
    assert_eq!(
        repository
            .reply_as_user(UserTicketReply {
                ticket_id,
                user_id,
                message: "duplicate user turn".to_string(),
                replied_at: 101,
            })
            .await
            .expect("apply user reply guard"),
        UserTicketReplyOutcome::AwaitingOperator
    );

    let target = repository
        .operator_reply_target(ticket_id)
        .await
        .expect("load reply target")
        .expect("reply target exists");
    let batch_key = format!("ticket-test-{marker}");
    let delivery = DurableMailDelivery {
        batch_key: batch_key.clone(),
        payload_hash: format!("hash-{marker}"),
        actor: format!("ticket:{user_id}"),
        recipient: target.recipient_email.clone(),
        message_id: format!("<{marker}@mail.v2board.local>"),
        sender: "Board <sender@example.test>".to_string(),
        template_name: "mail.default.notify".to_string(),
        subject: "Ticket reply".to_string(),
        body: "Reply body".to_string(),
    };
    assert_eq!(
        repository
            .reply_as_operator(
                OperatorTicketReply {
                    ticket_id,
                    expected_user_id: target.user_id,
                    operator_id,
                    message: "Operator reply".to_string(),
                    replied_at: 102,
                },
                Some(&delivery),
            )
            .await
            .expect("reply and enqueue notification"),
        OperatorTicketReplyOutcome::Replied
    );
    let outbox_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mail_outbox WHERE batch_key = $1")
            .bind(&batch_key)
            .fetch_one(&pool)
            .await
            .expect("count transactional outbox items");
    assert_eq!(outbox_count, 1);

    assert_eq!(
        repository
            .reply_as_user(UserTicketReply {
                ticket_id,
                user_id,
                message: "Thanks".to_string(),
                replied_at: 103,
            })
            .await
            .expect("reply after operator turn"),
        UserTicketReplyOutcome::Replied
    );
    let detail = repository
        .find_for_operator(ticket_id)
        .await
        .expect("load operator detail")
        .expect("ticket detail exists");
    assert_eq!(detail.messages.len(), 3);
    assert!(detail.messages[1].is_me, "operator message is marked is_me");
    assert!(
        !detail.messages[2].is_me,
        "owner message is not marked is_me"
    );

    let page = repository
        .list_for_operator(OperatorTicketListQuery {
            limit: 10,
            offset: 0,
            status: Some(0),
            reply_statuses: vec![0],
            email: Some(target.recipient_email),
            order: OperatorTicketOrder::UpdatedAt,
        })
        .await
        .expect("list operator tickets");
    assert!(page.items.iter().any(|ticket| ticket.id == ticket_id));
    assert!(
        repository
            .close_as_user(user_id, ticket_id, 104)
            .await
            .expect("close user ticket")
    );
}

async fn insert_user(pool: &PgPool, email: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users (email, password, uuid, token, created_at, updated_at) \
         VALUES ($1, 'not-used', $2, $3, 1, 1) RETURNING id",
    )
    .bind(email)
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(uuid::Uuid::new_v4().simple().to_string())
    .fetch_one(pool)
    .await
    .expect("insert ticket test user")
}
