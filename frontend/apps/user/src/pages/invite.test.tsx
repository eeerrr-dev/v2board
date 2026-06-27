import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import type { ReactNode } from 'react';
import { readFileSync } from 'node:fs';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import InvitePage from './invite';

const inviteSource = readFileSync(`${process.cwd()}/src/pages/invite.tsx`, 'utf8');

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
  detailRows: [] as Array<{ created_at: number; get_amount: number }>,
  detailsFetching: false,
  detailsTotal: 0,
  generateIsPending: false,
  generateMutateAsync: vi.fn(),
  invalidateQueries: vi.fn(),
  inviteCodes: [] as Array<{ code: string; created_at: number }>,
  inviteFetching: false,
  labels: {
    'common.items_per_page': '条/页',
    'common.next_5': '向后 5 页',
    'common.next_page': '下一页',
    'common.prev_5': '向前 5 页',
    'common.prev_page': '上一页',
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
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string, values?: Record<string, unknown>) => {
      let label = mocks.labels[key] ?? key;
      Object.entries(values ?? {}).forEach(([name, value]) => {
        label = label.replaceAll(`{{${name}}}`, String(value));
      });
      return label;
    },
  }),
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
    mutateAsync: mocks.generateMutateAsync,
  }),
  useInvite: () => ({
    data: {
      codes: mocks.inviteCodes,
      stat: [7, 2345, 678, 12],
    },
    isFetching: mocks.inviteFetching,
  }),
  useInviteDetails: (current?: number, pageSize?: number) => {
    mocks.detailQueryCalls.push({ current, pageSize });
    return {
      data: { data: mocks.detailRows, total: mocks.detailsTotal },
      isFetching: mocks.detailsFetching,
    };
  },
  useUserInfo: () => ({
    data: mocks.userInfo,
  }),
}));

vi.mock('@/lib/legacy-settings', () => ({
  copyText: mocks.copyText,
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    success: mocks.toastSuccess,
  },
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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
  mocks.detailsFetching = false;
  mocks.detailsTotal = 0;
  mocks.generateIsPending = false;
  mocks.generateMutateAsync.mockReset();
  mocks.generateMutateAsync.mockResolvedValue(true);
  mocks.invalidateQueries.mockReset();
  mocks.inviteCodes = [];
  mocks.inviteFetching = false;
  mocks.toastSuccess.mockReset();
  mocks.userInfo = { commission_balance: 12345 };
}

describe('InvitePage shadcn surface', () => {
  beforeEach(resetMocks);

  it('renders the shadcn commission summary, stats, code table, and history table shell', () => {
    mocks.inviteCodes = [{ code: 'ABC123', created_at: 1_700_000_000 }];
    mocks.detailRows = [{ created_at: 1_700_000_600, get_amount: 1234 }];
    mocks.detailsTotal = 1;

    const html = renderToStaticMarkup(<InvitePage />);

    expect(html).toContain('data-testid="invite-summary-card"');
    expect(html).toContain('data-testid="invite-stats-card"');
    expect(html).toContain('data-testid="invite-code-card"');
    expect(html).toContain('data-testid="invite-history-card"');
    expect(html).toContain('我的邀请');
    expect(html).toContain('123.45');
    expect(html).toContain('CNY');
    expect(html).toContain('当前剩余佣金');
    expect(html).toContain('data-testid="invite-transfer-trigger"');
    expect(html).toContain('划转');
    expect(html).not.toContain('推广佣金提现');
    expect(html).toContain('已注册用户数');
    expect(html).toContain('7人');
    expect(html).toContain('佣金比例');
    expect(html).toContain('12%');
    expect(html).toContain('确认中的佣金');
    expect(html).toContain('¥ 6.78');
    expect(html).toContain('累计获得佣金');
    expect(html).toContain('¥ 23.45');
    expect(html).toContain('邀请码管理');
    expect(html).toContain('生成邀请码');
    expect(html).toContain('data-testid="invite-code-table"');
    expect(html).toContain('data-testid="invite-history-table"');
    expect(html).toContain('ABC123');
    expect(html).toContain('复制链接');
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_000));
    expect(html).toContain('佣金发放记录');
    expect(html).toContain('发放时间');
    expect(html).toContain('佣金');
    expect(html).toContain(formatLegacyDateMinuteSlash(1_700_000_600));
    expect(html).toContain('12.34');
    expect(html.match(/data-row-key="0"/g)).toHaveLength(2);
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('ant-table-wrapper');
  });

  it('keeps invite table row keys as index DOM attributes', () => {
    expect(inviteSource).toContain('data-row-key={index}');
    expect(inviteSource).not.toContain('data-row-key={code.code}');
    expect(inviteSource).not.toContain('data-row-key={row.created_at}');
  });

  it('renders the distribution-rate branch and withdraw button when enabled', () => {
    mocks.comm = {
      ...mocks.comm,
      commission_distribution_enable: 1,
      withdraw_close: 0,
    };

    const html = renderToStaticMarkup(<InvitePage />);

    expect(html).toContain('三级分销比例');
    expect(html).toContain('6%,3.5999999999999996%,2.4%');
    expect(html).toContain('推广佣金提现');
    expect(html).toContain('data-testid="invite-withdraw-trigger"');
  });

  it('keeps loading state in the shadcn cards while invite/fetch is pending', () => {
    mocks.inviteFetching = true;

    const html = renderToStaticMarkup(<InvitePage />);

    expect(html).toContain('data-testid="invite-stats-card"');
    expect(html).toContain('opacity-80');
    expect(html).not.toContain('block-mode-loading');
  });
});

describe('InvitePage shadcn pagination', () => {
  beforeEach(resetMocks);

  it('omits table pagination for an empty commission history', () => {
    const html = renderToStaticMarkup(<InvitePage />);

    expect(html).not.toContain('data-testid="invite-pagination"');
    expect(html).not.toContain('data-page="0"');
    expect(html).not.toContain('data-testid="invite-page-size"');
  });

  it('shows table pagination when commission history has rows', () => {
    mocks.detailRows = [{ created_at: 1, get_amount: 100 }];
    mocks.detailsTotal = 1;

    const html = renderToStaticMarkup(<InvitePage />);

    expect(html).toContain('data-testid="invite-pagination"');
    expect(html).toContain('data-page="1"');
    expect(html).toContain('data-testid="invite-page-size"');
  });

  it('does not blur the commission history table before the mount details dispatch equivalent', () => {
    mocks.detailsFetching = true;

    const html = renderToStaticMarkup(<InvitePage />);

    expect(html).not.toContain('Loading...');
    expect(html).not.toContain('ant-spin-spinning');
    expect(html).not.toContain('ant-spin-blur');
  });
});

describe('InvitePage shadcn actions', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    resetMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderInvite() {
    await act(async () => {
      root.render(<InvitePage />);
      await Promise.resolve();
    });
  }

  it('copies the exact legacy register URL and shows the original success toast', async () => {
    mocks.inviteCodes = [{ code: 'ABC123', created_at: 1_700_000_000 }];
    await renderInvite();

    const copy = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '复制链接',
    )!;

    await act(async () => {
      copy.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.copyText).toHaveBeenCalledWith(
      `${window.location.origin}${window.location.pathname}#/register?code=ABC123`,
    );
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');
  });

  it('generates a code once, shows the old hard-coded success toast, and refetches invite data', async () => {
    await renderInvite();

    const generate = container.querySelector<HTMLButtonElement>(
      '[data-testid="invite-generate"]',
    )!;

    await act(async () => {
      generate.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.generateMutateAsync).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('已生成');
    expect(mocks.invalidateQueries).toHaveBeenCalledWith({
      exact: true,
      queryKey: ['user', 'invite'],
    });
  });

  it('does not generate another code while saveLoading is active', async () => {
    mocks.generateIsPending = true;
    await renderInvite();

    const generate = container.querySelector<HTMLButtonElement>(
      '[data-testid="invite-generate"]',
    )!;

    expect(generate.disabled).toBe(true);
    expect(generate.getAttribute('aria-busy')).toBe('true');

    await act(async () => {
      generate.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.generateMutateAsync).not.toHaveBeenCalled();
  });

  it('requests invite/details with the page selected from the old table pagination', async () => {
    mocks.detailRows = [{ created_at: 1_700_000_600, get_amount: 1234 }];
    mocks.detailsTotal = 25;

    await renderInvite();

    const pageTwo = container.querySelector<HTMLButtonElement>(
      '[data-testid="invite-page"][data-page="2"]',
    )!;

    await act(async () => {
      pageTwo.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(pageTwo.textContent).toBe('2');
    expect(
      container
        .querySelector<HTMLButtonElement>('[data-testid="invite-page"][data-page="2"]')
        ?.getAttribute('aria-current'),
    ).toBe('page');
    expect(mocks.detailQueryCalls.at(-1)).toEqual({ current: 2, pageSize: 10 });
  });

  it('clamps the visible commission-history page like the legacy pagination helper', async () => {
    mocks.detailRows = [{ created_at: 1_700_000_600, get_amount: 1234 }];
    mocks.detailsTotal = 45;

    await renderInvite();

    const pageFour = container.querySelector<HTMLButtonElement>(
      '[data-testid="invite-page"][data-page="4"]',
    )!;

    await act(async () => {
      pageFour.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(
      container
        .querySelector<HTMLButtonElement>('[data-testid="invite-page"][data-page="4"]')
        ?.getAttribute('aria-current'),
    ).toBe('page');
    expect(mocks.detailQueryCalls.at(-1)).toEqual({ current: 4, pageSize: 10 });

    mocks.detailsTotal = 25;

    await renderInvite();

    expect(
      container
        .querySelector<HTMLButtonElement>('[data-testid="invite-page"][data-page="3"]')
        ?.getAttribute('aria-current'),
    ).toBe('page');
    expect(
      container
        .querySelector<HTMLButtonElement>('[data-testid="invite-page"][data-page="4"]')
        ?.getAttribute('aria-current'),
    ).toBeUndefined();
  });
});
