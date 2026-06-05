import { act } from 'react';
import { readFileSync } from 'node:fs';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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
    ticketError: false,
  };
});

const toastMocks = vi.hoisted(() => ({
  loading: vi.fn(),
  destroy: vi.fn(),
  success: vi.fn(),
}));

const labels: Record<string, string> = {
  'ticket.reply_placeholder': '输入内容回复工单...',
};

vi.mock('react-router-dom', () => ({
  useParams: () => ({ ticket_id: '7' }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string) => labels[key] ?? key,
  }),
}));

vi.mock('@/lib/queries', () => ({
  useTicket: () => ({
    data: state.ticket,
    isError: state.ticketError,
    isFetching: false,
    refetch: state.refetch,
  }),
  useReplyTicketMutation: () => ({
    isPending: state.replyPending,
    mutateAsync: state.replyMutateAsync,
  }),
}));

vi.mock('@/lib/legacy-toast', () => ({
  toast: toastMocks,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

afterEach(() => {
  state.ticket = state.makeTicket();
  state.ticketError = false;
});

describe('TicketDetailPage bundled-theme chat view', () => {
  it('keeps the bundled-theme route ticket id as the fetch and reply input', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');

    expect(source).toContain('const ticketId = ticket_id;');
    expect(source).toContain('id: ticketId as string');
    expect(source).not.toContain("const ticketId = ticket_id ?? ''");
  });

  it('keeps the old unkeyed ticket message map from the bundled theme', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');
    const messageSource = source.slice(
      source.indexOf('{data?.message.map((item) =>'),
      source.indexOf('<div className="js-chat-form'),
    );

    expect(messageSource).toContain('{data?.message.map((item) =>');
    expect(messageSource).not.toContain('key={index}');
    expect(messageSource).not.toContain('key={item.id}');
    expect(messageSource).not.toContain('key=');
    expect(source).not.toContain('const messages = data?.message ?? []');
    expect(source).not.toContain('data?.message?.length');
  });

  it('renders the legacy subject strip, message bubbles, dates, and reply input shell', () => {
    const html = renderToStaticMarkup(<TicketDetailPage />);

    expect(html).toContain('block-content-full bg-gray-lighter p-3');
    expect(html).toContain('class="tag___12_9H"');
    expect(html).toContain('Need help');
    expect(html).toContain(
      'bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1',
    );
    expect(html).toContain('font-size-sm text-muted my-2 text-right');
    expect(html).toContain('text-right ml-4');
    expect(html).toContain('d-inline-block bg-gray-lighter px-3 py-2 mb-2 mw-100 rounded text-left');
    expect(html).toContain('My message');
    expect(html).toContain('font-size-sm text-muted my-2');
    expect(html).toContain('mr-4');
    expect(html).toContain('d-inline-block bg-success-lighter px-3 py-2 mb-2 mw-100 rounded text-left');
    expect(html).toContain('Support reply');
    expect(html).toContain('2023/11/14 22:13');
    expect(html).toContain('2023/11/14 22:14');
    expect(html).toContain('js-chat-form block-content p-2 bg-body-dark input___1j_ND');
    expect(html).toContain('js-chat-input bg-body-dark border-0 form-control form-control-alt');
    expect(html).toContain('placeholder="输入内容回复工单..."');
  });

  it('keeps the chat shell visible when the ticket fetch fails', () => {
    state.ticket = undefined;
    state.ticketError = true;

    const html = renderToStaticMarkup(<TicketDetailPage />);

    expect(html).toContain('block-content-full bg-gray-lighter p-3');
    expect(html).toContain('class="tag___12_9H"');
    expect(html).toContain(
      'bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1',
    );
    expect(html).toContain('js-chat-form block-content p-2 bg-body-dark input___1j_ND');
    expect(html).toContain('js-chat-input bg-body-dark border-0 form-control form-control-alt');
    expect(html).toContain('placeholder="输入内容回复工单..."');
    expect(html).toContain('工单不存在或已被删除');
    expect(html).not.toContain('class="ant-empty ant-empty-normal"');
    expect(html).not.toContain('暂无数据');
    expect(html).not.toContain('Need help');
  });
});

describe('TicketDetailPage legacy polling and reply', () => {
  let container: HTMLDivElement;
  let root: Root | null;
  let scrollTo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    state.refetch.mockReset();
    state.replyMutateAsync.mockReset();
    state.replyMutateAsync.mockResolvedValue(undefined);
    state.replyPending = false;
    toastMocks.loading.mockReset();
    toastMocks.destroy.mockReset();
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

  it('scrolls chat to bottom on mount and leaves one scheduled fetch after unmount', () => {
    act(() => {
      root!.render(<TicketDetailPage />);
    });

    expect(scrollTo).toHaveBeenCalledWith(0, 480);

    act(() => root?.unmount());
    root = null;

    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(state.refetch).toHaveBeenCalledTimes(1);

    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(state.refetch).toHaveBeenCalledTimes(1);
  });

  it('sends a reply on Enter with the legacy toast sequence and clears only the input', async () => {
    const source = readFileSync(`${process.cwd()}/src/pages/tickets/detail.tsx`, 'utf8');
    const submitSource = source.slice(
      source.indexOf('const submitReply = async () => {'),
      source.indexOf('const data = ticket.data;'),
    );

    expect(submitSource).toContain("toast.success('发送成功');");
    expect(submitSource).toContain('setMessage(undefined);');
    expect(submitSource).toContain("if (inputRef.current) inputRef.current.value = '';");
    expect(submitSource.indexOf("toast.success('发送成功');")).toBeLessThan(
      submitSource.indexOf('setMessage(undefined);'),
    );
    expect(submitSource.indexOf('setMessage(undefined);')).toBeLessThan(
      submitSource.indexOf("if (inputRef.current) inputRef.current.value = '';"),
    );
    expect(submitSource).not.toContain('ticket.refetch');

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
      const event = new KeyboardEvent('keydown', { bubbles: true });
      Object.defineProperty(event, 'keyCode', { value: 13 });
      input.dispatchEvent(event);
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
      const event = new KeyboardEvent('keydown', { bubbles: true });
      Object.defineProperty(event, 'keyCode', { value: 13 });
      input.dispatchEvent(event);
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
      const event = new KeyboardEvent('keydown', { bubbles: true });
      Object.defineProperty(event, 'keyCode', { value: 13 });
      input.dispatchEvent(event);
      await Promise.resolve();
    });

    expect(toastMocks.loading).not.toHaveBeenCalled();
    expect(state.replyMutateAsync).not.toHaveBeenCalled();
  });
});
