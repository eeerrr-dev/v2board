import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  internalApiAdminPlanItemSchema,
  internalApiOperations,
  internalApiPath,
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

  it('pins one exact success status on every generated operation', () => {
    expect(
      Object.fromEntries(
        Object.entries(internalApiOperations).map(([id, operation]) => [
          id,
          operation.successStatus,
        ]),
      ),
    ).toEqual({
      adminPlanCreate: 201,
      adminPlanDelete: 204,
      adminPlanPatch: 204,
      adminPlansList: 200,
      adminPlansSort: 204,
    });
  });
});
