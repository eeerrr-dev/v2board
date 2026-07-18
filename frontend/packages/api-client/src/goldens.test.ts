import { readFileSync, readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import {
  activeSessionSchema,
  adminKnowledgeSchema,
  adminKnowledgeSummarySchema,
  adminNoticeSchema,
  adminOrderSchema,
  adminPaymentSchema,
  adminUserDetailSchema,
  adminUserSchema,
  adminUserTrafficSchema,
  arraySchema,
  authDataSchema,
  availableServerSchema,
  checkoutResultSchema,
  commissionDetailSchema,
  couponSchema,
  createdOrderSchema,
  createdTicketSchema,
  envelopeSchema,
  giftCardRedemptionSchema,
  giftcardSchema,
  guestConfigSchema,
  inviteFetchSchema,
  knowledgeCategorySchema,
  knowledgeSchema,
  noticeSchema,
  orderStatusSchema,
  pageEnvelopeSchema,
  paymentMethodSchema,
  planSchema,
  quickLoginUrlSchema,
  resetSubscribeTokenSchema,
  serverGroupSchema,
  serverNodeSchema,
  serverRouteSchema,
  sessionStateSchema,
  stepUpGrantSchema,
  stringArraySchema,
  subscriptionSchema,
  telegramBotInfoSchema,
  ticketSchema,
  stripePaymentIntentSchema,
  trafficLogSchema,
  userCommConfigSchema,
  userCouponSchema,
  userOrderSchema,
  userOrdersSchema,
  userPlanSchema,
  userProfileSchema,
  userStatsSchema,
  userTicketDetailSchema,
  userTicketSchema,
} from './contracts';
import { pageSchema, problemDetailsSchema } from './dialect';

/**
 * Golden-response contract lane: every fixture in `../goldens` is a full
 * response body serialized by the Rust backend (`make contract-goldens`) and
 * pinned byte-for-byte by Rust tests. Parsing each one with the exact zod
 * contract the client uses closes the Rust→zod edge — a backend shape change
 * that would break the TypeScript contract fails here instead of drifting
 * silently. The fixture set and this schema map must stay bijective.
 */
const goldensUrl = new URL('../goldens/', import.meta.url);

const goldenSchemas: Record<string, z.ZodType> = {
  // DB-backed admin surface (v2board-contract golden-responses).
  'admin.coupon.fetch.json': pageEnvelopeSchema(couponSchema),
  'admin.giftcard.fetch.json': pageEnvelopeSchema(giftcardSchema),
  'admin.knowledge.detail.json': envelopeSchema(adminKnowledgeSchema),
  'admin.knowledge.fetch.json': envelopeSchema(arraySchema(adminKnowledgeSummarySchema)),
  'admin.knowledge.getCategory.json': envelopeSchema(stringArraySchema),
  'admin.notice.fetch.json': pageEnvelopeSchema(adminNoticeSchema),
  'admin.order.detail.json': envelopeSchema(adminOrderSchema),
  'admin.order.fetch.json': pageEnvelopeSchema(adminOrderSchema),
  'admin.payment.fetch.json': envelopeSchema(arraySchema(adminPaymentSchema)),
  'admin.payment.getPaymentMethods.json': envelopeSchema(stringArraySchema),
  'admin.plan.fetch.json': envelopeSchema(arraySchema(planSchema)),
  'admin.server.group.fetch.json': envelopeSchema(arraySchema(serverGroupSchema)),
  'admin.server.manage.getNodes.json': envelopeSchema(arraySchema(serverNodeSchema)),
  'admin.server.route.fetch.json': envelopeSchema(arraySchema(serverRouteSchema)),
  'admin.stat.getStatUser.json': pageEnvelopeSchema(adminUserTrafficSchema),
  'admin.ticket.detail.json': envelopeSchema(ticketSchema),
  'admin.ticket.fetch.json': pageEnvelopeSchema(ticketSchema),
  'admin.user.fetch.json': pageEnvelopeSchema(adminUserSchema),
  'admin.user.getUserInfoById.json': envelopeSchema(adminUserDetailSchema),
  'admin.user.getUserInfoById.no-inviter.json': envelopeSchema(adminUserDetailSchema),
};

// Dialect-v2 fixtures (docs/api-dialect.md §3, §5.2): bare success bodies and
// problem+json error bodies — parsed exactly as the wire delivers them, with
// no envelope emulation.
const dialectGoldenSchemas: Record<string, z.ZodType> = {
  'auth.login.json': authDataSchema,
  'auth.quick-login-url.json': quickLoginUrlSchema,
  'auth.session.admin.json': sessionStateSchema,
  'auth.session.json': sessionStateSchema,
  'auth.session.logged-out.json': sessionStateSchema,
  'auth.step-up.json': stepUpGrantSchema,
  // §6.1 (W9): the PATCH config stale-revision conflict problem.
  'problem.config-revision-conflict.json': problemDetailsSchema,
  'problem.session-expired.json': problemDetailsSchema,
  'problem.validation.json': problemDetailsSchema,
  'public.config.json': guestConfigSchema,
  'public.config.whitelist-disabled.json': guestConfigSchema,
  'user.commissions.json': pageSchema(commissionDetailSchema),
  'user.config.json': userCommConfigSchema,
  'user.coupons.check.json': userCouponSchema,
  'user.gift-card-redemptions.create.json': giftCardRedemptionSchema,
  'user.invite.json': inviteFetchSchema,
  'user.knowledge-categories.json': arraySchema(z.looseObject({ category: z.string() })),
  'user.knowledge.detail.json': knowledgeSchema,
  'user.knowledge.json': knowledgeCategorySchema,
  'user.notices.json': pageSchema(noticeSchema),
  'user.orders.checkout.qr.json': checkoutResultSchema,
  'user.orders.checkout.redirect.json': checkoutResultSchema,
  'user.orders.checkout.settled.json': checkoutResultSchema,
  'user.orders.create.json': createdOrderSchema,
  'user.orders.detail.deposit.json': userOrderSchema,
  'user.orders.detail.json': userOrderSchema,
  'user.orders.json': userOrdersSchema,
  'user.orders.status.json': orderStatusSchema,
  'user.orders.stripe-intent.json': stripePaymentIntentSchema,
  'user.payment-methods.json': arraySchema(paymentMethodSchema),
  'user.plans.detail.json': userPlanSchema,
  'user.plans.json': arraySchema(userPlanSchema),
  'user.profile.json': userProfileSchema,
  'user.servers.json': arraySchema(availableServerSchema),
  'user.sessions.json': arraySchema(activeSessionSchema),
  'user.stats.json': userStatsSchema,
  'user.subscription.json': subscriptionSchema,
  'user.subscription.reset-token.json': resetSubscribeTokenSchema,
  'user.telegram-bot.json': telegramBotInfoSchema,
  'user.tickets.create.json': createdTicketSchema,
  'user.tickets.detail.json': userTicketDetailSchema,
  'user.tickets.json': arraySchema(userTicketSchema),
  'user.traffic-logs.json': arraySchema(trafficLogSchema),
  'user.withdrawal-tickets.create.json': createdTicketSchema,
};

describe('golden response fixtures', () => {
  it('keeps the fixture directory and the schema maps bijective', () => {
    const files = readdirSync(goldensUrl)
      .filter((name) => name.endsWith('.json'))
      .sort();
    expect(files).toEqual(
      [...Object.keys(goldenSchemas), ...Object.keys(dialectGoldenSchemas)].sort(),
    );
  });

  for (const [name, schema] of Object.entries(goldenSchemas)) {
    it(`parses ${name} with its zod contract`, () => {
      const body = JSON.parse(readFileSync(new URL(name, goldensUrl), 'utf8')) as Record<
        string,
        unknown
      >;
      // The runtime client injects `code` from the HTTP status when the JSON
      // body omits it (client.ts unwrapBackendEnvelope); mirror that before
      // running the envelope schema.
      const unwrapped = { ...body, code: body.code ?? 200 };
      const result = schema.safeParse(unwrapped);
      if (!result.success) {
        throw new Error(
          `${name} no longer satisfies its zod contract:\n${JSON.stringify(result.error.issues, null, 2)}`,
        );
      }
    });
  }

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
