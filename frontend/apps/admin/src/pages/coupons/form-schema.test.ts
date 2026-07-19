import { describe, expect, it } from 'vitest';
import { couponEditorSchema, giftcardEditorSchema } from './form-schema';

const common = {
  name: '活动',
  started_at: 1,
  ended_at: 2,
  limit_use: null,
};

describe('coupon and gift-card value integrity', () => {
  it('rejects negative coupon values and percentages over 100', () => {
    expect(couponEditorSchema.safeParse({ ...common, type: 1, value: '-0.01' }).success).toBe(
      false,
    );
    expect(couponEditorSchema.safeParse({ ...common, type: 2, value: '101' }).success).toBe(false);
    expect(couponEditorSchema.safeParse({ ...common, type: 1, value: '21474836.48' }).success).toBe(
      false,
    );
    expect(couponEditorSchema.safeParse({ ...common, type: 2, value: '25' }).success).toBe(true);
  });

  it('rejects negative gift-card money, duration, traffic, and plan duration', () => {
    for (const type of [1, 2, 3] as const) {
      expect(giftcardEditorSchema.safeParse({ ...common, type, value: '-1' }).success).toBe(false);
    }
    expect(
      giftcardEditorSchema.safeParse({ ...common, type: 5, value: '-1', plan_id: 1 }).success,
    ).toBe(false);
    expect(
      giftcardEditorSchema.safeParse({ ...common, type: 5, value: '0', plan_id: 1 }).success,
    ).toBe(true);
    expect(
      giftcardEditorSchema.safeParse({ ...common, type: 3, value: '2147483648' }).success,
    ).toBe(false);
  });
});
