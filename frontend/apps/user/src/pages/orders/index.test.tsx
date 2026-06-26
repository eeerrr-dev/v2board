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
  legacyConfirm: vi.fn(),
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

vi.mock('react-router-dom', () => ({
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

vi.mock('@/components/legacy-confirm', () => ({
  legacyConfirm: mocks.legacyConfirm,
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

    expect(html).toContain('v2board-orders-card');
    expect(html).toContain('v2board-orders-table');
    expect(html).toContain('ant-table-column-title');
    expect(html).toContain('ant-table-tbody');
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
    expect(html).not.toContain('ant-badge-status');
    expect(html).not.toContain('am-list');
  });

  it('renders a shadcn empty state when there are no orders', () => {
    mocks.orders = [];

    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('v2board-orders-empty');
    expect(html).toContain('暂无订单');
    expect(html).not.toContain('v2board-orders-table');
  });

  it('keeps the order-period short-circuit without an empty-key fallback', () => {
    expect(ordersSource).toContain('PERIOD_LABEL[order.period]');
    expect(ordersSource).toContain('periodLabelKey ? t(periodLabelKey) : undefined');
    expect(ordersSource).not.toContain("periodKey ? t(periodKey) : ''");
    expect(ordersSource).not.toContain("PERIOD_LABEL[order.period] ?? ''");
    expect(ordersSource).not.toContain('<span className="ant-tag">{periodLabel}</span>');
  });

  it('keys table rows by trade number now that the shadcn table owns the DOM', () => {
    expect(ordersSource).toContain('<tr key={order.trade_no}');
    expect(ordersSource).not.toContain('data-row-key={index}');
    expect(ordersSource).not.toContain('data-row-key={order.trade_no}');
  });

  it('does not show the fetch loading strip before the mount fetch dispatch equivalent', () => {
    mocks.fetching = true;

    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('v2board-orders-card');
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
    mocks.legacyConfirm.mockClear();
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

    const tradeLink = [...container.querySelectorAll<HTMLAnchorElement>('a')].find(
      (link) => link.textContent === 'ORDER123',
    )!;
    await act(async () => {
      tradeLink.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');

    const detailLink = [...container.querySelectorAll<HTMLAnchorElement>('a')].find(
      (link) => link.textContent === '查看详情',
    )!;
    await act(async () => {
      detailLink.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');
  });

  it('fires cancel through a non-thenable confirm onOk for unpaid orders', async () => {
    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    const cancelLinks = Array.from(container.querySelectorAll<HTMLAnchorElement>('a')).filter(
      (link) => link.textContent === '取消',
    );
    expect(cancelLinks).toHaveLength(2);
    expect(cancelLinks[0]!.getAttribute('aria-disabled')).toBe('false');

    await act(async () => {
      cancelLinks[0]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).toHaveBeenCalledTimes(1);
    const options = mocks.legacyConfirm.mock.calls[0]![0] as {
      title: string;
      content: string;
      okText: string;
      onOk: () => unknown;
    };
    expect(options.title).toBe('注意');
    expect(options.content).toBe('如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？');
    expect(options.okText).toBe('关闭订单');
    expect(options.onOk()).toBeUndefined();

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

    const detailLinks = Array.from(container.querySelectorAll<HTMLAnchorElement>('a')).filter(
      (link) => link.textContent === '查看详情',
    );
    await act(async () => {
      detailLinks[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER456');

    const cancelLinks = Array.from(container.querySelectorAll<HTMLAnchorElement>('a')).filter(
      (link) => link.textContent === '取消',
    );
    expect(cancelLinks[1]!.getAttribute('aria-disabled')).toBe('true');

    await act(async () => {
      cancelLinks[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).not.toHaveBeenCalled();
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
