import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import OrderDetailPage from './detail';

const orderDetailSource = readFileSync(`${process.cwd()}/src/pages/orders/detail.tsx`, 'utf8');

const orderRefetch = vi.hoisted(() => vi.fn());
const checkoutOrder = vi.hoisted(() => vi.fn());
const checkOrder = vi.hoisted(() => vi.fn());
const getStripePublicKey = vi.hoisted(() => vi.fn());
const legacyConfirm = vi.hoisted(() => vi.fn());
const cancelMutateAsync = vi.hoisted(() => vi.fn());
const cancelState = vi.hoisted(() => ({ isPending: false }));
const invalidateQueries = vi.hoisted(() => vi.fn());
const removeQueries = vi.hoisted(() => vi.fn());
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
  isFetching: false,
}));

vi.mock('react-router-dom', () => ({
  useParams: () => ({ trade_no: 'ORDER123' }),
  useNavigate: () => vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'zh-CN' } }),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    invalidateQueries,
    removeQueries,
  }),
}));

vi.mock('@/lib/api', () => ({
  apiClient: {},
}));

vi.mock('@v2board/api-client', () => ({
  user: {
    checkoutOrder,
    checkOrder,
    getStripePublicKey,
  },
}));

vi.mock('@/components/legacy-confirm', () => ({
  legacyConfirm,
}));

vi.mock('@/lib/queries', () => ({
  userKeys: {
    orders: () => ['user', 'orders', 'all'],
    orderDetail: (tradeNo: string) => ['user', 'orders', 'detail', tradeNo],
    payments: ['user', 'payments'],
  },
  useOrder: () => ({
    data: orderState.data,
    isFetching: orderState.isFetching,
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
  useUserInfo: () => ({}),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('OrderDetailPage bundled-theme quirks', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    orderRefetch.mockReset();
    checkoutOrder.mockReset();
    checkOrder.mockReset();
    getStripePublicKey.mockReset();
    legacyConfirm.mockReset();
    cancelMutateAsync.mockReset();
    cancelMutateAsync.mockResolvedValue(true);
    cancelState.isPending = false;
    invalidateQueries.mockReset();
    removeQueries.mockReset();
    paymentState.data = [{ id: 1, name: 'Legacy Pay', payment: 'LegacyPay' }];
    orderState.isFetching = false;
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
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  it('keeps the bundled-theme traffic-only product-info block', () => {
    const html = renderToStaticMarkup(<OrderDetailPage />);

    expect(html).toContain('order.product_info');
    expect(html).not.toContain('order.product_name');
    expect(html).not.toContain('Legacy Plan</span>');
    expect(html).not.toContain('order.product_period');
    expect(html).not.toContain('plan.monthly</span>');
    expect(html).toContain('order.product_traffic');
    expect(html).toContain('123 GB');
  });

  it('keeps the bundled-theme deposit product-name row', () => {
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

    const html = renderToStaticMarkup(<OrderDetailPage />);

    expect(html).toContain('order.product_info');
    expect(html).toContain('order.product_name');
    expect(html).toContain('充值');
    expect(html).not.toContain('order.product_period');
    expect(html).not.toContain('order.product_traffic');
    expect(html).not.toContain(' GB');
  });

  it('keeps the bundled-theme period label short-circuit without an empty-key fallback', () => {
    expect(orderDetailSource).toContain('PERIOD_LABEL_KEY[order.period]');
    expect(orderDetailSource).not.toContain("PERIOD_LABEL_KEY[order.period] ?? ''");
    expect(orderDetailSource).not.toContain("t(PERIOD_LABEL_KEY[order.period] ?? '')");
  });

  it('does not show the detail spinner before the mount detail dispatch equivalent', () => {
    orderState.data = undefined;
    orderState.isFetching = true;

    const html = renderToStaticMarkup(<OrderDetailPage />);

    expect(html).toContain('id="cashier"');
    expect(html).not.toContain('spinner-grow');
  });

  it('selects the first payment method and precomputes its pre-handling amount in the first rendered pass', () => {
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

    const html = renderToStaticMarkup(<OrderDetailPage />);

    expect(html).toContain('class="v2board-select active border-primary"');
    expect(html).toContain('Fee Pay');
    expect(html).toContain('order.handling_fee');
    expect(html).toContain('2.50');
    expect(html).toContain('¥ 12.50 CNY');
  });

  it('updates the bundled-theme pre-handling amount when the payment method changes', async () => {
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

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    expect(document.body.textContent).toContain('order.handling_fee');
    expect(document.body.textContent).toContain('¥ 12.50 CNY');

    const backupMethod = [...document.querySelectorAll('.v2board-select')].find((item) =>
      item.textContent?.includes('Backup Pay'),
    );
    expect(backupMethod).toBeDefined();

    await act(async () => {
      backupMethod?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(document.body.textContent).not.toContain('order.handling_fee');
    expect(document.body.textContent).toContain('¥ 10.00 CNY');
  });

  it('renders payment fees through the bundled-theme pre_handling_amount path', () => {
    expect(orderDetailSource).toContain('preHandlingAmount');
    expect(orderDetailSource).toContain('order.pre_handling_amount');
    expect(orderDetailSource).toContain('calculatePreHandlingAmount(orderQuery.data, first)');
    expect(orderDetailSource).toContain('calculatePreHandlingAmount(order, method)');
    expect(orderDetailSource).toContain(
      'order.total_amount * ((method.handling_fee_percent as number) / 100) +',
    );
    expect(orderDetailSource).toContain('(method.handling_fee_fixed as number)');
    expect(orderDetailSource).not.toContain('selectedPayment.handling_fee_fixed');
    expect(orderDetailSource).not.toContain('selectedPayment.handling_fee_percent');
    expect(orderDetailSource).not.toContain('method?.handling_fee_percent ?? 0');
    expect(orderDetailSource).not.toContain('method?.handling_fee_fixed ?? 0');
  });

  it('keeps payment method items keyed by their legacy method id without index fallback', () => {
    const paymentMethodSource = orderDetailSource.slice(
      orderDetailSource.indexOf('paymentMethods?.map((method) => ('),
      orderDetailSource.indexOf('{isStripePayment && stripePk', orderDetailSource.indexOf('paymentMethods?.map')),
    );

    expect(paymentMethodSource).toContain('paymentMethods?.map((method) => (');
    expect(paymentMethodSource).toContain('className={`v2board-select ${effectiveMethodId === method.id ? \'active border-primary\' : \'false\'}`}');
    expect(paymentMethodSource).not.toContain('key={index}');
    expect(paymentMethodSource).toContain('key={method.id}');
  });

  it('keeps the bundled-theme Stripe form keyed by public key', () => {
    expect(orderDetailSource).toContain(
      '<StripeCardForm key={stripePk} publicKey={stripePk} onToken={handleStripeToken} />',
    );
    expect(orderDetailSource).not.toContain(
      '<StripeCardForm publicKey={stripePk} onToken={handleStripeToken} />',
    );
  });

  it('keeps the bundled-theme hard-coded Stripe verification loading message', () => {
    expect(orderDetailSource).toContain("toast.loading('请稍等，我们正在验证该笔支付', { duration: 5000 })");
    expect(orderDetailSource).not.toContain("toast.loading(t('order.stripe_verifying')");
  });

  it('keeps the bundled-theme QR code props for payment polling', () => {
    const modalSource = orderDetailSource.slice(
      orderDetailSource.indexOf('<DialogContent'),
      orderDetailSource.indexOf('</DialogContent>', orderDetailSource.indexOf('<DialogContent')),
    );
    const qrSource = orderDetailSource.slice(
      orderDetailSource.indexOf('<QRCode'),
      orderDetailSource.indexOf('/>', orderDetailSource.indexOf('<QRCode')) + 2,
    );

    expect(modalSource).toContain('className="v2board-payment-qrcode"');
    expect(modalSource).toContain('closable={false}');
    expect(modalSource).toContain('maskClosable');
    expect(modalSource).toContain('width={300}');
    expect(modalSource).toContain('centered');
    expect(modalSource).toContain("footer={<div style={{ textAlign: 'center' }}>{t('order.waiting_pay')}</div>}");
    expect(qrSource).toContain('value={payUrl}');
    expect(qrSource).toContain('renderAs="svg"');
    expect(qrSource).toContain('size="250"');
    expect(qrSource).not.toContain('level=');
    expect(qrSource).not.toContain('bgColor=');
    expect(qrSource).not.toContain('fgColor=');
    expect(qrSource).not.toContain('includeMargin=');
  });

  it('hides the QR modal after paid polling without clearing the generated QR content', async () => {
    checkoutOrder.mockResolvedValue({ type: 0, data: 'https://pay.example.test/order' });
    checkOrder.mockResolvedValue(1);

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    const checkoutButton = [...document.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('order.checkout'),
    );
    expect(checkoutButton).toBeDefined();

    await act(async () => {
      checkoutButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(document.querySelector('.v2board-payment-qrcode svg')).not.toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(checkOrder).toHaveBeenCalledWith(expect.anything(), 'ORDER123');
    expect(orderRefetch).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.v2board-payment-qrcode svg')).not.toBeNull();
  });

  it('keeps checkout stuck in the legacy loading state after a status-0 transport failure', async () => {
    checkoutOrder.mockRejectedValue({ status: 0, message: 'Network Error' });

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    const checkoutButton = [...document.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('order.checkout'),
    ) as HTMLButtonElement | undefined;
    expect(checkoutButton).toBeDefined();

    await act(async () => {
      checkoutButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(checkoutButton?.disabled).toBe(true);
    expect(checkoutButton?.textContent).not.toContain('order.checkout');
    expect(orderDetailSource).toContain('isLegacyCheckoutNetworkError(error)');
    expect(orderDetailSource).toContain('(error as { status?: unknown }).status === 0');
  });

  it('restores the checkout button after a non-transport checkout failure', async () => {
    checkoutOrder.mockRejectedValue({ status: 500, message: '支付失败' });

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    const checkoutButton = [...document.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('order.checkout'),
    ) as HTMLButtonElement | undefined;
    expect(checkoutButton).toBeDefined();

    await act(async () => {
      checkoutButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(checkoutButton?.disabled).toBe(false);
    expect(checkoutButton?.textContent).toContain('order.checkout');
  });

  it('drops the locally injected payment fee after the paid poll detail refresh replaces the order', async () => {
    paymentState.data = [
      {
        id: 9,
        name: 'Fee Pay',
        payment: 'LegacyPay',
        handling_fee_fixed: 150,
        handling_fee_percent: 10,
      },
    ];

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    expect(document.body.textContent).toContain('order.handling_fee');
    expect(document.body.textContent).toContain('2.50');

    orderState.data = {
      ...orderState.data!,
      status: 3,
    };

    await act(async () => {
      root.render(<OrderDetailPage />);
      await Promise.resolve();
    });

    expect(document.body.textContent).not.toContain('order.handling_fee');
    expect(orderDetailSource).toContain('previous === 0 && status !== 0');
    expect(orderDetailSource).toContain('setPreHandlingAmount(undefined);');
  });

  it('keeps polling after the pending order detail object refreshes', async () => {
    checkOrder.mockResolvedValue(0);

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(checkOrder).toHaveBeenCalledTimes(1);

    orderState.data = {
      ...orderState.data!,
      created_at: orderState.data!.created_at + 1,
    };
    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    await act(async () => {
      vi.advanceTimersByTime(3000);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(checkOrder).toHaveBeenCalledTimes(2);
  });

  it('cancels with the loaded trade number without the missing legacy detail refresh', async () => {
    orderState.data = {
      ...orderState.data!,
      trade_no: 'DETAIL123',
    };

    await act(async () => {
      root.render(<OrderDetailPage />);
    });

    const cancelButton = [...document.querySelectorAll('button')].find((button) =>
      button.textContent?.includes('order.cancel'),
    );
    expect(cancelButton).toBeDefined();

    await act(async () => {
      cancelButton?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(legacyConfirm).toHaveBeenCalledTimes(1);
    const confirmOptions = legacyConfirm.mock.calls[0]?.[0] as { onOk?: () => void };
    expect(confirmOptions.onOk?.()).toBeUndefined();

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(cancelMutateAsync).toHaveBeenCalledWith('DETAIL123');
    expect(invalidateQueries).not.toHaveBeenCalled();
    expect(orderRefetch).not.toHaveBeenCalled();
    expect(orderDetailSource).toContain("Legacy order/cancel dispatches `fetch`, then `details` (plural).");
    expect(orderDetailSource).toContain('void cancel.mutateAsync(cancelTradeNo).catch(() => {});');
    expect(orderDetailSource).not.toContain('queryClient.invalidateQueries({ queryKey: userKeys.orders() })');
    expect(orderDetailSource).not.toContain('void orderQuery.refetch();');
  });

  it('clears the order model state on unmount like the bundled order/empty reducer', async () => {
    const localContainer = document.createElement('div');
    document.body.appendChild(localContainer);
    const localRoot = createRoot(localContainer);

    removeQueries.mockClear();
    try {
      await act(async () => {
        localRoot.render(<OrderDetailPage />);
        await Promise.resolve();
      });

      act(() => localRoot.unmount());

      expect(removeQueries).toHaveBeenCalledWith({ queryKey: ['user', 'orders'] });
      expect(removeQueries).toHaveBeenCalledWith({
        queryKey: ['user', 'orders', 'detail', 'ORDER123'],
      });
      expect(removeQueries).toHaveBeenCalledWith({ queryKey: ['user', 'payments'] });
      expect(orderDetailSource).toContain(
        'queryClient.removeQueries({ queryKey: userKeys.orderDetail(tradeNo) });',
      );
    } finally {
      localContainer.remove();
    }
  });

  it('renders the legacy cancel loading icon inside the original wrapper element', () => {
    cancelState.isPending = true;

    const html = renderToStaticMarkup(<OrderDetailPage />);

    expect(html).toContain('btn btn-primary btn-sm btn-danger btn-rounded px-3');
    expect(html).toContain('<div><i aria-label="图标: loading"');
    expect(orderDetailSource).toContain('cancel.isPending && (');
    expect(orderDetailSource).toContain('<div>\n                      <LegacyLoadingIcon />');
  });
});
