import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  internalApiAdminPlanItemSchema,
  internalApiOperations,
  internalApiPath,
  internalApiProblemDetailsSchema,
} from './generated/internal-api';

const adminPlansGolden = JSON.parse(
  readFileSync(new URL('../goldens/admin.plans.json', import.meta.url), 'utf8'),
) as unknown[];

describe('generated internal API contract', () => {
  it('validates the Rust admin-plan response as required and nullable', () => {
    const plan = adminPlansGolden[0];

    expect(internalApiAdminPlanItemSchema.safeParse(plan).success).toBe(true);
    expect(
      internalApiAdminPlanItemSchema.safeParse({
        ...(plan as Record<string, unknown>),
        content: null,
        month_price: null,
      }).success,
    ).toBe(true);

    const missingContent = { ...(plan as Record<string, unknown>) };
    delete missingContent.content;
    expect(internalApiAdminPlanItemSchema.safeParse(missingContent).success).toBe(false);
  });

  it('keeps operation metadata, request validation, and path expansion together', () => {
    const create = internalApiOperations.adminPlanCreate;
    expect(create.method).toBe('POST');
    expect(internalApiPath(create.adminPath)).toBe('/plans');
    expect(
      create.requestSchema.safeParse({
        name: 'Generated contract plan',
        group_id: 7,
        transfer_enable: 100,
        month_price: 1_999,
      }).success,
    ).toBe(true);
    expect(
      create.requestSchema.safeParse({
        name: 'Generated contract plan',
        group_id: 7,
        transfer_enable: 100,
        month_price: -1,
      }).success,
    ).toBe(true);
    expect(
      create.requestSchema.safeParse({
        name: 'Generated contract plan',
        group_id: 7,
        transfer_enable: 100,
        month_price: -2_147_483_649,
      }).success,
    ).toBe(false);

    const patch = internalApiOperations.adminPlanPatch;
    expect(internalApiPath(patch.adminPath, { id: 'a/b' })).toBe('/plans/a%2Fb');
    expect(patch.requestSchema.safeParse({ month_price: null }).success).toBe(true);
    expect(patch.requestSchema.safeParse({}).success).toBe(true);
    expect(patch.requestSchema.safeParse({ show: false, force_update: true }).success).toBe(true);
    for (const field of ['name', 'group_id', 'transfer_enable', 'show', 'renew', 'force_update']) {
      expect(patch.requestSchema.safeParse({ [field]: null }).success).toBe(false);
    }

    const sort = internalApiOperations.adminPlansSort;
    expect(sort.requestSchema.safeParse({ ids: [3, 1, 2] }).success).toBe(true);
    expect(sort.requestSchema.safeParse({ plan_ids: [3, 1, 2] }).success).toBe(false);
  });

  it('enforces the RFC 9457 type, code/status/title tuple, and validation-only errors bag', () => {
    const invalidParameter = {
      type: 'about:blank',
      title: 'Bad Request',
      status: 400,
      code: 'invalid_parameter',
      detail: 'Invalid parameter',
    };
    expect(internalApiProblemDetailsSchema.safeParse(invalidParameter).success).toBe(true);
    expect(
      internalApiProblemDetailsSchema.safeParse({
        ...invalidParameter,
        title: 'Unauthorized',
      }).success,
    ).toBe(false);
    expect(
      internalApiProblemDetailsSchema.safeParse({
        ...invalidParameter,
        type: 'https://example.test/problem',
      }).success,
    ).toBe(false);
    expect(
      internalApiProblemDetailsSchema.safeParse({
        ...invalidParameter,
        errors: { field: ['not allowed for this code'] },
      }).success,
    ).toBe(false);

    const validation = {
      ...invalidParameter,
      title: 'Unprocessable Entity',
      status: 422,
      code: 'validation_failed',
      errors: { field: ['is invalid'] },
    };
    expect(internalApiProblemDetailsSchema.safeParse(validation).success).toBe(true);
    expect(internalApiProblemDetailsSchema.safeParse({ ...validation, errors: null }).success).toBe(
      false,
    );
  });

  it('pins every exact success response without inventing a primary for multi-status operations', () => {
    for (const operation of Object.values(internalApiOperations)) {
      const statuses = Object.keys(operation.successResponses).map(Number);
      expect(statuses.length).toBeGreaterThan(0);
      expect(statuses.every((status) => status >= 200 && status < 400)).toBe(true);
      expect(operation.successStatus).toBe(statuses.length === 1 ? statuses[0] : undefined);
    }

    expect({
      adminPlanCreate: internalApiOperations.adminPlanCreate.successStatus,
      adminPlanDelete: internalApiOperations.adminPlanDelete.successStatus,
      adminPlanPatch: internalApiOperations.adminPlanPatch.successStatus,
      adminPlansList: internalApiOperations.adminPlansList.successStatus,
      adminPlansSort: internalApiOperations.adminPlansSort.successStatus,
    }).toEqual({
      adminPlanCreate: 201,
      adminPlanDelete: 204,
      adminPlanPatch: 204,
      adminPlansList: 200,
      adminPlansSort: 204,
    });
  });

  it('generates required, bounded, enum, pagination, and repeated query validators', () => {
    const quick = internalApiOperations.authQuickLogin.parameters.query;
    expect(quick.safeParse({}).success).toBe(false);
    expect(quick.safeParse({ token: 'temporary-token' }).success).toBe(true);
    expect(quick.safeParse({ token: '' }).success).toBe(false);

    const rank = internalApiOperations.adminStatsServerRank.parameters.query;
    expect(rank.safeParse({ window: 'today' }).success).toBe(true);
    expect(rank.safeParse({ window: 'previous' }).success).toBe(true);
    expect(rank.safeParse({}).success).toBe(false);
    expect(rank.safeParse({ window: 'week' }).success).toBe(false);

    const notices = internalApiOperations.userNoticesList.parameters.query;
    expect(notices.safeParse({ page: 1, per_page: 100 }).success).toBe(true);
    expect(notices.safeParse({ page: 0 }).success).toBe(false);
    expect(notices.safeParse({ per_page: 101 }).success).toBe(false);

    const adminTickets = internalApiOperations.adminTicketsList.parameters.query;
    expect(adminTickets.safeParse({ reply_status: [0, 1] }).success).toBe(true);
    expect(adminTickets.safeParse({ reply_status: '0,1' }).success).toBe(false);

    const staffTickets = internalApiOperations.staffTicketsList.parameters.query;
    expect(staffTickets.safeParse({ status: 0 }).success).toBe(true);
    expect(staffTickets.safeParse({ reply_status: [0] }).success).toBe(false);
    expect(staffTickets.safeParse({ email: 'staff@example.test' }).success).toBe(false);
  });

  it('generates the common locale and conditional operation header validators', () => {
    for (const operation of Object.values(internalApiOperations)) {
      expect(operation.parameters.header.safeParse({}).success).toBe(true);
      expect(
        operation.parameters.header.safeParse({ 'Accept-Language': 'ja-JP,en;q=0.5' }).success,
      ).toBe(true);
    }

    const login = internalApiOperations.authLogin.parameters.header;
    expect(login.safeParse({ 'User-Agent': 'browser' }).success).toBe(true);

    const mail = internalApiOperations.adminUsersMail.parameters.header;
    expect(
      mail.safeParse({
        'Accept-Language': 'en-US',
        'Idempotency-Key': 'mail-run-7',
        'X-V2Board-Step-Up': 'step-up-token',
      }).success,
    ).toBe(true);
    expect(mail.safeParse({ 'Idempotency-Key': 'x'.repeat(513) }).success).toBe(false);
  });
});
