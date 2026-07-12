import type { ComponentProps } from 'react';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Plan } from '@v2board/types';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import PlanCheckoutPage from './checkout';

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  invalidateQueries: vi.fn(),
  removeQueries: vi.fn(),
  confirmDialog: vi.fn(),
  usePlan: vi.fn(),
  checkCoupon: vi.fn(),
  saveOrder: vi.fn(),
  cancelOrder: vi.fn(),
  refetchOrders: vi.fn(),
  refetchPlan: vi.fn(),
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
  ordersIsError: false,
  ordersPending: false,
  planIsError: false,
  planPending: false,
  planFetching: false,
}));

const labels: Record<string, string> = {
  'common.attention': '注意',
  'common.loading': '加载中...',
  'common.error_title': '加载失败',
  'common.retry': '重试',
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
  'errors.This payment period cannot be purchased, please choose another period':
    '该订阅周期无法进行购买，请选择其它周期',
  'errors.Coupon failed': '优惠券使用失败',
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
  mocks.planIsError = false;
  mocks.planPending = false;
  mocks.planFetching = false;
  mocks.ordersIsError = false;
  mocks.ordersPending = false;
}

vi.mock('react-router', () => ({
  Link: ({ to, onClick, children, ...rest }: { to: string } & Omit<ComponentProps<'a'>, 'href'>) => (
    <a
      href={to}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) {
          event.preventDefault();
          mocks.navigate(to);
        }
      }}
      {...rest}
    >
      {children}
    </a>
  ),
  useParams: () => ({ plan_id: mocks.planId }),
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation(labels),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    invalidateQueries: mocks.invalidateQueries,
    removeQueries: mocks.removeQueries,
  }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({
  confirmDialog: mocks.confirmDialog,
}));

vi.mock('@/lib/queries', () => ({
  usePlan: mocks.usePlan.mockImplementation(() => ({
    data: mocks.planPending || mocks.planIsError ? undefined : mocks.plan,
    isError: mocks.planIsError,
    isPending: mocks.planPending,
    isFetching: mocks.planPending || mocks.planFetching,
    refetch: mocks.refetchPlan,
  })),
  useCommConfig: () => ({
    data: {
      currency: 'CNY',
      currency_symbol: '¥',
    },
  }),
  useOrders: () => ({
    data: mocks.ordersPending || mocks.ordersIsError ? undefined : mocks.orders,
    isError: mocks.ordersIsError,
    isPending: mocks.ordersPending,
    isSuccess: !mocks.ordersPending && !mocks.ordersIsError,
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
  useCheckCouponMutation: () => ({
    isPending: false,
    mutate: (
      payload: unknown,
      options?: { onError?: (error: unknown) => void; onSuccess?: (data: unknown) => void },
    ) => {
      void Promise.resolve(mocks.checkCoupon(payload)).then(options?.onSuccess, options?.onError);
    },
  }),
  useSaveOrderMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveOrder(payload)).then(options?.onSuccess, () => undefined);
    },
  }),
}));

beforeEach(() => {
  resetPlan();
  mocks.navigate.mockClear();
  mocks.invalidateQueries.mockClear();
  mocks.removeQueries.mockClear();
  mocks.confirmDialog.mockClear();
  mocks.usePlan.mockClear();
  mocks.checkCoupon.mockReset();
  mocks.saveOrder.mockReset();
  mocks.cancelOrder.mockReset();
  mocks.refetchOrders.mockClear();
  mocks.refetchPlan.mockClear();
  mocks.info = { plan_id: 1 };
  mocks.subscribe = { expired_at: 4_102_444_800 };
  mocks.orders = [];
});

describe('PlanCheckoutPage rendering', () => {
  it('renders the cashier shell, default period, coupon input, and summary card', () => {
    const { container } = renderWithProviders(<PlanCheckoutPage />);

    // #cashier is a Tier-1 interaction-parity hook.
    expect(container.querySelector('#cashier')).toBeInTheDocument();

    // The route plan id feeds the plan query untouched.
    expect(mocks.usePlan).toHaveBeenCalledWith('1');

    // Plan feature content renders with the supported check icon. The icon has
    // no accessible name, so the lucide class is the only queryable hook.
    expect(screen.getByText('Feature A')).toBeInTheDocument();
    expect(container.querySelector('.lucide-check')).toBeInTheDocument();

    // Every priced period is selectable; the first priced one is pre-checked.
    expect(screen.getByText('付款周期')).toBeInTheDocument();
    expect(screen.getAllByTestId('checkout-period-option')).toHaveLength(3);
    const monthOption = screen.getByRole('radio', { name: /月付/, checked: true });
    // The parity harness selects [data-testid="checkout-period-option"][data-state="checked"].
    expect(monthOption).toHaveAttribute('data-testid', 'checkout-period-option');
    expect(monthOption).toHaveAttribute('data-state', 'checked');
    expect(monthOption).toHaveTextContent('¥10.00');

    const summary = screen.getByTestId('checkout-summary');
    expect(summary).toHaveTextContent('Legacy Plan x 月付');
    expect(summary).toHaveTextContent('¥10.00');
    expect(summary).toHaveTextContent('¥ 10.00 CNY');

    // coupon-input and commerce-submit are Tier-1 hooks (AGENTS.md).
    expect(screen.getByPlaceholderText('有优惠券？')).toHaveAttribute(
      'data-testid',
      'coupon-input',
    );
    expect(screen.getByTestId('commerce-submit')).toBeInTheDocument();
  });

  it('renders the non-renewable branch and routes back to the plan list', async () => {
    mocks.plan.renew = 0;
    mocks.info = { plan_id: 1 };

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    const card = screen.getByTestId('plan-non-renewable');
    expect(card).toHaveTextContent('该订阅无法续费，仅允许新用户购买');

    await user.click(within(card).getByRole('link', { name: '选择其它订阅' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/plan');
  });

  it('renders markdown/html plan content through the direct content handoff', () => {
    mocks.plan.content = '<p>HTML plan body</p>';

    renderWithProviders(<PlanCheckoutPage />);

    expect(screen.getByText('HTML plan body')).toBeInTheDocument();
  });

  it('keeps reset_price as the old default period without rendering it as a selectable period', () => {
    const plan = mocks.plan as Plan;
    plan.month_price = null;
    plan.year_price = null;
    plan.onetime_price = null;
    plan.reset_price = 300;

    renderWithProviders(<PlanCheckoutPage />);

    const summary = screen.getByTestId('checkout-summary');
    expect(summary).toHaveTextContent('Legacy Plan x 流量重置包');
    expect(summary).toHaveTextContent('¥3.00');
    expect(summary).toHaveTextContent('¥ 3.00 CNY');
    expect(screen.queryByTestId('checkout-period-option')).not.toBeInTheDocument();
  });

  it('selects a period without remounting it and submits the selected period', async () => {
    mocks.saveOrder.mockResolvedValue('TRADE-YEAR');
    const { user } = renderWithProviders(<PlanCheckoutPage />);

    const yearOption = screen.getByRole('radio', { name: /年付/ });
    await user.click(yearOption);

    expect(yearOption).toHaveAttribute('data-state', 'checked');
    // Stable keys: the clicked option must survive the re-render (a random key
    // would remount the node and drop focus).
    expect(yearOption).toHaveFocus();
    const summary = screen.getByTestId('checkout-summary');
    expect(summary).toHaveTextContent('Legacy Plan x 年付');
    expect(summary).toHaveTextContent('¥ 90.00 CNY');

    await user.click(screen.getByTestId('commerce-submit'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE-YEAR'));
    expect(mocks.saveOrder).toHaveBeenCalledWith({ plan_id: 1, period: 'year_price' });
  });

  it('lets TanStack Query retain checkout cache instead of clearing it on unmount', () => {
    const { unmount } = renderWithProviders(<PlanCheckoutPage />);

    unmount();

    expect(mocks.invalidateQueries).not.toHaveBeenCalled();
    expect(mocks.removeQueries).not.toHaveBeenCalled();
  });
});

describe('PlanCheckoutPage query states', () => {
  it('shows the full-page spinner only while the initial plan load is pending', () => {
    mocks.planPending = true;

    renderWithProviders(<PlanCheckoutPage />);

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByTestId('checkout-page')).not.toBeInTheDocument();
  });

  it('keeps rendering cached plan data during a background refetch instead of blanking to a spinner', () => {
    mocks.planFetching = true;

    const { container } = renderWithProviders(<PlanCheckoutPage />);

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(container.querySelector('#cashier')).toBeInTheDocument();
    expect(screen.getByTestId('checkout-summary')).toHaveTextContent('¥ 10.00 CNY');
  });

  it('surfaces a failed plan fetch as a retryable error state instead of a NaN cashier or redirect', async () => {
    mocks.planIsError = true;

    const { user, container } = renderWithProviders(<PlanCheckoutPage />);

    expect(screen.getByTestId('checkout-error')).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(container.querySelector('#cashier')).not.toBeInTheDocument();
    expect(mocks.navigate).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('error-state-retry'));

    expect(mocks.refetchPlan).toHaveBeenCalledTimes(1);
  });

  it('waits for the unfinished-orders query and guards the action behind the disabled button', () => {
    mocks.ordersPending = true;

    renderWithProviders(<PlanCheckoutPage />);

    expect(screen.getByTestId('unfinished-orders-loading')).toHaveTextContent('加载中...');
    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeDisabled();

    // Exercise the controller guard independently from the disabled UI.
    submit.removeAttribute('disabled');
    fireEvent.click(submit);

    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(mocks.cancelOrder).not.toHaveBeenCalled();
    expect(mocks.confirmDialog).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('fails closed on an unfinished-orders error and retries without creating an order', async () => {
    mocks.ordersIsError = true;

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    expect(screen.getByTestId('unfinished-orders-error')).toBeInTheDocument();
    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeDisabled();

    submit.removeAttribute('disabled');
    fireEvent.click(submit);
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(mocks.confirmDialog).not.toHaveBeenCalled();

    await user.click(
      within(screen.getByTestId('unfinished-orders-error')).getByTestId('error-state-retry'),
    );

    expect(mocks.refetchOrders).toHaveBeenCalledTimes(1);
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(mocks.cancelOrder).not.toHaveBeenCalled();
  });

  it('rechecks the latest orders state before a delayed confirmation action', async () => {
    mocks.info = { plan_id: 2 };
    mocks.saveOrder.mockResolvedValue('MUST-NOT-SAVE');

    const { user, rerender } = renderWithProviders(<PlanCheckoutPage />);

    await user.click(screen.getByTestId('commerce-submit'));
    const changeOptions = mocks.confirmDialog.mock.calls[0]![0] as {
      onConfirm: () => Promise<void>;
    };

    // The request can fail while the async confirmation UI remains open. Its
    // old callback must not retain permission from the earlier success render.
    mocks.ordersIsError = true;
    rerender(<PlanCheckoutPage />);
    await changeOptions.onConfirm();

    expect(screen.getByTestId('unfinished-orders-error')).toBeInTheDocument();
    expect(mocks.confirmDialog).toHaveBeenCalledTimes(1);
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(mocks.cancelOrder).not.toHaveBeenCalled();
  });
});

describe('PlanCheckoutPage commerce behavior', () => {
  it('creates a new order only after a successful empty unfinished-orders result', async () => {
    mocks.orders = [];
    mocks.saveOrder.mockResolvedValue('TRADE-EMPTY');

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE-EMPTY'));
    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.cancelOrder).not.toHaveBeenCalled();
    expect(mocks.confirmDialog).not.toHaveBeenCalled();
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

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.type(screen.getByTestId('coupon-input'), 'SAVE');
    await user.click(screen.getByRole('button', { name: '验证' }));

    expect(mocks.checkCoupon).toHaveBeenCalledWith({ code: 'SAVE', planId: '1' });
    const summary = screen.getByTestId('checkout-summary');
    expect(await within(summary).findByText('Legacy Coupon')).toBeInTheDocument();
    expect(summary).toHaveTextContent('-¥2.50');
    expect(summary).toHaveTextContent('¥ 7.50 CNY');

    await user.click(screen.getByTestId('commerce-submit'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE123'));
    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 1,
      period: 'month_price',
      coupon_code: 'SAVE',
    });
  });

  it('drops a previously applied coupon when a re-verify fails, so no stale discount is submitted', async () => {
    mocks.checkCoupon
      .mockResolvedValueOnce({
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
      })
      .mockRejectedValueOnce(new Error('invalid'));
    mocks.saveOrder.mockResolvedValue('TRADE-NO-COUPON');

    const { user } = renderWithProviders(<PlanCheckoutPage />);
    const couponInput = screen.getByTestId('coupon-input');
    const verifyButton = screen.getByRole('button', { name: '验证' });
    const summary = screen.getByTestId('checkout-summary');

    // Apply a valid coupon: the discount and reduced total appear.
    await user.type(couponInput, 'SAVE');
    await user.click(verifyButton);
    expect(await within(summary).findByText('Legacy Coupon')).toBeInTheDocument();
    expect(summary).toHaveTextContent('¥ 7.50 CNY');

    // Re-verify a different, now-invalid code: the previously applied coupon
    // must be dropped rather than left silently in place.
    await user.clear(couponInput);
    await user.type(couponInput, 'BAD');
    await user.click(verifyButton);
    expect(mocks.checkCoupon).toHaveBeenLastCalledWith({ code: 'BAD', planId: '1' });
    await waitFor(() =>
      expect(within(summary).queryByText('Legacy Coupon')).not.toBeInTheDocument(),
    );
    expect(summary).toHaveTextContent('¥ 10.00 CNY');

    // The order is then placed with no stale coupon_code.
    await user.click(screen.getByTestId('commerce-submit'));
    await waitFor(() =>
      expect(mocks.saveOrder).toHaveBeenCalledWith({ plan_id: 1, period: 'month_price' }),
    );
  });

  it('blocks checkout with an explicit error when no purchasable period exists', async () => {
    const plan = mocks.plan as Plan;
    plan.month_price = null;
    plan.year_price = null;
    plan.onetime_price = null;
    plan.reset_price = null;
    renderWithProviders(<PlanCheckoutPage />);

    const summary = screen.getByTestId('checkout-summary');
    expect(summary).toHaveTextContent('Legacy Plan x');
    expect(summary).not.toHaveTextContent('NaN');
    expect(screen.getByTestId('checkout-validation-error')).toHaveTextContent(
      '该订阅周期无法进行购买，请选择其它周期',
    );
    expect(screen.getByTestId('commerce-submit')).toBeDisabled();
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('blocks checkout instead of rendering NaN for an unknown coupon type', async () => {
    mocks.checkCoupon.mockResolvedValue({
      id: 1,
      code: 'UNKNOWN',
      name: 'Unknown Coupon',
      type: 99,
      value: 25,
    });

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.type(screen.getByTestId('coupon-input'), 'UNKNOWN');
    await user.click(screen.getByRole('button', { name: '验证' }));

    const summary = screen.getByTestId('checkout-summary');
    expect(await within(summary).findByText('Unknown Coupon')).toBeInTheDocument();
    expect(summary).not.toHaveTextContent('NaN');
    expect(screen.getByTestId('checkout-validation-error')).toHaveTextContent('优惠券使用失败');
    expect(screen.getByTestId('commerce-submit')).toBeDisabled();
    expect(mocks.saveOrder).not.toHaveBeenCalled();
  });

  it('confirms and cancels the first unfinished order before creating a new one', async () => {
    mocks.orders = [{ trade_no: 'PENDING123', status: 0 }];
    mocks.cancelOrder.mockResolvedValue(true);
    mocks.saveOrder.mockResolvedValue('TRADE456');

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    expect(mocks.confirmDialog).toHaveBeenCalledTimes(1);
    const options = mocks.confirmDialog.mock.calls[0]![0] as {
      description: string;
      confirmText: string;
      cancelText: string;
      onConfirm: () => Promise<void>;
      onCancel: () => void;
    };
    expect(options.description).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );
    expect(options.confirmText).toBe('确定取消');
    expect(options.cancelText).toBe('返回我的订单');

    options.onCancel();
    expect(mocks.navigate).toHaveBeenCalledWith('/order');

    await options.onConfirm();

    expect(mocks.cancelOrder).toHaveBeenCalledWith('PENDING123');
    // Cancelling the stale order must not trigger refetch/invalidation churn.
    expect(mocks.refetchOrders).not.toHaveBeenCalled();
    expect(mocks.invalidateQueries).not.toHaveBeenCalled();
    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE456');
  });

  it('shows the change-subscription warning before saving', async () => {
    mocks.info = { plan_id: 2 };
    mocks.subscribe = { expired_at: 4_102_444_800 };
    mocks.saveOrder.mockResolvedValue('TRADE789');

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    expect(mocks.confirmDialog).toHaveBeenCalledTimes(1);
    const options = mocks.confirmDialog.mock.calls[0]![0] as {
      description: string;
      onConfirm: () => Promise<void>;
    };
    expect(options.description).toBe('请注意，变更订阅会导致当前订阅被新订阅覆盖。');

    await options.onConfirm();

    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE789');
  });

  it('does not let the change-subscription confirmation bypass an unfinished order', async () => {
    mocks.info = { plan_id: 2 };
    mocks.subscribe = { expired_at: 4_102_444_800 };
    mocks.orders = [{ trade_no: 'PENDING-CHANGE', status: 1 }];
    mocks.cancelOrder.mockResolvedValue(true);
    mocks.saveOrder.mockResolvedValue('TRADE-CHANGED');

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    const changeOptions = mocks.confirmDialog.mock.calls[0]![0] as {
      description: string;
      onConfirm: () => Promise<void>;
    };
    expect(changeOptions.description).toBe('请注意，变更订阅会导致当前订阅被新订阅覆盖。');

    await changeOptions.onConfirm();

    expect(mocks.confirmDialog).toHaveBeenCalledTimes(2);
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    const unfinishedOptions = mocks.confirmDialog.mock.calls[1]![0] as {
      description: string;
      onConfirm: () => Promise<void>;
    };
    expect(unfinishedOptions.description).toBe(
      '您还有未完成的订单，购买前需要先取消，确定要取消之前的订单吗？',
    );

    await unfinishedOptions.onConfirm();

    expect(mocks.cancelOrder).toHaveBeenCalledWith('PENDING-CHANGE');
    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 1,
      period: 'month_price',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE-CHANGED');
  });

  it('skips the change-subscription warning when the old subscription is expired', async () => {
    mocks.info = { plan_id: 2 };
    mocks.subscribe = { expired_at: 1 };
    mocks.saveOrder.mockResolvedValue('TRADE999');

    const { user } = renderWithProviders(<PlanCheckoutPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order/TRADE999'));
    expect(mocks.confirmDialog).not.toHaveBeenCalled();
    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 1,
      period: 'month_price',
    });
  });
});
