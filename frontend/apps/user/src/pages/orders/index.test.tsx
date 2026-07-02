import { screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { VIRTUALIZE_MIN_ROWS } from '@/components/ui/table';
import { renderWithProviders } from '@/test/render';
import OrdersPage from './index';

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
  ] as Array<{
    trade_no: string;
    period: string | null;
    total_amount: number;
    status: number;
    created_at: number;
    plan: { name: string } | null;
  }>,
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
  // Empty-key sentinel: a regression to a `t('')` fallback in the period cell
  // would leak this marker into the DOM instead of leaving the badge empty.
  useTranslation: () => ({
    t: (key: string) => (key === '' ? 'EMPTY_KEY_TRANSLATED' : (labels[key] ?? key)),
  }),
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

beforeEach(() => {
  mocks.navigate.mockClear();
  mocks.cancelMutateAsync.mockReset();
  mocks.cancelMutateAsync.mockResolvedValue(true);
  mocks.confirmDialog.mockClear();
  mocks.refetchOrders.mockClear();
  mocks.orderError = undefined;
  mocks.fetching = false;
  mocks.orders = defaultOrders();
});

function bodyRowKeys(table: HTMLElement) {
  return Array.from(table.querySelectorAll('tbody tr[data-row-key]')).map((row) =>
    row.getAttribute('data-row-key'),
  );
}

describe('OrdersPage shadcn commerce table', () => {
  it('renders the harness-pinned table hooks, columns, status pills, and row formatting', () => {
    renderWithProviders(<OrdersPage />);

    expect(screen.getByTestId('orders-card')).toBeInTheDocument();
    const table = screen.getByTestId('orders-table');
    for (const header of ['# 订单号', '周期', '订单金额', '订单状态', '创建时间', '操作']) {
      expect(within(table).getByRole('columnheader', { name: header })).toBeInTheDocument();
    }

    const rows = within(table).getAllByRole('row');
    const unpaidRow = rows[1]!;
    expect(within(unpaidRow).getByRole('button', { name: 'ORDER123' })).toBeInTheDocument();
    expect(within(unpaidRow).getByText('月付')).toBeInTheDocument();
    expect(within(unpaidRow).getByText('10.00')).toBeInTheDocument();
    expect(within(unpaidRow).getByText('待支付')).toBeInTheDocument();
    expect(
      within(unpaidRow).getByText(formatLegacyDateMinuteSlash(1_700_000_000)),
    ).toBeInTheDocument();
    expect(within(unpaidRow).getByRole('button', { name: '查看详情' })).toBeInTheDocument();
    expect(within(unpaidRow).getByRole('button', { name: '取消' })).toBeInTheDocument();

    const completedRow = rows[2]!;
    expect(within(completedRow).getByRole('button', { name: 'ORDER456' })).toBeInTheDocument();
    expect(within(completedRow).getByText('流量重置包')).toBeInTheDocument();
    expect(within(completedRow).getByText('2.50')).toBeInTheDocument();
    expect(within(completedRow).getByText('已完成')).toBeInTheDocument();
    expect(within(completedRow).getByText(formatLegacyDateMinuteSlash(0))).toBeInTheDocument();
  });

  it('renders a shadcn empty state when there are no orders', () => {
    mocks.orders = [];

    renderWithProviders(<OrdersPage />);

    expect(screen.getByTestId('orders-empty')).toHaveTextContent('暂无订单');
    expect(screen.queryByTestId('orders-table')).not.toBeInTheDocument();
  });

  it('leaves the period badge empty for unknown or missing periods instead of translating a fallback key', () => {
    mocks.orders = [
      {
        trade_no: 'ORDER-UNKNOWN',
        period: 'lifetime_price',
        total_amount: 100,
        status: 2,
        created_at: 1,
        plan: null,
      },
      {
        trade_no: 'ORDER-NOPERIOD',
        period: null,
        total_amount: 200,
        status: 2,
        created_at: 2,
        plan: null,
      },
    ];

    renderWithProviders(<OrdersPage />);

    const table = screen.getByTestId('orders-table');
    const rows = within(table).getAllByRole('row');
    for (const row of [rows[1]!, rows[2]!]) {
      const periodCell = within(row).getAllByRole('cell')[1]!;
      expect(periodCell.textContent?.trim()).toBe('');
    }
    // No empty-key translation and no raw period key may leak into the table.
    expect(within(table).queryByText('EMPTY_KEY_TRANSLATED')).not.toBeInTheDocument();
    expect(within(table).queryByText('lifetime_price')).not.toBeInTheDocument();
  });

  it('keys table rows by trade number now that the shadcn table owns the DOM', () => {
    renderWithProviders(<OrdersPage />);

    expect(bodyRowKeys(screen.getByTestId('orders-table'))).toEqual(['ORDER123', 'ORDER456']);
  });

  it('virtualizes rows only above the shared threshold', () => {
    const { unmount } = renderWithProviders(<OrdersPage />);

    // Below the threshold every order renders as a plain row without
    // virtualizer spacer rows or measurement attributes.
    let table = screen.getByTestId('orders-table');
    expect(bodyRowKeys(table)).toHaveLength(2);
    expect(table.querySelectorAll('tbody tr[data-index]')).toHaveLength(0);
    expect(table.querySelectorAll('tbody tr[aria-hidden="true"]')).toHaveLength(0);

    unmount();
    mocks.orders = Array.from({ length: VIRTUALIZE_MIN_ROWS + 1 }, (_, index) => ({
      trade_no: `TRADE${index}`,
      period: 'month_price',
      total_amount: 100,
      status: 0,
      created_at: index,
      plan: { name: 'Bulk Plan' },
    }));
    renderWithProviders(<OrdersPage />);

    // Above the threshold the virtualizer windows the rows: only a slice of the
    // data renders (none in happy-dom's zero-height viewport) and an aria-hidden
    // spacer row keeps the scroll height in place of the culled rows.
    table = screen.getByTestId('orders-table');
    const virtualRows = table.querySelectorAll('tbody tr[data-row-key]');
    expect(virtualRows.length).toBeLessThan(VIRTUALIZE_MIN_ROWS + 1);
    for (const row of virtualRows) {
      expect(row).toHaveAttribute('data-index');
    }
    expect(
      table.querySelectorAll('tbody tr[aria-hidden="true"]').length,
    ).toBeGreaterThan(0);
  });

  it('sorts by the amount and created-at columns on header click while other columns stay inert', async () => {
    const { user } = renderWithProviders(<OrdersPage />);

    const table = screen.getByTestId('orders-table');
    const amountHeader = within(table).getByRole('columnheader', { name: '订单金额' });
    const createdHeader = within(table).getByRole('columnheader', { name: '创建时间' });
    expect(amountHeader).toHaveAttribute('aria-sort', 'none');
    expect(createdHeader).toHaveAttribute('aria-sort', 'none');
    expect(within(table).getByRole('columnheader', { name: '# 订单号' })).not.toHaveAttribute(
      'aria-sort',
    );

    // Default preserves the server's row order.
    expect(bodyRowKeys(table)).toEqual(['ORDER123', 'ORDER456']);

    // Numeric columns sort descending first (TanStack auto sort direction).
    await user.click(within(amountHeader).getByRole('button'));
    expect(amountHeader).toHaveAttribute('aria-sort', 'descending');
    expect(bodyRowKeys(table)).toEqual(['ORDER123', 'ORDER456']);

    await user.click(within(amountHeader).getByRole('button'));
    expect(amountHeader).toHaveAttribute('aria-sort', 'ascending');
    expect(bodyRowKeys(table)).toEqual(['ORDER456', 'ORDER123']);

    // Sorting created-at replaces the amount sort and reorders the rows again.
    await user.click(within(createdHeader).getByRole('button'));
    expect(createdHeader).toHaveAttribute('aria-sort', 'descending');
    expect(amountHeader).toHaveAttribute('aria-sort', 'none');
    expect(bodyRowKeys(table)).toEqual(['ORDER123', 'ORDER456']);
  });
});

describe('OrdersPage commerce behavior', () => {
  it('navigates from both the trade number and detail action', async () => {
    const { user } = renderWithProviders(<OrdersPage />);

    await user.click(screen.getByRole('button', { name: 'ORDER123' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');

    mocks.navigate.mockClear();
    const table = screen.getByTestId('orders-table');
    const firstRow = within(table).getAllByRole('row')[1]!;
    await user.click(within(firstRow).getByRole('button', { name: '查看详情' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');
  });

  it('fires cancel through the confirm dialog action for unpaid orders', async () => {
    const { user } = renderWithProviders(<OrdersPage />);

    const table = screen.getByTestId('orders-table');
    const cancelButtons = within(table).getAllByRole('button', { name: '取消' });
    expect(cancelButtons).toHaveLength(2);
    expect(cancelButtons[0]!).toBeEnabled();

    await user.click(cancelButtons[0]!);

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
    expect(mocks.cancelMutateAsync).not.toHaveBeenCalled();

    await options.onConfirm();

    expect(mocks.cancelMutateAsync).toHaveBeenCalledWith('ORDER123');
    expect(mocks.refetchOrders).not.toHaveBeenCalled();
  });

  it('disables cancel for non-unpaid orders while keeping detail navigation available', async () => {
    const { user } = renderWithProviders(<OrdersPage />);

    const table = screen.getByTestId('orders-table');
    const detailButtons = within(table).getAllByRole('button', { name: '查看详情' });
    await user.click(detailButtons[1]!);
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER456');

    const cancelButtons = within(table).getAllByRole('button', { name: '取消' });
    expect(cancelButtons[1]!).toBeDisabled();

    await user.click(cancelButtons[1]!);

    expect(mocks.confirmDialog).not.toHaveBeenCalled();
    expect(mocks.cancelMutateAsync).not.toHaveBeenCalled();
  });

  it('shows the loading strip while fetching but not after a settled fetch error', () => {
    mocks.fetching = true;

    const { rerender } = renderWithProviders(<OrdersPage />);

    expect(screen.getByText('正在加载')).toBeInTheDocument();

    // A settled transport failure (status 0) no longer pins a perpetual spinner —
    // loading tracks the query's isFetching, so it clears like any other error.
    mocks.fetching = false;
    mocks.orderError = { status: 0, message: 'timeout of 30000ms exceeded' };
    rerender(<OrdersPage />);

    expect(screen.queryByText('正在加载')).not.toBeInTheDocument();

    mocks.orderError = { status: 500, message: 'Server Error' };
    rerender(<OrdersPage />);

    expect(screen.queryByText('正在加载')).not.toBeInTheDocument();
  });
});
