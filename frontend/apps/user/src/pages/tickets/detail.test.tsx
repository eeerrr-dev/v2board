import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import TicketDetailPage from './detail';

const state = vi.hoisted(() => {
  const makeTicket = () => ({
    subject: 'Need help',
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
    ticket: makeTicket() as ReturnType<typeof makeTicket> | undefined,
    ticketCalls: [] as Array<{ id: number | string | undefined; options?: unknown }>,
    ticketError: false,
  };
});

const toastMocks = vi.hoisted(() => ({
  destroy: vi.fn(),
  loading: vi.fn(),
  success: vi.fn(),
}));

const labels: Record<string, string> = {
  'Ticket does not exist': '工单不存在',
  'common.loading': '加载中...',
  'ticket.reply_placeholder': '输入内容回复工单...',
  'ticket.reply_sending': '发送中',
  'ticket.reply_success': '发送成功',
};

vi.mock('react-router', () => ({
  useParams: () => ({ ticket_id: '7' }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string) => labels[key] ?? key,
  }),
}));

vi.mock('@/lib/queries', () => ({
  useTicket: (id: number | string | undefined, options?: unknown) => {
    state.ticketCalls.push({ id, options });
    return {
      data: state.ticket,
      isError: state.ticketError,
      isFetching: false,
      refetch: state.refetch,
    };
  },
  useReplyTicketMutation: () => ({
    isPending: state.replyPending,
    mutateAsync: state.replyMutateAsync,
  }),
}));

vi.mock('@/lib/toast', () => ({
  toast: toastMocks,
}));

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  state.ticket = state.makeTicket();
  state.ticketError = false;
});

describe('TicketDetailPage shadcn chat surface', () => {
  it('keeps the route ticket id as the fetch and reply input', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');

    expect(source).toContain('const ticketId = ticket_id;');
    expect(source).toContain('id: ticketId as string');
    expect(source).not.toContain("const ticketId = ticket_id ?? ''");
  });

  it('keys ticket messages by the legacy list index without introducing a message id fallback', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');
    const messageSource = source.slice(
      source.indexOf('{data?.message.map((item, index) =>'),
      source.indexOf('<div className="js-chat-form'),
    );

    expect(messageSource).toContain('{data?.message.map((item, index) =>');
    expect(messageSource).toContain('key={index}');
    expect(messageSource).not.toContain('key={item.id}');
    expect(source).toContain('const messages = ticket.data?.message ?? [];');
    expect(source).toContain('}, [messages.length]);');
  });

  it('renders the shadcn subject header, message bubbles, dates, and reply form', () => {
    const html = renderToStaticMarkup(<TicketDetailPage />);

    expect(html).toContain('data-testid="ticket-detail"');
    expect(html).toContain('data-testid="ticket-detail-header"');
    expect(html).toContain('#7');
    expect(html).toContain('Need help');
    expect(html).toContain('js-chat-messages');
    expect(html).toContain('data-testid="ticket-chat"');
    expect(html).toContain('My message');
    expect(html).toContain('Support reply');
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_000));
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_060));
    expect(html).toContain('js-chat-form');
    expect(html).toContain('data-testid="ticket-reply-form"');
    expect(html).toContain('js-chat-input');
    expect(html).toContain('data-testid="ticket-reply-input"');
    expect(html).toContain('data-testid="ticket-reply-send"');
    expect(html).toContain('placeholder="输入内容回复工单..."');
    expect(html).not.toContain('content___DW5w1');
    expect(html).not.toContain('input___1j_ND');
    expect(html).not.toContain('tag___12_9H');
  });

  it('keeps the chat shell visible when the ticket fetch fails', () => {
    state.ticket = undefined;
    state.ticketError = true;

    const html = renderToStaticMarkup(<TicketDetailPage />);

    expect(html).toContain('工单不存在');
    expect(html).toContain('data-testid="ticket-chat"');
    expect(html).toContain('data-testid="ticket-reply-form"');
    expect(html).toContain('placeholder="输入内容回复工单..."');
    expect(html).not.toContain('页面加载失败');
    expect(html).not.toContain('刷新页面');
    expect(html).not.toContain('Need help');
  });

  it('renders visible loading text before the ticket fetch resolves', () => {
    state.ticket = undefined;
    state.ticketError = false;

    const html = renderToStaticMarkup(<TicketDetailPage />);

    expect(html).toContain('加载中...');
    expect(html).toContain('data-testid="ticket-detail-header"');
    expect(html).toContain('text-center text-sm text-muted-foreground');
  });
});

describe('TicketDetailPage query polling and reply behavior', () => {
  let container: HTMLDivElement;
  let root: Root | null;
  let scrollTo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    state.refetch.mockReset();
    state.ticketCalls = [];
    state.replyMutateAsync.mockReset();
    state.replyMutateAsync.mockResolvedValue(undefined);
    state.replyPending = false;
    toastMocks.destroy.mockReset();
    toastMocks.loading.mockReset();
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
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) {
      act(() => root?.unmount());
      root = null;
    }
    container.remove();
    document.body.innerHTML = '';
    vi.useRealTimers();
  });

  it('uses React Query polling and scrolls chat to bottom on mount', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');

    expect(source).toContain('useTicket(ticketId, { refetchInterval: 5000 })');
    expect(source).not.toContain('window.setTimeout');

    act(() => {
      root!.render(<TicketDetailPage />);
    });

    expect(scrollTo).toHaveBeenCalledWith(0, 480);
    expect(state.ticketCalls.at(-1)).toEqual({
      id: '7',
      options: { refetchInterval: 5000 },
    });
  });

  it('sends a reply through the native form with the toast sequence and clears the input', async () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');
    const submitSource = source.slice(
      source.indexOf('const submitReply = async (event?: SyntheticEvent<HTMLFormElement>) => {'),
      source.indexOf('const data = {'),
    );

    expect(submitSource).toContain("toast.success(t('ticket.reply_success'));");
    expect(submitSource).toContain("setMessage('');");
    expect(submitSource.indexOf("toast.success(t('ticket.reply_success'));")).toBeLessThan(
      submitSource.indexOf("setMessage('');"),
    );
    expect(submitSource).not.toContain('ticket.refetch');
    expect(source).not.toContain('keyCode');
    expect(source).not.toContain('inputRef');

    await act(async () => {
      root!.render(<TicketDetailPage />);
      await Promise.resolve();
    });

    const input = container.querySelector('.js-chat-input') as HTMLInputElement;

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        input,
        'Please help me',
      );
      input.dispatchEvent(new Event('input', { bubbles: true }));
      container
        .querySelector<HTMLFormElement>('[data-testid="ticket-reply-form"]')!
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(toastMocks.loading).toHaveBeenCalledWith('发送中');
    expect(state.replyMutateAsync).toHaveBeenCalledWith({
      id: '7',
      message: 'Please help me',
    });
    expect(toastMocks.destroy).toHaveBeenCalledTimes(1);
    expect(toastMocks.success).toHaveBeenCalledWith('发送成功');
    expect(input.value).toBe('');
    expect(state.refetch).not.toHaveBeenCalled();
  });

  it('keeps failed replies in the input while only clearing the loading toast', async () => {
    state.replyMutateAsync.mockRejectedValue(new Error('failed'));

    await act(async () => {
      root!.render(<TicketDetailPage />);
      await Promise.resolve();
    });

    const input = container.querySelector('.js-chat-input') as HTMLInputElement;

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        input,
        'Please keep this',
      );
      input.dispatchEvent(new Event('input', { bubbles: true }));
      container
        .querySelector<HTMLFormElement>('[data-testid="ticket-reply-form"]')!
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(toastMocks.loading).toHaveBeenCalledWith('发送中');
    expect(state.replyMutateAsync).toHaveBeenCalledWith({
      id: '7',
      message: 'Please keep this',
    });
    expect(toastMocks.destroy).toHaveBeenCalledTimes(1);
    expect(toastMocks.success).not.toHaveBeenCalled();
    expect(input.value).toBe('Please keep this');
    expect(state.refetch).not.toHaveBeenCalled();
  });

  it('does not reply while replyLoading is active', async () => {
    state.replyPending = true;

    await act(async () => {
      root!.render(<TicketDetailPage />);
      await Promise.resolve();
    });

    const input = container.querySelector('.js-chat-input') as HTMLInputElement;

    await act(async () => {
      input
        .closest('form')!
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(toastMocks.loading).not.toHaveBeenCalled();
    expect(state.replyMutateAsync).not.toHaveBeenCalled();
  });
});
