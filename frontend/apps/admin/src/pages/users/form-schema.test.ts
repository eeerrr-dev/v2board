import { describe, expect, it } from 'vitest';
import {
  assignOrderSchema,
  generateUserSchema,
  sendMailSchema,
  userFilterSchema,
} from './form-schema';

describe('admin user action schemas', () => {
  it('accepts the complete order assignment protocol and rejects partial values', () => {
    expect(
      assignOrderSchema.parse({
        email: ' buyer@example.com ',
        plan_id: 1,
        period: 'month_price',
        total_amount: '50.00',
      }),
    ).toEqual({
      email: 'buyer@example.com',
      plan_id: 1,
      period: 'month_price',
      total_amount: '50.00',
    });
    expect(assignOrderSchema.safeParse({ email: '' }).success).toBe(false);
    expect(
      assignOrderSchema.safeParse({
        email: 'buyer@example.com',
        plan_id: 1,
        period: 'weekly',
        total_amount: '12.345.6',
      }).success,
    ).toBe(false);
    for (const total_amount of ['-0.01', '21474836.48']) {
      expect(
        assignOrderSchema.safeParse({
          email: 'buyer@example.com',
          plan_id: 1,
          period: 'month_price',
          total_amount,
        }).success,
      ).toBe(false);
    }
  });

  it('requires exactly one single/bulk generation selector and enforces the backend limit', () => {
    const base = {
      email_suffix: 'example.com',
      password: '',
      plan_id: null,
      expired_at: null,
    };
    expect(
      generateUserSchema.safeParse({
        ...base,
        email_prefix: 'one',
        generate_count: '',
      }).success,
    ).toBe(true);
    expect(
      generateUserSchema.safeParse({
        ...base,
        email_prefix: '',
        generate_count: '500',
      }).success,
    ).toBe(true);
    expect(
      generateUserSchema.safeParse({
        ...base,
        email_prefix: 'one',
        generate_count: '2',
      }).success,
    ).toBe(false);
    expect(
      generateUserSchema.safeParse({
        ...base,
        email_prefix: '',
        generate_count: '501',
      }).success,
    ).toBe(false);
  });

  it('rejects blank mail and filter values without rejecting numeric zero', () => {
    expect(sendMailSchema.safeParse({ subject: ' ', content: 'body' }).success).toBe(false);
    expect(
      userFilterSchema.safeParse({ rows: [{ key: 'id', condition: '=', value: '' }] }).success,
    ).toBe(false);
    expect(
      userFilterSchema.safeParse({ rows: [{ key: 'banned', condition: '=', value: 0 }] }).success,
    ).toBe(true);
  });
});
