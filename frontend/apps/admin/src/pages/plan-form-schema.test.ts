import { describe, expect, expectTypeOf, it } from 'vitest';
import type { AdminPlanDto, AdminPlanModel } from '@v2board/types';
import { adminPlanFormToSaveRequest, planEditorSchema } from './plan-form-schema';

const validPlan = {
  name: '标准套餐',
  group_id: 1,
  transfer_enable: 100,
  device_limit: null,
  capacity_limit: null,
  speed_limit: null,
  month_price: null,
  quarter_price: null,
  half_year_price: null,
  year_price: null,
  two_year_price: null,
  three_year_price: null,
  onetime_price: null,
  reset_price: null,
};

describe('planEditorSchema', () => {
  it('keeps wire minor units distinct from the admin major-unit model', () => {
    expectTypeOf<NonNullable<AdminPlanDto['month_price']>>().not.toEqualTypeOf<
      NonNullable<AdminPlanModel['month_price']>
    >();
  });

  it('accepts non-negative limits and the legacy signed price range', () => {
    const parsed = planEditorSchema.safeParse({
      ...validPlan,
      transfer_enable: '100',
      capacity_limit: '0',
      month_price: '-0.01',
    });
    expect(parsed.success).toBe(true);
    if (parsed.success) {
      expect(adminPlanFormToSaveRequest(parsed.data).month_price).toBe(-1);
    }
  });

  it.each([
    ['transfer_enable', '-1'],
    ['device_limit', '-1'],
    ['capacity_limit', '-1'],
    ['speed_limit', '-1'],
    ['transfer_enable', '2147483648'],
    ['month_price', '21474836.48'],
    ['month_price', '-21474836.49'],
  ])('rejects an invalid or out-of-range %s', (field, value) => {
    expect(planEditorSchema.safeParse({ ...validPlan, [field]: value }).success).toBe(false);
  });

  it('maps validated form strings to typed JSON numbers and minor-unit prices', () => {
    const parsed = planEditorSchema.parse({
      ...validPlan,
      transfer_enable: '100',
      device_limit: '3',
      month_price: '19.99',
    });

    expect(adminPlanFormToSaveRequest(parsed)).toMatchObject({
      group_id: 1,
      transfer_enable: 100,
      device_limit: 3,
      month_price: 1999,
      quarter_price: null,
    });
  });

  it('allows force_update only for an existing plan', () => {
    expect(planEditorSchema.safeParse({ ...validPlan, force_update: true }).success).toBe(false);
    expect(planEditorSchema.safeParse({ ...validPlan, id: 1, force_update: true }).success).toBe(
      true,
    );
  });
});
