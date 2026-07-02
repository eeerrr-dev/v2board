import { act, type Ref } from 'react';
import { fireEvent, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import OrderDetailPage from './detail';

const orderRefetch = vi.hoisted(() => vi.fn());
const checkoutOrder = vi.hoisted(() => vi.fn());
const checkOrder = vi.hoisted(() => vi.fn());
const stripePublicKey = vi.hoisted(() => ({ value: undefined as string | undefined }));
const stripeTokenize = vi.hoisted(() => vi.fn());
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
  data: [{ id: 1, name: 'Legacy Pay', payment: 'LegacyPay' }] as Array<
    { id: number; name: string; payment: string } & Record<string, unknown>
  >,
}));
const orderState = vi.hoisted(() => ({
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
        pre_handling_amount?: number;
      }
    | undefined,
}));
const orderStatusState = vi.hoisted(() => ({
  data: undefined as number | undefined,
  isError: false,
  calls: [] as Array<{ tradeNo: string | undefined; options?: { enabled?: boolean } }>,
}));

vi.mock('react-router', () => ({
  useParams: () => ({ trade_no: 'ORDER123' }),
  useNavigate: () => vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => labels[key as keyof typeof labels] ?? key,
    i18n: { language: 'zh-CN' },
  }),
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

vi.mock('@/components/ui/confirm-dialog', () => ({
  confirmDialog,
}));

vi.mock('@/lib/toast', () => ({
  toast: toastSpies,
}));

// The QR value prop is what the user's payment app scans; surface it as a DOM
// attribute so the payUrl wiring stays observable (same stub as dashboard.test).
vi.mock('qrcode.react', () => ({
  QRCodeSVG: ({ value }: { value?: string }) => <svg data-qrcode={value} />,
}));

// Stub the Stripe card form at the module boundary: it exposes the same
// submit-time tokenize() handle the page consumes, reports the public key it
// was mounted with, and immediately signals a complete card so the checkout
// button un-gates.
vi.mock('@/components/stripe-card-form', async () => {
  const { useEffect, useImperativeHandle } = await import('react');
  return {
    StripeCardForm: ({
      publicKey,
      onCompleteChange,
      ref,
    }: {
      publicKey: string;
      onCompleteChange?: (complete: boolean) => void;
      ref?: Ref<{ tokenize: () => Promise<{ id: string } | null> }>;
    }) => {
      useImperativeHandle(ref, () => ({ tokenize: () => stripeTokenize() }), []);
      useEffect(() => {
        stripeMounts.count += 1;
        onCompleteChange?.(true);
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, []);
      return <div data-testid="stripe-card-form" data-public-key={publicKey} />;
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
    isPending: orderState.data === undefined,
    refetch: orderRefetch,
  }),
  usePaymentMethods: () => ({
    data: paymentState.data,
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
    mutateAsync: checkoutOrder,
    isPending: false,
  }),
  useStripePublicKey: () => ({
    data: stripePublicKey.value,
  }),
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
    checkoutOrder.mockReset();
    checkOrder.mockReset();
    orderStatusState.data = undefined;
    orderStatusState.isError = false;
    orderStatusState.calls = [];
    stripePublicKey.value = undefined;
    stripeTokenize.mockReset();
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

  it('preselects the first payment method and derives its handling fee from the method config', () => {
    paymentState.data = [
      {
        id: 9,
        name: 'Fee Pay',
        payment: 'LegacyPay',
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
    // The Radix indicator (harness hook payment-option-radio) renders on the
    // checked option only.
    expect(within(options[0]!).getByTestId('payment-option-radio')).toBeInTheDocument();

    // 1000 * 10% + 150 = 250 cents on top of the 1000-cent order.
    expect(screen.getAllByText('order.handling_fee').length).toBeGreaterThan(0);
    expect(screen.getByText('2.50')).toBeInTheDocument();
    expect(screen.getByText('¥ 12.50 CNY')).toBeInTheDocument();
  });

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

  it('prefers the server pre_handling_amount over the locally derived method fee', () => {
    paymentState.data = [
      {
        id: 9,
        name: 'Fee Pay',
        payment: 'LegacyPay',
        handling_fee_fixed: 150,
        handling_fee_percent: 10,
      },
    ];
    orderState.data = { ...orderState.data!, pre_handling_amount: 300 };

    renderWithProviders(<OrderDetailPage />);

    // Server value (300) wins over the locally computed 250.
    expect(screen.getByText('3.00')).toBeInTheDocument();
    expect(screen.queryByText('2.50')).toBeNull();
    expect(screen.getByText('¥ 13.00 CNY')).toBeInTheDocument();
  });

  it('tokenizes the card once at submit and sends the Stripe token in the checkout payload', async () => {
    paymentState.data = [{ id: 5, name: 'Stripe Pay', payment: 'StripeCredit' }];
    stripePublicKey.value = 'pk_test_live';
    stripeTokenize.mockResolvedValue({ id: 'tok_test_123' });
    checkoutOrder.mockResolvedValue({ type: 1, data: undefined });

    const { user } = renderWithProviders(<OrderDetailPage />);

    // The card form mounts with the fetched public key and no eager tokenization.
    expect(screen.getByTestId('stripe-card-form')).toHaveAttribute(
      'data-public-key',
      'pk_test_live',
    );
    expect(stripeTokenize).not.toHaveBeenCalled();

    const submit = screen.getByTestId('commerce-submit');
    expect(submit).toBeEnabled();
    await user.click(submit);
    await flushCheckout();

    expect(stripeTokenize).toHaveBeenCalledTimes(1);
    // Tier-1: the Stripe card token rides the /payment checkout payload.
    expect(checkoutOrder).toHaveBeenCalledWith({
      trade_no: 'ORDER123',
      method: 5,
      token: 'tok_test_123',
    });
    // The verification message stays behind i18n (key, not hardcoded Chinese).
    expect(toastSpies.loading).toHaveBeenCalledWith('order.stripe_verifying', { duration: 5000 });
    expect(screen.queryByTestId('payment-qrcode')).toBeNull();
  });

  it('remounts the Stripe card form when the public key changes', () => {
    paymentState.data = [{ id: 5, name: 'Stripe Pay', payment: 'StripeCredit' }];
    stripePublicKey.value = 'pk_a';

    const { rerender } = renderWithProviders(<OrderDetailPage />);
    expect(stripeMounts.count).toBe(1);

    stripePublicKey.value = 'pk_b';
    rerender(<OrderDetailPage />);

    // A new key must recreate the Stripe Elements instance, not update in place.
    expect(screen.getByTestId('stripe-card-form')).toHaveAttribute('data-public-key', 'pk_b');
    expect(stripeMounts.count).toBe(2);
  });

  it('opens an accessible QR dialog encoding the checkout pay URL', async () => {
    checkoutOrder.mockResolvedValue({ type: 0, data: 'https://pay.example.test/order' });

    const { user } = renderWithProviders(<OrderDetailPage />);

    await user.click(screen.getByTestId('commerce-submit'));

    // Tier-1: the non-Stripe checkout payload carries the trade_no and method.
    expect(checkoutOrder).toHaveBeenCalledWith({
      trade_no: 'ORDER123',
      method: 1,
      token: undefined,
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
    checkoutOrder.mockResolvedValue({ type: 0, data: 'https://pay.example.test/order' });
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

  it('settles a free / balance-covered order (type -1) instead of falling through silently', async () => {
    // total_amount <= 0 orders settle server-side with no gateway, so checkout
    // returns type -1 with no QR/redirect. onPay must still refresh the order
    // plus the balance (info) and subscription (subscribe) it just consumed.
    checkoutOrder.mockResolvedValue({ type: -1, data: undefined });

    const { user } = renderWithProviders(<OrderDetailPage />);

    await user.click(screen.getByTestId('commerce-submit'));
    await flushCheckout();

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
