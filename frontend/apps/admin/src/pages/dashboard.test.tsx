import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DashboardPage from './dashboard';

// The dashboard is a redesigned shadcn island (shadcn Alerts + shortcut cards +
// stat cards + token-aware echarts hosts) replacing the OneUI block layout. The
// darkreader canvas baseline, js-classic-nav shells, and stats-bar scroll log
// are retired. What stays covered is behavior: the pending-work alerts and their
// navigation, the pending-commission sessionStorage filter, the shortcut routes,
// the site currency lookup, and the cent-formatted income.

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  echartsInit: vi.fn(() => ({ setOption: vi.fn(), resize: vi.fn(), dispose: vi.fn() })),
}));

vi.mock('react-router', () => ({ useNavigate: () => mocks.navigate }));

vi.mock('echarts', () => ({ init: mocks.echartsInit }));

vi.mock('@/lib/legacy-settings', () => ({
  getAdminApiBaseUrl: () => 'http://localhost/api/v1',
}));

vi.mock('@/lib/queries', () => ({
  useConfig: () => ({ data: { site: { currency: 'CNY' }, currency: 'USD' } }),
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
    window.sessionStorage.clear();
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve({ json: () => Promise.resolve({ status: 'running' }) })),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders the pending-work alerts with their counts', () => {
    render(<DashboardPage />);
    expect(screen.getByText('有 2 条工单等待处理')).toBeInTheDocument();
    expect(screen.getByText('有 3 笔佣金等待确认')).toBeInTheDocument();
  });

  it('shows income cents against the site currency (not the top-level currency)', () => {
    render(<DashboardPage />);
    // day_income 12345 → 123.45
    expect(screen.getByText('123.45')).toBeInTheDocument();
    expect(screen.getAllByText('CNY').length).toBeGreaterThan(0);
    expect(screen.queryByText('USD')).not.toBeInTheDocument();
    expect(screen.getByText('9')).toBeInTheDocument();
  });

  it('routes the ticket alert action to the ticket console', async () => {
    const user = userEvent.setup();
    render(<DashboardPage />);
    await user.click(screen.getByTestId('dashboard-ticket-alert').querySelector('button')!);
    expect(mocks.navigate).toHaveBeenCalledWith('/ticket');
  });

  it('stashes the pending-commission order filter before opening orders', async () => {
    const user = userEvent.setup();
    render(<DashboardPage />);
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

  it('navigates from each quick shortcut', async () => {
    const user = userEvent.setup();
    render(<DashboardPage />);

    await user.click(screen.getByRole('button', { name: '系统设置' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/config/system');
    await user.click(screen.getByRole('button', { name: '订单管理' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    await user.click(screen.getByRole('button', { name: '订阅管理' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/plan');
    await user.click(screen.getByRole('button', { name: '用户管理' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/user');
  });

  it('initializes an echarts host for each chart card', async () => {
    render(<DashboardPage />);
    // order chart + 4 rank charts (a theme settle may re-init, so assert ≥ 5).
    await waitFor(() => expect(mocks.echartsInit.mock.calls.length).toBeGreaterThanOrEqual(5));
  });
});
