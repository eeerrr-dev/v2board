import { describe, expect, it } from 'vitest';
import { userManageSchema } from '@/components/user-manage-schema';
import { couponEditorSchema, giftcardEditorSchema } from '@/pages/coupons/form-schema';
import { paymentFormSchema } from '@/pages/payment-form-schema';
import { planEditorSchema } from '@/pages/plan-form-schema';

function issuePaths(result: ReturnType<typeof paymentFormSchema.safeParse>) {
  return result.success ? [] : result.error.issues.map((issue) => issue.path.join('.'));
}

describe('admin form contract schemas', () => {
  it('accepts a complete payment form and rejects backend-invalid fields', () => {
    expect(
      paymentFormSchema.safeParse({
        name: 'Stripe',
        payment: 'StripeCredit',
        config: { stripe_secret_key: 'sk_test' },
        notify_domain: 'https://pay.example.com',
        handling_fee_percent: '1.5',
        handling_fee_fixed: '2.50',
      }).success,
    ).toBe(true);

    const invalid = paymentFormSchema.safeParse({
      name: '',
      payment: '',
      config: {},
      notify_domain: 'javascript:alert(1)',
      handling_fee_percent: '0',
      handling_fee_fixed: '-0.01',
    });
    expect(issuePaths(invalid)).toEqual(
      expect.arrayContaining([
        'name',
        'payment',
        'config',
        'notify_domain',
        'handling_fee_percent',
        'handling_fee_fixed',
      ]),
    );
  });

  it('validates coupon amount/percentage, required dates, count, and date ordering', () => {
    expect(
      couponEditorSchema.safeParse({
        name: '十元券',
        type: 1,
        value: '10.25',
        started_at: '100',
        ended_at: '200',
        generate_count: '500',
      }).success,
    ).toBe(true);

    const invalid = couponEditorSchema.safeParse({
      name: '',
      type: 2,
      value: '12.5',
      started_at: '200',
      ended_at: '100',
      generate_count: '501',
    });
    if (invalid.success) throw new Error('expected coupon validation to fail');
    expect(invalid.error.issues.map((issue) => issue.path.join('.'))).toEqual(
      expect.arrayContaining(['name', 'value', 'ended_at', 'generate_count']),
    );
  });

  it('applies giftcard type-dependent value and plan requirements', () => {
    expect(
      giftcardEditorSchema.safeParse({
        name: '重置卡',
        type: 4,
        started_at: 100,
        ended_at: 200,
      }).success,
    ).toBe(true);

    const invalid = giftcardEditorSchema.safeParse({
      name: '套餐卡',
      type: 5,
      value: '30',
      plan_id: null,
      started_at: 100,
      ended_at: 200,
    });
    if (invalid.success) throw new Error('expected giftcard validation to fail');
    expect(invalid.error.issues.map((issue) => issue.path.join('.'))).toContain('plan_id');
  });

  it('requires the plan fields used by both backends and validates raw numeric inputs', () => {
    const validPlan = {
      name: '基础套餐',
      group_id: 1,
      transfer_enable: '100',
      month_price: '12.34',
      quarter_price: null,
      half_year_price: null,
      year_price: null,
      two_year_price: null,
      three_year_price: null,
      onetime_price: null,
      reset_price: null,
    };
    expect(planEditorSchema.safeParse(validPlan).success).toBe(true);

    const invalid = planEditorSchema.safeParse({
      ...validPlan,
      name: '',
      group_id: undefined,
      transfer_enable: '1.5',
      month_price: 'free',
      device_limit: 'many',
    });
    if (invalid.success) throw new Error('expected plan validation to fail');
    expect(invalid.error.issues.map((issue) => issue.path.join('.'))).toEqual(
      expect.arrayContaining([
        'name',
        'group_id',
        'transfer_enable',
        'month_price',
        'device_limit',
      ]),
    );
  });

  it('validates user money, integer, password, and percentage fields without coercing them', () => {
    const validUser = {
      email: 'user@example.com',
      password: '',
      balance: '12.34',
      commission_balance: '5.00',
      transfer_enable: '100',
      u: '1.25',
      d: 0,
      banned: 0,
      commission_type: 0,
      commission_rate: null,
      discount: '90',
      is_admin: 0,
      is_staff: 0,
      admin_permissions: [],
    };
    const valid = userManageSchema.safeParse(validUser);
    expect(valid.success).toBe(true);
    if (valid.success) expect(valid.data.balance).toBe('12.34');

    const invalid = userManageSchema.safeParse({
      ...validUser,
      password: 'short',
      balance: 'money',
      commission_type: null,
      commission_rate: '101',
      speed_limit: 'fast',
    });
    if (invalid.success) throw new Error('expected user validation to fail');
    expect(invalid.error.issues.map((issue) => issue.path.join('.'))).toEqual(
      expect.arrayContaining([
        'password',
        'balance',
        'commission_type',
        'commission_rate',
        'speed_limit',
      ]),
    );
  });

  it('rejects user money and GiB values that cannot become safe integer wire units', () => {
    const input = {
      email: 'user@example.com',
      password: '',
      balance: '900719925474099.99',
      commission_balance: '5.00',
      transfer_enable: '9007199254740992',
      u: '1',
      d: '1',
      banned: 0,
      commission_type: 0,
      is_admin: 0,
      is_staff: 0,
      admin_permissions: [],
    };

    const invalid = userManageSchema.safeParse(input);
    if (invalid.success) throw new Error('expected scaled wire validation to fail');
    expect(invalid.error.issues.map((issue) => issue.path.join('.'))).toEqual(
      expect.arrayContaining(['balance', 'transfer_enable']),
    );
  });
});
