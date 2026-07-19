import { screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import AuditPage from './audit';

// §6.11 operator audit trail: a read-only list of the append-only audit_log
// rows. The wire shape is pinned by the api-client contract schema and the
// backend production invariant; this covers the page behavior — the §7 filter
// clause the surface picker mints, pagination state, and the error path.

const mocks = vi.hoisted(() => ({
  useAuditLogs: vi.fn(),
  refetch: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAuditLogs: mocks.useAuditLogs,
}));

function auditRow(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    id: 1,
    actor_id: 1,
    actor_email: 'admin@example.com',
    session_id: 'session-1',
    surface: 'admin',
    method: 'PATCH',
    path: '/config',
    status_code: 200,
    client_ip: '203.0.113.7',
    request_id: 'req-1',
    created_at: '2026-07-18T03:00:00Z',
    ...overrides,
  };
}

describe('AuditPage', () => {
  beforeEach(() => {
    mocks.useAuditLogs.mockReset();
    mocks.refetch.mockReset();
    mocks.useAuditLogs.mockReturnValue({
      data: { items: [auditRow()], total: 1 },
      isError: false,
      isPending: false,
      refetch: mocks.refetch,
    });
  });

  it('renders the recorded mutations newest-first from the trail query', () => {
    renderWithProviders(<AuditPage />, { queryClient: true });

    expect(mocks.useAuditLogs).toHaveBeenCalledWith({
      page: 1,
      per_page: 20,
      filter: undefined,
    });
    const table = screen.getByTestId('audit-table');
    expect(within(table).getByText('admin@example.com')).toBeInTheDocument();
    expect(within(table).getByText('PATCH')).toBeInTheDocument();
    expect(within(table).getByText('/config')).toBeInTheDocument();
    expect(within(table).getByText('200')).toBeInTheDocument();
    expect(within(table).getByText('203.0.113.7')).toBeInTheDocument();
  });

  it('mints a §7 surface filter clause from the surface picker', async () => {
    const { user } = renderWithProviders(<AuditPage />, { queryClient: true });

    await user.click(screen.getByTestId('audit-surface-filter'));
    await user.click(await screen.findByRole('option', { name: '员工' }));

    expect(mocks.useAuditLogs).toHaveBeenLastCalledWith({
      page: 1,
      per_page: 20,
      filter: [{ field: 'surface', op: 'eq', value: 'staff' }],
    });
  });

  it('resets to the first page when the surface filter changes', async () => {
    mocks.useAuditLogs.mockReturnValue({
      data: { items: [auditRow()], total: 100 },
      isError: false,
      isPending: false,
      refetch: mocks.refetch,
    });
    const { user } = renderWithProviders(<AuditPage />, { queryClient: true });

    await user.click(screen.getByRole('button', { name: '下一页' }));
    expect(mocks.useAuditLogs).toHaveBeenLastCalledWith(
      expect.objectContaining({ page: 2, per_page: 20 }),
    );

    await user.click(screen.getByTestId('audit-surface-filter'));
    await user.click(await screen.findByRole('option', { name: '管理员' }));
    expect(mocks.useAuditLogs).toHaveBeenLastCalledWith({
      page: 1,
      per_page: 20,
      filter: [{ field: 'surface', op: 'eq', value: 'admin' }],
    });
  });

  it('shows the empty state without inventing rows', () => {
    mocks.useAuditLogs.mockReturnValue({
      data: { items: [], total: 0 },
      isError: false,
      isPending: false,
      refetch: mocks.refetch,
    });
    renderWithProviders(<AuditPage />, { queryClient: true });

    expect(screen.getByTestId('audit-empty')).toHaveTextContent('暂无审计记录');
  });

  it('surfaces a failed fetch with a retry that refetches', async () => {
    mocks.useAuditLogs.mockReturnValue({
      data: undefined,
      isError: true,
      isPending: false,
      refetch: mocks.refetch,
    });
    const { user } = renderWithProviders(<AuditPage />, { queryClient: true });

    const error = screen.getByTestId('audit-error');
    expect(error).toHaveTextContent('审计日志加载失败');
    await user.click(within(error).getByRole('button'));
    expect(mocks.refetch).toHaveBeenCalledTimes(1);
  });
});
