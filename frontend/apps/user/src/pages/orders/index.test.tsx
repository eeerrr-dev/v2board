import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import OrdersPage from './index';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const ordersSource = readFileSync(`${process.cwd()}/src/pages/orders/index.tsx`, 'utf8');

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  cancelMutateAsync: vi.fn(),
  legacyConfirm: vi.fn(),
  refetchOrders: vi.fn(),
  mobile: false,
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
  'order.cancel': '关闭订单',
  'order.cancel_confirm': '如果您已经付款，取消订单可能会导致支付失败，确定要取消订单吗？',
  'order.trade_no_col': '# 订单号',
  'order.period': '周期',
  'order.amount': '订单金额',
  'order.status': '订单状态',
  'order.created_at': '创建时间',
  'order.action_col': '操作',
  'order.return': '查看详情',
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

vi.mock('@/lib/legacy-settings', () => ({
  isLegacyMobile: () => mocks.mobile,
}));

describe('OrdersPage bundled-theme table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    mocks.mobile = false;
    mocks.fetching = false;
    mocks.orders = defaultOrders();
  });

  it('renders the legacy desktop table shell, columns, fixed action column, and row formatting', () => {
    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('block block-rounded  ');
    expect(html).toContain('ant-table-wrapper');
    expect(html).toContain('class="ant-table-fixed" style="width:900px;table-layout:auto"');
    expect(html).toContain('class="ant-table-fixed" style="table-layout:auto"');
    expect(html).toContain('ant-table-fixed-right');
    expect(html).toContain('# 订单号');
    expect(html).toContain('周期');
    expect(html).toContain('订单金额');
    expect(html).toContain('订单状态');
    expect(html).toContain('创建时间');
    expect(html).toContain('操作');
    expect(html).toContain('ORDER123');
    expect(html).toContain('月付');
    expect(html).toContain('10.00');
    expect(html).toContain('ant-badge-status-error');
    expect(html).toContain('待支付');
    expect(html).toContain('2023/11/14 22:13');
    expect(html).toContain('流量重置包');
    expect(html).toContain('2.50');
    expect(html).toContain('ant-badge-status-success');
    expect(html).toContain('已完成');
    expect(html).toContain('1970/01/01 00:00');
    expect(html.match(/ant-divider ant-divider-vertical/g)).toHaveLength(4);
    expect(html).not.toContain('role="separator"');
    expect(html).not.toContain('data-row-key');
  });

  it('renders the legacy mobile list item structure', () => {
    mocks.mobile = true;

    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('class="am-list"');
    expect(html).toContain('class="am-list-body"');
    expect(html).toContain('class="am-list-line am-list-line-multiple"');
    expect(html).toContain('Legacy Plan');
    expect(html).toContain('2023-11-14 22:13:20');
    expect(html).toContain('10.00');
    expect(html).toContain('ant-badge-status-error');
    expect(html).toContain('待支付');
    expect(html).toContain('class="am-list-arrow am-list-arrow-horizontal" aria-hidden="true"');
    expect(html).toContain('class="am-list-ripple" style="display:none"');
  });

  it('keeps the bundled-theme order-period short-circuit without an empty-key fallback', () => {
    expect(ordersSource).toContain('PERIOD_LABEL[order.period]');
    expect(ordersSource).toContain('<span className="ant-tag">{periodLabel}</span>');
    expect(ordersSource).not.toContain("periodKey ? t(periodKey) : ''");
    expect(ordersSource).not.toContain("PERIOD_LABEL[order.period] ?? ''");
  });

  it('keeps bundled antd table row keys internal-only', () => {
    expect(ordersSource).not.toContain('data-row-key');
  });

  it('does not apply the list loading class before the mount fetch dispatch equivalent', () => {
    mocks.fetching = true;

    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('block block-rounded  ');
    expect(html).not.toContain('block-mode-loading');
  });
});

describe('OrdersPage legacy cancel action', () => {
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
    mocks.mobile = false;
    mocks.fetching = false;
    mocks.orders = defaultOrders();
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  it('navigates from the legacy mobile list items instead of rendering the desktop table', async () => {
    mocks.mobile = true;

    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    expect(container.querySelector('.am-list')).not.toBeNull();
    expect(container.querySelector('.ant-table-wrapper')).toBeNull();

    const firstItem = container.querySelector('.am-list-item')!;
    await act(async () => {
      firstItem.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');
  });

  it('fires cancel through a non-thenable Modal.confirm onOk like the bundled theme', async () => {
    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    const cancelLinks = Array.from(container.querySelectorAll('a')).filter(
      (link) => link.textContent === '取消',
    );
    expect(cancelLinks).toHaveLength(4);

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

  it('keeps legacy disabled action links clickable because the bundle only stamps the attribute', async () => {
    mocks.orders = [
      mocks.orders[0]!,
      {
        ...mocks.orders[1]!,
        status: 2,
      },
    ];

    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    const detailLinks = Array.from(container.querySelectorAll<HTMLAnchorElement>('a')).filter(
      (link) => link.textContent === '查看详情',
    );
    expect(detailLinks[1]!.getAttribute('disabled')).toBe('');

    await act(async () => {
      detailLinks[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER456');

    const cancelLinks = Array.from(container.querySelectorAll<HTMLAnchorElement>('a')).filter(
      (link) => link.textContent === '取消',
    );
    expect(cancelLinks[1]!.getAttribute('disabled')).toBe('');

    await act(async () => {
      cancelLinks[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).toHaveBeenCalledTimes(1);
    const options = mocks.legacyConfirm.mock.calls[0]![0] as {
      onOk: () => unknown;
    };
    expect(options.onOk()).toBeUndefined();

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.cancelMutateAsync).toHaveBeenCalledWith('ORDER456');
  });

  it('keeps the original block loading class after the mount fetch dispatch equivalent', async () => {
    mocks.fetching = true;

    await act(async () => {
      root.render(<OrdersPage />);
      await Promise.resolve();
    });

    expect(container.querySelector('.block.block-rounded')?.className).toContain(
      'block-mode-loading',
    );
  });
});
