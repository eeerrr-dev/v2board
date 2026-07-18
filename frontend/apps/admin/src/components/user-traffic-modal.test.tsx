import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { UserTrafficModal } from './user-traffic-modal';

// The traffic modal is a redesigned shadcn island (Dialog + DataTable +
// PaginationControl) replacing the ant-modal / ant-table replica. The DOM
// byte-pins are retired; what stays covered is the Tier-1 fetch contract:
// `stats/user-traffic` (§6.8, W14) is queried with `{ current, pageSize }`,
// gated on open && userId != null, and jumps back to page 1 when opened for
// another user.

const mocks = vi.hoisted(() => ({
  useAdminUserTraffic: vi.fn(),
  refetch: vi.fn(),
}));

vi.mock('@/lib/queries', () => ({
  useAdminUserTraffic: mocks.useAdminUserTraffic,
}));

beforeEach(() => {
  mocks.useAdminUserTraffic.mockReset();
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.useAdminUserTraffic.mockReturnValue({
    data: {
      data: [{ record_at: '2023-11-14T22:13:20Z', u: 1024, d: 2048, server_rate: 1 }],
      total: 25,
    },
    isPending: false,
    isError: false,
    isFetching: false,
    refetch: mocks.refetch,
  });
});

describe('UserTrafficModal', () => {
  it('fetches the first page and renders formatted traffic rows when open', () => {
    render(<UserTrafficModal userId={1} open onClose={() => undefined} />);

    expect(mocks.useAdminUserTraffic).toHaveBeenCalledWith(1, { current: 1, pageSize: 10 }, true);

    const modal = screen.getByTestId('user-traffic-modal');
    expect(within(modal).getByText('流量记录')).toBeInTheDocument();
    const table = within(modal).getByTestId('user-traffic-table');
    expect(within(table).getByText('2023-11-14')).toBeInTheDocument();
    expect(within(table).getByText('1024.00 B')).toBeInTheDocument();
    expect(within(table).getByText('2.00 KB')).toBeInTheDocument();
    expect(within(table).getByText('1')).toBeInTheDocument();
  });

  it('gates the fetch on open and renders nothing when closed', () => {
    render(<UserTrafficModal userId={1} open={false} onClose={() => undefined} />);

    expect(mocks.useAdminUserTraffic).toHaveBeenCalledWith(1, { current: 1, pageSize: 10 }, false);
    expect(screen.queryByTestId('user-traffic-modal')).not.toBeInTheDocument();
  });

  it('renders an explicit loading state without an empty traffic table', () => {
    mocks.useAdminUserTraffic.mockReturnValue({
      data: undefined,
      isPending: true,
      isError: false,
      isFetching: true,
      refetch: mocks.refetch,
    });

    render(<UserTrafficModal userId={1} open onClose={() => undefined} />);

    expect(screen.getByTestId('user-traffic-loading')).toHaveAttribute('role', 'status');
    expect(screen.queryByTestId('user-traffic-table')).toBeNull();
    expect(screen.queryByTestId('user-traffic-empty')).toBeNull();
  });

  it('surfaces and retries a traffic failure even when stale rows remain cached', async () => {
    mocks.useAdminUserTraffic.mockReturnValue({
      data: {
        data: [{ record_at: '2023-11-14T22:13:20Z', u: 1024, d: 2048, server_rate: 1 }],
        total: 1,
      },
      isPending: false,
      isError: true,
      isFetching: false,
      refetch: mocks.refetch,
    });
    const user = userEvent.setup();

    render(<UserTrafficModal userId={1} open onClose={() => undefined} />);

    const error = screen.getByTestId('user-traffic-error');
    expect(error).toHaveTextContent('流量记录加载失败');
    expect(screen.queryByTestId('user-traffic-table')).toBeNull();
    expect(screen.queryByTestId('user-traffic-empty')).toBeNull();

    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.refetch).toHaveBeenCalledTimes(1);
  });

  it('renders the table empty state only after a successful empty response', () => {
    mocks.useAdminUserTraffic.mockReturnValue({
      data: { data: [], total: 0 },
      isPending: false,
      isError: false,
      isFetching: false,
      refetch: mocks.refetch,
    });

    render(<UserTrafficModal userId={1} open onClose={() => undefined} />);

    expect(screen.getByTestId('user-traffic-empty')).toHaveTextContent('暂无数据');
    expect(screen.queryByTestId('user-traffic-loading')).toBeNull();
    expect(screen.queryByTestId('user-traffic-error')).toBeNull();
  });

  it('sends the new page on pagination change and resets when opening another user', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<UserTrafficModal userId={1} open onClose={() => undefined} />);

    await user.click(screen.getByRole('button', { name: '2' }));

    expect(mocks.useAdminUserTraffic).toHaveBeenLastCalledWith(
      1,
      { current: 2, pageSize: 10 },
      true,
    );

    rerender(<UserTrafficModal userId={2} open onClose={() => undefined} />);

    await waitFor(() =>
      expect(mocks.useAdminUserTraffic).toHaveBeenLastCalledWith(
        2,
        { current: 1, pageSize: 10 },
        true,
      ),
    );
  });
});
