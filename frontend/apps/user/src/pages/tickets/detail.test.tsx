import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { formatBackendDateMinuteSlash } from '@v2board/config/format';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import TicketDetailPage from './detail';

const state = vi.hoisted(() => {
  const makeTicket = () => ({
    subject: 'Need help',
    status: 0,
    message: [
      {
        id: 1,
        user_id: 1,
        ticket_id: 7,
        is_me: true,
        message: 'My message',
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
      },
      {
        id: 2,
        user_id: 0,
        ticket_id: 7,
        is_me: false,
        message: 'Support reply',
        created_at: 1_700_000_060,
        updated_at: 1_700_000_060,
      },
    ],
  });

  return {
    makeTicket,
    refetch: vi.fn(),
    replyMutateAsync: vi.fn(),
    replyPending: false,
    routeTicketId: '7' as string | undefined,
    ticket: makeTicket() as ReturnType<typeof makeTicket> | undefined,
    ticketCalls: [] as Array<{ id: number | string | undefined; options?: unknown }>,
    ticketError: undefined as unknown,
  };
});

const toastMocks = vi.hoisted(() => ({
  dismiss: vi.fn(),
  loading: vi.fn(),
  success: vi.fn(),
}));

const labels: Record<string, string> = {
  'Ticket does not exist': '工单不存在',
  'common.loading': '加载中...',
  'ticket.reply_placeholder': '输入内容回复工单...',
  'ticket.reply_sending': '发送中',
  'ticket.reply_success': '发送成功',
  'ticket.closed_notice': '工单已关闭，无法回复。',
};

vi.mock('react-router', () => ({
  useParams: () => ({ ticket_id: state.routeTicketId }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation(labels),
}));

vi.mock('@/lib/queries', () => ({
  useTicket: (id: number | string | undefined, options?: unknown) => {
    state.ticketCalls.push({ id, options });
    return {
      data: state.ticket,
      error: state.ticketError,
      isError: Boolean(state.ticketError),
      isFetching: false,
      refetch: state.refetch,
    };
  },
  useReplyTicketMutation: () => ({
    isPending: state.replyPending,
    mutate: (
      payload: unknown,
      options?: {
        onError?: (error: unknown) => void;
        onSettled?: () => void;
        onSuccess?: (data: unknown) => void;
      },
    ) => {
      void Promise.resolve(state.replyMutateAsync(payload)).then(
        (data) => {
          options?.onSuccess?.(data);
          options?.onSettled?.();
        },
        (error: unknown) => {
          options?.onError?.(error);
          options?.onSettled?.();
        },
      );
    },
  }),
}));

vi.mock('@/lib/toast', () => ({
  toast: toastMocks,
}));

let scrollTo: ReturnType<typeof vi.fn>;

beforeEach(() => {
  state.routeTicketId = '7';
  state.ticket = state.makeTicket();
  state.ticketError = undefined;
  state.ticketCalls = [];
  state.replyPending = false;
  state.refetch.mockReset();
  state.replyMutateAsync.mockReset();
  state.replyMutateAsync.mockResolvedValue(undefined);
  toastMocks.dismiss.mockReset();
  toastMocks.loading.mockReset().mockReturnValue('ticket-reply-loading');
  toastMocks.success.mockReset();
  scrollTo = vi.fn();
  Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
    configurable: true,
    value: scrollTo,
  });
  Object.defineProperty(HTMLElement.prototype, 'scrollHeight', {
    configurable: true,
    get: () => 480,
  });
});

describe('TicketDetailPage shadcn chat surface', () => {
  it('renders the subject header, message bubbles in order, dates, and reply composer', () => {
    renderWithProviders(<TicketDetailPage />);

    expect(screen.getByTestId('ticket-detail-header')).toHaveTextContent('#7');
    expect(screen.getByRole('heading', { name: 'Need help' })).toBeInTheDocument();

    const chat = screen.getByTestId('ticket-chat');
    expect(screen.getByText('My message')).toBeInTheDocument();
    expect(screen.getByText('Support reply')).toBeInTheDocument();
    const chatText = chat.textContent ?? '';
    expect(chatText.indexOf('My message')).toBeLessThan(chatText.indexOf('Support reply'));
    expect(screen.getByText(formatBackendDateMinuteSlash(1_700_000_000))).toBeInTheDocument();
    expect(screen.getByText(formatBackendDateMinuteSlash(1_700_000_060))).toBeInTheDocument();

    expect(screen.getByTestId('ticket-reply-form')).toBeInTheDocument();
    const input = screen.getByPlaceholderText('输入内容回复工单...');
    expect(input).toBe(screen.getByTestId('ticket-reply-input'));
    expect(screen.getByTestId('ticket-reply-send')).toBeEnabled();
  });

  it('exposes the standalone ticket surface through a component slot', () => {
    renderWithProviders(<TicketDetailPage />);

    expect(screen.getByTestId('ticket-detail')).toHaveAttribute('data-slot', 'ticket-detail');
  });

  it('shows the backend not-found contract without offering a reply composer', () => {
    state.ticket = undefined;
    // GET /user/tickets/{id} is path-identified: a missing ticket is the
    // 404 `ticket_not_found` problem (docs/api-dialect.md §3.4, W8).
    state.ticketError = { status: 404, message: '工单不存在' };

    renderWithProviders(<TicketDetailPage />);

    expect(screen.getAllByText('工单不存在').length).toBeGreaterThan(0);
    expect(screen.getByTestId('ticket-chat')).toBeInTheDocument();
    expect(screen.queryByTestId('ticket-reply-form')).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText('输入内容回复工单...')).not.toBeInTheDocument();
    expect(screen.queryByText('页面加载失败')).not.toBeInTheDocument();
    expect(screen.queryByText('刷新页面')).not.toBeInTheDocument();
    expect(screen.queryByText('Need help')).not.toBeInTheDocument();
  });

  it('shows a retryable error for unrelated query failures', async () => {
    state.ticket = undefined;
    state.ticketError = { status: 503, message: '服务暂时不可用' };

    const { user } = renderWithProviders(<TicketDetailPage />);

    expect(screen.getByTestId('ticket-detail-error')).toHaveTextContent('服务暂时不可用');
    expect(screen.queryByText('工单不存在')).not.toBeInTheDocument();
    expect(screen.queryByTestId('ticket-reply-form')).not.toBeInTheDocument();
    await user.click(screen.getByTestId('error-state-retry'));
    expect(state.refetch).toHaveBeenCalledOnce();
  });

  it('renders visible loading text before the ticket fetch resolves', () => {
    state.ticket = undefined;
    state.ticketError = undefined;

    renderWithProviders(<TicketDetailPage />);

    expect(screen.getAllByText('加载中...').length).toBeGreaterThan(0);
    expect(screen.getByTestId('ticket-detail-header')).toBeInTheDocument();
  });

  it('replaces the reply composer with a closed notice for a closed ticket', () => {
    // status 1 = closed; the backend rejects replies, so the composer must not
    // be offered — show why instead of letting the user hit a silent failure.
    state.ticket = { ...state.makeTicket(), status: 1 };

    renderWithProviders(<TicketDetailPage />);

    expect(screen.getByTestId('ticket-closed-notice')).toHaveTextContent('工单已关闭，无法回复。');
    expect(screen.queryByTestId('ticket-reply-form')).not.toBeInTheDocument();
    expect(screen.queryByTestId('ticket-reply-input')).not.toBeInTheDocument();
  });
});

describe('TicketDetailPage query polling and reply behavior', () => {
  it('polls the ticket through React Query options and scrolls chat to bottom on mount', () => {
    renderWithProviders(<TicketDetailPage />);

    expect(state.ticketCalls.at(-1)).toEqual({
      id: '7',
      options: { refetchInterval: 5000 },
    });
    expect(scrollTo).toHaveBeenCalledWith(0, 480);
  });

  it('scrolls the chat to the bottom again only when a new message arrives', () => {
    const { rerender } = renderWithProviders(<TicketDetailPage />);
    scrollTo.mockClear();

    // A poll returning the same messages must not yank the scroll position.
    state.ticket = state.makeTicket();
    rerender(<TicketDetailPage />);
    expect(scrollTo).not.toHaveBeenCalled();

    const next = state.makeTicket();
    next.message.push({
      id: 3,
      user_id: 0,
      ticket_id: 7,
      is_me: false,
      message: 'One more reply',
      created_at: 1_700_000_120,
      updated_at: 1_700_000_120,
    });
    state.ticket = next;
    rerender(<TicketDetailPage />);
    expect(scrollTo).toHaveBeenCalledWith(0, 480);
  });

  it('passes the raw route ticket id to the query without coercing a missing id', () => {
    state.routeTicketId = undefined;

    renderWithProviders(<TicketDetailPage />);

    // Missing param stays undefined — never coerced to '' for the fetch.
    expect(state.ticketCalls.at(-1)).toEqual({
      id: undefined,
      options: { refetchInterval: 5000 },
    });
  });

  it('sends a reply through the native form with the toast sequence and clears the input', async () => {
    const { user } = renderWithProviders(<TicketDetailPage />);
    const input = screen.getByTestId('ticket-reply-input');

    await user.type(input, 'Please help me');
    await user.click(screen.getByTestId('ticket-reply-send'));

    expect(toastMocks.loading).toHaveBeenCalledWith('发送中');
    expect(state.replyMutateAsync).toHaveBeenCalledWith({
      id: '7',
      message: 'Please help me',
    });
    await waitFor(() => expect(toastMocks.success).toHaveBeenCalledWith('发送成功'));
    expect(toastMocks.dismiss).toHaveBeenCalledWith('ticket-reply-loading');
    expect(input).toHaveValue('');
    expect(state.refetch).not.toHaveBeenCalled();
  });

  it('keeps failed replies in the input while only clearing the loading toast', async () => {
    state.replyMutateAsync.mockRejectedValue(new Error('failed'));
    const { user } = renderWithProviders(<TicketDetailPage />);
    const input = screen.getByTestId('ticket-reply-input');

    // Enter in the input mirrors the interaction-parity harness reply flow.
    await user.type(input, 'Please keep this{Enter}');

    expect(toastMocks.loading).toHaveBeenCalledWith('发送中');
    expect(state.replyMutateAsync).toHaveBeenCalledWith({
      id: '7',
      message: 'Please keep this',
    });
    await waitFor(() => expect(toastMocks.dismiss).toHaveBeenCalledWith('ticket-reply-loading'));
    expect(toastMocks.success).not.toHaveBeenCalled();
    expect(input).toHaveValue('Please keep this');
    expect(state.refetch).not.toHaveBeenCalled();
  });

  it('does not send a reply while a previous reply is pending', () => {
    state.replyPending = true;

    renderWithProviders(<TicketDetailPage />);

    expect(screen.getByTestId('ticket-reply-send')).toBeDisabled();
    // Force a raw submit past the disabled button to exercise the isPending guard.
    fireEvent.submit(screen.getByTestId('ticket-reply-form'));

    expect(toastMocks.loading).not.toHaveBeenCalled();
    expect(state.replyMutateAsync).not.toHaveBeenCalled();
  });
});
