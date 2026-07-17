import { readFileSync, readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { z } from 'zod';
import {
  adminKnowledgeSummarySchema,
  adminOrderSchema,
  adminPaymentSchema,
  adminUserDetailSchema,
  adminUserSchema,
  adminUserTrafficSchema,
  arraySchema,
  authDataSchema,
  checkoutEnvelopeSchema,
  couponSchema,
  envelopeSchema,
  giftcardSchema,
  guestConfigSchema,
  knowledgeSchema,
  noticeSchema,
  ordersSchema,
  orderSchema,
  pageEnvelopeSchema,
  paymentMethodSchema,
  planSchema,
  quickLoginUrlSchema,
  serverGroupSchema,
  serverNodeSchema,
  serverRouteSchema,
  sessionStateSchema,
  stepUpGrantSchema,
  stringArraySchema,
  subscribeInfoSchema,
  ticketSchema,
  trafficLogSchema,
  userCommConfigSchema,
  userInfoSchema,
} from './contracts';
import { problemDetailsSchema } from './dialect';

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
  'admin.config.getEmailTemplate.json': envelopeSchema(stringArraySchema),
  'admin.coupon.fetch.json': pageEnvelopeSchema(couponSchema),
  'admin.giftcard.fetch.json': pageEnvelopeSchema(giftcardSchema),
  'admin.knowledge.detail.json': envelopeSchema(knowledgeSchema),
  'admin.knowledge.fetch.json': envelopeSchema(arraySchema(adminKnowledgeSummarySchema)),
  'admin.knowledge.getCategory.json': envelopeSchema(stringArraySchema),
  'admin.notice.fetch.json': pageEnvelopeSchema(noticeSchema),
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
  // Pure serde wire bodies (v2board-api golden_wire test).
  'guest.comm.config.json': envelopeSchema(guestConfigSchema),
  'guest.comm.config.whitelist-disabled.json': envelopeSchema(guestConfigSchema),
  'user.comm.config.json': envelopeSchema(userCommConfigSchema),
  'user.getSubscribe.json': envelopeSchema(subscribeInfoSchema),
  'user.info.json': envelopeSchema(userInfoSchema),
  'user.order.checkout.json': checkoutEnvelopeSchema,
  'user.order.checkout.qr.json': checkoutEnvelopeSchema,
  'user.order.detail.deposit.json': envelopeSchema(orderSchema),
  'user.order.detail.json': envelopeSchema(orderSchema),
  'user.order.fetch.json': envelopeSchema(ordersSchema),
  'user.order.getPaymentMethod.json': envelopeSchema(arraySchema(paymentMethodSchema)),
  'user.stat.getTrafficLog.json': envelopeSchema(arraySchema(trafficLogSchema)),
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
  'problem.session-expired.json': problemDetailsSchema,
  'problem.validation.json': problemDetailsSchema,
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
