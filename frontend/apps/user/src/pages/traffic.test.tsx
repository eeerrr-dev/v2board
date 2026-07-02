import { screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import TrafficPage from './traffic';

const queryState = vi.hoisted(() => ({
  rows: [] as Array<{
    u: number;
    d: number;
    record_at: number;
    user_id: number;
    server_rate: string;
  }>,
  pending: false,
  fetching: false,
  error: false,
  refetch: vi.fn(),
}));

const labels: Record<string, string> = {
  'traffic.notice': '流量明细仅保留近一个月数据以供查询。',
  'traffic.record_at': '记录时间',
  'traffic.actual_upload': '实际上行',
  'traffic.actual_download': '实际下行',
  'traffic.deduct_rate': '扣费倍率',
  'traffic.total_charged': '合计',
  'traffic.total_formula': '公式：(实际上行 + 实际下行) x 扣费倍率 = 扣除流量',
  'common.loading': 'Loading...',
  'common.error_title': '加载失败',
  'common.retry': '重试',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => labels[key] ?? key,
    i18n: { language: 'zh-CN' },
  }),
}));

vi.mock('@/lib/queries', () => ({
  useTrafficLog: () => ({
    data: queryState.rows,
    isPending: queryState.pending,
    isFetching: queryState.fetching,
    isError: queryState.error,
    refetch: queryState.refetch,
  }),
}));

beforeEach(() => {
  queryState.rows = [];
  queryState.pending = false;
  queryState.fetching = false;
  queryState.error = false;
  queryState.refetch.mockClear();
});

describe('TrafficPage loading state', () => {
  it('shows the inline loading status while the initial traffic fetch is pending', () => {
    queryState.pending = true;
    queryState.fetching = true;

    renderWithProviders(<TrafficPage />);

    expect(screen.getByRole('status')).toHaveTextContent('Loading...');
  });

  it('keeps cached rows rendered without the loading banner during a background refetch', () => {
    queryState.rows = [
      { u: 2048, d: 1024, record_at: 1_705_320_000, user_id: 1, server_rate: '1.5' },
    ];
    queryState.pending = false;
    queryState.fetching = true; // background refetch with cached data

    renderWithProviders(<TrafficPage />);

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.getByText('2024/01/15')).toBeInTheDocument();
  });

  it('renders the localized empty description when there are no rows', () => {
    renderWithProviders(<TrafficPage />);

    // useEmptyDescription resolves the antd empty message for zh-CN.
    expect(screen.getByTestId('traffic-empty')).toHaveTextContent('暂无数据');
  });
});

describe('TrafficPage service table', () => {
  beforeEach(() => {
    queryState.rows = [
      { u: 2048, d: 1024, record_at: 1_705_320_000, user_id: 1, server_rate: '1.5' },
      { u: 100, d: 200, record_at: 0, user_id: 1, server_rate: '0' },
    ];
  });

  it('renders the parity hooks, headers, and row formatting', () => {
    renderWithProviders(<TrafficPage />);

    expect(screen.getByTestId('traffic-card')).toBeInTheDocument();
    const table = screen.getByTestId('traffic-table');
    expect(table).toHaveAttribute('data-table-kind', 'service');
    // The parity harness reads data-scroll-position off this container.
    expect(screen.getByTestId('service-table-scroll')).toHaveAttribute('data-scroll-position');

    for (const header of ['记录时间', '实际上行', '实际下行', '扣费倍率', '合计']) {
      expect(within(table).getByRole('columnheader', { name: header })).toBeInTheDocument();
    }

    const [first, second] = within(table).getAllByRole('row').slice(1);
    // Rows keep server order with index-based keys, not record_at-derived keys.
    expect(first).toHaveAttribute('data-row-key', '0');
    expect(second).toHaveAttribute('data-row-key', '1');

    const firstCells = within(first!).getAllByRole('cell');
    expect(firstCells[0]).toHaveTextContent('2024/01/15');
    expect(firstCells[1]).toHaveTextContent('2.00 KB');
    expect(firstCells[2]).toHaveTextContent('1024.00 B');
    expect(firstCells[3]).toHaveTextContent('1.50 x');
    // Charged total contract: (u + d) * server_rate = (2048 + 1024) * 1.5 = 4608.
    expect(firstCells[4]).toHaveTextContent('4.50 KB');

    const secondCells = within(second!).getAllByRole('cell');
    expect(secondCells[0]).toHaveTextContent(/^-$/); // record_at 0 renders a dash
    expect(secondCells[1]).toHaveTextContent('100.00 B');
    expect(secondCells[2]).toHaveTextContent('200.00 B');
    expect(secondCells[3]).toHaveTextContent(/^-$/); // rate 0 renders a dash
    expect(secondCells[4]).toHaveTextContent(/^0\.00 B$/);
  });

  it('sorts rows by record time through the shared table header', async () => {
    const { user } = renderWithProviders(<TrafficPage />);

    const table = screen.getByTestId('traffic-table');
    const recordHeader = within(table).getByRole('columnheader', { name: '记录时间' });
    expect(recordHeader).toHaveAttribute('aria-sort', 'none');

    const sortButton = within(recordHeader).getByRole('button', { name: '记录时间' });

    // Numeric columns toggle descending first in the shared table.
    await user.click(sortButton);
    expect(recordHeader).toHaveAttribute('aria-sort', 'descending');

    await user.click(sortButton);
    expect(recordHeader).toHaveAttribute('aria-sort', 'ascending');
    const [first] = within(table).getAllByRole('row').slice(1);
    // Ascending record_at puts the record_at: 0 row ('-' date, 100 B upload) first.
    expect(within(first!).getAllByRole('cell')[0]).toHaveTextContent(/^-$/);
    expect(within(first!).getByText('100.00 B')).toBeInTheDocument();
  });

  it('opens the charged-total formula tooltip from the parity header trigger', async () => {
    const { user } = renderWithProviders(<TrafficPage />);

    const trigger = screen.getByText('合计');
    // The parity harness hovers [data-testid="traffic-table"] .v2board-service-tooltip-trigger.
    expect(trigger).toHaveClass('v2board-service-tooltip-trigger');
    // The shared HeaderTooltip keeps traffic's end alignment via className.
    expect(trigger).toHaveClass('justify-end');

    await user.hover(trigger);

    expect(await screen.findByRole('tooltip')).toHaveTextContent(
      '公式：(实际上行 + 实际下行) x 扣费倍率 = 扣除流量',
    );
  });

  it('coerces the whole server_rate string in the charged total (legacy behavior)', () => {
    queryState.rows = [{ u: 300, d: 400, record_at: 1_705_320_000, user_id: 1, server_rate: '' }];

    renderWithProviders(<TrafficPage />);

    const [row] = within(screen.getByTestId('traffic-table')).getAllByRole('row').slice(1);
    const cells = within(row!).getAllByRole('cell');
    // A falsy server_rate short-circuits upload/download to a literal 0...
    expect(cells[1]).toHaveTextContent(/^0$/);
    expect(cells[2]).toHaveTextContent(/^0$/);
    expect(cells[3]).toHaveTextContent(/^-$/);
    // ...while the charged total multiplies by the whole coerced string:
    // (300 + 400) * Number('') = 0. parseFloat semantics would render 'NaN B'.
    expect(cells[4]).toHaveTextContent(/^0\.00 B$/);
  });
});

describe('TrafficPage error state', () => {
  it('renders a retryable error state instead of an empty traffic table on fetch failure', async () => {
    queryState.error = true;

    const { user } = renderWithProviders(<TrafficPage />);

    // A failed fetch must not render as an empty "no usage" table.
    expect(screen.getByTestId('traffic-error')).toBeInTheDocument();
    expect(screen.queryByTestId('traffic-table')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '重试' }));

    expect(queryState.refetch).toHaveBeenCalled();
  });
});
