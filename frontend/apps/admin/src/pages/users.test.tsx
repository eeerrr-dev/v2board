import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import UsersPage from './users';

// The admin user manager is a redesigned shadcn island (PageHeader + DataTable +
// a Sheet filter builder + a Sheet edit drawer + Dialog create/mail/assign
// forms) replacing the ant-table / filter-drawer / ant-modal replica. The DOM
// and source byte-pins are retired. What stays covered is the Tier-1 contract:
// the fetch query shape ({ current, pageSize, filter }), the cross-page
// sessionStorage user filter read-apply-clear, the filter-builder payload shape
// ({ key, condition, value }[]), the drawer save payload (exact keys + GB/cents
// scaling + date→unix-seconds coercion) for edit and the generate-user payload
// for create, the bulk ban payload, per-row delete confirm-gating, and the
// copied subscribe URL.

const USER_ROW_ONE = {
  id: 1,
  email: 'user@example.com',
  password: '',
  balance: '12.00',
  commission_balance: '34.00',
  transfer_enable: '100.00',
  device_limit: 3,
  u: 0,
  d: 0,
  total_used: '5.00',
  alive_ip: 2,
  ips: '127.0.0.1',
  plan_id: 1,
  plan_name: '基础套餐',
  group_id: 1,
  expired_at: 1893456000,
  uuid: 'uuid',
  token: 'token',
  subscribe_url: 'https://example.com/sub',
  banned: 0,
  is_admin: 0,
  is_staff: 0,
  invite_user_id: null,
  discount: null,
  commission_rate: null,
  telegram_id: null,
  last_login_at: 1700000000,
  created_at: 1700000000,
  updated_at: 1700000000,
};

const USER_ROW_TWO = {
  ...USER_ROW_ONE,
  id: 2,
  email: 'blocked@example.com',
  balance: '0.00',
  commission_balance: '0.00',
  transfer_enable: '1.00',
  device_limit: null,
  total_used: '2.00',
  alive_ip: 0,
  ips: '',
  plan_id: null,
  plan_name: null,
  group_id: null,
  expired_at: null,
  subscribe_url: '',
  banned: 1,
};

// getUserInfoById returns the same edit-form shape the drawer reads on open.
const USER_INFO = {
  id: 1,
  email: 'user@example.com',
  balance: '12.00',
  commission_balance: '34.00',
  transfer_enable: '100.00',
  device_limit: 3,
  u: 0,
  d: 0,
  plan_id: 1,
  expired_at: 1893456000,
  banned: 0,
  is_admin: 0,
  is_staff: 0,
};

const mocks = vi.hoisted(() => ({
  userQueries: [] as Array<Record<string, unknown>>,
  navigate: vi.fn(),
  refetch: vi.fn(),
  removeQueries: vi.fn(),
  invalidateQueries: vi.fn(),
  confirm: vi.fn(),
  writeText: vi.fn(),
  remove: vi.fn(),
  reset: vi.fn(),
  generate: vi.fn(),
  dumpCsv: vi.fn(),
  sendMail: vi.fn(),
  ban: vi.fn(),
  deleteAll: vi.fn(),
  assign: vi.fn(),
  update: vi.fn(),
}));

vi.mock('react-router', () => ({ useNavigate: () => mocks.navigate }));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    removeQueries: mocks.removeQueries,
    invalidateQueries: mocks.invalidateQueries,
  }),
}));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/toast', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    loading: vi.fn(() => 'toast-id'),
    dismiss: vi.fn(),
    destroy: vi.fn(),
  },
}));

vi.mock('@/lib/queries', () => ({
  useAdminUsers: (query: Record<string, unknown>) => {
    mocks.userQueries.push(query);
    return {
      isPending: false,
      isFetching: false,
      refetch: mocks.refetch,
      data: { data: [USER_ROW_ONE, USER_ROW_TWO], total: 2 },
    };
  },
  useAdminPlans: () => ({ data: [{ id: 1, name: '基础套餐' }] }),
  useServerGroups: () => ({ data: [{ id: 1, name: '默认权限组' }] }),
  useAdminUserInfo: () => ({ data: USER_INFO }),
  useAdminUserTraffic: () => ({ data: { data: [], total: 0 }, isFetching: false }),
  useUpdateUserMutation: () => ({ isPending: false, mutateAsync: mocks.update }),
  useDeleteUserMutation: () => ({ mutateAsync: mocks.remove }),
  useResetUserSecretMutation: () => ({ mutateAsync: mocks.reset }),
  useGenerateUserMutation: () => ({ isPending: false, mutateAsync: mocks.generate }),
  useDumpUsersCsvMutation: () => ({ mutateAsync: mocks.dumpCsv }),
  useSendMailToUsersMutation: () => ({ isPending: false, mutateAsync: mocks.sendMail }),
  useBanUsersMutation: () => ({ mutateAsync: mocks.ban }),
  useDeleteAllUsersMutation: () => ({ mutateAsync: mocks.deleteAll }),
  useAssignOrderMutation: () => ({ isPending: false, mutateAsync: mocks.assign }),
}));

const USER_FILTER_KEY = 'v2board-admin-user-filter';
const ORDER_FILTER_KEY = 'v2board-admin-order-filter';

beforeEach(() => {
  window.sessionStorage.clear();
  mocks.userQueries = [];
  mocks.navigate.mockReset();
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.removeQueries.mockReset();
  mocks.invalidateQueries.mockReset().mockResolvedValue(undefined);
  mocks.confirm.mockReset().mockResolvedValue(true);
  mocks.writeText.mockReset();
  mocks.remove.mockReset().mockResolvedValue(true);
  mocks.reset.mockReset().mockResolvedValue(true);
  mocks.generate.mockReset().mockResolvedValue({ buffer: new Uint8Array() });
  mocks.dumpCsv.mockReset().mockResolvedValue({ buffer: new Uint8Array() });
  mocks.sendMail.mockReset().mockResolvedValue(true);
  mocks.ban.mockReset().mockResolvedValue(true);
  mocks.deleteAll.mockReset().mockResolvedValue(true);
  mocks.assign.mockReset().mockResolvedValue('trade');
  mocks.update.mockReset().mockResolvedValue(true);
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: mocks.writeText },
  });
});

describe('UsersPage', () => {
  it('renders user rows with email, plan, group, status, and usage values', () => {
    render(<UsersPage />);

    expect(screen.getByText('用户管理')).toBeInTheDocument();
    const table = screen.getByTestId('users-table');
    expect(within(table).getByText('user@example.com')).toBeInTheDocument();
    expect(within(table).getByText('基础套餐')).toBeInTheDocument();
    expect(within(table).getByText('默认权限组')).toBeInTheDocument();
    expect(within(table).getByText('正常')).toBeInTheDocument();
    expect(within(table).getByText('封禁')).toBeInTheDocument();
    expect(within(table).getByText('5.00')).toBeInTheDocument();
    expect(within(table).getByText('长期有效')).toBeInTheDocument();
    expect(within(table).getByText('2 / 3')).toBeInTheDocument();
  });

  it('fetches the first page with the { current, pageSize, filter } shape', () => {
    render(<UsersPage />);
    expect(mocks.userQueries[0]).toMatchObject({ current: 1, pageSize: 10, filter: [] });
  });

  it('reads, applies, and clears the sessionStorage user filter on mount', () => {
    const stored = [{ key: 'invite_user_id', condition: '=', value: 7 }];
    window.sessionStorage.setItem(USER_FILTER_KEY, JSON.stringify(stored));

    render(<UsersPage />);

    expect(mocks.userQueries[0]).toMatchObject({ current: 1, pageSize: 10, filter: stored });
    expect(window.sessionStorage.getItem(USER_FILTER_KEY)).toBeNull();
  });

  it('builds a { key, condition, value }[] filter through the filter sheet', async () => {
    const user = userEvent.setup();
    render(<UsersPage />);
    mocks.userQueries = [];

    await user.click(screen.getByTestId('user-filter-open'));
    await user.click(await screen.findByTestId('user-filter-add'));
    await user.type(await screen.findByTestId('user-filter-value-0'), 'foo');
    await user.click(screen.getByTestId('user-filter-apply'));

    await waitFor(() =>
      expect(mocks.userQueries[mocks.userQueries.length - 1]).toMatchObject({
        current: 1,
        filter: [{ key: 'email', condition: '模糊', value: 'foo' }],
      }),
    );
  });

  it('saves the edited user with GB/cents scaling and the untouched expired_at passthrough', async () => {
    const user = userEvent.setup();
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-actions-1'));
    await user.click(await screen.findByTestId('user-edit-1'));
    await user.click(await screen.findByTestId('user-manage-submit'));

    await waitFor(() => expect(mocks.update).toHaveBeenCalled());
    expect(mocks.update).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 1,
        email: 'user@example.com',
        transfer_enable: 100 * 1_073_741_824,
        u: 0,
        d: 0,
        balance: 1200,
        commission_balance: 3400,
        expired_at: 1893456000,
        is_admin: 0,
        is_staff: 0,
      }),
    );
  });

  it('coerces a newly picked expiry date to a unix-seconds string in the save payload', async () => {
    const user = userEvent.setup();
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-actions-1'));
    await user.click(await screen.findByTestId('user-edit-1'));
    // Pick a date distinct from the pre-filled expired_at (1893456000 ≈
    // 2030-01-01) so the controlled input actually fires a change event; then the
    // payload must carry the coerced value, not the untouched passthrough number.
    fireEvent.change(await screen.findByTestId('user-drawer-expired'), {
      target: { value: '2018-08-08' },
    });
    await user.click(screen.getByTestId('user-manage-submit'));

    await waitFor(() => expect(mocks.update).toHaveBeenCalled());
    expect(mocks.update).toHaveBeenCalledWith(
      expect.objectContaining({ expired_at: dayjs('2018-08-08').format('X') }),
    );
  });

  it('creates users with the email/date generate payload (date → unix seconds)', async () => {
    const user = userEvent.setup();
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-create'));
    await user.type(await screen.findByTestId('generate-email-prefix'), 'admin');
    await user.type(screen.getByTestId('generate-email-suffix'), 'test.com');
    fireEvent.change(screen.getByTestId('generate-expired'), { target: { value: '2030-01-01' } });
    await user.click(screen.getByTestId('generate-submit'));

    await waitFor(() => expect(mocks.generate).toHaveBeenCalled());
    expect(mocks.generate).toHaveBeenCalledWith(
      expect.objectContaining({
        email_prefix: 'admin',
        email_suffix: 'test.com',
        expired_at: dayjs('2030-01-01').format('X'),
      }),
    );
  });

  it('bulk-bans the current filter after the confirm dialog resolves true', async () => {
    const filter = [{ key: 'banned', condition: '=', value: 1 }];
    window.sessionStorage.setItem(USER_FILTER_KEY, JSON.stringify(filter));
    const user = userEvent.setup();
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-bulk-actions'));
    await user.click(await screen.findByTestId('user-bulk-ban'));

    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.ban).toHaveBeenCalledWith(filter));
  });

  it('deletes a user by id after confirmation, and skips when dismissed', async () => {
    const user = userEvent.setup();
    const { unmount } = render(<UsersPage />);

    await user.click(screen.getByTestId('user-actions-1'));
    await user.click(await screen.findByTestId('user-delete-1'));
    await waitFor(() => expect(mocks.remove).toHaveBeenCalledWith(1));

    mocks.remove.mockClear();
    mocks.confirm.mockResolvedValue(false);
    unmount();
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-actions-1'));
    await user.click(await screen.findByTestId('user-delete-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.remove).not.toHaveBeenCalled();
  });

  it('copies the row subscribe URL to the clipboard', async () => {
    const user = userEvent.setup();
    // user-event's setup() installs its own navigator.clipboard, overriding the
    // beforeEach stub; re-stub AFTER setup so the copy writes through our spy.
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: mocks.writeText },
    });
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-actions-1'));
    await user.click(await screen.findByTestId('user-copy-1'));

    expect(mocks.writeText).toHaveBeenCalledWith('https://example.com/sub');
  });

  it('seeds the order filter and navigates to /order for a user’s orders', async () => {
    const user = userEvent.setup();
    render(<UsersPage />);

    await user.click(screen.getByTestId('user-actions-1'));
    await user.click(await screen.findByRole('menuitem', { name: 'TA的订单' }));

    expect(window.sessionStorage.getItem(ORDER_FILTER_KEY)).toBe(
      JSON.stringify({ filter: [{ key: 'user_id', condition: '=', value: 1 }], total: 2 }),
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/order');
  });
});
