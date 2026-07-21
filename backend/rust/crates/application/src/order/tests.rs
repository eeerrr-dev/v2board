use std::{
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use super::*;

#[derive(Clone)]
struct FixedClock(i64);

impl OrderClock for FixedClock {
    fn now(&self) -> i64 {
        self.0
    }
}

#[derive(Clone)]
struct FixedNumbers;

impl OrderNumberGenerator for FixedNumbers {
    fn generate(&self) -> String {
        "fixed-trade".to_string()
    }
}

#[derive(Clone)]
struct FixedPolicy;

impl OrderPolicy for FixedPolicy {
    fn create_policy(&self) -> CreateOrderPolicy {
        CreateOrderPolicy {
            plan_change_enabled: true,
            surplus_enabled: true,
            commission_first_time_enabled: false,
            default_commission_rate: 10,
        }
    }

    fn fulfillment_policy(&self, total_amount: i32) -> FulfillmentPolicy {
        FulfillmentPolicy {
            deposit_bonus: if total_amount >= 1_000 { 100 } else { 0 },
            new_order_event_id: 1,
            renewal_order_event_id: 2,
            change_order_event_id: 3,
        }
    }

    fn try_out_plan_id(&self) -> i32 {
        7
    }
}

#[derive(Clone, Default)]
struct FakeRepository(Arc<Mutex<FakeState>>);

#[derive(Default)]
struct FakeState {
    plans: Vec<Plan>,
    plan_lookup: Option<UserPlanLookup>,
    payment_methods: Vec<AvailablePaymentMethod>,
    pending_candidates: Vec<PendingOrderCandidate>,
    create: Option<CreateOrderCommand>,
    checkout: Option<CheckoutPreparation>,
    bound_checkout: Option<BindCheckoutCommand>,
    stripe: Option<StripePreparation>,
    bound_stripe: Option<BindStripeCommand>,
    cancel: Option<CancelCandidate>,
    cancelled: bool,
    payment: Option<PaymentMethod>,
    verification_payment: Option<PaymentMethod>,
    settlement: Option<SettlePaymentCommand>,
    pending: Option<PendingOrderSnapshot>,
    processed: usize,
    order: Option<UserOrder>,
}

impl OrderRepository for FakeRepository {
    async fn visible_plans(&self) -> PortResult<Vec<Plan>> {
        Ok(self.0.lock().unwrap().plans.clone())
    }

    async fn user_plan(&self, _: i64, _: i32) -> PortResult<UserPlanLookup> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .plan_lookup
            .clone()
            .unwrap_or(UserPlanLookup::PlanNotFound))
    }

    async fn available_payment_methods(&self) -> PortResult<Vec<AvailablePaymentMethod>> {
        Ok(self.0.lock().unwrap().payment_methods.clone())
    }

    async fn pending_order_candidates(
        &self,
        after_id: i64,
        limit: i64,
    ) -> PortResult<Vec<PendingOrderCandidate>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .pending_candidates
            .iter()
            .filter(|candidate| candidate.id > after_id)
            .take(usize::try_from(limit).unwrap_or(usize::MAX))
            .cloned()
            .collect())
    }

    async fn create_order(&self, command: CreateOrderCommand) -> PortResult<()> {
        self.0.lock().unwrap().create = Some(command);
        Ok(())
    }

    async fn prepare_checkout(
        &self,
        _: i64,
        _: &str,
        _: Option<i32>,
        _: i64,
        _: FulfillmentPolicy,
    ) -> PortResult<CheckoutPreparation> {
        Ok(self.0.lock().unwrap().checkout.clone().unwrap())
    }

    async fn bind_checkout(
        &self,
        command: BindCheckoutCommand,
    ) -> PortResult<CheckoutBindingOutcome> {
        self.0.lock().unwrap().bound_checkout = Some(command);
        Ok(CheckoutBindingOutcome::Bound)
    }

    async fn prepare_stripe(&self, _: i64, _: &str, _: i32) -> PortResult<StripePreparation> {
        Ok(self.0.lock().unwrap().stripe.clone().unwrap())
    }

    async fn bind_stripe(&self, command: BindStripeCommand) -> PortResult<CheckoutBindingOutcome> {
        self.0.lock().unwrap().bound_stripe = Some(command);
        Ok(CheckoutBindingOutcome::Bound)
    }

    async fn payment_binding_material(&self, _: i32) -> PortResult<Option<PaymentMethod>> {
        Ok(self.0.lock().unwrap().payment.clone())
    }

    async fn cancel_candidate(&self, _: i64, _: &str) -> PortResult<Option<CancelCandidate>> {
        Ok(self.0.lock().unwrap().cancel.clone())
    }

    async fn cancel_order(&self, _: CancelOrderCommand) -> PortResult<CancelOrderOutcome> {
        self.0.lock().unwrap().cancelled = true;
        Ok(CancelOrderOutcome::Cancelled)
    }

    async fn payment_for_notification(
        &self,
        _: &str,
        _: &str,
    ) -> PortResult<Option<PaymentMethod>> {
        Ok(self.0.lock().unwrap().verification_payment.clone())
    }

    async fn settle_payment(
        &self,
        command: SettlePaymentCommand,
    ) -> PortResult<PaymentSettlementNotices> {
        self.0.lock().unwrap().settlement = Some(command.clone());
        Ok(PaymentSettlementNotices {
            paid: Some(PaidOrderNotice {
                trade_no: command.notification.trade_no,
                total_amount: 1_000,
            }),
            late: None,
        })
    }

    async fn manual_payment_candidate(
        &self,
        _: &str,
    ) -> PortResult<Option<ManualPaymentCandidate>> {
        Ok(None)
    }

    async fn settle_manually(
        &self,
        _: &str,
        _: ManualPaymentCandidate,
        _: i64,
        _: FulfillmentPolicy,
    ) -> PortResult<ManualPaymentOutcome> {
        Ok(ManualPaymentOutcome::Settled)
    }

    async fn pending_order(&self, _: &str) -> PortResult<Option<PendingOrderSnapshot>> {
        Ok(self.0.lock().unwrap().pending.clone())
    }

    async fn process_pending_order(
        &self,
        _: &str,
        _: PendingOrderSnapshot,
        _: bool,
        _: i64,
        _: FulfillmentPolicy,
    ) -> PortResult<()> {
        self.0.lock().unwrap().processed += 1;
        Ok(())
    }

    async fn user_orders(&self, _: i64, _: Option<i16>) -> PortResult<Vec<UserOrder>> {
        Ok(Vec::new())
    }

    async fn user_order(&self, _: i64, _: &str, _: i32) -> PortResult<Option<UserOrder>> {
        Ok(self.0.lock().unwrap().order.clone())
    }

    async fn user_order_status(&self, _: i64, _: &str) -> PortResult<Option<i16>> {
        Ok(Some(3))
    }
}

#[derive(Clone, Default)]
struct FakeGateway(Arc<Mutex<Vec<&'static str>>>);

impl PaymentSnapshotVerifier for FakeGateway {
    fn equivalent(&self, _: &PaymentMethod, _: &PaymentMethod) -> PortResult<bool> {
        Ok(true)
    }
}

impl PaymentGateway for FakeGateway {
    async fn checkout(&self, _: &PaymentMethod, _: &GatewayOrder) -> PortResult<CheckoutOutcome> {
        self.0.lock().unwrap().push("checkout");
        Ok(CheckoutOutcome::Redirect("https://pay.test".to_string()))
    }

    async fn prepare_stripe_intent(
        &self,
        _: &PaymentMethod,
        _: &GatewayOrder,
        _: Option<&str>,
    ) -> PortResult<PreparedStripeIntent> {
        self.0.lock().unwrap().push("prepare");
        Ok(PreparedStripeIntent {
            intent_id: "pi_new".to_string(),
            response: StripePaymentIntent {
                public_key: "pk".to_string(),
                client_secret: "secret".to_string(),
                amount: 1_000,
                currency: "usd".to_string(),
            },
        })
    }

    async fn cancel_stripe_intent(
        &self,
        _: Option<&PaymentMethod>,
        _: Option<&str>,
    ) -> PortResult<bool> {
        self.0.lock().unwrap().push("cancel");
        Ok(true)
    }

    async fn verify_notification(
        &self,
        _: &PaymentMethod,
        _: &PaymentNotifyInput,
    ) -> PortResult<PaymentVerification> {
        self.0.lock().unwrap().push("verify");
        Ok(PaymentVerification::Verified(VerifiedPaymentNotification {
            trade_no: "fixed-trade".to_string(),
            callback_no: "callback".to_string(),
            custom_response: None,
            authenticated_user_id: None,
            settled_amount_cents: Some(1_000),
        }))
    }
}

fn payment() -> PaymentMethod {
    PaymentMethod {
        id: 3,
        provider: "EPay".to_string(),
        enabled: true,
        uuid: "uuid".to_string(),
        sealed_config: "sealed".to_string(),
        notify_domain: None,
        handling_fee_fixed: None,
        handling_fee_percent: None,
    }
}

fn service(
    repository: FakeRepository,
    gateway: FakeGateway,
) -> OrderService<FakeRepository, FakeGateway, FixedClock, FixedNumbers, FixedPolicy> {
    OrderService::new(
        repository,
        gateway,
        FixedClock(10_000),
        FixedNumbers,
        FixedPolicy,
    )
}

fn plan(id: i32, capacity_limit: Option<i32>, count: i64) -> Plan {
    Plan {
        id,
        group_id: 1,
        transfer_enable: 100,
        device_limit: None,
        name: format!("plan-{id}"),
        speed_limit: None,
        show: true,
        sort: Some(id),
        renew: true,
        content: None,
        prices: Default::default(),
        reset_traffic_method: None,
        capacity_limit,
        count,
        created_at: 1,
        updated_at: 1,
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
fn save_mints_identity_and_passes_only_policy_and_time_to_persistence() {
    let repository = FakeRepository::default();
    let service = service(repository.clone(), FakeGateway::default());
    let trade_no = run(service.save(
        42,
        SaveOrderInput::Plan {
            plan_id: 7,
            period: PlanPricePeriod::Month,
            coupon_code: None,
        },
    ))
    .unwrap();
    assert_eq!(trade_no, "fixed-trade");
    let command = repository.0.lock().unwrap().create.clone().unwrap();
    assert_eq!(command.user_id, 42);
    assert_eq!(command.now, 10_000);
    assert!(command.policy.plan_change_enabled);
}

#[test]
fn catalog_use_cases_hide_persistence_and_apply_availability_policy() {
    let repository = FakeRepository::default();
    {
        let mut state = repository.0.lock().unwrap();
        state.plans = vec![plan(1, Some(10), 4)];
        let mut hidden = plan(2, None, 0);
        hidden.show = false;
        state.plan_lookup = Some(UserPlanLookup::Found {
            plan: hidden,
            current_plan_id: Some(1),
        });
        state.pending_candidates = vec![
            PendingOrderCandidate {
                id: 1,
                trade_no: "one".into(),
            },
            PendingOrderCandidate {
                id: 2,
                trade_no: "two".into(),
            },
        ];
    }
    let service = service(repository, FakeGateway::default());
    let plans = run(service.catalog_plans()).unwrap();
    assert_eq!(plans[0].capacity_limit, Some(6));
    assert!(matches!(
        run(service.catalog_plan(7, 2)),
        Err(OrderError::Business(OrderFailure::PlanNotFound))
    ));
    assert_eq!(run(service.pending_candidates(1, 10)).unwrap().len(), 1);
}

#[test]
fn checkout_cancels_old_binding_then_cas_binds_before_gateway_call() {
    let repository = FakeRepository::default();
    {
        let mut state = repository.0.lock().unwrap();
        state.payment = Some(payment());
        state.checkout = Some(CheckoutPreparation::Gateway {
            order_id: 9,
            payment: Box::new(payment()),
            order: GatewayOrder {
                trade_no: "fixed-trade".to_string(),
                total_amount: 1_000,
                user_id: 42,
                user_email: None,
            },
            previous_binding: PaymentBinding {
                payment_id: Some(3),
                callback_no: Some("pi_old".to_string()),
            },
            handling_amount: None,
        });
    }
    let gateway = FakeGateway::default();
    let outcome = run(service(repository.clone(), gateway.clone()).checkout(
        42,
        "fixed-trade".to_string(),
        Some(3),
    ))
    .unwrap();
    assert_eq!(
        outcome,
        CheckoutOutcome::Redirect("https://pay.test".into())
    );
    assert!(repository.0.lock().unwrap().bound_checkout.is_some());
    assert_eq!(*gateway.0.lock().unwrap(), ["cancel", "checkout"]);
}

#[test]
fn authenticated_notification_settles_then_attempts_durable_fulfillment() {
    let repository = FakeRepository::default();
    {
        let mut state = repository.0.lock().unwrap();
        state.verification_payment = Some(payment());
        state.pending = Some(PendingOrderSnapshot {
            status: 1,
            created_at: 9_000,
            total_amount: 1_000,
            binding: PaymentBinding {
                payment_id: Some(3),
                callback_no: Some("callback".into()),
            },
        });
    }
    let gateway = FakeGateway::default();
    let response = run(
        service(repository.clone(), gateway.clone()).handle_payment_notify(
            "EPay",
            "uuid",
            PaymentNotifyInput::default(),
        ),
    )
    .unwrap();
    assert_eq!(response.body, "success");
    assert!(response.paid_notice.is_some());
    let state = repository.0.lock().unwrap();
    assert!(state.settlement.is_some());
    assert_eq!(state.processed, 1);
    assert_eq!(*gateway.0.lock().unwrap(), ["verify"]);
}
