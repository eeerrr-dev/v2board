import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import OrdersPage from './orders';

// The admin order manager is a redesigned shadcn island (PageHeader + DataTable
// + a Sheet detail + a Dialog assign form) replacing the ant-table /
// filter-drawer / ant-modal replica. The DOM and source byte-pins are retired.
// What stays covered is the Tier-1 contract: the order-fetch query shape
// ({ current, pageSize, filter }), the cross-page sessionStorage order filter
// read-apply-clear on mount, the assign / paid / cancel / commission-update
// payloads, the detail user-filter seeding, and the cents amount interpretation.

const ORDER_PENDING = {
  id: 1,
  trade_no: '202601010001',
  callback_no: null,
  plan_id: 1,
  period: 'month_price',
  type: 1,
  total_amount: 1200,
  handling_amount: null,
  discount_amount: 0,
  surplus_amount: 0,
  refund_amount: 0,
  balance_amount: 0,
  surplus_order_ids: null,
  status: 0,
  commission_status: 0,
  commission_balance: 0,
  coupon_id: null,
  payment_id: null,
  invite_user_id: null,
  paid_at: null,
  created_at: 1700000000,
  updated_at: 1700000000,
  user_id: 1,
  plan_name: '基础套餐',
};

const ORDER_COMPLETED = {
  id: 2,
  trade_no: '202601010002',
  callback_no: 'cb-1',
  plan_id: 1,
  period: 'year_price',
  type: 2,
  total_amount: 8800,
  handling_amount: null,
  discount_amount: 0,
  surplus_amount: 0,
  refund_amount: 0,
  balance_amount: 0,
  surplus_order_ids: null,
  status: 3,
  commission_status: 1,
  commission_balance: 1200,
  coupon_id: null,
  payment_id: null,
  invite_user_id: 3,
  paid_at: null,
  created_at: 1700086400,
  updated_at: 1700086400,
  user_id: 2,
  plan_name: '年度套餐',
};

const mocks = vi.hoisted(() => ({
  orderQueries: [] as Array<Record<string, unknown>>,
  total: 2,
  navigate: vi.fn(),
  refetch: vi.fn(),
  detail: undefined as Record<string, unknown> | undefined,
  detailPending: false,
  detailError: false,
  detailRefetch: vi.fn(),
  userInfo: undefined as Record<string, unknown> | undefined,
  userInfoPending: false,
  userInfoError: false,
  userInfoRefetch: vi.fn(),
  inviteUserInfo: undefined as Record<string, unknown> | undefined,
  inviteUserInfoPending: false,
  inviteUserInfoError: false,
  inviteUserInfoRefetch: vi.fn(),
  assignMutateAsync: vi.fn(),
  paidMutateAsync: vi.fn(),
  cancelMutateAsync: vi.fn(),
  updateMutateAsync: vi.fn(),
  confirm: vi.fn(),
  plansError: false,
  plansRefetch: vi.fn(),
}));

vi.mock('react-router', () => ({ useNavigate: () => mocks.navigate }));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/queries', () => ({
  useAdminOrders: (query: Record<string, unknown>) => {
    mocks.orderQueries.push(query);
    return {
      isPending: false,
      isFetching: false,
      refetch: mocks.refetch,
      data: { data: [ORDER_PENDING, ORDER_COMPLETED], total: mocks.total },
    };
  },
  useAdminPlans: () => ({
    data: mocks.plansError ? undefined : [{ id: 1, name: '基础套餐' }],
    isError: mocks.plansError,
    refetch: mocks.plansRefetch,
  }),
  useAdminOrderDetail: () => ({
    data: mocks.detail,
    isPending: mocks.detailPending,
    isError: mocks.detailError,
    refetch: mocks.detailRefetch,
  }),
  useAdminUserInfo: (id?: number | null) => {
    const invite = id === 3;
    return {
      data: id == null ? undefined : invite ? mocks.inviteUserInfo : mocks.userInfo,
      isPending: id != null && (invite ? mocks.inviteUserInfoPending : mocks.userInfoPending),
      isError: id != null && (invite ? mocks.inviteUserInfoError : mocks.userInfoError),
      refetch: invite ? mocks.inviteUserInfoRefetch : mocks.userInfoRefetch,
    };
  },
  useAssignOrderMutation: () => ({ isPending: false, mutateAsync: mocks.assignMutateAsync }),
  useMarkOrderPaidMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.paidMutateAsync(payload)).then(options?.onSuccess);
    },
  }),
  useCancelOrderMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.cancelMutateAsync(payload)).then(options?.onSuccess);
    },
  }),
  useUpdateOrderMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.updateMutateAsync(payload)).then(options?.onSuccess);
    },
  }),
}));

const ORDER_FILTER_KEY = 'v2board-admin-order-filter';
const USER_FILTER_KEY = 'v2board-admin-user-filter';

beforeEach(() => {
  window.sessionStorage.clear();
  mocks.orderQueries = [];
  mocks.total = 2;
  mocks.navigate.mockReset();
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.detail = undefined;
  mocks.detailPending = false;
  mocks.detailError = false;
  mocks.detailRefetch.mockReset().mockResolvedValue(undefined);
  mocks.userInfo = undefined;
  mocks.userInfoPending = false;
  mocks.userInfoError = false;
  mocks.userInfoRefetch.mockReset().mockResolvedValue(undefined);
  mocks.inviteUserInfo = undefined;
  mocks.inviteUserInfoPending = false;
  mocks.inviteUserInfoError = false;
  mocks.inviteUserInfoRefetch.mockReset().mockResolvedValue(undefined);
  mocks.assignMutateAsync.mockReset().mockResolvedValue('trade');
  mocks.paidMutateAsync.mockReset().mockResolvedValue(true);
  mocks.cancelMutateAsync.mockReset().mockResolvedValue(true);
  mocks.updateMutateAsync.mockReset().mockResolvedValue(true);
  mocks.confirm.mockReset().mockResolvedValue(true);
  mocks.plansError = false;
  mocks.plansRefetch.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('OrdersPage', () => {
  it('renders order rows with type, period, cents amounts, and status labels', () => {
    render(<OrdersPage />);

    expect(screen.getByText('订单管理')).toBeInTheDocument();
    const table = screen.getByTestId('orders-table');
    expect(within(table).getByText('202601010001')).toBeInTheDocument();
    expect(within(table).getByText('新购')).toBeInTheDocument();
    expect(within(table).getByText('月付')).toBeInTheDocument();
    // 12.00 appears twice: order 1 payment amount and order 2 commission amount.
    expect(within(table).getAllByText('12.00').length).toBe(2);
    expect(within(table).getByText('待支付')).toBeInTheDocument();
    expect(within(table).getByText('续费')).toBeInTheDocument();
    expect(within(table).getByText('88.00')).toBeInTheDocument();
    expect(within(table).getByText('已完成')).toBeInTheDocument();
    expect(within(table).getByText('发放中')).toBeInTheDocument();
    expect(
      within(table).getByText(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm')),
    ).toBeInTheDocument();
  });

  it('blocks order assignment and retries when plans fail to load', async () => {
    mocks.plansError = true;
    const user = userEvent.setup();
    render(<OrdersPage />);

    expect(screen.getByText('订阅列表加载失败')).toBeInTheDocument();
    expect(screen.getByTestId('order-assign-open')).toBeDisabled();
    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.plansRefetch).toHaveBeenCalledOnce();
  });

  it('fetches the first page with the { current, pageSize, filter } shape', () => {
    render(<OrdersPage />);
    expect(mocks.orderQueries[0]).toMatchObject({ current: 1, pageSize: 10, filter: [] });
  });

  it('reads, applies, and clears the sessionStorage order filter on mount', () => {
    const stored = [
      { key: 'status', condition: '=', value: '3' },
      { key: 'commission_status', condition: '=', value: '0' },
      { key: 'commission_balance', condition: '>', value: '0' },
    ];
    window.sessionStorage.setItem(ORDER_FILTER_KEY, JSON.stringify(stored));

    render(<OrdersPage />);

    expect(mocks.orderQueries[0]).toMatchObject({ current: 1, pageSize: 10, filter: stored });
    expect(window.sessionStorage.getItem(ORDER_FILTER_KEY)).toBeNull();
  });

  it('searches by trade number through the debounced fuzzy filter and resets to page 1', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);
    mocks.orderQueries = [];

    await user.type(screen.getByTestId('order-search'), '2026');

    await waitFor(() =>
      expect(mocks.orderQueries[mocks.orderQueries.length - 1]).toMatchObject({
        current: 1,
        filter: [{ key: 'trade_no', condition: '模糊', value: '2026' }],
      }),
    );
  });

  it('filters by order status via the dropdown with the = condition', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);
    mocks.orderQueries = [];

    await user.click(screen.getByTestId('order-status-filter'));
    await user.click(await screen.findByRole('menuitemradio', { name: '已完成' }));

    expect(mocks.orderQueries[mocks.orderQueries.length - 1]).toMatchObject({
      current: 1,
      filter: [{ key: 'status', condition: '=', value: '3' }],
    });
  });

  it('marks a pending order as paid by trade number', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-status-trigger-202601010001'));
    await user.click(await screen.findByTestId('order-mark-paid-202601010001'));

    expect(mocks.paidMutateAsync).toHaveBeenCalledWith('202601010001');
  });

  it('cancels a pending order after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-status-trigger-202601010001'));
    await user.click(await screen.findByTestId('order-cancel-202601010001'));

    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.cancelMutateAsync).toHaveBeenCalledWith('202601010001'));
  });

  it('does not cancel when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-status-trigger-202601010001'));
    await user.click(await screen.findByTestId('order-cancel-202601010001'));

    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.cancelMutateAsync).not.toHaveBeenCalled();
  });

  it('updates commission status with the commission_status key and string value', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('commission-status-trigger-202601010002'));
    await user.click(await screen.findByRole('menuitem', { name: '无效' }));

    expect(mocks.updateMutateAsync).toHaveBeenCalledWith({
      tradeNo: '202601010002',
      key: 'commission_status',
      value: '3',
    });
  });

  it('opens the detail sheet and seeds the user filter before navigating to /user', async () => {
    mocks.detail = { ...ORDER_PENDING };
    mocks.userInfo = { email: 'buyer@example.com' };
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010001'));
    const sheet = await screen.findByTestId('order-detail');
    expect(within(sheet).getByText('202601010001')).toBeInTheDocument();

    await user.click(within(sheet).getByTestId('order-detail-user'));

    expect(window.sessionStorage.getItem(USER_FILTER_KEY)).toBe(
      JSON.stringify([{ key: 'email', condition: '模糊', value: 'buyer@example.com' }]),
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/user');
  });

  it('renders an explicit loading state while the order detail request is pending', async () => {
    mocks.detailPending = true;
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010001'));
    const sheet = await screen.findByTestId('order-detail');

    expect(within(sheet).getByTestId('order-detail-loading')).toHaveAttribute('role', 'status');
    expect(within(sheet).queryByTestId('order-detail-empty')).toBeNull();
  });

  it('surfaces and retries an order-detail failure instead of spinning forever', async () => {
    mocks.detailError = true;
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010001'));
    const error = await screen.findByTestId('order-detail-error');
    expect(error).toHaveTextContent('订单详情加载失败');
    expect(screen.queryByTestId('order-detail-loading')).toBeNull();

    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.detailRefetch).toHaveBeenCalledTimes(1);
  });

  it('renders a real empty detail result instead of retaining the loading spinner', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010001'));

    expect(await screen.findByTestId('order-detail-empty')).toHaveTextContent('暂无订单详情');
    expect(screen.queryByTestId('order-detail-loading')).toBeNull();
  });

  it('surfaces and retries the nested order-user failure', async () => {
    mocks.detail = { ...ORDER_PENDING };
    mocks.userInfoError = true;
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010001'));
    const error = await screen.findByTestId('order-detail-user-error');
    expect(error).toHaveTextContent('订单用户加载失败');
    expect(screen.queryByTestId('order-detail-user-empty')).toBeNull();

    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.userInfoRefetch).toHaveBeenCalledTimes(1);
  });

  it('renders an explicit empty state when the nested order-user result is absent', async () => {
    mocks.detail = { ...ORDER_PENDING };
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010001'));

    expect(await screen.findByTestId('order-detail-user-empty')).toHaveTextContent(
      '未找到订单用户',
    );
    expect(screen.queryByTestId('order-detail-user-loading')).toBeNull();
  });

  it('surfaces and retries the nested invite-user failure', async () => {
    mocks.detail = { ...ORDER_COMPLETED };
    mocks.userInfo = { email: 'buyer@example.com' };
    mocks.inviteUserInfoError = true;
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010002'));
    const error = await screen.findByTestId('order-detail-invite-error');
    expect(error).toHaveTextContent('邀请人信息加载失败');
    expect(screen.queryByTestId('order-detail-invite-empty')).toBeNull();

    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.inviteUserInfoRefetch).toHaveBeenCalledTimes(1);
  });

  it('distinguishes nested invite-user loading from a successful empty result', async () => {
    mocks.detail = { ...ORDER_COMPLETED };
    mocks.userInfo = { email: 'buyer@example.com' };
    mocks.inviteUserInfoPending = true;
    const user = userEvent.setup();
    const { rerender } = render(<OrdersPage />);

    await user.click(screen.getByTestId('order-open-202601010002'));
    expect(await screen.findByTestId('order-detail-invite-loading')).toHaveAttribute(
      'role',
      'status',
    );
    expect(screen.queryByTestId('order-detail-invite-empty')).toBeNull();

    mocks.inviteUserInfoPending = false;
    rerender(<OrdersPage />);

    expect(await screen.findByTestId('order-detail-invite-empty')).toHaveTextContent(
      '未找到邀请人',
    );
    expect(screen.queryByTestId('order-detail-invite-loading')).toBeNull();
  });

  it('assigns an order with the raw (un-multiplied) total_amount payload', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-assign-open'));
    const dialog = await screen.findByTestId('order-assign-dialog');
    await user.type(within(dialog).getByTestId('order-assign-email'), 'buyer@example.com');
    const selects = within(dialog).getAllByRole('combobox');
    await user.click(selects[0]!);
    await user.click(await screen.findByRole('option', { name: '基础套餐' }));
    await user.click(selects[1]!);
    await user.click(await screen.findByRole('option', { name: '月付' }));
    await user.type(within(dialog).getByTestId('order-assign-amount'), '50');
    await user.click(screen.getByTestId('order-assign-submit'));

    await waitFor(() =>
      expect(mocks.assignMutateAsync).toHaveBeenCalledWith({
        email: 'buyer@example.com',
        plan_id: 1,
        period: 'month_price',
        total_amount: '50',
      }),
    );
  });

  it('blocks an incomplete assignment and renders field errors without calling the API', async () => {
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-assign-open'));
    await user.click(await screen.findByTestId('order-assign-submit'));

    expect(mocks.assignMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByText('邮箱不能为空')).toBeInTheDocument();
    expect(screen.getByText('订阅不能为空')).toBeInTheDocument();
    expect(screen.getByText('订阅周期不能为空')).toBeInTheDocument();
    expect(screen.getByText('支付金额不能为空')).toBeInTheDocument();
  });

  it('keeps the assignment dialog open and surfaces a server failure inline', async () => {
    mocks.assignMutateAsync.mockRejectedValue(new Error('该用户不存在'));
    const user = userEvent.setup();
    render(<OrdersPage />);

    await user.click(screen.getByTestId('order-assign-open'));
    const dialog = await screen.findByTestId('order-assign-dialog');
    await user.type(within(dialog).getByTestId('order-assign-email'), 'missing@example.com');
    const selects = within(dialog).getAllByRole('combobox');
    await user.click(selects[0]!);
    await user.click(await screen.findByRole('option', { name: '基础套餐' }));
    await user.click(selects[1]!);
    await user.click(await screen.findByRole('option', { name: '月付' }));
    await user.type(within(dialog).getByTestId('order-assign-amount'), '50');
    await user.click(within(dialog).getByTestId('order-assign-submit'));

    expect(await within(dialog).findByText('该用户不存在')).toBeInTheDocument();
    expect(screen.getByTestId('order-assign-dialog')).toBeInTheDocument();
  });

  it('sends the new current on pagination change', async () => {
    mocks.total = 25;
    const user = userEvent.setup();
    render(<OrdersPage />);
    mocks.orderQueries = [];

    await user.click(screen.getByRole('button', { name: '2' }));

    expect(mocks.orderQueries[mocks.orderQueries.length - 1]).toMatchObject({
      current: 2,
      pageSize: 10,
    });
  });
});
