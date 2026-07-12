import { describe, expect, it } from 'vitest';
import { planEditorSchema } from './plan-form-schema';

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
  it('accepts non-negative traffic, capacity, and prices', () => {
    expect(
      planEditorSchema.safeParse({
        ...validPlan,
        transfer_enable: '100',
        capacity_limit: '0',
        month_price: '19.99',
      }).success,
    ).toBe(true);
  });

  it.each([
    ['transfer_enable', '-1'],
    ['device_limit', '-1'],
    ['capacity_limit', '-1'],
    ['speed_limit', '-1'],
    ['month_price', '-0.01'],
    ['transfer_enable', '2147483648'],
    ['month_price', '21474836.48'],
  ])('rejects a negative %s', (field, value) => {
    expect(planEditorSchema.safeParse({ ...validPlan, [field]: value }).success).toBe(false);
  });
});
