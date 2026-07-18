import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import dayjs from 'dayjs';
import TicketsPage from './tickets';

// The admin ticket console is a redesigned shadcn island (PageHeader + DataTable
// + a Sheet chat panel) replacing the ant-table / ant-dropdown / OneUI chat
// replica. The DOM and source byte-pins are retired. What stays covered is the
// Tier-1 contract: the ticket-fetch query shape, the close-ticket call
// (ticket-id passthrough), the reply payload ({ id, message }) with the same
// ticket-id passthrough, and the /ticket/:ticket_id route rendering the chat.

const mocks = vi.hoisted(() => {
  // §6.5 (W14): timestamps cross the wire as RFC 3339 instants.
  const OPEN_TICKET = {
    id: 1,
    user_id: 7,
    subject: '支付问题',
    level: 2,
    status: 0,
    reply_status: 0,
    last_reply_user_id: null,
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-15T22:13:20Z',
  };

  const CLOSED_TICKET = {
    id: 2,
    user_id: 8,
    subject: '已完成',
    level: 0,
    status: 1,
    reply_status: 1,
    last_reply_user_id: null,
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-15T22:13:20Z',
  };

  const makeDetail = () => ({
    ...OPEN_TICKET,
    message: [
      {
        id: 1,
        user_id: 7,
        ticket_id: 1,
        message: '用户消息',
        is_me: false,
        created_at: '2023-11-14T22:13:20Z',
        updated_at: '2023-11-14T22:13:20Z',
      },
      {
        id: 2,
        user_id: 1,
        ticket_id: 1,
        message: '客服回复',
        is_me: true,
        created_at: '2023-11-15T22:13:20Z',
        updated_at: '2023-11-15T22:13:20Z',
      },
    ],
  });

  return {
    OPEN_TICKET,
    CLOSED_TICKET,
    makeDetail,
    params: {} as Record<string, string>,
    ticketQueries: [] as Array<Record<string, unknown>>,
    refetch: vi.fn(),
    ticketRefetch: vi.fn(),
    closeMutate: vi.fn(),
    replyMutateAsync: vi.fn(),
    userInfoIds: [] as Array<number | null | undefined>,
    detail: makeDetail() as ReturnType<typeof makeDetail> | undefined,
    detailError: undefined as unknown,
    confirm: vi.fn(),
    toastLoading: vi.fn(),
    toastDismiss: vi.fn(),
    toastSuccess: vi.fn(),
  };
});

vi.mock('react-router', () => ({ useParams: () => mocks.params }));

vi.mock('@/components/user-manage-drawer', () => ({
  UserManageDrawer: ({ open }: { open: boolean }) =>
    open ? <div data-testid="user-manage-drawer" /> : null,
}));

vi.mock('@/components/user-traffic-modal', () => ({
  UserTrafficModal: ({ open }: { open: boolean }) =>
    open ? <div data-testid="user-traffic-modal" /> : null,
}));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/toast', () => ({
  toast: {
    loading: (...args: unknown[]) => mocks.toastLoading(...args),
    dismiss: (...args: unknown[]) => mocks.toastDismiss(...args),
    success: (...args: unknown[]) => mocks.toastSuccess(...args),
  },
}));

vi.mock('@/lib/queries', () => ({
  useAdminTickets: (query: Record<string, unknown>) => {
    mocks.ticketQueries.push(query);
    return {
      isPending: false,
      isFetching: false,
      error: undefined,
      refetch: mocks.refetch,
      data: { data: [mocks.OPEN_TICKET, mocks.CLOSED_TICKET], total: 42 },
    };
  },
  useCloseTicketMutation: () => ({ mutate: mocks.closeMutate }),
  useReplyTicketMutation: () => ({
    isPending: false,
    mutate: (
      payload: unknown,
      options?: { onSettled?: () => void; onSuccess?: (data: unknown) => void },
    ) => {
      void Promise.resolve(mocks.replyMutateAsync(payload)).then(
        (data) => {
          options?.onSuccess?.(data);
          options?.onSettled?.();
        },
        () => options?.onSettled?.(),
      );
    },
  }),
  useAdminTicket: () => ({
    refetch: mocks.ticketRefetch,
    data: mocks.detail,
    error: mocks.detailError,
    isError: Boolean(mocks.detailError),
    isFetching: false,
  }),
  useAdminUserInfo: (id?: number | null) => {
    mocks.userInfoIds.push(id);
    return { data: undefined };
  },
}));

beforeEach(() => {
  mocks.params = {};
  mocks.ticketQueries = [];
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.ticketRefetch.mockReset().mockResolvedValue(undefined);
  mocks.closeMutate.mockReset();
  mocks.replyMutateAsync.mockReset().mockResolvedValue(true);
  mocks.userInfoIds = [];
  mocks.detail = mocks.makeDetail();
  mocks.detailError = undefined;
  mocks.confirm.mockReset().mockResolvedValue(true);
  mocks.toastLoading.mockReset().mockReturnValue('toast-id');
  mocks.toastDismiss.mockReset();
  mocks.toastSuccess.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('TicketsPage list', () => {
  it('renders ticket rows with level and formatted times', () => {
    render(<TicketsPage />);

    expect(screen.getByText('工单管理')).toBeInTheDocument();
    const table = screen.getByTestId('tickets-table');
    expect(within(table).getByText('支付问题')).toBeInTheDocument();
    expect(within(table).getByText('高')).toBeInTheDocument();
    expect(within(table).getByText('待回复')).toBeInTheDocument();
    expect(within(table).getByText('已关闭')).toBeInTheDocument();
    expect(
      within(table).getAllByText(dayjs('2023-11-14T22:13:20Z').format('YYYY/MM/DD HH:mm')).length,
    ).toBeGreaterThan(0);
  });

  it('fetches the first open-ticket page with the in-app pagination model', () => {
    render(<TicketsPage />);
    expect(mocks.ticketQueries[0]).toMatchObject({ current: 1, pageSize: 10, status: 0 });
  });

  it('searches tickets by email through the debounced query', async () => {
    const user = userEvent.setup();
    render(<TicketsPage />);
    mocks.ticketQueries = [];

    await user.type(screen.getByTestId('ticket-email-search'), 'buyer@example.com');

    await waitFor(() =>
      expect(mocks.ticketQueries[mocks.ticketQueries.length - 1]).toMatchObject({
        current: 1,
        status: 0,
        email: 'buyer@example.com',
      }),
    );
  });

  it('sends the reply_status filter array and resets to page 1', async () => {
    const user = userEvent.setup();
    render(<TicketsPage />);
    mocks.ticketQueries = [];

    await user.click(screen.getByTestId('ticket-reply-filter'));
    await user.click(await screen.findByRole('menuitemcheckbox', { name: '已回复' }));

    expect(mocks.ticketQueries[mocks.ticketQueries.length - 1]).toMatchObject({
      current: 1,
      reply_status: [1],
    });
  });

  it('switches to closed tickets and drops the reply-status filter control', async () => {
    const user = userEvent.setup();
    render(<TicketsPage />);
    mocks.ticketQueries = [];

    await user.click(screen.getByRole('radio', { name: '已关闭' }));

    expect(mocks.ticketQueries[mocks.ticketQueries.length - 1]).toMatchObject({
      current: 1,
      status: 1,
    });
    expect(screen.queryByTestId('ticket-reply-filter')).not.toBeInTheDocument();
  });

  it('closes a ticket by id after the confirm dialog resolves true', async () => {
    const user = userEvent.setup();
    render(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-close-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    await waitFor(() => expect(mocks.closeMutate).toHaveBeenCalledWith(1, expect.any(Object)));
  });

  it('does not close a ticket when the confirm dialog is dismissed', async () => {
    mocks.confirm.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-close-1'));
    await waitFor(() => expect(mocks.confirm).toHaveBeenCalled());
    expect(mocks.closeMutate).not.toHaveBeenCalled();
  });

  it('disables the close action for an already closed ticket', () => {
    render(<TicketsPage />);
    expect(screen.getByTestId('ticket-close-2')).toBeDisabled();
  });

  it('opens the chat panel from a row and replies with { id, message }', async () => {
    const user = userEvent.setup();
    render(<TicketsPage />);

    await user.click(screen.getByTestId('ticket-view-1'));
    const panel = await screen.findByTestId('ticket-chat');
    expect(within(panel).getByText('用户消息')).toBeInTheDocument();
    expect(within(panel).getByText('客服回复')).toBeInTheDocument();

    await user.type(within(panel).getByTestId('ticket-reply-input'), '这是回复');
    await user.click(within(panel).getByTestId('ticket-reply-submit'));

    await waitFor(() =>
      expect(mocks.replyMutateAsync).toHaveBeenCalledWith({ id: 1, message: '这是回复' }),
    );
  });
});

describe('TicketsPage standalone chat route', () => {
  it('renders /ticket/:ticket_id as the chat view and passes the ticket id through', async () => {
    mocks.params = { ticket_id: '1' };
    const user = userEvent.setup();
    render(<TicketsPage />);

    expect(screen.getByText('支付问题')).toBeInTheDocument();
    expect(screen.getByText('用户消息')).toBeInTheDocument();
    expect(mocks.userInfoIds).toContain(7);

    await user.type(screen.getByTestId('ticket-reply-input'), 'hi');
    await user.click(screen.getByTestId('ticket-reply-submit'));

    await waitFor(() =>
      expect(mocks.replyMutateAsync).toHaveBeenCalledWith({ id: '1', message: 'hi' }),
    );
  });

  it('shows the not-found notice when the ticket fails to load', () => {
    mocks.params = { ticket_id: '1' };
    mocks.detail = undefined;
    mocks.detailError = { status: 500, message: '工单不存在' };
    render(<TicketsPage />);

    expect(screen.getByText('工单不存在')).toBeInTheDocument();
    expect(screen.queryByTestId('ticket-reply-input')).not.toBeInTheDocument();
  });

  it('keeps non-not-found failures retryable instead of claiming the ticket is absent', async () => {
    mocks.params = { ticket_id: '1' };
    mocks.detail = undefined;
    mocks.detailError = { status: 503, message: '服务暂时不可用' };
    const user = userEvent.setup();
    render(<TicketsPage />);

    expect(screen.getByTestId('ticket-detail-error')).toHaveTextContent('服务暂时不可用');
    expect(screen.queryByText('工单不存在')).not.toBeInTheDocument();
    expect(screen.queryByTestId('ticket-reply-input')).not.toBeInTheDocument();
    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.ticketRefetch).toHaveBeenCalledOnce();
  });

  it('shows the loading notice before the ticket resolves', () => {
    mocks.params = { ticket_id: '1' };
    mocks.detail = undefined;
    mocks.detailError = undefined;
    render(<TicketsPage />);

    expect(screen.getByText('加载中...')).toBeInTheDocument();
  });
});
