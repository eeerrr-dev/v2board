import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import TicketsPage, { startLegacyTicketPolling } from './tickets';

const ticketsSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'tickets.tsx'),
  'utf8',
);
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => {
  const makeAdminTicket = () => ({
    id: 1,
    user_id: 1,
    subject: '支付问题',
    level: 2,
    status: 0,
    reply_status: 0,
    last_reply_user_id: null,
    created_at: 1700000000,
    updated_at: 1700086400,
    message: [
      {
        id: 1,
        user_id: 1,
        ticket_id: 1,
        message: '用户消息',
        is_me: false,
        created_at: 1700000000,
        updated_at: 1700000000,
      },
      {
        id: 2,
        user_id: 1,
        ticket_id: 1,
        message: '客服回复',
        is_me: true,
        created_at: 1700086400,
        updated_at: 1700086400,
      },
    ],
  });

  return {
    makeAdminTicket,
    params: {} as Record<string, string>,
    closeTicketMutate: vi.fn(),
    ticketRefetch: vi.fn(),
    ticketQueries: [] as Array<Record<string, unknown>>,
    adminUserInfoIds: [] as Array<number | null | undefined>,
    adminTicket: makeAdminTicket() as ReturnType<typeof makeAdminTicket> | undefined,
    adminTicketError: false,
  };
});

vi.mock('react-router-dom', () => ({
  useParams: () => mocks.params,
}));

vi.mock('@/lib/queries', () => ({
  useAdminTickets: (query: Record<string, unknown>) => {
    mocks.ticketQueries.push(query);
    return {
      isLoading: false,
      isFetching: false,
      refetch: vi.fn(),
      data: {
        data: [
          {
            id: 1,
            user_id: 1,
            subject: '支付问题',
            level: 2,
            status: 0,
            reply_status: 0,
            last_reply_user_id: null,
            created_at: 1700000000,
            updated_at: 1700086400,
          },
          {
            id: 2,
            user_id: 2,
            subject: '已完成',
            level: 0,
            status: 1,
            reply_status: 1,
            last_reply_user_id: null,
            created_at: 1700000000,
            updated_at: 1700086400,
          },
        ],
        total: 2,
      },
    };
  },
  useCloseTicketMutation: () => ({
    mutate: mocks.closeTicketMutate,
  }),
  useAdminTicket: () => ({
    refetch: mocks.ticketRefetch,
    data: mocks.adminTicket,
    isError: mocks.adminTicketError,
    isFetching: false,
  }),
  useReplyTicketMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useAdminPlans: () => ({
    data: [{ id: 1, name: '基础套餐' }],
  }),
  useAdminUserInfo: (id?: number | null) => {
    mocks.adminUserInfoIds.push(id);
    return {
      data: {
        id: 1,
        email: 'user@example.com',
        balance: 1200,
        commission_balance: 3400,
        transfer_enable: 107374182400,
        device_limit: 3,
        u: 0,
        d: 0,
        plan_id: 1,
        expired_at: 1893456000,
        banned: 0,
        is_admin: 0,
        is_staff: 0,
      },
    };
  },
  useUpdateUserMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useAdminUserTraffic: () => ({
    isFetching: false,
    data: {
      data: [{ record_at: 1700000000, u: 1024, d: 2048, server_rate: 1 }],
      total: 1,
    },
  }),
}));

beforeEach(() => {
  mocks.params = {};
  mocks.closeTicketMutate.mockClear();
  mocks.ticketRefetch.mockClear();
  mocks.ticketQueries = [];
  mocks.adminUserInfoIds = [];
  mocks.adminTicket = mocks.makeAdminTicket();
  mocks.adminTicketError = false;
});

afterEach(() => {
  vi.useRealTimers();
});

describe('TicketsPage legacy ticket manager', () => {
  it('renders the original ticket table shell, filters, and actions', () => {
    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('class="block border-bottom"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('class="p-3"');
    expect(html).toContain('class="ant-radio-group ant-radio-group-outline"');
    expect(html).toContain('class="ant-radio-button-inner"');
    expect(html).toContain('class="ant-input"');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain(
      'class="ant-table ant-table-default ant-table-scroll-position-left ant-table-scroll-position-right"',
    );
    expect(html).toContain('class="ant-table-scroll"');
    expect(html).toContain('tabindex="-1" class="ant-table-body" style="overflow-x:scroll"');
    expect(html).toContain('class="ant-table-fixed" style="width:900px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain('ant-table-column-has-actions ant-table-column-has-filters');
    expect(html).toContain('aria-label="图标: filter"');
    expect(html).toContain('已开启');
    expect(html).toContain('已关闭');
    expect(html).toContain('输入邮箱搜索');
    expect(html).toContain('主题');
    expect(html).toContain('工单级别');
    expect(html).toContain('工单状态');
    expect(html).toContain('创建时间');
    expect(html).toContain('最后回复');
    expect(html).toContain('支付问题');
    expect(html).toContain('高');
    expect(html).toContain('待回复');
    expect(html).toContain('已完成');
    expect(html).toContain(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm'));
    expect(html).toContain('查看');
    expect(html).toContain('关闭');
    expect(html).not.toContain('ant-drawer');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('css-dev-only');
    expect(html).not.toContain('ant-typography');
  });

  it('keeps the legacy close link behavior for already closed rows', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    const closeLinks = Array.from(container.querySelectorAll('a')).filter(
      (link) => link.textContent === '关闭',
    );
    expect(closeLinks).toHaveLength(4);

    await act(async () => {
      closeLinks[1]!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.closeTicketMutate).toHaveBeenCalledWith(2, expect.any(Object));

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });

  it('debounces the legacy email search before fetching the first ticket page', async () => {
    vi.useFakeTimers();
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    expect(mocks.ticketQueries[mocks.ticketQueries.length - 1]).toMatchObject({
      current: 1,
      pageSize: 10,
      status: 0,
    });

    mocks.ticketQueries = [];
    const emailInput = container.querySelector<HTMLInputElement>(
      'input[placeholder="输入邮箱搜索"]',
    )!;

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        emailInput,
        'buyer@example.com',
      );
      emailInput.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.ticketQueries).toHaveLength(0);

    await act(async () => {
      vi.advanceTimersByTime(299);
      await Promise.resolve();
    });

    expect(mocks.ticketQueries).toHaveLength(0);

    await act(async () => {
      vi.advanceTimersToNextTimer();
      await Promise.resolve();
    });

    expect(mocks.ticketQueries[mocks.ticketQueries.length - 1]).toMatchObject({
      current: 1,
      pageSize: 10,
      status: 0,
      email: 'buyer@example.com',
    });
    expect(ticketsSource).toContain('setTimeout(() => filter(key, value), 300)');

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });

  it('keeps the legacy reply-status filter dropdown wired to ticket queries', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    let root: Root | null = createRoot(container);

    await act(async () => {
      root!.render(<TicketsPage />);
      await Promise.resolve();
    });

    const dropdown = document.body.querySelector<HTMLElement>('.ant-dropdown');
    expect(dropdown?.className).toContain('ant-dropdown-hidden');

    const filterIcon = container.querySelector<HTMLElement>('i.anticon-filter')!;

    await act(async () => {
      filterIcon.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(dropdown?.className).not.toContain('ant-dropdown-hidden');

    const checkedInput = Array.from(
      document.body.querySelectorAll<HTMLInputElement>('.ant-checkbox-input'),
    )[0]!;

    await act(async () => {
      checkedInput.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    mocks.ticketQueries = [];
    const confirm = document.body.querySelector<HTMLElement>(
      '.ant-table-filter-dropdown-link.confirm',
    )!;

    await act(async () => {
      confirm.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.ticketQueries[mocks.ticketQueries.length - 1]).toMatchObject({
      current: 1,
      pageSize: 10,
      status: 0,
      reply_status: [1],
    });

    await act(async () => {
      root?.unmount();
      root = null;
    });
    container.remove();
  });

  it('keeps the bundled anchor disabled prop shape for ticket close links', () => {
    expect(ticketsSource).toContain('type AnchorHTMLAttributes');
    expect(ticketsSource).toContain(
      'function legacyDisabledAnchorProps(disabled: unknown): AnchorHTMLAttributes<HTMLAnchorElement>',
    );
    expect(ticketsSource).toContain(
      'return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;',
    );
    expect(ticketsSource).toContain('{...legacyDisabledAnchorProps(row.status)}');
    expect(ticketsSource).not.toContain('...(row.status ? { disabled: true } : {})');
  });

  it('uses the original fetchLoading-style page spinner for ticket refetches', () => {
    expect(ticketsSource).toContain('<LegacySpin loading={tickets.isFetching}>');
    expect(ticketsSource).not.toContain('loading={tickets.isLoading}');
  });

  it('keeps ticket list and chat queries on the shared legacy admin query defaults', () => {
    expect(queriesSource).toContain('queryFn: () => admin.fetchTickets(apiClient, query),');
    expect(queriesSource).toContain(
      'queryFn: () => admin.ticketDetail(apiClient, id as number | string),',
    );
    expect(queriesSource).not.toContain('legacyTicketQueryOptions');
    expect(queriesSource).not.toContain('staleTime: 30_000');
    expect(queriesSource).not.toContain('refetchOnMount: false');
  });

  it('keeps the original desktop ticket chat popup behavior', () => {
    expect(ticketsSource).toContain(
      'const url = `${window.location.origin}${window.location.pathname}#/ticket/${id}`;',
    );
    expect(ticketsSource).toContain('const userAgent = window.navigator.userAgent.toLowerCase();');
    expect(ticketsSource).toContain("!userAgent.includes('mobile') && !userAgent.includes('ipad')");
    expect(ticketsSource).toContain('window.open(');
    expect(ticketsSource).toContain("'_blank'");
    expect(ticketsSource).toContain(
      "'height=600,width=800,top=0,left=0,toolbar=no,menubar=no,scrollbars=no,resizable=no,location=no,status=no'",
    );
    expect(ticketsSource).toContain('window.location.href = url;');
    expect(ticketsSource).not.toContain('navigate(`/ticket/${id}`)');
  });

  it('keeps ticket close fetching from the list page after success', () => {
    const closeBlock = queriesSource.slice(
      queriesSource.indexOf('export function useCloseTicketMutation()'),
      queriesSource.indexOf('export function useSaveNoticeMutation()'),
    );
    const closeStart = ticketsSource.indexOf('closeTicket.mutate(row.id, {');
    const closeRefetch = ticketsSource.indexOf('void tickets.refetch();', closeStart);

    expect(closeStart).toBeGreaterThan(-1);
    expect(closeRefetch).toBeGreaterThan(closeStart);
    expect(closeBlock).not.toContain('onSuccess');
    expect(closeBlock).not.toContain(
      "queryClient.invalidateQueries({ queryKey: ['admin', 'tickets'] })",
    );
  });

  it('keeps the legacy ticket table without an explicit rowKey', () => {
    expect(ticketsSource).toContain('<LegacyStandaloneTable');
    expect(ticketsSource).toContain('scrollX={900}');
    expect(ticketsSource).toContain('{...legacyTableRowKey(index)}');
    expect(ticketsSource).not.toContain('<Table<Ticket>');
    expect(ticketsSource).not.toContain('tableLayout="auto"');
    expect(ticketsSource).not.toContain('rowKey="id"');
  });

  it('keeps the original ticket query shape without AntD5 pagination prop rewrites', () => {
    expect(ticketsSource).toContain('total?: number;');
    expect(ticketsSource).toContain('[key]: value,');
    expect(ticketsSource).toContain('current: 1,');
    expect(ticketsSource).toContain('pageSize: 10,');
    expect(ticketsSource).not.toContain('...pagination,');
    expect(ticketsSource).not.toContain('current: pagination.current');
    expect(ticketsSource).not.toContain('pageSize: pagination.pageSize');
  });

  it('renders the bundled ticket table empty state without AntD5 pagination totals', () => {
    expect(ticketsSource).toContain('isEmpty={data.length === 0}');
    expect(ticketsSource).not.toContain('total: tickets.data?.total,');
    expect(ticketsSource).not.toContain('total: tickets.data?.total ?? 0');
    expect(ticketsSource).not.toContain('pagination={{');
  });

  it('uses the bundled reply-status filter header and dropdown shape', () => {
    expect(ticketsSource).toContain("'ant-table-column-has-actions ant-table-column-has-filters'");
    expect(ticketsSource).toContain('<LegacyFilterIcon');
    expect(ticketsSource).toContain('title="筛选"');
    expect(ticketsSource).toContain('className="ant-dropdown-trigger"');
    expect(ticketsSource).toContain('ant-table-filter-dropdown');
    expect(ticketsSource).toContain('ant-table-filter-dropdown-link confirm');
    expect(ticketsSource).toContain("filter('reply_status', replyStatusFilterValue)");
    expect(ticketsSource).not.toContain('filters: (query.status !== 1 && [');
    expect(ticketsSource).not.toContain("]) as ColumnType<Ticket>['filters'],");
  });

  it('keeps the original vertical divider markup in ticket action columns', () => {
    expect(
      ticketsSource.match(/<div className="ant-divider ant-divider-vertical" \/>/g),
    ).toHaveLength(2);
    expect(ticketsSource).not.toContain('<span className="ant-divider ant-divider-vertical"');
    expect(ticketsSource).not.toContain('role="separator"');
  });

  it('renders /ticket/:ticket_id as the original chat window', () => {
    mocks.params = { ticket_id: '1' };
    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('block-content-full bg-gray-lighter p-3');
    expect(html).toContain('tag___12_9H');
    expect(html).toContain('ctrl___UqDJ7');
    expect(html).toContain('支付问题');
    expect(html).toContain('js-chat-messages');
    expect(html).toContain('content___DW5w1');
    expect(html).toContain('用户消息');
    expect(html).toContain('客服回复');
    expect(html).toContain('bg-success-lighter');
    expect(html).toContain('bg-gray-lighter px-3');
    expect(html).toContain('js-chat-form');
    expect(html).toContain('input___1j_ND');
    expect(html).toContain('输入内容回复工单...');
    expect(ticketsSource).toContain('setUserOpen(true)');
    expect(ticketsSource).toContain('<UserManageDrawer');
    expect(html).toContain('aria-label="图标: user"');
    expect(html).toContain('class="anticon anticon-user"');
    expect(ticketsSource).toContain(
      '<LegacyUserIcon onClick={() => current?.user_id && setUserOpen(true)} />',
    );
    expect(ticketsSource).not.toContain('@ant-design/icons');
    expect(ticketsSource).not.toContain('UserOutlined');
    expect(ticketsSource).not.toContain(
      '<span onClick={() => current?.user_id && setUserOpen(true)}>',
    );
    expect(mocks.adminUserInfoIds).toContain(1);
    expect(ticketsSource).toContain('setTrafficOpen(true)');
    expect(ticketsSource).toContain('<UserTrafficModal');
    expect(html).toContain('aria-label="图标: solution"');
    expect(html).toContain('class="anticon anticon-solution"');
    expect(ticketsSource).toContain(
      '<LegacySolutionIcon onClick={() => current?.user_id && setTrafficOpen(true)} />',
    );
    expect(ticketsSource).not.toContain('SolutionOutlined');
    expect(ticketsSource).not.toContain(
      '<span onClick={() => current?.user_id && setTrafficOpen(true)}>',
    );
    expect(ticketsSource).toContain('key={current?.user_id}');
    expect(ticketsSource).toContain("messageApi.loading('发送中')");
    expect(ticketsSource).toContain('messageApi.destroy()');
    expect(html).not.toContain('ant-drawer');
    expect(html).not.toContain('ant-card');
  });

  it('keeps the old admin chat shell visible when ticket fetch fails', () => {
    mocks.params = { ticket_id: '1' };
    mocks.adminTicket = undefined;
    mocks.adminTicketError = true;

    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('工单不存在');
    expect(html).toContain('block-content-full bg-gray-lighter p-3');
    expect(html).toContain('tag___12_9H');
    expect(html).toContain('ctrl___UqDJ7');
    expect(html).toContain('js-chat-messages');
    expect(html).toContain('content___DW5w1');
    expect(html).toContain('js-chat-form');
    expect(html).toContain('input___1j_ND');
    expect(html).toContain('js-chat-input bg-body-dark border-0 form-control form-control-alt');
    expect(html).toContain('输入内容回复工单...');
    expect(html).not.toContain('加载中...');
    expect(html).not.toContain('ant-empty');
    expect(html).not.toContain('暂无数据');
    expect(html).not.toContain('支付问题');
    expect(mocks.adminUserInfoIds).toContain(undefined);
  });

  it('renders visible loading text before the admin ticket fetch resolves', () => {
    mocks.params = { ticket_id: '1' };
    mocks.adminTicket = undefined;
    mocks.adminTicketError = false;

    const html = renderToStaticMarkup(<TicketsPage />);

    expect(html).toContain('加载中...');
    expect(html).toContain('tag___12_9H');
    expect(html).toContain('font-size-sm text-muted my-2 text-center');
  });

  it('keeps the old chat reply message state lifetime', () => {
    const replyBlock = ticketsSource.slice(
      ticketsSource.indexOf('const sendReply = async () => {'),
      ticketsSource.indexOf('const current = ticket.data;'),
    );
    const mutationBlock = queriesSource.slice(
      queriesSource.indexOf('export function useReplyTicketMutation()'),
      queriesSource.indexOf('export function useCloseTicketMutation()'),
    );

    expect(ticketsSource).toContain(
      'const [message, setMessage] = useState<string | undefined>(undefined);',
    );
    expect(replyBlock).toContain('await reply.mutateAsync({ id: ticketId, message });');
    expect(replyBlock).toContain('messageApi.destroy();');
    expect(replyBlock).toContain('void ticket.refetch();');
    expect(replyBlock).toContain("if (inputRef.current) inputRef.current.value = '';");
    expect(replyBlock.indexOf('await reply.mutateAsync({ id: ticketId, message });')).toBeLessThan(
      replyBlock.indexOf('messageApi.destroy();'),
    );
    expect(replyBlock.indexOf('messageApi.destroy();')).toBeLessThan(
      replyBlock.indexOf('void ticket.refetch();'),
    );
    expect(replyBlock.indexOf('void ticket.refetch();')).toBeLessThan(
      replyBlock.indexOf("if (inputRef.current) inputRef.current.value = '';"),
    );
    expect(ticketsSource).not.toContain("setMessage('');");
    expect(ticketsSource).not.toContain('await ticket.refetch().catch(() => undefined);');
    expect(mutationBlock).not.toContain(
      "queryClient.invalidateQueries({ queryKey: ['admin', 'tickets'] })",
    );
  });

  it('keeps the old unkeyed ticket message map from the bundled chat component', () => {
    const messageSource = ticketsSource.slice(
      ticketsSource.indexOf('{current?.message!.map((item) =>'),
      ticketsSource.indexOf(
        '<div className="js-chat-form',
        ticketsSource.indexOf('{current?.message!.map((item) =>'),
      ),
    );

    expect(messageSource).toContain('{current?.message!.map((item) =>');
    expect(messageSource).not.toContain('key={item.id}');
    expect(messageSource).not.toContain('key={index}');
    expect(messageSource).not.toContain('key=');
    expect(ticketsSource).not.toContain('current?.message?.map');
    expect(ticketsSource).not.toContain('ticket.data?.message?.length');
    expect(ticketsSource).toContain('ticket.data?.message!.length');
  });

  it('scrolls the bundled admin chat window to the latest message', () => {
    expect(ticketsSource).toContain('chat.scrollTo(0, chat.scrollHeight)');
    expect(ticketsSource).toContain('[messageCount]');
  });

  it('polls the bundled admin chat ticket every five seconds like the old class component', () => {
    const timeoutHandlers: Array<() => void> = [];
    const timeoutIds = [
      {} as ReturnType<typeof window.setTimeout>,
      {} as ReturnType<typeof window.setTimeout>,
    ];
    const setTimeoutSpy = vi.spyOn(window, 'setTimeout').mockImplementation((handler) => {
      if (typeof handler === 'function') timeoutHandlers.push(handler);
      return timeoutIds[timeoutHandlers.length - 1] ?? timeoutIds[0]!;
    });
    const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout').mockImplementation(() => undefined);

    try {
      const stopPolling = startLegacyTicketPolling(mocks.ticketRefetch);

      expect(ticketsSource).toContain('startLegacyTicketPolling(ticket.refetch)');
      expect(ticketsSource).toContain('5000');
      expect(ticketsSource).toContain('window.setTimeout');
      expect(ticketsSource).toContain('window.clearTimeout');
      expect(ticketsSource).not.toContain('window.setInterval');
      expect(queriesSource).not.toContain('refetchInterval: 5000');
      expect(setTimeoutSpy).toHaveBeenCalledWith(expect.any(Function), 5000);

      timeoutHandlers[0]?.();

      expect(mocks.ticketRefetch).toHaveBeenCalledTimes(1);
      expect(setTimeoutSpy).toHaveBeenCalledTimes(2);

      stopPolling();

      expect(clearTimeoutSpy).toHaveBeenCalledWith(timeoutIds[1]);
    } finally {
      setTimeoutSpy.mockRestore();
      clearTimeoutSpy.mockRestore();
    }
  });
});
