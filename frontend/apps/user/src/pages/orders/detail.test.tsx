import { act, type ComponentProps, type Ref } from 'react';
import { fireEvent, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import OrderDetailPage from './detail';

const orderRefetch = vi.hoisted(() => vi.fn());
const paymentRefetch = vi.hoisted(() => vi.fn());
const checkoutOrder = vi.hoisted(() => vi.fn());
const checkOrder = vi.hoisted(() => vi.fn());
const stripeIntent = vi.hoisted(() => ({
  value: undefined as
    { public_key: string; client_secret: string; amount: number; currency: string } | undefined,
}));
const stripeConfirm = vi.hoisted(() => vi.fn());
const stripeIntentRefetch = vi.hoisted(() => vi.fn());
const stripeIntentCalls = vi.hoisted(
  () =>
    [] as Array<{
      tradeNo: string | undefined;
      methodId: number | undefined;
      options?: { enabled?: boolean };
    }>,
);
const stripeMounts = vi.hoisted(() => ({ count: 0 }));
const confirmDialog = vi.hoisted(() => vi.fn());
const cancelMutateAsync = vi.hoisted(() => vi.fn());
const cancelState = vi.hoisted(() => ({ isPending: false }));
const invalidateQueries = vi.hoisted(() => vi.fn());
const toastSpies = vi.hoisted(() => ({
  error: vi.fn(),
  info: vi.fn(),
  loading: vi.fn(),
  success: vi.fn(),
}));
const labels = vi.hoisted(() => ({
  'order.deposit': '充值',
}));
const paymentState = vi.hoisted(() => ({
  data: [{ id: 1, name: 'Legacy Pay', payment: 'LegacyPay' }] as
    Array<{ id: number; name: string; payment: string } & Record<string, unknown>> | undefined,
  isPending: false,
  error: null as Error | null,
}));
const orderState = vi.hoisted(() => ({
  error: null as Error | null,
  data: {
    trade_no: 'ORDER123',
    period: 'month_price',
    total_amount: 1000,
    discount_amount: null,
    surplus_amount: null,
    refund_amount: null,
    balance_amount: null,
    status: 0,
    created_at: 1_700_000_000,
    plan: {
      id: 1,
      name: 'Legacy Plan',
      transfer_enable: 123,
      month_price: 1000,
    },
  } as
    | {
        trade_no: string;
        period: string;
        total_amount: number;
        discount_amount: null;
        surplus_amount: null;
        refund_amount: null;
        balance_amount: null;
        status: number;
        created_at: number;
        plan: {
          id: number;
          name: string;
          transfer_enable: number;
          month_price: number;
        };
      }
    | undefined,
}));
const orderStatusState = vi.hoisted(() => ({
  data: undefined as number | undefined,
  isError: false,
  calls: [] as Array<{ tradeNo: string | undefined; options?: { enabled?: boolean } }>,
}));

vi.mock('react-router', () => ({
  Link: ({ to, children, ...rest }: { to: string } & Omit<ComponentProps<'a'>, 'href'>) => (
    <a href={to} {...rest}>
      {children}
    </a>
  ),
  useParams: () => ({ trade_no: 'ORDER123' }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation(labels),
}));

vi.mock('@tanstack/react-query', () => {
  // Return a stable client reference so the page's useCallback memoizes the way
  // a real QueryClient would; a fresh object each render would defeat that and
  // make the payment-refresh effect re-run on every render. The client only
  // exposes invalidateQueries: any cache teardown call (removeQueries,
  // cancelQueries, ...) would throw and fail the unmount test below.
  const client = { invalidateQueries };
  return { useQueryClient: () => client };
});

vi.mock('@v2board/ui/confirm-dialog', () => ({
  confirmDialog,
}));

vi.mock('@v2board/app-shell/toast', () => ({
  toast: toastSpies,
}));

// The QR value prop is what the user's payment app scans; surface it as a DOM
// attribute so the payUrl wiring stays observable (same stub as dashboard.test).
vi.mock('qrcode.react', () => ({
  QRCodeSVG: ({ value }: { value?: string }) => <svg data-qrcode={value} />,
}));

vi.mock('@/components/stripe-payment-form', async () => {
  const { useEffect, useImperativeHandle } = await import('react');
  return {
    StripePaymentForm: ({
      publicKey,
      clientSecret,
      onCompleteChange,
      ref,
    }: {
      publicKey: string;
      clientSecret: string;
      onCompleteChange?: (complete: boolean) => void;
      ref?: Ref<{ confirm: () => Promise<{ status?: 'succeeded'; error?: string }> }>;
    }) => {
      useImperativeHandle(ref, () => ({ confirm: () => stripeConfirm() }), []);
      useEffect(() => {
        stripeMounts.count += 1;
        onCompleteChange?.(true);
      }, [onCompleteChange]);
      return (
        <div
          data-testid="stripe-payment-form"
          data-public-key={publicKey}
          data-client-secret={clientSecret}
        />
      );
    },
  };
});

vi.mock('@/lib/queries', () => ({
  userKeys: {
    info: ['user', 'info'],
    subscribe: ['user', 'subscribe'],
    orders: () => ['user', 'orders', 'all'],
    orderDetail: (tradeNo: string) => ['user', 'orders', 'detail', tradeNo],
    payments: ['user', 'payments'],
  },
  useOrder: () => ({
    data: orderState.data,
    // The page now gates its full-page spinner on isPending (no data yet), not
    // isFetching, so a background refetch no longer blanks the cashier.
    error: orderState.error,
    isError: orderState.error !== null,
    isPending: orderState.data === undefined && orderState.error === null,
    refetch: orderRefetch,
  }),
  usePaymentMethods: () => ({
    data: paymentState.data,
    isPending: paymentState.isPending,
    error: paymentState.error,
    refetch: paymentRefetch,
  }),
  useCommConfig: () => ({
    data: {
      currency: 'CNY',
      currency_symbol: '¥',
    },
  }),
  useCancelOrderMutation: () => ({
    isPending: cancelState.isPending,
    mutateAsync: cancelMutateAsync,
  }),
  useOrderStatus: (tradeNo: string | undefined, options?: { enabled?: boolean }) => {
    orderStatusState.calls.push({ tradeNo, options });
    if (options?.enabled) checkOrder(tradeNo);
    return {
      data: options?.enabled ? orderStatusState.data : undefined,
      isError: Boolean(options?.enabled && orderStatusState.isError),
    };
  },
  useCheckoutOrderMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(checkoutOrder(payload)).then(options?.onSuccess, () => undefined);
    },
    isPending: false,
  }),
  useStripePaymentIntent: (
    tradeNo: string | undefined,
    methodId: number | undefined,
    options?: { enabled?: boolean },
  ) => {
    stripeIntentCalls.push({ tradeNo, methodId, options });
    return {
      data: stripeIntent.value,
      isPending: false,
      isFetching: false,
      error: null,
      refetch: stripeIntentRefetch,
    };
  },
  useUserInfo: () => ({}),
}));

/** Flush the microtask chain of an in-flight onPay inside act. */
async function flushCheckout() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('OrderDetailPage shadcn commerce behavior', () => {
  beforeEach(() => {
    orderRefetch.mockReset();
    paymentRefetch.mockReset();
    checkoutOrder.mockReset();
    checkOrder.mockReset();
    orderStatusState.data = undefined;
    orderStatusState.isError = false;
    orderStatusState.calls = [];
    stripeIntent.value = undefined;
    stripeConfirm.mockReset();
    stripeIntentRefetch.mockReset();
    stripeIntentCalls.length = 0;
    stripeMounts.count = 0;
    confirmDialog.mockReset();
    cancelMutateAsync.mockReset();
    cancelMutateAsync.mockResolvedValue(true);
    cancelState.isPending = false;
    invalidateQueries.mockReset();
    toastSpies.error.mockReset();
    toastSpies.info.mockReset();
    toastSpies.loading.mockReset();
    toastSpies.success.mockReset();
    paymentState.data = [{ id: 1, name: 'Legacy Pay', payment: 'LegacyPay' }];
    paymentState.isPending = false;
    paymentState.error = null;
    orderState.error = null;
    orderState.data = {
      trade_no: 'ORDER123',
      period: 'month_price',
      total_amount: 1000,
      discount_amount: null,
      surplus_amount: null,
      refund_amount: null,
      balance_amount: null,
      status: 0,
      created_at: 1_700_000_000,
      plan: {
        id: 1,
        name: 'Legacy Plan',
        transfer_enable: 123,
        month_price: 1000,
      },
    };
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders the product-info rows and the commerce hooks for a pending non-deposit order', () => {
    const { container } = renderWithProviders(<OrderDetailPage />);

    // Interaction/visual parity selects these hooks; keep them stable.
    expect(container.querySelector('#cashier')).not.toBeNull();
    expect(screen.getAllByTestId('order-info')).toHaveLength(2);
    expect(screen.getAllByTestId('order-info-title').map((el) => el.textContent)).toEqual([
      'order.product_info',
      'order.info',
    ]);
    expect(screen.getByTestId('order-summary')).toBeInTheDocument();
    expect(screen.getByTestId('commerce-submit')).toHaveTextContent('order.checkout');

    expect(screen.getByText('order.product_name')).toBeInTheDocument();
    expect(screen.getByText('Legacy Plan')).toBeInTheDocument();
    expect(screen.getByText('order.product_period')).toBeInTheDocument();
    expect(screen.getByText('plan.monthly')).toBeInTheDocument();
    expect(screen.getByText('order.product_traffic')).toBeInTheDocument();
    expect(screen.getByText('123 GB')).toBeInTheDocument();
  });

  it('keeps the deposit product-name row', () => {
    orderState.data = {
      ...orderState.data!,
      period: 'deposit',
      plan: {
        id: 0,
        name: 'deposit',
        transfer_enable: undefined as unknown as number,
        month_price: 0,
      },
    };

    renderWithProviders(<OrderDetailPage />);

    expect(screen.getByText('order.product_name')).toBeInTheDocument();
    expect(screen.getByText('充值')).toBeInTheDocument();
    expect(screen.queryByText('order.product_period')).toBeNull();
    expect(screen.queryByText('order.product_traffic')).toBeNull();
    expect(screen.queryByText(/GB/)).toBeNull();
  });

  it('renders an empty period value for an unknown period key instead of a raw i18n fallback', () => {
    orderState.data = { ...orderState.data!, period: 'mystery_price' };

    renderWithProviders(<OrderDetailPage />);

    const periodLabel = screen.getByText('order.product_period');
    expect(periodLabel.nextElementSibling).toBeEmptyDOMElement();
  });

  it('shows the centered detail spinner while the order detail fetch is pending', () => {
    orderState.data = undefined;

    const { container } = renderWithProviders(<OrderDetailPage />);

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(container.querySelector('#cashier')).toBeNull();
  });

  it('shows a retryable error without calculating or exposing checkout before an order exists', async () => {
    orderState.data = undefined;
    orderState.error = new Error('Order detail failed');

    const { user, container } = renderWithProviders(<OrderDetailPage />);

    expect(screen.getByTestId('order-detail-error')).toHaveTextContent('Order detail failed');
    expect(container.querySelector('#cashier')).toBeNull();
    expect(screen.queryByTestId('order-summary')).toBeNull();
    expect(screen.queryByTestId('commerce-submit')).toBeNull();
    expect(checkoutOrder).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('error-state-retry'));

    expect(orderRefetch).toHaveBeenCalledTimes(1);
  });

  it('shows payment-method loading explicitly and cannot check out before a method is selected', () => {
    paymentState.data = undefined;
    paymentState.isPending = true;

    renderWithProviders(<OrderDetailPage />);

    expect(screen.getByTestId('payment-methods-loading')).toHaveAttribute('role', 'status');
    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeDisabled();
    fireEvent.click(submit);

    expect(checkoutOrder).not.toHaveBeenCalled();
    expect(stripeIntentCalls.at(-1)).toEqual({
      tradeNo: 'ORDER123',
      methodId: undefined,
      options: { enabled: false },
    });
  });

  it('surfaces a retryable payment-method fetch error and keeps checkout guarded', async () => {
    // TanStack Query retains the last successful data when a refetch fails. The
    // failure must still win over that stale gateway and prevent a submission.
    paymentState.error = new Error('Payment methods failed');

    const { user } = renderWithProviders(<OrderDetailPage />);

    expect(screen.getByTestId('payment-methods-error')).toHaveTextContent('Payment methods failed');
    expect(screen.queryByTestId('payment-option')).toBeNull();
    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeDisabled();
    fireEvent.click(submit);
    expect(checkoutOrder).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('error-state-retry'));
    expect(paymentRefetch).toHaveBeenCalledTimes(1);
  });

  it('renders an explicit empty state and never prepares or submits an invalid method', () => {
    paymentState.data = [];

    renderWithProviders(<OrderDetailPage />);

    expect(screen.getByTestId('payment-methods-empty')).toHaveTextContent('common.empty');
    expect(screen.queryByTestId('payment-option')).toBeNull();
    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeDisabled();
    fireEvent.click(submit);

    expect(checkoutOrder).not.toHaveBeenCalled();
    expect(stripeIntentCalls.at(-1)).toEqual({
      tradeNo: 'ORDER123',
      methodId: undefined,
      options: { enabled: false },
    });
  });

  it('preselects the first payment method and derives its handling fee from the method config', () => {
    paymentState.data = [
      {
        id: 9,
        name: 'Fee Pay',
        payment: 'LegacyPay',
        icon: 'https://cdn.example.test/fee-pay.svg',
        handling_fee_fixed: 150,
        handling_fee_percent: 10,
      },
      { id: 10, name: 'Backup Pay', payment: 'LegacyPay' },
    ];

    renderWithProviders(<OrderDetailPage />);

    const options = screen.getAllByTestId('payment-option');
    expect(options).toHaveLength(2);
    // The parity harness selects [data-state="checked"] on payment-option.
    expect(options[0]).toHaveAttribute('data-state', 'checked');
    expect(options[0]).toHaveTextContent('Fee Pay');
    expect(options[1]).toHaveAttribute('data-state', 'unchecked');
    const paymentIcon = options[0]!.querySelector('img');
    expect(paymentIcon).toHaveAttribute('src', 'https://cdn.example.test/fee-pay.svg');
    expect(paymentIcon).toHaveAttribute('loading', 'lazy');
    expect(paymentIcon).toHaveAttribute('decoding', 'async');
    // The Radix indicator (harness hook payment-option-radio) renders on the
    // checked option only.
    expect(within(options[0]!).getByTestId('payment-option-radio')).toBeInTheDocument();

    // 1000 * 10% + 150 = 250 cents on top of the 1000-cent order.
    expect(screen.getAllByText('order.handling_fee').length).toBeGreaterThan(0);
    expect(screen.getByText('2.50')).toBeInTheDocument();
    expect(screen.getByText('¥ 12.50 CNY')).toBeInTheDocument();
  });

  it.each([
    ['fixed-only', { handling_fee_fixed: 150 }, '1.50', '¥ 11.50 CNY'],
    ['percent-only', { handling_fee_percent: 10 }, '1.00', '¥ 11.00 CNY'],
  ])(
    'derives a %s handling fee without relying on nullable numeric coercion',
    (_label, feeConfig, expectedFee, expectedTotal) => {
      paymentState.data = [{ id: 9, name: 'Fee Pay', payment: 'LegacyPay', ...feeConfig }];

      renderWithProviders(<OrderDetailPage />);

      expect(screen.getByText(expectedFee)).toBeInTheDocument();
      expect(screen.getByText(expectedTotal)).toBeInTheDocument();
    },
  );

  it('updates the handling fee when the payment method changes', async () => {
    paymentState.data = [
      {
        id: 9,
        name: 'Fee Pay',
        payment: 'LegacyPay',
        handling_fee_fixed: 150,
        handling_fee_percent: 10,
      },
      {
        id: 10,
        name: 'Backup Pay',
        payment: 'LegacyPay',
        handling_fee_fixed: 0,
        handling_fee_percent: 0,
      },
    ];

    const { user } = renderWithProviders(<OrderDetailPage />);

    expect(screen.getAllByText('order.handling_fee').length).toBeGreaterThan(0);
    expect(screen.getByText('¥ 12.50 CNY')).toBeInTheDocument();

    await user.click(screen.getByRole('radio', { name: 'Backup Pay' }));

    expect(screen.getByRole('radio', { name: 'Backup Pay' })).toHaveAttribute(
      'data-state',
      'checked',
    );
    expect(screen.queryByText('order.handling_fee')).toBeNull();
    expect(screen.getByText('¥ 10.00 CNY')).toBeInTheDocument();
  });

  it('confirms the server-owned PaymentIntent without sending a legacy token', async () => {
    paymentState.data = [{ id: 5, name: 'Stripe Pay', payment: 'StripeCredit' }];
    stripeIntent.value = {
      public_key: 'pk_test_live',
      client_secret: 'pi_test_secret_123',
      amount: 1000,
      currency: 'cny',
    };
    stripeConfirm.mockResolvedValue({ status: 'succeeded' });

    const { user } = renderWithProviders(<OrderDetailPage />);

    expect(screen.getByTestId('stripe-payment-form')).toHaveAttribute(
      'data-public-key',
      'pk_test_live',
    );
    expect(screen.getByTestId('stripe-payment-form')).toHaveAttribute(
      'data-client-secret',
      'pi_test_secret_123',
    );
    expect(stripeIntentCalls.at(-1)).toEqual({
      tradeNo: 'ORDER123',
      methodId: 5,
      options: { enabled: true },
    });
    expect(stripeConfirm).not.toHaveBeenCalled();

    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeEnabled();
    await user.click(submit);
    await flushCheckout();

    expect(stripeConfirm).toHaveBeenCalledTimes(1);
    expect(checkoutOrder).not.toHaveBeenCalled();
    // The verification message stays behind i18n (key, not hardcoded Chinese).
    expect(toastSpies.loading).toHaveBeenCalledWith('order.stripe_verifying', { duration: 5000 });
    expect(screen.queryByTestId('payment-qrcode')).toBeNull();
  });

  it('surfaces a rejected Stripe confirmation and restores checkout controls', async () => {
    paymentState.data = [{ id: 5, name: 'Stripe Pay', payment: 'StripeCredit' }];
    stripeIntent.value = {
      public_key: 'pk_test_live',
      client_secret: 'pi_test_secret_123',
      amount: 1000,
      currency: 'cny',
    };
    stripeConfirm.mockRejectedValue(new Error('Stripe network unavailable'));

    const { user } = renderWithProviders(<OrderDetailPage />);
    const submit = screen.getByTestId('commerce-submit');
    await user.click(submit);
    await flushCheckout();

    expect(stripeConfirm).toHaveBeenCalledTimes(1);
    expect(toastSpies.error).toHaveBeenCalledWith('Stripe network unavailable');
    expect(submit).toBeEnabled();
  });

  it('remounts Stripe Elements when the PaymentIntent client secret changes', () => {
    paymentState.data = [{ id: 5, name: 'Stripe Pay', payment: 'StripeCredit' }];
    stripeIntent.value = {
      public_key: 'pk_a',
      client_secret: 'pi_a_secret',
      amount: 1000,
      currency: 'cny',
    };

    const { rerender } = renderWithProviders(<OrderDetailPage />);
    expect(stripeMounts.count).toBe(1);

    stripeIntent.value = {
      public_key: 'pk_a',
      client_secret: 'pi_b_secret',
      amount: 1000,
      currency: 'cny',
    };
    rerender(<OrderDetailPage />);

    expect(screen.getByTestId('stripe-payment-form')).toHaveAttribute(
      'data-client-secret',
      'pi_b_secret',
    );
    expect(stripeMounts.count).toBe(2);
  });

  it('opens an accessible QR dialog encoding the checkout pay URL', async () => {
    checkoutOrder.mockResolvedValue({ kind: 'qr_code', payload: 'https://pay.example.test/order' });

    const { user } = renderWithProviders(<OrderDetailPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    // Tier-1: the non-Stripe checkout payload carries the trade_no and method_id.
    expect(checkoutOrder).toHaveBeenCalledWith({
      trade_no: 'ORDER123',
      method_id: 1,
    });

    const dialog = await screen.findByRole('dialog', { name: 'order.checkout' });
    expect(dialog).toHaveAttribute('data-testid', 'payment-qrcode');
    expect(dialog).toHaveAccessibleDescription('order.waiting_pay');
    // The QR encodes the gateway payUrl the user's payment app scans.
    expect(dialog.querySelector('svg')).toHaveAttribute(
      'data-qrcode',
      'https://pay.example.test/order',
    );
    expect(within(dialog).getByTestId('payment-qrcode-status')).toHaveTextContent(
      'order.waiting_pay',
    );
    // showCloseButton={false}: waiting-pay dialog exposes no close control.
    expect(within(dialog).queryByRole('button')).toBeNull();
  });

  it('hides the QR dialog after paid polling', async () => {
    vi.useFakeTimers();
    checkoutOrder.mockResolvedValue({ kind: 'qr_code', payload: 'https://pay.example.test/order' });
    orderStatusState.data = 1;

    renderWithProviders(<OrderDetailPage />);

    // fireEvent (sync act) instead of userEvent: under vitest fake timers,
    // RTL's asyncWrapper awaits a setTimeout(0) that only Jest auto-advances.
    fireEvent.click(screen.getByTestId('commerce-submit'));
    await flushCheckout();

    expect(screen.getByTestId('payment-qrcode').querySelector('svg')).not.toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });

    expect(checkOrder).toHaveBeenCalledWith('ORDER123');
    expect(orderRefetch).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('payment-qrcode')).toBeNull();
  });

  it('settles a free / balance-covered order (kind "settled") instead of falling through silently', async () => {
    // total_amount <= 0 orders settle server-side with no gateway, so checkout
    // returns {kind:"settled"} with no QR/redirect. onPay must still refresh the
    // order plus the balance (info) and subscription (subscribe) it just consumed.
    orderState.data = { ...orderState.data!, total_amount: 0 };
    paymentState.data = [{ id: 5, name: 'Stripe Pay', payment: 'StripeCredit' }];
    checkoutOrder.mockResolvedValue({ kind: 'settled' });

    const { user } = renderWithProviders(<OrderDetailPage />);

    const submit = screen.getByTestId('commerce-submit');
    expect(screen.queryByTestId('stripe-payment-form')).toBeNull();
    expect(submit).toBeEnabled();
    await user.click(submit);
    await flushCheckout();

    expect(checkoutOrder).toHaveBeenCalledWith({ trade_no: 'ORDER123', method_id: 5 });
    expect(screen.queryByTestId('payment-qrcode')).toBeNull();
    expect(toastSpies.success).toHaveBeenCalledWith('order.success');
    expect(orderRefetch).toHaveBeenCalledTimes(1);
    const invalidatedKeys = invalidateQueries.mock.calls.map(
      (call) => (call[0] as { queryKey: readonly unknown[] }).queryKey,
    );
    expect(invalidatedKeys).toContainEqual(['user', 'info']);
    expect(invalidatedKeys).toContainEqual(['user', 'subscribe']);
  });

  it('restores the checkout button after a status-0 transport failure', async () => {
    checkoutOrder.mockRejectedValue({ status: 0, message: 'Network Error' });

    const { user } = renderWithProviders(<OrderDetailPage />);

    const submit = screen.getByTestId('commerce-submit');
    await user.click(submit);
    await flushCheckout();

    expect(submit).toBeEnabled();
    expect(submit).toHaveTextContent('order.checkout');
  });

  it('restores the checkout button after a non-transport checkout failure', async () => {
    checkoutOrder.mockRejectedValue({ status: 500, message: '支付失败' });

    const { user } = renderWithProviders(<OrderDetailPage />);

    const submit = screen.getByTestId('commerce-submit');
    await user.click(submit);
    await flushCheckout();

    expect(submit).toBeEnabled();
    expect(submit).toHaveTextContent('order.checkout');
  });

  it('drops the locally injected payment fee after the paid poll detail refresh replaces the order', () => {
    paymentState.data = [
      {
        id: 9,
        name: 'Fee Pay',
        payment: 'LegacyPay',
        handling_fee_fixed: 150,
        handling_fee_percent: 10,
      },
    ];

    const { rerender } = renderWithProviders(<OrderDetailPage />);

    expect(screen.getAllByText('order.handling_fee').length).toBeGreaterThan(0);
    expect(screen.getByText('2.50')).toBeInTheDocument();

    // The bundled poll-success refetch replaces the order detail without
    // re-running getPaymentMethod, so a paid (non-pending) order must carry no
    // locally derived fee.
    orderState.data = { ...orderState.data!, status: 3 };
    rerender(<OrderDetailPage />);

    expect(screen.queryByText('order.handling_fee')).toBeNull();
  });

  it('keeps polling after the pending order detail object refreshes', async () => {
    // This page only decides whether to poll (enabled); the 3s self-stopping
    // cadence is owned by useOrderStatus (queries.ts).
    vi.useFakeTimers();
    orderStatusState.data = 0;

    const { rerender } = renderWithProviders(<OrderDetailPage />);

    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
    });
    expect(checkOrder).toHaveBeenCalledTimes(1);
    expect(checkOrder).toHaveBeenCalledWith('ORDER123');
    expect(orderStatusState.calls.some((call) => call.options?.enabled)).toBe(true);

    orderState.data = {
      ...orderState.data!,
      created_at: orderState.data!.created_at + 1,
    };
    rerender(<OrderDetailPage />);

    expect(checkOrder).toHaveBeenCalledTimes(2);
  });

  it('cancels with the loaded trade number without invalidating or refetching the detail', async () => {
    orderState.data = { ...orderState.data!, trade_no: 'DETAIL123' };

    const { user } = renderWithProviders(<OrderDetailPage />);

    await user.click(screen.getByRole('button', { name: 'order.cancel' }));

    expect(confirmDialog).toHaveBeenCalledTimes(1);
    const confirmOptions = confirmDialog.mock.calls[0]?.[0] as { onConfirm?: () => Promise<void> };
    await confirmOptions.onConfirm?.();
    await flushCheckout();

    expect(cancelMutateAsync).toHaveBeenCalledWith('DETAIL123');
    // The cancel path itself neither invalidates nor refetches (the cancel
    // mutation owns the order-list invalidation); only payment settlement does.
    expect(invalidateQueries).not.toHaveBeenCalled();
    expect(orderRefetch).not.toHaveBeenCalled();
  });

  it('lets TanStack Query retain order detail cache on unmount', () => {
    // The mocked query client only exposes invalidateQueries, so any cache
    // teardown (removeQueries/cancelQueries) on unmount would throw here.
    const { unmount } = renderWithProviders(<OrderDetailPage />);

    unmount();

    expect(invalidateQueries).not.toHaveBeenCalled();
    expect(orderRefetch).not.toHaveBeenCalled();
  });

  it('renders cancel loading through the shadcn Button busy state', () => {
    cancelState.isPending = true;

    renderWithProviders(<OrderDetailPage />);

    const cancelButton = screen.getByRole('button', { name: 'order.cancel' });
    expect(cancelButton).toHaveAttribute('aria-busy', 'true');
    expect(cancelButton).toBeDisabled();
  });
});
