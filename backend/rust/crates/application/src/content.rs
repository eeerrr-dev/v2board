use std::collections::BTreeMap;

use v2board_domain_model::{
    ContentVisibility, KnowledgeAccess, KnowledgeTemplateValues, SubscriptionAvailability,
    render_knowledge_body,
};

use crate::{ApplicationError, RepositoryError};

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notice {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub visibility: ContentVisibility,
    pub img_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeSummary {
    pub id: i32,
    pub category: String,
    pub title: String,
    pub sort: Option<i32>,
    pub visibility: ContentVisibility,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeArticle {
    pub id: i32,
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub sort: Option<i32>,
    pub visibility: ContentVisibility,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentPage<T> {
    pub items: Vec<T>,
    pub total: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeCreateInput {
    pub title: String,
    pub content: String,
    pub img_url: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewNotice {
    pub title: String,
    pub content: String,
    pub visibility: ContentVisibility,
    pub img_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NullableUpdate<T> {
    #[default]
    Retain,
    Clear,
    Set(T),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NoticePatchInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub img_url: NullableUpdate<String>,
    pub tags: NullableUpdate<Vec<String>>,
    pub visibility: Option<ContentVisibility>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeChanges {
    pub title: Option<String>,
    pub content: Option<String>,
    pub img_url: NullableUpdate<String>,
    pub tags: NullableUpdate<Vec<String>>,
    pub visibility: Option<ContentVisibility>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeCreateInput {
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewKnowledge {
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub visibility: ContentVisibility,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KnowledgePatchInput {
    pub language: Option<String>,
    pub category: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub visibility: Option<ContentVisibility>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeChanges {
    pub language: Option<String>,
    pub category: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub visibility: Option<ContentVisibility>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeSearch {
    pub language: String,
    pub keyword: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoticePageRequest {
    pub limit: i64,
    pub offset: i64,
}

/// Persistence facts needed for the pure subscription-access decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeReaderFacts {
    pub user_id: i64,
    pub token: String,
    pub banned: bool,
    pub transfer_enable: i64,
    pub expiry: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KnowledgeReader {
    user_id: i64,
    token: String,
    access: KnowledgeAccess,
}

/// A loaded, access-classified article. The raw body and access decision stay
/// private so the inbound adapter cannot accidentally bypass rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedKnowledgeDetail {
    article: KnowledgeArticle,
    reader: KnowledgeReader,
}

impl PreparedKnowledgeDetail {
    pub const fn user_id(&self) -> i64 {
        self.reader.user_id
    }

    pub fn subscribe_token(&self) -> &str {
        &self.reader.token
    }

    pub fn render(mut self, values: KnowledgeTemplateContext) -> KnowledgeArticle {
        self.article.body = render_knowledge_body(
            &self.article.body,
            self.reader.access,
            KnowledgeTemplateValues {
                site_name: &values.site_name,
                subscribe_url: &values.subscribe_url,
                percent_encoded_subscribe_url: &values.percent_encoded_subscribe_url,
                safe_base64_subscribe_url: &values.safe_base64_subscribe_url,
                subscribe_token: &self.reader.token,
            },
        );
        self.article
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeTemplateContext {
    pub site_name: String,
    pub subscribe_url: String,
    pub percent_encoded_subscribe_url: String,
    pub safe_base64_subscribe_url: String,
}

/// Content persistence is expressed in business records and atomic outcomes,
/// never database rows or transactions.
#[allow(async_fn_in_trait)]
pub trait ContentRepository: Send + Sync {
    async fn list_notices(&self) -> RepositoryResult<Vec<Notice>>;
    async fn create_notice(&self, notice: NewNotice) -> RepositoryResult<i32>;
    async fn update_notice(&self, id: i64, changes: NoticeChanges) -> RepositoryResult<bool>;
    async fn delete_notice(&self, id: i64) -> RepositoryResult<bool>;

    async fn list_knowledge(&self) -> RepositoryResult<Vec<KnowledgeSummary>>;
    async fn find_knowledge(&self, id: i64) -> RepositoryResult<Option<KnowledgeArticle>>;
    async fn create_knowledge(&self, knowledge: NewKnowledge) -> RepositoryResult<i32>;
    async fn update_knowledge(&self, id: i64, changes: KnowledgeChanges) -> RepositoryResult<bool>;
    async fn delete_knowledge(&self, id: i64) -> RepositoryResult<bool>;
    async fn list_knowledge_categories(&self) -> RepositoryResult<Vec<String>>;
    async fn sort_knowledge(&self, ids: &[i64]) -> RepositoryResult<()>;

    async fn search_published_knowledge(
        &self,
        search: &KnowledgeSearch,
    ) -> RepositoryResult<Vec<KnowledgeSummary>>;
    async fn find_published_knowledge(&self, id: i64)
    -> RepositoryResult<Option<KnowledgeArticle>>;
    async fn list_published_knowledge_categories(
        &self,
        language: &str,
    ) -> RepositoryResult<Vec<String>>;
    async fn list_published_notices(
        &self,
        page: NoticePageRequest,
    ) -> RepositoryResult<ContentPage<Notice>>;
    async fn find_knowledge_reader(
        &self,
        user_id: i64,
    ) -> RepositoryResult<Option<KnowledgeReaderFacts>>;
}

#[derive(Clone, Debug)]
pub struct ContentService<R> {
    repository: R,
}

impl<R> ContentService<R>
where
    R: ContentRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn notices(&self) -> Result<Vec<Notice>, ApplicationError> {
        Ok(self.repository.list_notices().await?)
    }

    pub async fn create_notice(
        &self,
        input: NoticeCreateInput,
        now: i64,
    ) -> Result<i32, ApplicationError> {
        Ok(self
            .repository
            .create_notice(NewNotice {
                title: input.title,
                content: input.content,
                visibility: ContentVisibility::Visible,
                img_url: input.img_url,
                tags: input.tags,
                created_at: now,
                updated_at: now,
            })
            .await?)
    }

    pub async fn patch_notice(
        &self,
        id: i64,
        input: NoticePatchInput,
        now: i64,
    ) -> Result<(), ApplicationError> {
        let found = self
            .repository
            .update_notice(
                id,
                NoticeChanges {
                    title: input.title,
                    content: input.content,
                    img_url: input.img_url,
                    tags: input.tags,
                    visibility: input.visibility,
                    updated_at: now,
                },
            )
            .await?;
        if found {
            Ok(())
        } else {
            Err(ApplicationError::NoticeNotFound)
        }
    }

    pub async fn delete_notice(&self, id: i64) -> Result<(), ApplicationError> {
        if self.repository.delete_notice(id).await? {
            Ok(())
        } else {
            Err(ApplicationError::NoticeNotFound)
        }
    }

    pub async fn knowledge(&self) -> Result<Vec<KnowledgeSummary>, ApplicationError> {
        Ok(self.repository.list_knowledge().await?)
    }

    pub async fn knowledge_detail(&self, id: i64) -> Result<KnowledgeArticle, ApplicationError> {
        self.repository
            .find_knowledge(id)
            .await?
            .ok_or(ApplicationError::KnowledgeNotFound)
    }

    pub async fn create_knowledge(
        &self,
        input: KnowledgeCreateInput,
        now: i64,
    ) -> Result<i32, ApplicationError> {
        Ok(self
            .repository
            .create_knowledge(NewKnowledge {
                language: input.language,
                category: input.category,
                title: input.title,
                body: input.body,
                visibility: ContentVisibility::Hidden,
                created_at: now,
                updated_at: now,
            })
            .await?)
    }

    pub async fn patch_knowledge(
        &self,
        id: i64,
        input: KnowledgePatchInput,
        now: i64,
    ) -> Result<(), ApplicationError> {
        let found = self
            .repository
            .update_knowledge(
                id,
                KnowledgeChanges {
                    language: input.language,
                    category: input.category,
                    title: input.title,
                    body: input.body,
                    visibility: input.visibility,
                    updated_at: now,
                },
            )
            .await?;
        if found {
            Ok(())
        } else {
            Err(ApplicationError::KnowledgeNotFound)
        }
    }

    pub async fn delete_knowledge(&self, id: i64) -> Result<(), ApplicationError> {
        if self.repository.delete_knowledge(id).await? {
            Ok(())
        } else {
            Err(ApplicationError::KnowledgeNotFound)
        }
    }

    pub async fn knowledge_categories(&self) -> Result<Vec<String>, ApplicationError> {
        Ok(self.repository.list_knowledge_categories().await?)
    }

    pub async fn sort_knowledge(&self, ids: &[i64]) -> Result<(), ApplicationError> {
        Ok(self.repository.sort_knowledge(ids).await?)
    }

    pub async fn published_knowledge(
        &self,
        language: String,
        keyword: Option<String>,
    ) -> Result<BTreeMap<String, Vec<KnowledgeSummary>>, ApplicationError> {
        let keyword = keyword.filter(|value| !value.trim().is_empty());
        let rows = self
            .repository
            .search_published_knowledge(&KnowledgeSearch { language, keyword })
            .await?;
        let mut grouped = BTreeMap::<String, Vec<KnowledgeSummary>>::new();
        for row in rows {
            grouped.entry(row.category.clone()).or_default().push(row);
        }
        Ok(grouped)
    }

    pub async fn prepare_published_knowledge_detail(
        &self,
        user_id: i64,
        id: i64,
        now: i64,
    ) -> Result<PreparedKnowledgeDetail, ApplicationError> {
        let reader = self
            .repository
            .find_knowledge_reader(user_id)
            .await?
            .ok_or(ApplicationError::ReaderNotFound)?;
        let article = self
            .repository
            .find_published_knowledge(id)
            .await?
            .ok_or(ApplicationError::ArticleNotFound)?;
        let access = if (SubscriptionAvailability {
            banned: reader.banned,
            transfer_enable: reader.transfer_enable,
            expiry: reader.expiry,
        })
        .is_available(now)
        {
            KnowledgeAccess::Full
        } else {
            KnowledgeAccess::Restricted
        };
        Ok(PreparedKnowledgeDetail {
            article,
            reader: KnowledgeReader {
                user_id: reader.user_id,
                token: reader.token,
                access,
            },
        })
    }

    pub async fn published_knowledge_categories(
        &self,
        language: &str,
    ) -> Result<Vec<String>, ApplicationError> {
        Ok(self
            .repository
            .list_published_knowledge_categories(language)
            .await?)
    }

    pub async fn published_notices(
        &self,
        page: NoticePageRequest,
    ) -> Result<ContentPage<Notice>, ApplicationError> {
        Ok(self.repository.list_published_notices(page).await?)
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
        notices: Vec<Notice>,
        knowledge: Vec<KnowledgeArticle>,
        summaries: Vec<KnowledgeSummary>,
        reader: Option<KnowledgeReaderFacts>,
        created_notice: Option<NewNotice>,
        created_knowledge: Option<NewKnowledge>,
    }

    #[derive(Clone, Default)]
    struct FakeContentRepository(Arc<Mutex<FakeState>>);

    impl ContentRepository for FakeContentRepository {
        async fn list_notices(&self) -> RepositoryResult<Vec<Notice>> {
            Ok(self.0.lock().unwrap().notices.clone())
        }

        async fn create_notice(&self, notice: NewNotice) -> RepositoryResult<i32> {
            self.0.lock().unwrap().created_notice = Some(notice);
            Ok(11)
        }

        async fn update_notice(&self, id: i64, _: NoticeChanges) -> RepositoryResult<bool> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .notices
                .iter()
                .any(|notice| i64::from(notice.id) == id))
        }

        async fn delete_notice(&self, id: i64) -> RepositoryResult<bool> {
            self.update_notice(
                id,
                NoticeChanges {
                    title: None,
                    content: None,
                    img_url: NullableUpdate::Retain,
                    tags: NullableUpdate::Retain,
                    visibility: None,
                    updated_at: 0,
                },
            )
            .await
        }

        async fn list_knowledge(&self) -> RepositoryResult<Vec<KnowledgeSummary>> {
            Ok(self.0.lock().unwrap().summaries.clone())
        }

        async fn find_knowledge(&self, id: i64) -> RepositoryResult<Option<KnowledgeArticle>> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .knowledge
                .iter()
                .find(|article| i64::from(article.id) == id)
                .cloned())
        }

        async fn create_knowledge(&self, knowledge: NewKnowledge) -> RepositoryResult<i32> {
            self.0.lock().unwrap().created_knowledge = Some(knowledge);
            Ok(12)
        }

        async fn update_knowledge(&self, id: i64, _: KnowledgeChanges) -> RepositoryResult<bool> {
            Ok(self.find_knowledge(id).await?.is_some())
        }

        async fn delete_knowledge(&self, id: i64) -> RepositoryResult<bool> {
            Ok(self.find_knowledge(id).await?.is_some())
        }

        async fn list_knowledge_categories(&self) -> RepositoryResult<Vec<String>> {
            Ok(vec!["All".to_string()])
        }

        async fn sort_knowledge(&self, _: &[i64]) -> RepositoryResult<()> {
            Ok(())
        }

        async fn search_published_knowledge(
            &self,
            _: &KnowledgeSearch,
        ) -> RepositoryResult<Vec<KnowledgeSummary>> {
            Ok(self.0.lock().unwrap().summaries.clone())
        }

        async fn find_published_knowledge(
            &self,
            id: i64,
        ) -> RepositoryResult<Option<KnowledgeArticle>> {
            self.find_knowledge(id).await
        }

        async fn list_published_knowledge_categories(
            &self,
            _: &str,
        ) -> RepositoryResult<Vec<String>> {
            Ok(vec!["Published".to_string()])
        }

        async fn list_published_notices(
            &self,
            _: NoticePageRequest,
        ) -> RepositoryResult<ContentPage<Notice>> {
            let items = self.0.lock().unwrap().notices.clone();
            Ok(ContentPage {
                total: items.len() as i64,
                items,
            })
        }

        async fn find_knowledge_reader(
            &self,
            _: i64,
        ) -> RepositoryResult<Option<KnowledgeReaderFacts>> {
            Ok(self.0.lock().unwrap().reader.clone())
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

    fn article(body: &str) -> KnowledgeArticle {
        KnowledgeArticle {
            id: 7,
            language: "en-US".to_string(),
            category: "Guides".to_string(),
            title: "Setup".to_string(),
            body: body.to_string(),
            sort: Some(1),
            visibility: ContentVisibility::Visible,
            created_at: 10,
            updated_at: 20,
        }
    }

    #[test]
    fn create_defaults_are_application_policy_not_database_defaults() {
        let repository = FakeContentRepository::default();
        let service = ContentService::new(repository.clone());
        assert_eq!(
            block_on(service.create_notice(
                NoticeCreateInput {
                    title: "Notice".to_string(),
                    content: "Body".to_string(),
                    img_url: None,
                    tags: None,
                },
                42,
            ))
            .unwrap(),
            11
        );
        assert_eq!(
            block_on(service.create_knowledge(
                KnowledgeCreateInput {
                    language: "en-US".to_string(),
                    category: "Guides".to_string(),
                    title: "Setup".to_string(),
                    body: "Body".to_string(),
                },
                43,
            ))
            .unwrap(),
            12
        );
        let state = repository.0.lock().unwrap();
        let notice = state.created_notice.as_ref().unwrap();
        assert_eq!(notice.visibility, ContentVisibility::Visible);
        assert_eq!((notice.created_at, notice.updated_at), (42, 42));
        let knowledge = state.created_knowledge.as_ref().unwrap();
        assert_eq!(knowledge.visibility, ContentVisibility::Hidden);
        assert_eq!((knowledge.created_at, knowledge.updated_at), (43, 43));
    }

    #[test]
    fn published_list_grouping_is_application_owned() {
        let repository = FakeContentRepository::default();
        repository.0.lock().unwrap().summaries = vec![
            KnowledgeSummary {
                id: 1,
                category: "Apps".to_string(),
                title: "One".to_string(),
                sort: Some(1),
                visibility: ContentVisibility::Visible,
                updated_at: 1,
            },
            KnowledgeSummary {
                id: 2,
                category: "Billing".to_string(),
                title: "Two".to_string(),
                sort: Some(2),
                visibility: ContentVisibility::Visible,
                updated_at: 2,
            },
        ];
        let grouped = block_on(
            ContentService::new(repository)
                .published_knowledge("en-US".to_string(), Some("   ".to_string())),
        )
        .unwrap();
        assert_eq!(
            grouped.keys().cloned().collect::<Vec<_>>(),
            ["Apps", "Billing"]
        );
    }

    #[test]
    fn prepared_detail_cannot_bypass_access_masking_or_template_rendering() {
        let repository = FakeContentRepository::default();
        {
            let mut state = repository.0.lock().unwrap();
            state.knowledge = vec![article(
                "{{siteName}} <!--access start-->secret<!--access end--> {{subscribeUrl}} {{urlEncodeSubscribeUrl}} {{safeBase64SubscribeUrl}} {{subscribeToken}}",
            )];
            state.reader = Some(KnowledgeReaderFacts {
                user_id: 9,
                token: "token".to_string(),
                banned: true,
                transfer_enable: 100,
                expiry: None,
            });
        }
        let prepared =
            block_on(ContentService::new(repository).prepare_published_knowledge_detail(9, 7, 100))
                .unwrap();
        assert_eq!(prepared.user_id(), 9);
        assert_eq!(prepared.subscribe_token(), "token");
        let rendered = prepared.render(KnowledgeTemplateContext {
            site_name: "Board".to_string(),
            subscribe_url: "https://example.test/sub".to_string(),
            percent_encoded_subscribe_url: "encoded".to_string(),
            safe_base64_subscribe_url: "base64".to_string(),
        });
        assert_eq!(
            rendered.body,
            "Board <div class=\"v2board-no-access\">You must have a valid subscription to view content in this area</div> https://example.test/sub encoded base64 token"
        );
    }

    #[test]
    fn resource_specific_not_found_outcomes_survive_the_port_boundary() {
        let service = ContentService::new(FakeContentRepository::default());
        assert!(matches!(
            block_on(service.delete_notice(404)),
            Err(ApplicationError::NoticeNotFound)
        ));
        assert!(matches!(
            block_on(service.knowledge_detail(404)),
            Err(ApplicationError::KnowledgeNotFound)
        ));
    }
}
