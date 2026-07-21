use v2board_domain_model::{Coupon, CouponRuleViolation, CouponUseContext, validate_coupon};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeSort {
    CreatedAtAscending,
    CreatedAtDescending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageRequest {
    pub limit: i64,
    pub offset: i64,
    pub sort: CodeSort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminCoupon {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub kind_code: i16,
    pub value: i32,
    pub visible: bool,
    pub remaining_uses: Option<i32>,
    pub per_user_limit: Option<i32>,
    pub plan_ids: Option<Vec<i64>>,
    pub periods: Option<Vec<String>>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CouponPage {
    pub items: Vec<AdminCoupon>,
    pub total: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GiftCard {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub kind_code: i16,
    pub value: Option<i32>,
    pub plan_id: Option<i32>,
    pub remaining_uses: Option<i32>,
    pub redeemed_user_ids: Vec<i64>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GiftCardPage {
    pub items: Vec<GiftCard>,
    pub total: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CouponCreateInput {
    pub name: String,
    pub kind_code: i64,
    pub value: i64,
    pub starts_at: i64,
    pub ends_at: i64,
    pub remaining_uses: Option<i64>,
    pub per_user_limit: Option<i64>,
    pub plan_ids: Option<Vec<i64>>,
    pub periods: Option<Vec<String>>,
    pub code: Option<String>,
    pub generate_count: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCoupon {
    pub input: CouponCreateInput,
    pub requested_code: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CouponPatchInput {
    pub name: Option<String>,
    pub kind_code: Option<i64>,
    pub value: Option<i64>,
    pub starts_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub remaining_uses: Option<Option<i64>>,
    pub per_user_limit: Option<Option<i64>>,
    pub plan_ids: Option<Option<Vec<i64>>>,
    pub periods: Option<Option<Vec<String>>>,
    pub code: Option<String>,
    pub visible: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CouponChanges {
    pub input: CouponPatchInput,
    pub requested_code: Option<String>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GiftCardCreateInput {
    pub name: String,
    pub kind_code: i64,
    pub value: Option<i64>,
    pub plan_id: Option<i64>,
    pub starts_at: i64,
    pub ends_at: i64,
    pub remaining_uses: Option<i64>,
    pub code: Option<String>,
    pub generate_count: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewGiftCard {
    pub input: GiftCardCreateInput,
    pub requested_code: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GiftCardPatchInput {
    pub name: Option<String>,
    pub kind_code: Option<i64>,
    pub value: Option<Option<i64>>,
    pub plan_id: Option<Option<i64>>,
    pub starts_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub remaining_uses: Option<Option<i64>>,
    pub code: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GiftCardChanges {
    pub input: GiftCardPatchInput,
    pub requested_code: Option<String>,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateCodeOutcome {
    Created(i32),
    DuplicateCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchCodeOutcome {
    Updated,
    NotFound,
    DuplicateCode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteCodeOutcome {
    Deleted,
    NotFound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GenerateCodeOutcome {
    Created(i32),
    Batch(Vec<String>),
}

#[allow(async_fn_in_trait)]
pub trait PromotionRepository: Send + Sync {
    async fn coupons(&self, page: PageRequest) -> RepositoryResult<CouponPage>;
    async fn gift_cards(&self, page: PageRequest) -> RepositoryResult<GiftCardPage>;
    async fn create_coupon(&self, coupon: NewCoupon) -> RepositoryResult<CreateCodeOutcome>;
    async fn generate_coupons(
        &self,
        coupon: NewCoupon,
        count: usize,
    ) -> RepositoryResult<Vec<String>>;
    async fn patch_coupon(
        &self,
        id: i64,
        changes: CouponChanges,
    ) -> RepositoryResult<PatchCodeOutcome>;
    async fn delete_coupon(&self, id: i64) -> RepositoryResult<DeleteCodeOutcome>;
    async fn create_gift_card(&self, card: NewGiftCard) -> RepositoryResult<CreateCodeOutcome>;
    async fn generate_gift_cards(
        &self,
        card: NewGiftCard,
        count: usize,
    ) -> RepositoryResult<Vec<String>>;
    async fn patch_gift_card(
        &self,
        id: i64,
        changes: GiftCardChanges,
    ) -> RepositoryResult<PatchCodeOutcome>;
    async fn delete_gift_card(&self, id: i64) -> RepositoryResult<DeleteCodeOutcome>;
    async fn coupon_by_code(&self, code: &str) -> RepositoryResult<Option<Coupon>>;
    async fn coupon_use_count(&self, coupon_id: i32, user_id: i64) -> RepositoryResult<i64>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromotionInputViolation {
    CouponGenerateCountTooLarge,
    CouponTypeInvalid,
    CouponValueInvalid,
    GiftCardGenerateCountTooLarge,
    GiftCardTypeInvalid,
    GiftCardValueRequired,
    GiftCardValueInvalid,
    GiftCardPlanRequired,
}

#[derive(Debug, thiserror::Error)]
pub enum PromotionError {
    #[error("invalid promotion input: {0:?}")]
    InvalidInput(PromotionInputViolation),
    #[error("sort_by field {0} is not sortable")]
    InvalidSortBy(String),
    #[error("sort_dir must be asc or desc, got {0}")]
    InvalidSortDirection(String),
    #[error("coupon code cannot be empty")]
    CouponCodeEmpty,
    #[error("coupon is invalid")]
    CouponInvalid,
    #[error("coupon rule rejected use: {0:?}")]
    CouponRule(CouponRuleViolation),
    #[error("coupon not found")]
    CouponNotFound,
    #[error("gift card not found")]
    GiftCardNotFound,
    #[error("coupon code already exists")]
    DuplicateCouponCode,
    #[error("gift card code already exists")]
    DuplicateGiftCardCode,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub struct PromotionService<R> {
    repository: R,
}

impl<R> PromotionService<R>
where
    R: PromotionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn coupons(
        &self,
        limit: i64,
        offset: i64,
        sort_by: Option<&str>,
        sort_direction: Option<&str>,
    ) -> Result<CouponPage, PromotionError> {
        let sort = resolve_sort(sort_by, sort_direction)?;
        Ok(self
            .repository
            .coupons(PageRequest {
                limit,
                offset,
                sort,
            })
            .await?)
    }

    pub async fn gift_cards(
        &self,
        limit: i64,
        offset: i64,
        sort_by: Option<&str>,
        sort_direction: Option<&str>,
    ) -> Result<GiftCardPage, PromotionError> {
        let sort = resolve_sort(sort_by, sort_direction)?;
        Ok(self
            .repository
            .gift_cards(PageRequest {
                limit,
                offset,
                sort,
            })
            .await?)
    }

    pub async fn generate_coupon(
        &self,
        input: CouponCreateInput,
        now: i64,
    ) -> Result<GenerateCodeOutcome, PromotionError> {
        validate_coupon_create(&input).map_err(PromotionError::InvalidInput)?;
        let requested_code = requested_code(input.code.as_deref());
        let count = positive_generation_count(input.generate_count);
        let coupon = NewCoupon {
            input,
            requested_code,
            created_at: now,
        };
        if let Some(count) = count {
            return Ok(GenerateCodeOutcome::Batch(
                self.repository.generate_coupons(coupon, count).await?,
            ));
        }
        match self.repository.create_coupon(coupon).await? {
            CreateCodeOutcome::Created(id) => Ok(GenerateCodeOutcome::Created(id)),
            CreateCodeOutcome::DuplicateCode => Err(PromotionError::DuplicateCouponCode),
        }
    }

    pub async fn patch_coupon(
        &self,
        id: i64,
        input: CouponPatchInput,
        now: i64,
    ) -> Result<(), PromotionError> {
        validate_coupon_patch(&input).map_err(PromotionError::InvalidInput)?;
        let requested_code = requested_code(input.code.as_deref());
        match self
            .repository
            .patch_coupon(
                id,
                CouponChanges {
                    input,
                    requested_code,
                    updated_at: now,
                },
            )
            .await?
        {
            PatchCodeOutcome::Updated => Ok(()),
            PatchCodeOutcome::NotFound => Err(PromotionError::CouponNotFound),
            PatchCodeOutcome::DuplicateCode => Err(PromotionError::DuplicateCouponCode),
        }
    }

    pub async fn delete_coupon(&self, id: i64) -> Result<(), PromotionError> {
        match self.repository.delete_coupon(id).await? {
            DeleteCodeOutcome::Deleted => Ok(()),
            DeleteCodeOutcome::NotFound => Err(PromotionError::CouponNotFound),
        }
    }

    pub async fn generate_gift_card(
        &self,
        input: GiftCardCreateInput,
        now: i64,
    ) -> Result<GenerateCodeOutcome, PromotionError> {
        validate_gift_card_create(&input).map_err(PromotionError::InvalidInput)?;
        let requested_code = requested_code(input.code.as_deref());
        let count = positive_generation_count(input.generate_count);
        let card = NewGiftCard {
            input,
            requested_code,
            created_at: now,
        };
        if let Some(count) = count {
            return Ok(GenerateCodeOutcome::Batch(
                self.repository.generate_gift_cards(card, count).await?,
            ));
        }
        match self.repository.create_gift_card(card).await? {
            CreateCodeOutcome::Created(id) => Ok(GenerateCodeOutcome::Created(id)),
            CreateCodeOutcome::DuplicateCode => Err(PromotionError::DuplicateGiftCardCode),
        }
    }

    pub async fn patch_gift_card(
        &self,
        id: i64,
        input: GiftCardPatchInput,
        now: i64,
    ) -> Result<(), PromotionError> {
        validate_gift_card_patch(&input).map_err(PromotionError::InvalidInput)?;
        let requested_code = requested_code(input.code.as_deref());
        match self
            .repository
            .patch_gift_card(
                id,
                GiftCardChanges {
                    input,
                    requested_code,
                    updated_at: now,
                },
            )
            .await?
        {
            PatchCodeOutcome::Updated => Ok(()),
            PatchCodeOutcome::NotFound => Err(PromotionError::GiftCardNotFound),
            PatchCodeOutcome::DuplicateCode => Err(PromotionError::DuplicateGiftCardCode),
        }
    }

    pub async fn delete_gift_card(&self, id: i64) -> Result<(), PromotionError> {
        match self.repository.delete_gift_card(id).await? {
            DeleteCodeOutcome::Deleted => Ok(()),
            DeleteCodeOutcome::NotFound => Err(PromotionError::GiftCardNotFound),
        }
    }

    pub async fn check_coupon(
        &self,
        user_id: i64,
        code: &str,
        plan_id: Option<i32>,
        now: i64,
    ) -> Result<Coupon, PromotionError> {
        if code.trim().is_empty() {
            return Err(PromotionError::CouponCodeEmpty);
        }
        let coupon = self
            .repository
            .coupon_by_code(code)
            .await?
            .ok_or(PromotionError::CouponInvalid)?;
        let user_use_count = if coupon.per_user_limit.is_some() {
            self.repository.coupon_use_count(coupon.id, user_id).await?
        } else {
            0
        };
        validate_coupon(
            &coupon,
            CouponUseContext {
                plan_id,
                period: None,
                user_use_count,
                now,
            },
        )
        .map_err(PromotionError::CouponRule)?;
        Ok(coupon)
    }
}

fn resolve_sort(
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
) -> Result<CodeSort, PromotionError> {
    let field = sort_by.unwrap_or("created_at");
    if field != "created_at" {
        return Err(PromotionError::InvalidSortBy(field.to_string()));
    }
    match sort_direction {
        None | Some("desc") => Ok(CodeSort::CreatedAtDescending),
        Some("asc") => Ok(CodeSort::CreatedAtAscending),
        Some(direction) => Err(PromotionError::InvalidSortDirection(direction.to_string())),
    }
}

fn positive_generation_count(count: Option<i64>) -> Option<usize> {
    count
        .filter(|count| *count > 0)
        .and_then(|count| usize::try_from(count).ok())
}

fn requested_code(code: Option<&str>) -> Option<String> {
    code.map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
}

fn validate_coupon_create(input: &CouponCreateInput) -> Result<(), PromotionInputViolation> {
    if input.generate_count.is_some_and(|count| count > 500) {
        return Err(PromotionInputViolation::CouponGenerateCountTooLarge);
    }
    if !matches!(input.kind_code, 1 | 2) {
        return Err(PromotionInputViolation::CouponTypeInvalid);
    }
    if !(0..=i64::from(i32::MAX)).contains(&input.value)
        || (input.kind_code == 2 && input.value > 100)
    {
        return Err(PromotionInputViolation::CouponValueInvalid);
    }
    Ok(())
}

fn validate_coupon_patch(input: &CouponPatchInput) -> Result<(), PromotionInputViolation> {
    if input
        .kind_code
        .is_some_and(|kind_code| !matches!(kind_code, 1 | 2))
    {
        return Err(PromotionInputViolation::CouponTypeInvalid);
    }
    if input.value.is_some_and(|value| {
        !(0..=i64::from(i32::MAX)).contains(&value) || (input.kind_code == Some(2) && value > 100)
    }) {
        return Err(PromotionInputViolation::CouponValueInvalid);
    }
    Ok(())
}

fn validate_gift_card_create(input: &GiftCardCreateInput) -> Result<(), PromotionInputViolation> {
    if input.generate_count.is_some_and(|count| count > 500) {
        return Err(PromotionInputViolation::GiftCardGenerateCountTooLarge);
    }
    if !matches!(input.kind_code, 1..=5) {
        return Err(PromotionInputViolation::GiftCardTypeInvalid);
    }
    match input.value {
        None if matches!(input.kind_code, 1 | 2 | 3 | 5) => {
            return Err(PromotionInputViolation::GiftCardValueRequired);
        }
        Some(value) if !(0..=i64::from(i32::MAX)).contains(&value) => {
            return Err(PromotionInputViolation::GiftCardValueInvalid);
        }
        None | Some(_) => {}
    }
    if input.kind_code == 5 && input.plan_id.is_none() {
        return Err(PromotionInputViolation::GiftCardPlanRequired);
    }
    Ok(())
}

fn validate_gift_card_patch(input: &GiftCardPatchInput) -> Result<(), PromotionInputViolation> {
    if input
        .kind_code
        .is_some_and(|kind_code| !matches!(kind_code, 1..=5))
    {
        return Err(PromotionInputViolation::GiftCardTypeInvalid);
    }
    if input
        .value
        .is_some_and(|value| value.is_some_and(|value| !(0..=i64::from(i32::MAX)).contains(&value)))
    {
        return Err(PromotionInputViolation::GiftCardValueInvalid);
    }
    Ok(())
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
        calls: Vec<&'static str>,
        coupon: Option<Coupon>,
        use_count: i64,
        coupon_create: Option<NewCoupon>,
        coupon_batch_count: Option<usize>,
        gift_create: Option<NewGiftCard>,
        coupon_create_outcome: Option<CreateCodeOutcome>,
        coupon_patch_outcome: Option<PatchCodeOutcome>,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    impl PromotionRepository for FakeRepository {
        async fn coupons(&self, _: PageRequest) -> RepositoryResult<CouponPage> {
            self.0.lock().unwrap().calls.push("coupons");
            Ok(CouponPage {
                items: Vec::new(),
                total: 0,
            })
        }

        async fn gift_cards(&self, _: PageRequest) -> RepositoryResult<GiftCardPage> {
            self.0.lock().unwrap().calls.push("gift_cards");
            Ok(GiftCardPage {
                items: Vec::new(),
                total: 0,
            })
        }

        async fn create_coupon(&self, coupon: NewCoupon) -> RepositoryResult<CreateCodeOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("create_coupon");
            state.coupon_create = Some(coupon);
            Ok(state
                .coupon_create_outcome
                .unwrap_or(CreateCodeOutcome::Created(7)))
        }

        async fn generate_coupons(
            &self,
            coupon: NewCoupon,
            count: usize,
        ) -> RepositoryResult<Vec<String>> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("generate_coupons");
            state.coupon_create = Some(coupon);
            state.coupon_batch_count = Some(count);
            Ok(vec!["GENERATED".to_string(); count])
        }

        async fn patch_coupon(
            &self,
            _: i64,
            _: CouponChanges,
        ) -> RepositoryResult<PatchCodeOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("patch_coupon");
            Ok(state
                .coupon_patch_outcome
                .unwrap_or(PatchCodeOutcome::Updated))
        }

        async fn delete_coupon(&self, _: i64) -> RepositoryResult<DeleteCodeOutcome> {
            self.0.lock().unwrap().calls.push("delete_coupon");
            Ok(DeleteCodeOutcome::Deleted)
        }

        async fn create_gift_card(&self, card: NewGiftCard) -> RepositoryResult<CreateCodeOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("create_gift_card");
            state.gift_create = Some(card);
            Ok(CreateCodeOutcome::Created(8))
        }

        async fn generate_gift_cards(
            &self,
            _: NewGiftCard,
            count: usize,
        ) -> RepositoryResult<Vec<String>> {
            self.0.lock().unwrap().calls.push("generate_gift_cards");
            Ok(vec!["GENERATED-GIFT".to_string(); count])
        }

        async fn patch_gift_card(
            &self,
            _: i64,
            _: GiftCardChanges,
        ) -> RepositoryResult<PatchCodeOutcome> {
            self.0.lock().unwrap().calls.push("patch_gift_card");
            Ok(PatchCodeOutcome::Updated)
        }

        async fn delete_gift_card(&self, _: i64) -> RepositoryResult<DeleteCodeOutcome> {
            self.0.lock().unwrap().calls.push("delete_gift_card");
            Ok(DeleteCodeOutcome::Deleted)
        }

        async fn coupon_by_code(&self, _: &str) -> RepositoryResult<Option<Coupon>> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("coupon_by_code");
            Ok(state.coupon.clone())
        }

        async fn coupon_use_count(&self, _: i32, _: i64) -> RepositoryResult<i64> {
            let mut state = self.0.lock().unwrap();
            state.calls.push("coupon_use_count");
            Ok(state.use_count)
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

    fn coupon_input() -> CouponCreateInput {
        CouponCreateInput {
            name: "Save".to_string(),
            kind_code: 1,
            value: 1_000,
            starts_at: 10,
            ends_at: 20,
            remaining_uses: None,
            per_user_limit: None,
            plan_ids: None,
            periods: None,
            code: Some("  SAVE  ".to_string()),
            generate_count: None,
        }
    }

    fn coupon() -> Coupon {
        Coupon {
            id: 7,
            code: "SAVE".to_string(),
            name: "Save".to_string(),
            kind_code: 1,
            value: 100,
            visible: true,
            remaining_uses: None,
            per_user_limit: Some(1),
            plan_ids: Some(vec![3]),
            periods: None,
            starts_at: 10,
            ends_at: 20,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn invalid_generation_fails_before_the_repository() {
        let repository = FakeRepository::default();
        let mut input = coupon_input();
        input.generate_count = Some(501);
        assert!(matches!(
            block_on(PromotionService::new(repository.clone()).generate_coupon(input, 15)),
            Err(PromotionError::InvalidInput(
                PromotionInputViolation::CouponGenerateCountTooLarge
            ))
        ));
        assert!(repository.0.lock().unwrap().calls.is_empty());
    }

    #[test]
    fn single_and_batch_generation_are_explicit_use_case_arms() {
        let repository = FakeRepository::default();
        assert_eq!(
            block_on(PromotionService::new(repository.clone()).generate_coupon(coupon_input(), 15))
                .unwrap(),
            GenerateCodeOutcome::Created(7)
        );
        let created = repository.0.lock().unwrap().coupon_create.clone().unwrap();
        assert_eq!(created.requested_code.as_deref(), Some("SAVE"));
        assert_eq!(created.created_at, 15);

        let mut batch = coupon_input();
        batch.generate_count = Some(3);
        assert!(matches!(
            block_on(PromotionService::new(repository.clone()).generate_coupon(batch, 16)),
            Ok(GenerateCodeOutcome::Batch(codes)) if codes.len() == 3
        ));
        assert_eq!(repository.0.lock().unwrap().coupon_batch_count, Some(3));
    }

    #[test]
    fn coupon_check_loads_usage_only_when_policy_requires_it() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().coupon = Some(coupon());
        let checked = block_on(PromotionService::new(repository.clone()).check_coupon(
            9,
            "SAVE",
            Some(3),
            15,
        ))
        .unwrap();
        assert_eq!(checked.id, 7);
        assert_eq!(
            repository.0.lock().unwrap().calls,
            ["coupon_by_code", "coupon_use_count"]
        );

        repository.0.lock().unwrap().use_count = 1;
        assert!(matches!(
            block_on(PromotionService::new(repository).check_coupon(9, "SAVE", Some(3), 15)),
            Err(PromotionError::CouponRule(
                CouponRuleViolation::UserLimitExceeded(1)
            ))
        ));
    }

    #[test]
    fn sort_and_duplicate_outcomes_remain_typed() {
        let repository = FakeRepository::default();
        assert!(matches!(
            block_on(PromotionService::new(repository.clone()).coupons(
                10,
                0,
                Some("name"),
                None
            )),
            Err(PromotionError::InvalidSortBy(field)) if field == "name"
        ));
        assert!(repository.0.lock().unwrap().calls.is_empty());

        repository.0.lock().unwrap().coupon_create_outcome = Some(CreateCodeOutcome::DuplicateCode);
        assert!(matches!(
            block_on(PromotionService::new(repository).generate_coupon(coupon_input(), 15)),
            Err(PromotionError::DuplicateCouponCode)
        ));
    }

    #[test]
    fn coupon_validation_rejects_every_invalid_discount_shape() {
        for (kind_code, value, expected) in [
            (9, 10, PromotionInputViolation::CouponTypeInvalid),
            (1, -1, PromotionInputViolation::CouponValueInvalid),
            (
                1,
                i64::from(i32::MAX) + 1,
                PromotionInputViolation::CouponValueInvalid,
            ),
            (2, -1, PromotionInputViolation::CouponValueInvalid),
            (2, 101, PromotionInputViolation::CouponValueInvalid),
        ] {
            let mut input = coupon_input();
            input.kind_code = kind_code;
            input.value = value;
            assert_eq!(validate_coupon_create(&input), Err(expected));
        }
        let mut maximum_batch = coupon_input();
        maximum_batch.generate_count = Some(500);
        assert_eq!(validate_coupon_create(&maximum_batch), Ok(()));
    }

    #[test]
    fn gift_card_validation_keeps_required_if_rules_explicit() {
        let mut input = GiftCardCreateInput {
            name: "Gift".to_string(),
            kind_code: 4,
            value: None,
            plan_id: None,
            starts_at: 10,
            ends_at: 20,
            remaining_uses: None,
            code: None,
            generate_count: None,
        };
        assert_eq!(validate_gift_card_create(&input), Ok(()));

        input.kind_code = 5;
        assert_eq!(
            validate_gift_card_create(&input),
            Err(PromotionInputViolation::GiftCardValueRequired)
        );
        input.value = Some(30);
        assert_eq!(
            validate_gift_card_create(&input),
            Err(PromotionInputViolation::GiftCardPlanRequired)
        );
        input.plan_id = Some(7);
        assert_eq!(validate_gift_card_create(&input), Ok(()));

        input.kind_code = 6;
        assert_eq!(
            validate_gift_card_create(&input),
            Err(PromotionInputViolation::GiftCardTypeInvalid)
        );
        input.kind_code = 3;
        input.value = Some(-1);
        assert_eq!(
            validate_gift_card_create(&input),
            Err(PromotionInputViolation::GiftCardValueInvalid)
        );
    }

    #[test]
    fn blank_codes_mean_generate_or_retain_without_a_compatibility_branch() {
        assert_eq!(requested_code(None), None);
        assert_eq!(requested_code(Some("   ")), None);
        assert_eq!(requested_code(Some("  SAVE  ")), Some("SAVE".to_string()));
        assert_eq!(positive_generation_count(Some(-1)), None);
        assert_eq!(positive_generation_count(Some(0)), None);
        assert_eq!(positive_generation_count(Some(3)), Some(3));
    }
}
