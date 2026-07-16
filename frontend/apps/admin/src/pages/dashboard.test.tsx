import type { ComponentProps, ReactNode } from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { buildOrderChartModel } from '@/components/admin-chart';
import DashboardPage from './dashboard';

// The dashboard is a redesigned shadcn island (shadcn Alerts + shortcut cards +
// stat cards + token-aware shadcn/Recharts composition) replacing the OneUI block
// layout. What stays covered is behavior: pending-work navigation, the pending-
// commission sessionStorage filter, shortcut routes, currency, and chart semantics.

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  responsiveContainerRender: vi.fn(),
  lineChartRender: vi.fn(),
  barChartRender: vi.fn(),
}));

vi.mock('react-router', () => ({
  Link: ({ to, children, ...props }: { to: string } & Omit<ComponentProps<'a'>, 'href'>) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
  useNavigate: () => mocks.navigate,
}));

vi.mock('recharts', async () => {
  const React = await import('react');
  type MockChartProps = { children?: ReactNode; [key: string]: unknown };

  const primitive = () => null;
  return {
    ResponsiveContainer: ({ children, ...props }: MockChartProps) => {
      mocks.responsiveContainerRender(props);
      return React.createElement(React.Fragment, null, children);
    },
    LineChart: ({ children, ...props }: MockChartProps) => {
      mocks.lineChartRender(props);
      return React.createElement('div', { 'data-testid': 'mock-line-chart' }, children);
    },
    BarChart: ({ children, ...props }: MockChartProps) => {
      mocks.barChartRender(props);
      return React.createElement('div', { 'data-testid': 'mock-bar-chart' }, children);
    },
    Bar: primitive,
    CartesianGrid: primitive,
    Legend: primitive,
    Line: primitive,
    Tooltip: primitive,
    XAxis: primitive,
    YAxis: primitive,
  };
});

vi.mock('@/lib/runtime-config', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/runtime-config')>()),
  getAdminApiBaseUrl: () => 'http://localhost/api/v1',
}));
vi.mock('@/lib/queries', () => ({
  useConfig: () => ({ data: { site: { currency: 'CNY' }, currency: 'USD' } }),
  useQueueStats: () => ({
    data: { status: 'running' },
    isError: false,
    refetch: vi.fn(),
  }),
  useStat: () => ({
    data: {
      online_user: 9,
      day_income: 12345,
      day_register_total: 7,
      month_income: 67890,
      last_month_income: 45678,
      commission_last_month_payout: 1234,
      month_register_total: 42,
      ticket_pending_total: 2,
      commission_pending_total: 3,
    },
  }),
  useStatOrder: () => ({ data: [] }),
  useStatServerToday: () => ({ data: [] }),
  useStatServerLast: () => ({ data: [] }),
  useStatUserToday: () => ({ data: [] }),
  useStatUserLast: () => ({ data: [] }),
}));

describe('DashboardPage', () => {
  beforeEach(() => {
    mocks.navigate.mockReset();
    mocks.responsiveContainerRender.mockReset();
    mocks.lineChartRender.mockReset();
    mocks.barChartRender.mockReset();
    window.sessionStorage.clear();
  });

  it('aligns sparse order series by their actual date instead of array position', () => {
    expect(
      buildOrderChartModel([
        { type: '新购', date: '07-01', value: 10 },
        { type: '新购', date: '07-02', value: 20 },
        { type: '续费', date: '07-01', value: 4 },
      ]),
    ).toEqual({
      series: [
        { dataKey: 'series_0', label: '新购' },
        { dataKey: 'series_1', label: '续费' },
      ],
      rows: [
        { date: '07-01', series_0: 10, series_1: 4 },
        { date: '07-02', series_0: 20 },
      ],
    });
  });

  async function renderDashboard() {
    render(<DashboardPage />);
    await screen.findByTestId('admin-order-chart');
  }

  it('renders the pending-work alerts with their counts', async () => {
    await renderDashboard();
    expect(screen.getByText('有 2 条工单等待处理')).toBeInTheDocument();
    expect(screen.getByText('有 3 笔佣金等待确认')).toBeInTheDocument();
  });

  it('shows income cents against the site currency (not the top-level currency)', async () => {
    await renderDashboard();
    // day_income 12345 → 123.45
    expect(screen.getByText('123.45')).toBeInTheDocument();
    expect(screen.getAllByText('CNY').length).toBeGreaterThan(0);
    expect(screen.queryByText('USD')).not.toBeInTheDocument();
    expect(screen.getByText('9')).toBeInTheDocument();
  });

  it('links the ticket alert action to the ticket console', async () => {
    await renderDashboard();
    expect(screen.getByTestId('dashboard-ticket-alert').querySelector('a')).toHaveAttribute(
      'href',
      '/ticket',
    );
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('stashes the pending-commission order filter before opening orders', async () => {
    const user = userEvent.setup();
    await renderDashboard();
    await user.click(screen.getByTestId('dashboard-commission-action'));

    expect(window.sessionStorage.getItem('v2board-admin-order-filter')).toBe(
      JSON.stringify([
        { key: 'status', condition: '=', value: '3' },
        { key: 'commission_status', condition: '=', value: '0' },
        { key: 'commission_balance', condition: '>', value: '0' },
      ]),
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/order');
  });

  it('renders each quick shortcut as a real link', async () => {
    await renderDashboard();

    for (const [name, href] of [
      ['系统设置', '/config/system'],
      ['订单管理', '/order'],
      ['订阅管理', '/plan'],
      ['用户管理', '/user'],
    ] as const) {
      expect(screen.getByRole('link', { name })).toHaveAttribute('href', href);
    }
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('composes one accessible line chart and four vertical ranking charts', async () => {
    await renderDashboard();

    expect(screen.getByTestId('admin-order-chart')).toBeInTheDocument();
    expect(await screen.findAllByTestId('admin-ranking-chart')).toHaveLength(4);
    expect(mocks.lineChartRender).toHaveBeenCalledWith(
      expect.objectContaining({ accessibilityLayer: true, 'aria-label': '订单统计折线图' }),
    );
    expect(mocks.barChartRender).toHaveBeenCalledWith(
      expect.objectContaining({ accessibilityLayer: true, layout: 'vertical' }),
    );
    expect(mocks.responsiveContainerRender).toHaveBeenCalledWith(
      expect.objectContaining({ initialDimension: { width: 320, height: 360 } }),
    );
  });
});
