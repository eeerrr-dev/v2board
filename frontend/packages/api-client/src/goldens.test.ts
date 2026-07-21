import { readFileSync, readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { z } from 'zod';
import type { InternalOperationName } from './internal-operation';
import { problemDetailsSchema } from './dialect';
import { internalApiOperations } from './generated/internal-api';

/**
 * Golden-response contract lane: every fixture in `../goldens` is a full
 * response body serialized by the Rust backend (`make contract-goldens`) and
 * pinned byte-for-byte by Rust tests. Parsing each one with the exact zod
 * contract the client uses closes the Rust→zod edge — a backend shape change
 * that would break the TypeScript contract fails here instead of drifting
 * silently. The fixture set and this schema map must stay bijective.
 */
const goldensUrl = new URL('../goldens/', import.meta.url);

const responseSchema = (name: InternalOperationName): z.ZodType =>
  internalApiOperations[name].responseSchema;

// Dialect-v2 fixtures (docs/api-dialect.md §3, §5.2): bare success bodies and
// problem+json error bodies — parsed exactly as the wire delivers them, with
// no envelope emulation. Since W14 closed the wave series, this is the only
// fixture dialect.
const dialectGoldenSchemas: Record<string, z.ZodType> = {
  'auth.login.json': responseSchema('authLogin'),
  'auth.mfa-status.enabled.json': responseSchema('adminAccountMfaGet'),
  'auth.mfa-status.json': responseSchema('adminAccountMfaGet'),
  'auth.mfa-totp.json': responseSchema('adminAccountMfaTotpSetup'),
  'auth.quick-login-url.json': responseSchema('authQuickLoginUrl'),
  'auth.session.admin.json': responseSchema('authSessionGet'),
  'auth.session.json': responseSchema('authSessionGet'),
  'auth.session.logged-out.json': responseSchema('authSessionGet'),
  'auth.session.staff.json': responseSchema('authSessionGet'),
  'auth.step-up.json': responseSchema('authStepUp'),
  // §6.2/§6.4 (W11): the admin commerce family — bare arrays for plans,
  // payments, and payment-providers; `{items, total}` pages for orders and
  // payment-reconciliations; the bare order detail.
  'admin.plans.json': responseSchema('adminPlansList'),
  'admin.payments.json': responseSchema('adminPaymentsList'),
  'admin.payment-providers.json': responseSchema('adminPaymentProvidersList'),
  'admin.orders.json': responseSchema('adminOrdersList'),
  'admin.order.detail.json': responseSchema('adminOrdersGet'),
  'admin.payment-reconciliations.json': responseSchema('adminPaymentReconciliationsList'),
  // §6.6 (W12): the admin users family — the `{items, total}` list page and
  // the bare user detail (with the conditional `invite_user` object).
  'admin.users.json': responseSchema('adminUsersList'),
  'admin.user.detail.json': responseSchema('adminUsersGet'),
  'admin.user.detail.no-inviter.json': responseSchema('adminUsersGet'),
  // §6.3 (W10): the admin content family — bare arrays for the unpaginated
  // notice/knowledge lists, `{items, total}` pages for coupons/gift-cards.
  'admin.coupons.json': responseSchema('adminCouponsList'),
  'admin.gift-cards.json': responseSchema('adminGiftCardsList'),
  'admin.knowledge-categories.json': responseSchema('adminKnowledgeCategoriesList'),
  'admin.knowledge.detail.json': responseSchema('adminKnowledgeGet'),
  'admin.knowledge.json': responseSchema('adminKnowledgeList'),
  'admin.notices.json': responseSchema('adminNoticesList'),
  // §6.7 (W13): the admin servers family — bare arrays for the nodes list
  // (typed booleans/numbers, RFC 3339 timestamps) and the group/route lists.
  'admin.nodes.json': responseSchema('adminNodesList'),
  'admin.server-groups.json': responseSchema('adminServerGroupsList'),
  'admin.server-routes.json': responseSchema('adminServerRoutesList'),
  // §6.5/§6.8 (W14): the admin ticket family (`{items, total}` page + bare
  // detail with the `message[]` thread) and the stats family (§8 user-traffic
  // page; `{series, date, value}` rows with snake_case slugs, integer cents).
  'admin.tickets.json': responseSchema('adminTicketsList'),
  'admin.ticket.detail.json': responseSchema('adminTicketsGet'),
  'admin.stats.user-traffic.json': responseSchema('adminStatsUserTraffic'),
  'admin.stats.orders.json': responseSchema('adminStatsOrders'),
  'admin.stats.records.json': responseSchema('adminStatsRecords'),
  // §6.1 (W9): the PATCH config stale-revision conflict problem.
  'problem.config-revision-conflict.json': problemDetailsSchema,
  'problem.session-expired.json': problemDetailsSchema,
  'problem.validation.json': problemDetailsSchema,
  'public.config.json': responseSchema('publicConfig'),
  'public.config.whitelist-disabled.json': responseSchema('publicConfig'),
  'user.commissions.json': responseSchema('userCommissionsList'),
  'user.config.json': responseSchema('userConfigGet'),
  'user.coupons.check.json': responseSchema('userCouponsCheck'),
  'user.gift-card-redemptions.create.json': responseSchema('userGiftCardRedemptionsCreate'),
  'user.invite.json': responseSchema('userInviteGet'),
  'user.knowledge-categories.json': responseSchema('userKnowledgeCategoriesList'),
  'user.knowledge.detail.json': responseSchema('userKnowledgeGet'),
  'user.knowledge.json': responseSchema('userKnowledgeList'),
  'user.notices.json': responseSchema('userNoticesList'),
  'user.orders.checkout.qr.json': responseSchema('userOrdersCheckout'),
  'user.orders.checkout.redirect.json': responseSchema('userOrdersCheckout'),
  'user.orders.checkout.settled.json': responseSchema('userOrdersCheckout'),
  'user.orders.create.json': responseSchema('userOrdersCreate'),
  'user.orders.detail.deposit.json': responseSchema('userOrdersGet'),
  'user.orders.detail.json': responseSchema('userOrdersGet'),
  'user.orders.json': responseSchema('userOrdersList'),
  'user.orders.status.json': responseSchema('userOrdersStatus'),
  'user.orders.stripe-intent.json': responseSchema('userOrdersStripeIntent'),
  'user.payment-methods.json': responseSchema('userPaymentMethodsList'),
  'user.plans.detail.json': responseSchema('userPlansGet'),
  'user.plans.json': responseSchema('userPlansList'),
  'user.profile.json': responseSchema('userProfileGet'),
  'user.servers.json': responseSchema('userServersList'),
  'user.sessions.json': responseSchema('userSessionsList'),
  'user.stats.json': responseSchema('userStatsGet'),
  'user.subscription.json': responseSchema('userSubscriptionGet'),
  'user.subscription.reset-token.json': responseSchema('userSubscriptionResetToken'),
  'user.telegram-bot.json': responseSchema('userTelegramBotGet'),
  'user.tickets.create.json': responseSchema('userTicketsCreate'),
  'user.tickets.detail.json': responseSchema('userTicketsGet'),
  'user.tickets.json': responseSchema('userTicketsList'),
  'user.traffic-logs.json': responseSchema('userTrafficLogsList'),
  'user.withdrawal-tickets.create.json': responseSchema('userWithdrawalTicketsCreate'),
};

describe('golden response fixtures', () => {
  it('keeps the fixture directory and the schema map bijective', () => {
    const files = readdirSync(goldensUrl)
      .filter((name) => name.endsWith('.json'))
      .sort();
    expect(files).toEqual(Object.keys(dialectGoldenSchemas).sort());
  });

  for (const [name, schema] of Object.entries(dialectGoldenSchemas)) {
    it(`parses ${name} with its zod contract`, () => {
      const body = JSON.parse(readFileSync(new URL(name, goldensUrl), 'utf8')) as unknown;
      const result = schema.safeParse(body);
      if (!result.success) {
        throw new Error(
          `${name} no longer satisfies its zod contract:\n${JSON.stringify(result.error.issues, null, 2)}`,
        );
      }
    });
  }
});
