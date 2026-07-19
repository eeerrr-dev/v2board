// Browser-injected oracle seeding scripts (split out of api-fixtures.mjs):
// the Stripe SDK stub served for js.stripe.com plus the legacy dva-store
// seeds the frozen oracle world needs before admin scenarios run.
import {
  adminConfigFixture,
  adminCouponStoreFixtures,
  adminEmailTemplateFixtures,
  adminGiftcardStoreFixtures,
  adminKnowledgeFixtures,
  adminNoticeFixtures,
  adminPaymentFixtures,
  adminPlanStoreFixtures,
  adminQueueStatsFixture,
  adminQueueWorkloadFixtures,
  adminServerGroupFixtures,
  adminServerRouteFixtures,
  adminStatFixture,
  adminTicketDetailFixture,
  adminTicketFixtures,
  adminUserStoreFixtures,
  toAdminPlanStoreFixtures,
  toAdminUserStoreFixtures,
  userInfoFixture,
} from './fixture-data.mjs';
import {
  adminOrderFixturesFor,
  adminPlanFixturesFor,
  adminServerNodeFixturesFor,
  adminUserFixturesFor,
} from './api-fixture-response.mjs';

export function stripeFixtureScript({
  confirmError = false,
  legacyToken = null,
  paymentElementComplete = false,
} = {}) {
  const tokenPayload = legacyToken ? { id: legacyToken, object: 'token' } : null;
  return `
(() => {
  const legacyOracleToken = ${JSON.stringify(tokenPayload)};
  const visualPaymentElementComplete = ${JSON.stringify(Boolean(paymentElementComplete))};
  window.Stripe = function Stripe() {
    let lastElement = null;
    const createElement = () => {
      const handlers = new Map();
      const fire = (event, payload) => {
        const eventHandlers = handlers.get(event) || [];
        eventHandlers.forEach((handler) => handler(payload));
      };
      const fireElementComplete = () => {
        if (!visualPaymentElementComplete) return;
        [0, 50, 150, 300, 750].forEach((delay) => {
          setTimeout(() => fire('change', { complete: true, empty: false }), delay);
        });
      };
      return {
        blur() {},
        clear() {},
        destroy() {},
        focus() {},
        mount(target) {
          const element = document.createElement('div');
          element.className = 'StripeElement';
          target.appendChild(element);
          fire('ready', {});
          fireElementComplete();
        },
        off(event, handler) {
          const eventHandlers = handlers.get(event) || [];
          handlers.set(event, eventHandlers.filter((item) => item !== handler));
        },
        on(event, handler) {
          handlers.set(event, [...(handlers.get(event) || []), handler]);
          if (event === 'change') fireElementComplete();
        },
        unmount() {},
        update() {},
      };
    };
    return {
      _registerWrapper() {},
      registerAppInfo() {},
      elements() {
        return {
          getElement() {
            return lastElement;
          },
          create() {
            lastElement = createElement();
            return lastElement;
          },
        };
      },
      // React Stripe.js validates the complete Stripe instance shape, which
      // includes createToken even when the application only uses Payment
      // Element. Keep the SDK-shaped method in both worlds, but make any source
      // invocation fail loudly: only the frozen oracle may receive a token.
      createToken() {
        if (!legacyOracleToken) {
          window.__visualParityUnexpectedStripeCreateTokenCount =
            (window.__visualParityUnexpectedStripeCreateTokenCount ?? 0) + 1;
          return Promise.reject(new Error('Modern source must not call Stripe createToken'));
        }
        return Promise.resolve({ token: legacyOracleToken });
      },
      createPaymentMethod() {
        return Promise.resolve({});
      },
      confirmCardPayment() {
        return Promise.resolve({});
      },
      confirmPayment() {
        window.__visualParityUserStripeConfirmCount =
          (window.__visualParityUserStripeConfirmCount ?? 0) + 1;
        return Promise.resolve(
          ${JSON.stringify(Boolean(confirmError))}
            ? { error: { message: 'Stripe confirmation failed' } }
            : { paymentIntent: { status: 'succeeded' } },
        );
      },
    };
  };
  window.Stripe.version = 'dahlia';
})();
`;
}

export async function seedLegacyAdminTicketDetailStore(page) {
  await page.waitForFunction(() => window.g_app?._store, null, { timeout: 5_000 });
  await page.evaluate(
    ({ plans, ticket, userInfo, users }) => {
      const store = window.g_app?._store;
      if (!store) return;
      store.dispatch({ type: 'plan/setState', payload: { plans } });
      store.dispatch({
        type: 'user/setState',
        payload: {
          pagination: { current: 1, pageSize: 10, total: users.length },
          userInfo,
          users,
        },
      });
      store.dispatch({
        type: 'ticket/setState',
        payload: {
          filter: { status: 0 },
          pagination: { current: 1, pageSize: 10, total: 1 },
          ticket,
          tickets: [ticket],
        },
      });
    },
    {
      plans: adminPlanStoreFixtures,
      ticket: adminTicketDetailFixture,
      userInfo: userInfoFixture,
      users: adminUserStoreFixtures,
    },
  );
}

export async function seedLegacyAdminStore(page, scenario = {}) {
  await page
    .waitForFunction(() => window.g_app?._store, null, { timeout: 5_000 })
    .catch(() => undefined);
  for (let attempt = 0; attempt < 6; attempt += 1) {
    await page.evaluate(
      ({
        config,
        coupons,
        emailTemplates,
        giftcards,
        knowledgeCategories,
        knowledges,
        notices,
        orders,
        payments,
        plans,
        queueStats,
        queueWorkload,
        serverGroups,
        serverNodes,
        serverRoutes,
        skipCoupons,
        skipGiftcards,
        skipKnowledge,
        skipNotices,
        skipOrders,
        skipPayments,
        skipPlans,
        skipServerManage,
        skipTickets,
        skipUsers,
        stat,
        ticketDetail,
        tickets,
        userInfo,
        users,
      }) => {
        const store = window.g_app?._store;
        if (!store) return;
        if (!store.__visualParityStateGuard) {
          const originalGetState = store.getState.bind(store);
          store.getState = () => {
            const state = originalGetState();
            const ensureArrayState = (namespace, key, value) => {
              if (!state[namespace] || typeof state[namespace] !== 'object') {
                state[namespace] = {};
              }
              if (!Array.isArray(state[namespace][key])) {
                state[namespace][key] = value;
              }
            };

            ensureArrayState('serverGroup', 'groups', serverGroups);
            ensureArrayState('serverManage', 'servers', serverNodes);
            ensureArrayState('serverRoute', 'routes', serverRoutes);
            return state;
          };
          store.__visualParityStateGuard = true;
        }
        if (!skipUsers) {
          store.dispatch({
            type: 'user/setState',
            payload: {
              pagination: { current: 1, pageSize: 10, total: users.length },
              userInfo,
              users,
            },
          });
        }
        store.dispatch({ type: 'stat/save', payload: stat });
        store.dispatch({
          type: 'config/setState',
          payload: {
            ...config,
            emailTemplate: emailTemplates,
          },
        });
        if (!skipCoupons) {
          store.dispatch({
            type: 'coupon/setState',
            payload: {
              coupons,
              pagination: { current: 1, pageSize: 10, total: coupons.length },
            },
          });
        }
        if (!skipGiftcards) {
          store.dispatch({
            type: 'giftcard/setState',
            payload: {
              giftcards,
              pagination: { current: 1, pageSize: 10, total: giftcards.length },
            },
          });
        }
        if (!skipOrders) {
          store.dispatch({
            type: 'order/setState',
            payload: {
              orders,
              pagination: { current: 0, pageSize: 10, total: orders.length },
            },
          });
        }
        if (!skipPayments) {
          store.dispatch({ type: 'payment/setState', payload: { payments } });
        }
        if (!skipPlans) {
          store.dispatch({ type: 'plan/setState', payload: { plans } });
        }
        store.dispatch({
          type: 'system/save',
          payload: { queueStats, queueWorkload },
        });
        if (!skipKnowledge) {
          store.dispatch({
            type: 'knowledge/setState',
            payload: { categorys: knowledgeCategories, knowledges },
          });
        }
        if (!skipNotices) {
          store.dispatch({ type: 'notice/setState', payload: { notices } });
        }
        store.dispatch({ type: 'serverGroup/setState', payload: { groups: serverGroups } });
        if (!skipServerManage) {
          store.dispatch({
            type: 'serverManage/setState',
            payload: { fetchLoading: false, servers: serverNodes, sortMode: false },
          });
        }
        store.dispatch({
          type: 'serverRoute/setState',
          payload: { fetchLoading: false, routes: serverRoutes },
        });
        if (!skipTickets) {
          store.dispatch({
            type: 'ticket/setState',
            payload: {
              filter: { status: 0 },
              pagination: { current: 1, pageSize: 10, total: tickets.length },
              ticket: ticketDetail,
              tickets,
            },
          });
        }
      },
      {
        config: adminConfigFixture,
        coupons: adminCouponStoreFixtures,
        emailTemplates: adminEmailTemplateFixtures,
        giftcards: adminGiftcardStoreFixtures,
        knowledgeCategories: Array.from(
          new Set(adminKnowledgeFixtures.map((knowledge) => knowledge.category)),
        ),
        knowledges: adminKnowledgeFixtures,
        notices: adminNoticeFixtures,
        orders: adminOrderFixturesFor(scenario),
        payments: adminPaymentFixtures,
        plans: toAdminPlanStoreFixtures(adminPlanFixturesFor(scenario)),
        queueStats: adminQueueStatsFixture,
        queueWorkload: adminQueueWorkloadFixtures,
        serverGroups: adminServerGroupFixtures,
        serverNodes: scenario.adminServerManageTimeout ? [] : adminServerNodeFixturesFor(scenario),
        serverRoutes: adminServerRouteFixtures,
        skipCoupons: Boolean(scenario.adminCouponsTimeout),
        skipGiftcards: Boolean(scenario.adminGiftcardsTimeout),
        skipKnowledge: Boolean(scenario.adminKnowledgeTimeout),
        skipNotices: Boolean(scenario.adminNoticesTimeout),
        skipOrders: Boolean(scenario.adminOrdersHttpError || scenario.adminOrdersTimeout),
        skipPayments: Boolean(scenario.adminPaymentsTimeout),
        skipPlans: Boolean(scenario.adminPlansTimeout),
        skipServerManage: Boolean(scenario.adminServerManageTimeout),
        skipTickets: Boolean(scenario.adminTicketsTimeout),
        skipUsers: Boolean(scenario.adminUsersHttpError || scenario.adminUsersTimeout),
        stat: adminStatFixture,
        ticketDetail: adminTicketDetailFixture,
        tickets: adminTicketFixtures,
        userInfo: userInfoFixture,
        users: toAdminUserStoreFixtures(adminUserFixturesFor(scenario)),
      },
    );
    await page.waitForTimeout(50);
  }
  await page.waitForFunction(
    () => {
      const state = window.g_app?._store?.getState?.();
      return (
        Array.isArray(state?.serverGroup?.groups) &&
        Array.isArray(state?.serverManage?.servers) &&
        Array.isArray(state?.serverRoute?.routes)
      );
    },
    null,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(100);
}
