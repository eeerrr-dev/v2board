import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import OrdersPage from './index';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const ordersSource = readFileSync(`${process.cwd()}/src/pages/orders/index.tsx`, 'utf8');

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  cancelMutateAsync: vi.fn(),
  confirmDialog: vi.fn(),
  refetchOrders: vi.fn(),
  orderError: undefined as unknown,
  fetching: false,
  orders: [
    {
      trade_no: 'ORDER123',
      period: 'month_price',
      total_amount: 1000,
      status: 0,
      created_at: 1_700_000_000,
      plan: { name: 'Legacy Plan' },
    },
    {
      trade_no: 'ORDER456',
      period: 'reset_price',
      total_amount: 250,
      status: 3,
      created_at: 0,
      plan: { name: 'Reset Pack' },
    },
  ],
}));

const labels: Record<string, string> = {
  'common.attention': '注意',
  'common.cancel': '取消',
  'common.loading': '正在加载',
  'order.cancel': '关闭订单',
  'order.cancel_confirm': '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
  'order.trade_no_col': '# 订单号',
  'order.period': '周期',
  'order.amount': '订单金额',
  'order.status': '订单状态',
  'order.created_at': '创建时间',
  'order.action_col': '操作',
  'order.return': '查看详情',
  'order.no_orders': '暂无订单',
  'order.status_unpaid': '待支付',
  'order.status_completed': '已完成',
  'plan.monthly': '月付',
  'plan.reset': '流量重置包',
};

function defaultOrders() {
  return [
    {
      trade_no: 'ORDER123',
      period: 'month_price',
      total_amount: 1000,
      status: 0,
      created_at: 1_700_000_000,
      plan: { name: 'Legacy Plan' },
    },
    {
      trade_no: 'ORDER456',
      period: 'reset_price',
      total_amount: 250,
      status: 3,
      created_at: 0,
      plan: { name: 'Reset Pack' },
    },
  ];
}

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key }),
}));

vi.mock('@/lib/queries', () => ({
  useOrders: () => ({
    data: mocks.orders,
    error: mocks.orderError,
    isFetching: mocks.fetching,
    refetch: mocks.refetchOrders,
  }),
  useCancelOrderMutation: () => ({
    isPending: false,
    mutateAsync: mocks.cancelMutateAsync,
  }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({
  confirmDialog: mocks.confirmDialog,
}));

describe('OrdersPage shadcn commerce table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    mocks.orderError = undefined;
    mocks.fetching = false;
    mocks.orders = defaultOrders();
  });

  it('renders the shadcn table shell, columns, status pills, actions, and row formatting', () => {
    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('data-testid="orders-card"');
    expect(html).toContain('data-testid="orders-table"');
    expect(html).toContain('# 订单号');
    expect(html).toContain('周期');
    expect(html).toContain('订单金额');
    expect(html).toContain('订单状态');
    expect(html).toContain('创建时间');
    expect(html).toContain('操作');
    expect(html).toContain('ORDER123');
    expect(html).toContain('月付');
    expect(html).toContain('10.00');
    expect(html).toContain('待支付');
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_000));
    expect(html).toContain('流量重置包');
    expect(html).toContain('2.50');
    expect(html).toContain('已完成');
    expect(html).toContain(formatLegacyDateMinuteSlash(0));
    expect(html).toContain('查看详情');
    expect(html).toContain('取消');
    expect(html).not.toContain('ant-table-wrapper');
    expect(html).not.toContain('ant-table-column-title');
    expect(html).not.toContain('ant-table-tbody');
    expect(html).not.toContain('ant-badge-status');
    expect(html).not.toContain('am-list');
  });

  it('renders a shadcn empty state when there are no orders', () => {
    mocks.orders = [];

    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('data-testid="orders-empty"');
    expect(html).toContain('暂无订单');
    expect(html).not.toContain('data-testid="orders-table"');
  });

  it('keeps the order-period short-circuit without an empty-key fallback', () => {
    expect(ordersSource).toContain('PERIOD_LABEL[row.original.period]');
    expect(ordersSource).toContain('periodLabelKey ? t(periodLabelKey) : undefined');
    expect(ordersSource).not.toContain("periodKey ? t(periodKey) : ''");
    expect(ordersSource).not.toContain("PERIOD_LABEL[row.original.period] ?? ''");
    expect(ordersSource).not.toContain('<span className="ant-tag">{periodLabel}</span>');
  });

  it('keys table rows by trade number now that the shadcn table owns the DOM', () => {
    expect(ordersSource).toContain('satisfies DataTableColumn<(typeof orders)[number]>[]');
    expect(ordersSource).toContain('virtualizer={{ enabled: orders.length > 30 }}');
    expect(ordersSource).not.toContain('data-row-key={index}');
    expect(ordersSource).not.toContain('data-row-key={order.trade_no}');
  });

  it('does not show the fetch loading strip before the mount fetch dispatch equivalent', () => {
    mocks.fetching = true;

    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('data-testid="orders-card"');
    expect(html).not.toContain('正在加载');
    expect(html).not.toContain('block-mode-loading');
  });
});

describe('OrdersPage commerce behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.navigate.mockClear();
    mocks.cancelMutateAsync.mockReset();
    mocks.cancelMutateAsync.mockResolvedValue(true);
    mocks.confirmDialog.mockClear();
    mocks.refetchOrders.mockClear();
    mocks.orderError = undefined;
    mocks.fetching = false;
    mocks.orders = defaultOrders();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  it('navigates from both the trade number and detail action', async () => {
    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    const tradeButton = [...container.querySelectorAll<HTMLButtonElement>('button')].find(
      (button) => button.textContent === 'ORDER123',
    )!;
    await act(async () => {
      tradeButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');

    const detailButton = [...container.querySelectorAll<HTMLButtonElement>('button')].find(
      (button) => button.textContent === '查看详情',
    )!;
    await act(async () => {
      detailButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');
  });

  it('fires cancel through the confirm dialog action for unpaid orders', async () => {
    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    const cancelButtons = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).filter(
      (button) => button.textContent === '取消',
    );
    expect(cancelButtons).toHaveLength(2);
    expect(cancelButtons[0]!.disabled).toBe(false);

    await act(async () => {
      cancelButtons[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.confirmDialog).toHaveBeenCalledTimes(1);
    const options = mocks.confirmDialog.mock.calls[0]![0] as {
      title: string;
      description: string;
      confirmText: string;
      onConfirm: () => unknown;
    };
    expect(options.title).toBe('注意');
    expect(options.description).toBe('如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？');
    expect(options.confirmText).toBe('关闭订单');
    await options.onConfirm();

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.cancelMutateAsync).toHaveBeenCalledWith('ORDER123');
    expect(mocks.refetchOrders).not.toHaveBeenCalled();
    expect(ordersSource).not.toContain('ordersQuery.refetch()');
  });

  it('disables cancel for non-unpaid orders while keeping detail navigation available', async () => {
    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    const detailButtons = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).filter(
      (button) => button.textContent === '查看详情',
    );
    await act(async () => {
      detailButtons[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER456');

    const cancelButtons = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).filter(
      (button) => button.textContent === '取消',
    );
    expect(cancelButtons[1]!.disabled).toBe(true);

    await act(async () => {
      cancelButtons[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.confirmDialog).not.toHaveBeenCalled();
    expect(mocks.cancelMutateAsync).not.toHaveBeenCalled();
  });

  it('shows the loading strip after a mounted fetch or transport timeout but not an API 500', async () => {
    mocks.fetching = true;

    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('正在加载');

    mocks.fetching = false;
    mocks.orderError = { status: 0, message: 'timeout of 30000ms exceeded' };

    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain('正在加载');

    mocks.orderError = { status: 500, message: 'Server Error' };

    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    expect(container.textContent).not.toContain('正在加载');
  });
});
