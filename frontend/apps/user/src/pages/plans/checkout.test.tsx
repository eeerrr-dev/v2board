import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Plan } from '@v2board/types';
import PlanCheckoutPage from './checkout';

const checkoutSource = readFileSync(`${process.cwd()}/src/pages/plans/checkout.tsx`, 'utf8');

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  removeQueries: vi.fn(),
  invalidateQueries: vi.fn(),
  legacyConfirm: vi.fn(),
  checkCoupon: vi.fn(),
  saveOrder: vi.fn(),
  cancelOrder: vi.fn(),
  refetchOrders: vi.fn(),
  planId: '1',
  plan: {
    id: 1,
    group_id: 1,
    transfer_enable: 100,
    device_limit: null,
    speed_limit: null,
    reset_traffic_method: null,
    name: 'Legacy Plan',
    show: 1,
    sort: 0,
    renew: 1,
    content: JSON.stringify([{ feature: 'Feature A', support: true }]),
    month_price: 1000,
    quarter_price: null,
    half_year_price: null,
    year_price: 9000,
    two_year_price: null,
    three_year_price: null,
    onetime_price: 50000,
    reset_price: null,
    capacity_limit: null,
    created_at: 0,
    updated_at: 0,
  },
  info: { plan_id: 1 },
  subscribe: { expired_at: 4_102_444_800 },
  orders: [] as Array<{ trade_no: string; status: number }>,
  planError: null as unknown,
  planFetching: false,
}));

const labels: Record<string, string> = {
  'common.attention': '注意',
  'plan.monthly': '月付',
  'plan.yearly': '年付',
  'plan.onetime': '一次性',
  'plan.reset': '流量重置包',
  'plan.select_period': '付款周期',
  'plan.coupon_question': '有优惠券？',
  'plan.verify': '验证',
  'plan.order_total': '订单总额',
  'plan.grand_total': '总计',
  'plan.discount': '折扣',
  'plan.place_order': '下单',
  'plan.cannot_renew_current': '该订阅无法续费，仅允许新用户购买',
  'plan.select_other': '选择其它订阅',
  'plan.change_warning': '请注意，变更订阅会导致当前订阅被新订阅覆盖。',
  'plan.unfinished_order_confirm': '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
  'plan.confirm_cancel_previous': '确定取消',
  'plan.return_orders': '返回我的订单',
};

function resetPlan() {
  mocks.planId = '1';
  Object.assign(mocks.plan, {
    id: 1,
    group_id: 1,
    transfer_enable: 100,
    device_limit: null,
    speed_limit: null,
    reset_traffic_method: null,
    name: 'Legacy Plan',
    show: 1,
    sort: 0,
    renew: 1,
    content: JSON.stringify([{ feature: 'Feature A', support: true }]),
    month_price: 1000,
    quarter_price: null,
    half_year_price: null,
    year_price: 9000,
    two_year_price: null,
    three_year_price: null,
    onetime_price: 50000,
    reset_price: null,
    capacity_limit: null,
    created_at: 0,
    updated_at: 0,
  });
  mocks.planError = null;
  mocks.planFetching = false;
}

vi.mock('react-router-dom', () => ({
  useParams: () => ({ plan_id: mocks.planId }),
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key, i18n: { language: 'zh-CN' } }),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    invalidateQueries: mocks.invalidateQueries,
    removeQueries: mocks.removeQueries,
  }),
}));

vi.mock('@/lib/api', () => ({
  apiClient: {},
}));

vi.mock('@v2board/api-client', () => ({
  user: {
    checkCoupon: mocks.checkCoupon,
    saveOrder: mocks.saveOrder,
  },
}));

vi.mock('@/components/legacy-confirm', () => ({
  legacyConfirm: mocks.legacyConfirm,
}));

vi.mock('@/components/legacy-loading-icon', () => ({
  LegacyLoadingIcon: () => <span>loading</span>,
}));

vi.mock('@/lib/queries', () => ({
  userKeys: {
    orderDetail: (tradeNo: string) => ['user', 'orders', 'detail', tradeNo],
    plans: ['user', 'plans'],
    plan: (id: string) => ['user', 'plan', id],
  },
  usePlan: () => ({
    data: mocks.plan,
    error: mocks.planError,
    isFetching: mocks.planFetching,
  }),
  useCommConfig: () => ({
    data: {
      currency: 'CNY',
      currency_symbol: '¥',
    },
  }),
  useOrders: () => ({
    data: mocks.orders,
    refetch: mocks.refetchOrders,
  }),
  useUserInfo: () => ({
    data: mocks.info,
  }),
  useSubscribe: () => ({
    data: mocks.subscribe,
  }),
  useCancelOrderMutation: () => ({
    isPending: false,
    mutateAsync: mocks.cancelOrder,
  }),
}));

describe('PlanCheckoutPage bundled-theme markup', () => {
  beforeEach(() => {
    resetPlan();
    mocks.plan.renew = 1;
    mocks.info = { plan_id: 1 };
    mocks.orders = [];
  });

  it('renders the cashier shell, default period, coupon block, and original class strings', () => {
    const html = renderToStaticMarkup(<PlanCheckoutPage />);

    expect(html).toContain('id="cashier"');
    expect(html).toContain('block block-link-pop block-rounded py-3');
    expect(html).toContain('v2board-plan-content px-3');
    expect(html).toContain('si si-check text-primary');
    expect(html).toContain('付款周期');
    expect(html).toContain('class="v2board-select active border-primary"');
    expect(html).toContain('class="v2board-select false"');
    expect(html).toContain('Legacy Plan x 月付');
    expect(html).toContain('¥10.00');
    expect(html).toContain('¥ 10.00 CNY');
    expect(html).toContain('block block-link-pop block-rounded  px-3 py-3 mb-2 text-light');
    expect(html).toContain('placeholder="有优惠券？"');
  });

  it('renders the legacy non-renewable result branch', () => {
    mocks.plan.renew = 0;
    mocks.info = { plan_id: 1 };

    const html = renderToStaticMarkup(<PlanCheckoutPage />);

    expect(html).toContain('ant-result ant-result-info');
    expect(html).toContain('该订阅无法续费，仅允许新用户购买');
    expect(html).toContain('选择其它订阅');
  });

  it('keeps reset_price as the old default period without rendering it as a selectable period', () => {
    const plan = mocks.plan as Plan;
    plan.month_price = null;
    plan.year_price = null;
    plan.onetime_price = null;
    plan.reset_price = 300;

    const html = renderToStaticMarkup(<PlanCheckoutPage />);

    expect(html).toContain('Legacy Plan x 流量重置包');
    expect(html).toContain('¥3.00');
    expect(html).toContain('¥ 3.00 CNY');
    expect(html).not.toContain('class="v2board-select');
  });

  it('uses stable period select keys without changing the bundled-theme markup', () => {
    const periodSource = checkoutSource.slice(
      checkoutSource.indexOf('periods.map((item) => {'),
      checkoutSource.indexOf('</div>', checkoutSource.indexOf('periods.map((item) => {')),
    );

    expect(periodSource).toContain('periods.map((item) => {');
    expect(periodSource).toContain('key={item.period}');
    expect(periodSource).not.toContain('key={Math.random()}');
  });

  it('keeps the bundled-theme direct plan content handoff on checkout', () => {
    expect(checkoutSource).toContain('content={plan.content}');
    expect(checkoutSource).not.toContain("content={plan.content ?? ''}");
  });

  it('keeps the bundled-theme route plan id as the checkout API input', () => {
    expect(checkoutSource).toContain('const planId = plan_id;');
    expect(checkoutSource).toContain('userKeys.plan(planId as string)');
    expect(checkoutSource).toContain('planId as string');
    expect(checkoutSource).not.toContain("const planId = plan_id ?? ''");
    expect(checkoutSource).not.toContain("userKeys.plan(planId ?? '')");
  });

  it('keeps the bundled-theme direct selected period in the order payload', () => {
    expect(checkoutSource).toContain('period: currentPeriod,');
    expect(checkoutSource).toContain("if (appliedCoupon?.name) payload.coupon_code = appliedCoupon.code;");
    expect(checkoutSource).toContain('useState<PlanPeriod | undefined>()');
    expect(checkoutSource).toContain('function getDefaultPeriod(plan: Plan): PlanPeriod | undefined');
    expect(checkoutSource).not.toContain('period: currentPeriod ?? undefined');
    expect(checkoutSource).not.toContain('coupon_code: appliedCoupon?.name ? appliedCoupon.code : undefined');
    expect(checkoutSource).not.toContain('useState<PlanPeriod | null>');
    expect(checkoutSource).not.toContain(
      'const currentPeriod = (period ?? getDefaultPeriod(planQuery.data)) as PlanPeriod;',
    );
  });

  it('keeps the bundled-theme direct coupon input value for coupon checks', () => {
    expect(checkoutSource).toContain('couponRef.current!.value');
    expect(checkoutSource).not.toContain("couponRef.current?.value ?? ''");
  });

  it('clears both plan list and detail query state on unmount like plan/empty', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(<PlanCheckoutPage />);
        await Promise.resolve();
      });

      act(() => root.unmount());

      expect(mocks.removeQueries).toHaveBeenCalledWith({ queryKey: ['user', 'plans'] });
      expect(mocks.removeQueries).toHaveBeenCalledWith({ queryKey: ['user', 'plan', '1'] });
      expect(checkoutSource).toContain('queryClient.removeQueries({ queryKey: userKeys.plans });');
    } finally {
      container.remove();
      document.body.innerHTML = '';
      mocks.removeQueries.mockClear();
    }
  });
});

describe('PlanCheckoutPage bundled-theme behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    resetPlan();
    mocks.navigate.mockClear();
    mocks.removeQueries.mockClear();
    mocks.invalidateQueries.mockClear();
    mocks.legacyConfirm.mockClear();
    mocks.checkCoupon.mockReset();
    mocks.saveOrder.mockReset();
    mocks.cancelOrder.mockReset();
    mocks.refetchOrders.mockClear();
    mocks.plan.renew = 1;
    mocks.info = { plan_id: 1 };
    mocks.subscribe = { expired_at: 4_102_444_800 };
    mocks.orders = [];
    mocks.planError = null;
    mocks.planFetching = false;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  it('applies coupon values through the original cents math and saves the order', async () => {
    mocks.checkCoupon.mockResolvedValue({
      id: 1,
      code: 'SAVE',
      name: 'Legacy Coupon',
      type: 1,
      value: 250,
      show: 1,
      limit_use: null,
      limit_use_with_user: null,
      limit_plan_ids: null,
      limit_period: null,
      started_at: 0,
      ended_at: 0,
      created_at: 0,
      updated_at: 0,
    });
    mocks.saveOrder.mockResolvedValue('TRADE123');

    await act(async () => {
      root.render(<PlanCheckoutPage />);
      await Promise.resolve();
    });

    const couponInput = container.querySelector<HTMLInputElement>('.v2board-input-coupon')!;
    couponInput.value = 'SAVE';
    const verifyButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('验证'),
    )!;

    await act(async () => {
      verifyButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.checkCoupon).toHaveBeenCalledWith(expect.anything(), 'SAVE', '1');
    expect(container.textContent).toContain('Legacy Coupon');
    expect(container.textContent).toContain('-¥2.50');
    expect(container.textContent).toContain('¥ 7.50 CNY');

    const submitButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('下单'),
    )!;

    await act(async () => {
      submitButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveOrder).toHaveBeenCalledWith(expect.anything(), {
      plan_id: 1,
      period: 'month_price',
      coupon_code: 'SAVE',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE123');
  });

  it('keeps the original undefined period payload when no price period exists', async () => {
    const plan = mocks.plan as Plan;
    plan.month_price = null;
    plan.year_price = null;
    plan.onetime_price = null;
    plan.reset_price = null;
    mocks.saveOrder.mockResolvedValue('TRADE-NO-PERIOD');

    await act(async () => {
      root.render(<PlanCheckoutPage />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('Legacy Plan x ');
    expect(container.textContent).toContain('¥NaN');
    expect(container.textContent).toContain('¥ NaN CNY');

    const submitButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('下单'),
    )!;

    await act(async () => {
      submitButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveOrder).toHaveBeenCalledWith(expect.anything(), {
      plan_id: 1,
      period: undefined,
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE-NO-PERIOD');
  });

  it('confirms and cancels the first unfinished order before creating a new one', async () => {
    mocks.orders = [{ trade_no: 'PENDING123', status: 0 }];
    mocks.cancelOrder.mockResolvedValue(true);
    mocks.saveOrder.mockResolvedValue('TRADE456');

    await act(async () => {
      root.render(<PlanCheckoutPage />);
      await Promise.resolve();
    });

    const submitButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('下单'),
    )!;

    await act(async () => {
      submitButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).toHaveBeenCalledTimes(1);
    const options = mocks.legacyConfirm.mock.calls[0]![0] as {
      content: string;
      okText: string;
      cancelText: string;
      onOk: () => void;
      onCancel: () => void;
    };
    expect(options.content).toBe('您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？');
    expect(options.okText).toBe('确定取消');
    expect(options.cancelText).toBe('返回我的订单');

    options.onCancel();
    expect(mocks.navigate).toHaveBeenCalledWith('/order');

    await act(async () => {
      options.onOk();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.cancelOrder).toHaveBeenCalledWith('PENDING123');
    expect(mocks.refetchOrders).not.toHaveBeenCalled();
    expect(mocks.invalidateQueries).not.toHaveBeenCalled();
    expect(checkoutSource).toContain('Legacy order/cancel owns the list refresh');
    expect(checkoutSource).not.toContain('userKeys.orderDetail(unfinishedOrder.trade_no)');
    expect(checkoutSource).not.toContain('orders.refetch()');
    expect(mocks.saveOrder).toHaveBeenCalledWith(expect.anything(), {
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE456');
  });

  it('shows the original change-subscription warning before saving', async () => {
    mocks.info = { plan_id: 2 };
    mocks.subscribe = { expired_at: 4_102_444_800 };
    mocks.saveOrder.mockResolvedValue('TRADE789');

    await act(async () => {
      root.render(<PlanCheckoutPage />);
      await Promise.resolve();
    });

    const submitButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('下单'),
    )!;

    await act(async () => {
      submitButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).toHaveBeenCalledTimes(1);
    const options = mocks.legacyConfirm.mock.calls[0]![0] as {
      content: string;
      onOk: () => void;
    };
    expect(options.content).toBe('请注意，变更订阅会导致当前订阅被新订阅覆盖。');

    await act(async () => {
      options.onOk();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveOrder).toHaveBeenCalledWith(expect.anything(), {
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE789');
  });

  it('skips the change-subscription warning when the old subscription is expired', async () => {
    mocks.info = { plan_id: 2 };
    mocks.subscribe = { expired_at: 1 };
    mocks.saveOrder.mockResolvedValue('TRADE999');

    await act(async () => {
      root.render(<PlanCheckoutPage />);
      await Promise.resolve();
    });

    const submitButton = [...container.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('下单'),
    )!;

    await act(async () => {
      submitButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).not.toHaveBeenCalled();
    expect(mocks.saveOrder).toHaveBeenCalledWith(expect.anything(), {
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE999');
  });
});
