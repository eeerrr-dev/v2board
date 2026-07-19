import type { ReactNode } from 'react';
import { screen, waitFor, within } from '@testing-library/react';
import { formatBackendDateMinuteSlash } from '@v2board/config/format';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import InvitePage from './invite';

const mocks = vi.hoisted(() => ({
  comm: {
    commission_distribution_enable: 0,
    commission_distribution_l1: 50,
    commission_distribution_l2: 30,
    commission_distribution_l3: 20,
    currency: 'CNY',
    currency_symbol: '¥',
    withdraw_close: 1,
    withdraw_methods: [],
  } as Record<string, unknown>,
  copyText: vi.fn(),
  detailQueryCalls: [] as Array<{ current?: number; pageSize?: number }>,
  detailRows: [] as Array<{ created_at: string; get_amount: number }>,
  detailsError: false,
  detailsFetching: false,
  detailsRefetch: vi.fn(),
  detailsTotal: 0,
  generateIsPending: false,
  generateMutateAsync: vi.fn(),
  invalidateQueries: vi.fn(),
  inviteCodes: [] as Array<{ code: string; created_at: string }>,
  inviteError: false,
  inviteFetching: false,
  inviteRefetch: vi.fn(),
  // §9.2 named stat object (W7): commissions in cents, rate integer percent.
  inviteStat: {
    registered_count: 7,
    valid_commission: 2345,
    pending_commission: 678,
    commission_rate: 12,
    available_commission: 12345,
  } as Record<string, number> | undefined,
  labels: {
    'common.empty': '暂无数据',
    'common.error_title': '出错了',
    'common.items_per_page': '条/页',
    'common.loading': 'Loading...',
    'common.next_5': '向后 5 页',
    'common.next_page': '下一页',
    'common.prev_5': '向前 5 页',
    'common.prev_page': '上一页',
    'common.retry': '重试',
    'dashboard.copy_success': '复制成功',
    'invite.available': '当前剩余佣金',
    'invite.code_col': '邀请码',
    'invite.commission_col': '佣金',
    'invite.commission_rate': '佣金比例',
    'invite.created_at_col': '创建时间',
    'invite.generate': '生成邀请码',
    'invite.generated': '已生成',
    'invite.history': '佣金发放记录',
    'invite.invite_link': '复制链接',
    'invite.issued_at': '发放时间',
    'invite.manage': '邀请码管理',
    'invite.pending_commission': '确认中的佣金',
    'invite.pending_hint': '佣金将会在确认后会到达你的佣金账户。',
    'invite.people_count': '{{count}}人',
    'invite.registered': '已注册用户数',
    'invite.title': '我的邀请',
    'invite.transfer': '划转',
    'invite.triple_hint': '您邀请的用户再次邀请用户将按照订单金额乘以分销等级的比例进行分成。',
    'invite.triple_rate': '三级分销比例',
    'invite.valid_commission': '累计获得佣金',
    'invite.withdraw_button': '推广佣金提现',
  } as Record<string, string>,
  toastSuccess: vi.fn(),
  userInfo: {
    commission_balance: 12345,
  } as Record<string, unknown>,
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({ invalidateQueries: mocks.invalidateQueries }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation(mocks.labels),
}));

vi.mock('@/components/dialogs/transfer-dialog', () => ({
  TransferDialog: ({ children }: { children: ReactNode }) => children,
}));

vi.mock('@/components/dialogs/withdraw-dialog', () => ({
  WithdrawDialog: ({ children }: { children: ReactNode }) => children,
}));

vi.mock('@/lib/queries', () => ({
  userKeys: {
    invite: ['user', 'invite'],
  },
  useCommConfig: () => ({
    data: mocks.comm,
  }),
  useGenerateInviteMutation: () => ({
    isPending: mocks.generateIsPending,
    mutate: (_payload: undefined, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.generateMutateAsync()).then(options?.onSuccess);
    },
  }),
  useInvite: () => ({
    data: mocks.inviteError
      ? undefined
      : {
          codes: mocks.inviteCodes,
          stat: mocks.inviteStat,
        },
    isError: mocks.inviteError,
    isFetching: mocks.inviteFetching,
    refetch: mocks.inviteRefetch,
  }),
  useInviteDetails: (current?: number, pageSize?: number) => {
    mocks.detailQueryCalls.push({ current, pageSize });
    return {
      data: mocks.detailsError ? undefined : { data: mocks.detailRows, total: mocks.detailsTotal },
      isError: mocks.detailsError,
      isFetching: mocks.detailsFetching,
      refetch: mocks.detailsRefetch,
    };
  },
  useUserInfo: () => ({
    data: mocks.userInfo,
  }),
}));

vi.mock('@v2board/config/clipboard', () => ({
  copyText: mocks.copyText,
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    success: mocks.toastSuccess,
  },
}));

function resetMocks() {
  mocks.comm = {
    commission_distribution_enable: 0,
    commission_distribution_l1: 50,
    commission_distribution_l2: 30,
    commission_distribution_l3: 20,
    currency: 'CNY',
    currency_symbol: '¥',
    withdraw_close: 1,
    withdraw_methods: [],
  };
  mocks.copyText.mockReset();
  mocks.copyText.mockResolvedValue(true);
  mocks.detailQueryCalls = [];
  mocks.detailRows = [];
  mocks.detailsError = false;
  mocks.detailsFetching = false;
  mocks.detailsRefetch.mockReset();
  mocks.detailsTotal = 0;
  mocks.generateIsPending = false;
  mocks.generateMutateAsync.mockReset();
  mocks.generateMutateAsync.mockResolvedValue(true);
  mocks.invalidateQueries.mockReset();
  mocks.inviteCodes = [];
  mocks.inviteError = false;
  mocks.inviteFetching = false;
  mocks.inviteRefetch.mockReset();
  mocks.inviteStat = {
    registered_count: 7,
    valid_commission: 2345,
    pending_commission: 678,
    commission_rate: 12,
    available_commission: 12345,
  };
  mocks.toastSuccess.mockReset();
  mocks.userInfo = { commission_balance: 12345 };
}

describe('InvitePage shadcn surface', () => {
  beforeEach(resetMocks);

  it('renders the commission summary, stats, code table, and history table', () => {
    mocks.inviteCodes = [{ code: 'ABC123', created_at: '2023-11-14T22:13:20Z' }];
    mocks.detailRows = [{ created_at: '2023-11-14T22:23:20Z', get_amount: 1234 }];
    mocks.detailsTotal = 1;

    renderWithProviders(<InvitePage />);

    // Summary card: balance in cents rendered as plain currency text.
    const summary = screen.getByTestId('invite-summary-card');
    expect(within(summary).getByText('我的邀请')).toBeInTheDocument();
    expect(within(summary).getByText('123.45')).toBeInTheDocument();
    expect(within(summary).getByText('CNY')).toBeInTheDocument();
    expect(within(summary).getByText('当前剩余佣金')).toBeInTheDocument();
    const transfer = within(summary).getByRole('button', { name: '划转' });
    expect(transfer).toHaveAttribute('data-testid', 'invite-transfer-trigger');
    // withdraw_close hides the withdraw entry entirely.
    expect(screen.queryByRole('button', { name: '推广佣金提现' })).toBeNull();
    expect(screen.queryByTestId('invite-withdraw-trigger')).toBeNull();

    // Stats card: the §9.2 named stat object (registered_count,
    // valid_commission, pending_commission, commission_rate).
    const stats = screen.getByTestId('invite-stats-card');
    expect(within(stats).getByText('确认中的佣金')).toHaveAttribute(
      'data-slot',
      'header-tooltip-trigger',
    );
    expect(within(stats).getByText('已注册用户数')).toBeInTheDocument();
    expect(within(stats).getByText('7人')).toBeInTheDocument();
    expect(within(stats).getByText('佣金比例')).toBeInTheDocument();
    expect(within(stats).getByText('12%')).toBeInTheDocument();
    expect(within(stats).getByText('确认中的佣金')).toBeInTheDocument();
    expect(within(stats).getByText('¥ 6.78')).toBeInTheDocument();
    expect(within(stats).getByText('累计获得佣金')).toBeInTheDocument();
    expect(within(stats).getByText('¥ 23.45')).toBeInTheDocument();

    // Code management card.
    const codeCard = screen.getByTestId('invite-code-card');
    expect(within(codeCard).getByText('邀请码管理')).toBeInTheDocument();
    expect(within(codeCard).getByRole('button', { name: '生成邀请码' })).toHaveAttribute(
      'data-testid',
      'invite-generate',
    );

    // Both tables render through the shared DataTable: real table semantics,
    // shared scroll container hook, and index-based data-row-key row hooks.
    expect(screen.getAllByTestId('invite-table-scroll')).toHaveLength(2);

    const codeTable = screen.getByTestId('invite-code-table');
    expect(codeTable).toContainElement(
      within(codeTable).getByRole('columnheader', { name: '邀请码' }),
    );
    expect(within(codeTable).getByRole('columnheader', { name: '创建时间' })).toBeInTheDocument();
    expect(within(codeTable).getByText('ABC123')).toBeInTheDocument();
    expect(within(codeTable).getByRole('button', { name: '复制链接' })).toBeInTheDocument();
    expect(
      within(codeTable).getByText(formatBackendDateMinuteSlash('2023-11-14T22:13:20Z')),
    ).toBeInTheDocument();
    expect(within(codeTable).getByText('ABC123').closest('tr')).toHaveAttribute(
      'data-row-key',
      '0',
    );

    const historyCard = screen.getByTestId('invite-history-card');
    expect(within(historyCard).getByText('佣金发放记录')).toBeInTheDocument();
    const historyTable = screen.getByTestId('invite-history-table');
    expect(
      within(historyTable).getByRole('columnheader', { name: '发放时间' }),
    ).toBeInTheDocument();
    expect(within(historyTable).getByRole('columnheader', { name: '佣金' })).toBeInTheDocument();
    expect(
      within(historyTable).getByText(formatBackendDateMinuteSlash('2023-11-14T22:23:20Z')),
    ).toBeInTheDocument();
    expect(within(historyTable).getByText('12.34')).toBeInTheDocument();
    expect(within(historyTable).getByText('12.34').closest('tr')).toHaveAttribute(
      'data-row-key',
      '0',
    );
  });

  it('renders the distribution-rate branch and withdraw button when enabled', () => {
    mocks.comm = {
      ...mocks.comm,
      commission_distribution_enable: 1,
      withdraw_close: 0,
    };

    renderWithProviders(<InvitePage />);

    expect(screen.getByText('三级分销比例')).toBeInTheDocument();
    expect(screen.getByText('6%,3.6%,2.4%')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '推广佣金提现' })).toHaveAttribute(
      'data-testid',
      'invite-withdraw-trigger',
    );
  });

  it('does not render NaN distribution rates while invite stats are loading', () => {
    mocks.comm = {
      ...mocks.comm,
      commission_distribution_enable: 1,
    };
    mocks.inviteStat = undefined;

    const { container } = renderWithProviders(<InvitePage />);

    expect(screen.getByText('三级分销比例')).toBeInTheDocument();
    expect(container.textContent).not.toContain('NaN');
  });

  it('keeps the stats card content visible and dimmed while invite/fetch is pending', () => {
    mocks.inviteFetching = true;

    renderWithProviders(<InvitePage />);

    const stats = screen.getByTestId('invite-stats-card');
    // Loading dims the card instead of replacing it with a blocking loader;
    // stat labels/values stay rendered. (The card has no accessible busy state,
    // so the dim is pinned via its class.)
    expect(stats).toHaveClass('opacity-80');
    expect(within(stats).getByText('已注册用户数')).toBeInTheDocument();
    expect(within(stats).getByText('7人')).toBeInTheDocument();
  });
});

describe('InvitePage fetch failures', () => {
  beforeEach(resetMocks);

  it('replaces the stat tiles and code table with a retryable error state when invite/fetch fails', async () => {
    mocks.inviteError = true;
    const { user } = renderWithProviders(<InvitePage />);

    // No perpetual stat spinners: the tile grid is replaced by the error state.
    const stats = screen.getByTestId('invite-stats-card');
    expect(within(stats).getByTestId('invite-stats-error')).toHaveTextContent('出错了');
    expect(within(stats).queryByText('已注册用户数')).toBeNull();
    expect(stats.querySelector('.animate-spin')).toBeNull();

    // The code table must not masquerade as "no invite codes" on failure.
    const codeCard = screen.getByTestId('invite-code-card');
    expect(within(codeCard).getByTestId('invite-code-error')).toBeInTheDocument();
    expect(screen.queryByTestId('invite-code-table')).toBeNull();
    expect(within(codeCard).queryByTestId('invite-empty')).toBeNull();

    await user.click(within(stats).getByRole('button', { name: '重试' }));
    expect(mocks.inviteRefetch).toHaveBeenCalledTimes(1);
  });

  it('replaces the commission history with a retryable error state when invite/details fails', async () => {
    mocks.detailsError = true;
    const { user } = renderWithProviders(<InvitePage />);

    const historyCard = screen.getByTestId('invite-history-card');
    expect(within(historyCard).getByTestId('invite-history-error')).toHaveTextContent('出错了');
    expect(screen.queryByTestId('invite-history-table')).toBeNull();
    expect(screen.queryByTestId('invite-pagination')).toBeNull();

    await user.click(within(historyCard).getByRole('button', { name: '重试' }));
    expect(mocks.detailsRefetch).toHaveBeenCalledTimes(1);
  });
});

describe('InvitePage shadcn pagination', () => {
  beforeEach(resetMocks);

  it('omits table pagination for an empty commission history', () => {
    renderWithProviders(<InvitePage />);

    expect(screen.queryByTestId('invite-pagination')).toBeNull();
    expect(screen.queryByTestId('invite-page-size')).toBeNull();
  });

  it('shows table pagination when commission history has rows', () => {
    mocks.detailRows = [{ created_at: '2023-11-14T22:13:20Z', get_amount: 100 }];
    mocks.detailsTotal = 1;

    renderWithProviders(<InvitePage />);

    expect(screen.getByTestId('invite-pagination')).toBeInTheDocument();
    expect(screen.getByRole('button', { current: 'page', name: '1' })).toBeInTheDocument();
    expect(screen.getByTestId('invite-page-size')).toBeInTheDocument();
  });

  it('shows the commission history loading indicator while the details fetch is pending', () => {
    mocks.detailsFetching = true;

    renderWithProviders(<InvitePage />);

    expect(screen.getByRole('status')).toHaveTextContent('Loading...');
  });
});

describe('InvitePage shadcn actions', () => {
  beforeEach(resetMocks);

  it('copies the path-style register URL and shows the original success toast', async () => {
    mocks.inviteCodes = [{ code: 'ABC123', created_at: '2023-11-14T22:13:20Z' }];
    const { user } = renderWithProviders(<InvitePage />);

    await user.click(screen.getByRole('button', { name: '复制链接' }));

    // Tier-1 copy-link URL under history routing (docs/api-dialect.md §10.1):
    // external invitees land on /register with the code preserved.
    expect(mocks.copyText).toHaveBeenCalledWith(`${window.location.origin}/register?code=ABC123`);
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功'));
  });

  it('generates a code once, shows the old hard-coded success toast, and refetches invite data', async () => {
    const { user } = renderWithProviders(<InvitePage />);

    await user.click(screen.getByTestId('invite-generate'));

    expect(mocks.generateMutateAsync).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('已生成'));
    expect(mocks.invalidateQueries).toHaveBeenCalledWith({
      exact: true,
      queryKey: ['user', 'invite'],
    });
  });

  it('does not generate another code while saveLoading is active', async () => {
    mocks.generateIsPending = true;
    const { user } = renderWithProviders(<InvitePage />);

    const generate = screen.getByTestId('invite-generate');
    expect(generate).toBeDisabled();
    expect(generate).toHaveAttribute('aria-busy', 'true');

    await user.click(generate);

    expect(mocks.generateMutateAsync).not.toHaveBeenCalled();
  });

  it('requests the commission history with the page selected from the table pagination', async () => {
    mocks.detailRows = [{ created_at: '2023-11-14T22:23:20Z', get_amount: 1234 }];
    mocks.detailsTotal = 25;
    const { user } = renderWithProviders(<InvitePage />);

    await user.click(screen.getByRole('button', { name: '2' }));

    expect(screen.getByRole('button', { current: 'page', name: '2' })).toBeInTheDocument();
    expect(mocks.detailQueryCalls.at(-1)).toEqual({ current: 2, pageSize: 10 });
  });

  it('clamps the visible commission-history page like the legacy pagination helper', async () => {
    mocks.detailRows = [{ created_at: '2023-11-14T22:23:20Z', get_amount: 1234 }];
    mocks.detailsTotal = 45;
    const { rerender, user } = renderWithProviders(<InvitePage />);

    await user.click(screen.getByRole('button', { name: '4' }));

    expect(screen.getByRole('button', { current: 'page', name: '4' })).toBeInTheDocument();
    expect(mocks.detailQueryCalls.at(-1)).toEqual({ current: 4, pageSize: 10 });

    // The total shrinks under the selected page: the raw page state stays 4,
    // but the visible current page clamps down to the last page (3).
    mocks.detailsTotal = 25;
    rerender(<InvitePage />);

    expect(screen.getByRole('button', { current: 'page', name: '3' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '4' })).toBeNull();
  });
});
