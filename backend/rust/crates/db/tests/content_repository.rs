use sqlx::PgPool;
use v2board_application::{
    ApplicationError,
    content::{
        ContentRepository, ContentService, KnowledgeCreateInput, KnowledgePatchInput,
        NoticeCreateInput, NoticePageRequest, NoticePatchInput, NullableUpdate,
    },
};
use v2board_db::content::PostgresContentRepository;
use v2board_domain_model::ContentVisibility;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so tests are safe to run in
// parallel.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn content_use_cases_round_trip_through_the_postgres_port(pool: PgPool) {
    let repository = PostgresContentRepository::new(pool.clone());
    let service = ContentService::new(repository.clone());
    let marker = format!("content-adapter-{}", uuid::Uuid::new_v4());

    let notice_id = service
        .create_notice(
            NoticeCreateInput {
                title: marker.clone(),
                content: "notice body".to_string(),
                img_url: Some("https://example.test/notice.png".to_string()),
                tags: Some(vec!["弹窗".to_string()]),
            },
            100,
        )
        .await
        .expect("create notice through the application port");
    let published = service
        .published_notices(NoticePageRequest {
            limit: i64::MAX,
            offset: 0,
        })
        .await
        .expect("list published notices");
    let created_notice = published
        .items
        .iter()
        .find(|notice| notice.id == notice_id)
        .expect("new notices are application-visible by policy");
    assert_eq!(created_notice.visibility, ContentVisibility::Visible);
    assert_eq!(
        created_notice.tags.as_deref(),
        Some(["弹窗".to_string()].as_slice())
    );

    service
        .patch_notice(
            i64::from(notice_id),
            NoticePatchInput {
                img_url: NullableUpdate::Clear,
                visibility: Some(ContentVisibility::Hidden),
                ..NoticePatchInput::default()
            },
            101,
        )
        .await
        .expect("patch notice through the application port");
    let patched_notice = repository
        .list_notices()
        .await
        .expect("read admin notices")
        .into_iter()
        .find(|notice| notice.id == notice_id)
        .expect("patched notice exists");
    assert_eq!(patched_notice.visibility, ContentVisibility::Hidden);
    assert_eq!(patched_notice.img_url, None);
    assert_eq!(patched_notice.updated_at, 101);

    let knowledge_id = service
        .create_knowledge(
            KnowledgeCreateInput {
                language: "en-US".to_string(),
                category: marker.clone(),
                title: marker.clone(),
                body: "<!--access start-->secret<!--access end-->".to_string(),
            },
            200,
        )
        .await
        .expect("create knowledge through the application port");
    assert!(
        service
            .published_knowledge("en-US".to_string(), Some(marker.clone()))
            .await
            .expect("search hidden knowledge")
            .is_empty(),
        "new knowledge remains hidden until an explicit publish decision"
    );
    service
        .patch_knowledge(
            i64::from(knowledge_id),
            KnowledgePatchInput {
                visibility: Some(ContentVisibility::Visible),
                ..KnowledgePatchInput::default()
            },
            201,
        )
        .await
        .expect("publish knowledge through the application port");
    let grouped = service
        .published_knowledge("en-US".to_string(), Some(marker.clone()))
        .await
        .expect("search published knowledge");
    assert_eq!(grouped.get(&marker).map(Vec::len), Some(1));
    assert!(
        service
            .published_knowledge_categories("en-US")
            .await
            .expect("list published categories")
            .contains(&marker)
    );

    service
        .delete_notice(i64::from(notice_id))
        .await
        .expect("delete notice");
    assert!(matches!(
        service.delete_notice(i64::from(notice_id)).await,
        Err(ApplicationError::NoticeNotFound)
    ));
    service
        .delete_knowledge(i64::from(knowledge_id))
        .await
        .expect("delete knowledge");
    assert!(matches!(
        service.knowledge_detail(i64::from(knowledge_id)).await,
        Err(ApplicationError::KnowledgeNotFound)
    ));
}
