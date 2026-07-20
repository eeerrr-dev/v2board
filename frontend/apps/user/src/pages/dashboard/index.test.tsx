// @vitest-environment jsdom
import type { ComponentProps, ReactNode } from 'react';
import { screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import {
  createTestTranslation,
  type TranslationInput,
  type TranslationValues,
} from '@/test/i18next-selector';
import { setRuntimeConfig } from '@/test/runtime-config';
import DashboardPage from './index';

const mocks = vi.hoisted(() => ({
  copyText: vi.fn(),
  labels: {
    'common.cancel': '取消',
    'common.confirm': '确定',
    'common.error_title': '加载失败',
    'common.retry': '重试',
    'dashboard.alert_open_ticket': '<strong>{{count}}</strong> 条工单正在处理中',
    'dashboard.alert_pending_order': '还有没支付的订单',
    'dashboard.alert_traffic_rate': '当前已使用流量达 {rate}%',
    'dashboard.alert_view': '立即查看',
    'dashboard.buy_reset_package': '购买流量重置包',
    'dashboard.buy_subscribe': '购买订阅',
    'dashboard.copy_subscribe': '复制订阅地址',
    'dashboard.copy_success': '复制成功',
    'dashboard.devices_online': '在线设备 {alive_ip}/{device_limit}',
    'dashboard.active': '生效中',
    'dashboard.expired_label': '已过期',
    'dashboard.expires_in': '于 {date} 到期，距离到期还有 {count} 天。',
    'dashboard.import_to': '导入到',
    'dashboard.long_term': '该订阅长期有效',
    'dashboard.new_period': '提前开启流量周期',
    'dashboard.new_period_confirm_content':
      '点击「确定」将会扣除当前流量周期剩余订阅时长（按月重置时扣除本周期剩余订阅时长，每月1号重置时扣除整月时间30天，年周期同理），系统将会重置您的已使用流量。',
    'dashboard.new_period_confirm_title': '确定开启下一个流量周期？',
    'dashboard.new_period_success': '提前开启流量周期成功',
    'dashboard.plan': '我的订阅',
    'dashboard.qrcode_client_tip': '使用支持扫码的客户端进行订阅',
    'dashboard.renew_subscribe': '续费订阅',
    'dashboard.reset_in_days': '已用流量将在 {count} 日后重置',
    'dashboard.reset_package_confirm_content':
      '点击「确定」将会跳转到收银台，支付订单后系统将会清空您当月已使用流量。',
    'dashboard.reset_package_confirm_title': '确定重置当前已用流量？',
    'dashboard.reset_today': '已用流量将在今日重置',
    'dashboard.scan_qrcode_subscribe': '扫描二维码订阅',
    'dashboard.shortcut_buy': '购买订阅',
    'dashboard.shortcut_buy_desc': '对您当前的订阅进行购买',
    'dashboard.shortcut_one_click': '一键订阅',
    'dashboard.shortcut_one_click_desc': '快速将节点导入对应客户端进行使用',
    'dashboard.shortcut_problem': '遇到问题',
    'dashboard.shortcut_problem_desc': '遇到问题可以通过工单与我们沟通',
    'dashboard.shortcut_renew_desc': '对您当前的订阅进行续费',
    'dashboard.shortcut_tutorial': '查看教程',
    'dashboard.shortcut_tutorial_desc': '学习如何使用',
    'dashboard.shortcuts': '捷径',
    'dashboard.use_tutorial': '不会使用，查看使用教程',
    'dashboard.used_traffic': '已用 {used} / 总计 {total}',
    'notice.title': '公告',
    'order.pay_now': '立即支付',
  } as Record<string, string>,
  navigate: vi.fn(),
  newPeriodMutateAsync: vi.fn(),
  commIsError: false,
  noticesIsError: false,
  notices: [] as Array<{
    content?: string;
    /** RFC 3339, as delivered by GET /user/notices (docs/api-dialect.md §4.5). */
    created_at: string;
    id: number;
    img_url?: string;
    tags?: string[] | null;
    title: string;
  }>,
  refetchSubscribe: vi.fn(),
  refetchComm: vi.fn(),
  refetchNotices: vi.fn(),
  refetchStat: vi.fn(),
  saveOrder: vi.fn(),
  stat: {
    pending_order_count: 0,
    pending_ticket_count: 0,
    invited_user_count: 0,
  },
  statIsError: false,
  subscribe: {} as Record<string, unknown> | undefined,
  subscribeIsError: false,
  subscribeIsLoading: false,
  toastSuccess: vi.fn(),
}));

// Embla drives the notice carousel via live layout, which jsdom cannot measure,
// so replace it with a deterministic api: scrollTo(i) selects slide i and fires
// 'select'. Real drag/keyboard/embla behavior is covered by the browser
// interaction-parity scenario user-dashboard-notice-carousel.
const embla = vi.hoisted(() => ({
  index: 0,
  listeners: {} as Record<string, Array<() => void>>,
}));

vi.mock('embla-carousel-react', () => {
  const api = {
    scrollSnapList: () => mocks.notices.map((_, index) => index),
    selectedScrollSnap: () => embla.index,
    slidesInView: () => [embla.index],
    scrollTo: (index: number) => {
      embla.index = index;
      (embla.listeners.select ?? []).forEach((cb) => cb());
    },
    scrollPrev: () => api.scrollTo(Math.max(0, embla.index - 1)),
    scrollNext: () => api.scrollTo(embla.index + 1),
    canScrollPrev: () => embla.index > 0,
    canScrollNext: () => true,
    on: (event: string, cb: () => void) => {
      (embla.listeners[event] ??= []).push(cb);
      return api;
    },
    off: (event: string, cb: () => void) => {
      embla.listeners[event] = (embla.listeners[event] ?? []).filter((listener) => listener !== cb);
      return api;
    },
  };
  return { default: () => [() => {}, api] as const };
});

vi.mock('react-router', () => ({
  Link: ({
    to,
    onClick,
    children,
    ...rest
  }: { to: string } & Omit<ComponentProps<'a'>, 'href'>) => (
    <a
      href={to}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) {
          event.preventDefault();
          mocks.navigate(to);
        }
      }}
      {...rest}
    >
      {children}
    </a>
  ),
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation(mocks.labels),
  // Trans resolves like t() with markup tags stripped: the bolded count is
  // presentation, and the assertions read textContent either way.
  Trans: ({
    i18nKey,
    values,
    count,
  }: {
    i18nKey: TranslationInput;
    values?: TranslationValues;
    count?: number;
  }) => (
    <>
      {createTestTranslation(mocks.labels)
        .t(i18nKey, { ...values, ...(count === undefined ? {} : { count }) })
        .replace(/<[^>]+>/g, '')}
    </>
  ),
}));

vi.mock('@v2board/ui/dialog', () => ({
  Dialog: ({
    children,
    open,
  }: {
    children: ReactNode;
    onOpenChange?: (open: boolean) => void;
    open?: boolean;
  }) => (open ? <div data-dialog="open">{children}</div> : null),
  DialogContent: ({
    children,
    className,
    ...props
  }: {
    children: ReactNode;
    className?: string;
    [key: string]: unknown;
  }) => (
    <div className={className} data-dialog-content="open" {...props}>
      {children}
    </div>
  ),
  DialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock('@v2board/ui/alert-dialog', () => ({
  AlertDialogAction: ({ children }: { children: ReactNode }) => <>{children}</>,
  AlertDialogCancel: ({ children }: { children: ReactNode }) => <>{children}</>,
  AlertDialog: ({
    children,
    open,
  }: {
    children: ReactNode;
    onOpenChange?: (open: boolean) => void;
    open?: boolean;
  }) => (open ? <div data-dialog="open">{children}</div> : null),
  AlertDialogContent: ({
    children,
    className,
    ...props
  }: {
    children: ReactNode;
    className?: string;
    [key: string]: unknown;
  }) => (
    <div className={className} data-dialog-content="open" {...props}>
      {children}
    </div>
  ),
  AlertDialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  AlertDialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  AlertDialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  AlertDialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock('@/lib/queries', () => ({
  useCommConfig: () => ({
    isError: mocks.commIsError,
    refetch: mocks.refetchComm,
  }),
  useNewPeriodMutation: () => ({
    isPending: false,
    mutate: (_payload: undefined, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.newPeriodMutateAsync()).then(options?.onSuccess);
    },
  }),
  useSaveOrderMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveOrder(payload)).then(options?.onSuccess);
    },
  }),
  useNotices: () => ({
    data: mocks.notices,
    isError: mocks.noticesIsError,
    refetch: mocks.refetchNotices,
  }),
  useSubscribe: () => ({
    data: mocks.subscribe,
    isError: mocks.subscribeIsError,
    isLoading: mocks.subscribeIsLoading,
    refetch: mocks.refetchSubscribe,
  }),
  useUserStat: () => ({
    data: mocks.stat,
    isError: mocks.statIsError,
    refetch: mocks.refetchStat,
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

vi.mock('qrcode.react', () => ({
  QRCodeSVG: ({ value }: { value?: string }) => (
    <svg data-testid="qrcode-svg" data-qrcode={value} />
  ),
}));

function baseSubscribe(overrides: Record<string, unknown> = {}) {
  return {
    alive_ip: 2,
    allow_new_period: false,
    d: 0,
    device_limit: null,
    email: 'user@example.com',
    expired_at: '2100-01-01T12:00:00Z',
    plan: { name: 'Pro', renew: true, reset_price: 100, show: true },
    plan_id: 1,
    reset_day: 5,
    subscribe_url: 'https://example.test/sub',
    transfer_enable: 1000,
    u: 850,
    ...overrides,
  };
}

function resetMocks() {
  mocks.copyText.mockReset();
  mocks.copyText.mockResolvedValue(true);
  mocks.navigate.mockReset();
  mocks.newPeriodMutateAsync.mockReset();
  mocks.newPeriodMutateAsync.mockResolvedValue(true);
  mocks.commIsError = false;
  mocks.notices = [];
  mocks.noticesIsError = false;
  mocks.refetchComm.mockReset();
  mocks.refetchNotices.mockReset();
  mocks.refetchStat.mockReset();
  mocks.refetchSubscribe.mockReset();
  mocks.refetchSubscribe.mockResolvedValue({});
  mocks.saveOrder.mockReset();
  mocks.saveOrder.mockResolvedValue('ORDER123');
  mocks.stat = {
    pending_order_count: 0,
    pending_ticket_count: 0,
    invited_user_count: 0,
  };
  mocks.statIsError = false;
  mocks.subscribe = baseSubscribe();
  mocks.subscribeIsError = false;
  mocks.subscribeIsLoading = false;
  mocks.toastSuccess.mockReset();
  embla.index = 0;
  embla.listeners = {};
  setRuntimeConfig({
    title: 'V2Board',
  });
}

describe('DashboardPage shadcn shell rendering', () => {
  beforeEach(resetMocks);

  it('renders alerts, notice, subscription card, and shortcuts', () => {
    mocks.stat = { pending_order_count: 2, pending_ticket_count: 3, invited_user_count: 0 };
    mocks.notices = [
      {
        content: '<p>Notice body</p>',
        created_at: '2023-11-14T22:13:20Z',
        id: 1,
        img_url: '/notice.jpg',
        tags: ['弹窗'],
        title: 'Notice A',
      },
    ];

    renderWithProviders(<DashboardPage />);

    expect(screen.getByTestId('dashboard-page')).toBeInTheDocument();

    const alerts = screen.getAllByRole('alert');
    const danger = alerts.find((alert) => alert.getAttribute('data-alert-kind') === 'danger')!;
    expect(danger).toHaveTextContent('还有没支付的订单');
    expect(within(danger).getByRole('link', { name: '立即支付' })).toHaveAttribute(
      'href',
      '/order',
    );

    const warning = alerts.find((alert) => alert.getAttribute('data-alert-kind') === 'warning')!;
    expect(warning).toHaveTextContent('3 条工单正在处理中');

    const info = alerts.find((alert) => alert.getAttribute('data-alert-kind') === 'info')!;
    expect(info).toHaveTextContent('当前已使用流量达 85%');

    expect(screen.getByTestId('dashboard-notice-card')).toHaveTextContent('Notice A');
    expect(screen.getByTestId('dashboard-notice-image')).toHaveAttribute('loading', 'eager');
    expect(screen.getByTestId('dashboard-notice-image')).toHaveAttribute('decoding', 'async');
    expect(screen.getByTestId('dashboard-notice-image')).toHaveAttribute('fetchpriority', 'high');
    // The backend `弹窗` tag auto-opens the notice dialog with its body.
    expect(screen.getByText('Notice body')).toBeInTheDocument();

    expect(screen.getByText('我的订阅')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Pro' })).toBeInTheDocument();
    expect(
      screen.getByRole('progressbar', { name: '已用 850.00 B / 总计 1000.00 B' }),
    ).toHaveAttribute('aria-valuenow', '85');
    expect(screen.getByTestId('dashboard-progress-bar')).toBeInTheDocument();
    expect(screen.getAllByText('在线设备 2/∞').length).toBeGreaterThan(0);

    expect(screen.getByText('捷径')).toBeInTheDocument();
    const shortcuts = screen.getAllByTestId('dashboard-shortcut');
    expect(shortcuts).toHaveLength(4);
    const tutorial = shortcuts.find((shortcut) => shortcut.textContent?.includes('查看教程'))!;
    expect(tutorial).toHaveTextContent('学习如何使用 V2Board');
    expect(screen.getByRole('button', { name: /一键订阅/ })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /续费订阅/ })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /遇到问题/ })).toBeInTheDocument();
  });

  it('renders the buy-subscribe empty state and routes it to the plan list', async () => {
    mocks.subscribe = baseSubscribe({ plan: null, plan_id: null });

    const { user } = renderWithProviders(<DashboardPage />);

    const emptyPlan = screen.getByTestId('dashboard-empty-plan');
    expect(emptyPlan).toHaveTextContent('购买订阅');

    await user.click(emptyPlan);
    expect(mocks.navigate).toHaveBeenCalledWith('/plan');
  });

  it('shows a retryable error state when the subscription fetch fails', async () => {
    mocks.subscribe = {};
    mocks.subscribeIsError = true;

    const { user } = renderWithProviders(<DashboardPage />);

    // A failed fetch surfaces the error, not the perpetual spinner or the
    // buy-subscribe empty state.
    expect(screen.getByTestId('dashboard-plan-error')).toBeInTheDocument();
    expect(screen.queryByTestId('dashboard-empty-plan')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Pro' })).not.toBeInTheDocument();

    await user.click(screen.getByTestId('error-state-retry'));
    expect(mocks.refetchSubscribe).toHaveBeenCalled();
  });

  it('surfaces auxiliary dashboard failures and retries every failed query', async () => {
    mocks.statIsError = true;
    mocks.noticesIsError = true;
    mocks.commIsError = true;

    const { user } = renderWithProviders(<DashboardPage />);

    const error = screen.getByTestId('dashboard-data-error');
    expect(error).toHaveTextContent('加载失败');
    await user.click(within(error).getByTestId('error-state-retry'));

    expect(mocks.refetchStat).toHaveBeenCalledTimes(1);
    expect(mocks.refetchNotices).toHaveBeenCalledTimes(1);
    expect(mocks.refetchComm).toHaveBeenCalledTimes(1);
  });

  it('renders a loading state without plan content while subscription data is incomplete', () => {
    mocks.subscribe = {};

    renderWithProviders(<DashboardPage />);

    expect(screen.getByText('我的订阅')).toBeInTheDocument();
    expect(screen.queryByTestId('dashboard-empty-plan')).not.toBeInTheDocument();
    expect(screen.queryByTestId('dashboard-progress-bar')).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Pro' })).not.toBeInTheDocument();
  });

  it('keeps dashboard dates behind the backend timestamp formatter output', () => {
    renderWithProviders(<DashboardPage />);

    expect(screen.getByText(/2100\/01\/01/)).toBeInTheDocument();
    expect(screen.getByText(/已用流量将在 5 日后重置/)).toBeInTheDocument();
  });
});

describe('DashboardPage shadcn shell actions', () => {
  beforeEach(resetMocks);

  it('opens one-click subscribe, copies the URL, and opens the QR dialog', async () => {
    const { user } = renderWithProviders(<DashboardPage />);

    await user.click(screen.getByRole('button', { name: /一键订阅/ }));

    const menu = screen.getByTestId('dashboard-subscribe-menu');
    expect(screen.getByTestId('dashboard-subscribe-copy')).toHaveTextContent('复制订阅地址');
    const hiddify = within(menu).getByRole('button', { name: /导入到 Hiddify/ });
    const targetButtons = within(menu).getAllByTestId('dashboard-subscribe-target');
    const targetIcons = targetButtons.map((button) => button.querySelector('svg'));
    const hiddifyIcon = hiddify.querySelector('svg');
    expect(hiddifyIcon).toHaveClass('size-5');
    expect(hiddifyIcon).toHaveAttribute('aria-hidden', 'true');
    expect(targetIcons).toHaveLength(targetButtons.length);
    for (const icon of targetIcons) expect(icon).toHaveClass('lucide-import');
    for (const button of targetButtons) expect(button.querySelector('img')).not.toBeInTheDocument();

    await user.click(screen.getByTestId('dashboard-subscribe-copy'));
    expect(mocks.copyText).toHaveBeenCalledWith('https://example.test/sub');
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功'));

    await user.click(screen.getByTestId('dashboard-subscribe-qrcode'));
    expect(screen.getByTestId('dashboard-subscribe-qrcode-image')).toBeInTheDocument();
    expect(screen.getByTestId('qrcode-svg')).toHaveAttribute(
      'data-qrcode',
      'https://example.test/sub',
    );
    expect(screen.getByText('使用支持扫码的客户端进行订阅')).toBeInTheDocument();
  });

  it('switches notice dots and opens the active notice dialog', async () => {
    mocks.notices = [
      {
        content: '<p>First notice</p>',
        created_at: '2023-11-14T22:13:20Z',
        id: 1,
        img_url: '/notice-a.jpg',
        title: 'Notice A',
      },
      {
        content: '<p>Second notice</p>',
        created_at: '2023-11-16T02:00:00Z',
        id: 2,
        img_url: '/notice-b.jpg',
        title: 'Notice B',
      },
    ];
    const { user } = renderWithProviders(<DashboardPage />);

    // Embla mounts every slide; the active one is flagged with data-active.
    expect(screen.getByText('Notice A')).toBeInTheDocument();
    expect(screen.getByText('Notice B')).toBeInTheDocument();
    const slides = screen.getAllByTestId('dashboard-notice-slide');
    expect(slides[0]).toHaveAttribute('data-active', 'true');
    expect(slides[1]).toHaveAttribute('data-active', 'false');
    expect(within(slides[0]!).getByTestId('dashboard-notice-image')).toHaveAttribute(
      'loading',
      'eager',
    );
    expect(within(slides[0]!).getByTestId('dashboard-notice-image')).toHaveAttribute(
      'decoding',
      'async',
    );
    expect(within(slides[1]!).getByTestId('dashboard-notice-image')).toHaveAttribute(
      'loading',
      'lazy',
    );
    expect(within(slides[1]!).getByTestId('dashboard-notice-image')).toHaveAttribute(
      'decoding',
      'async',
    );

    // Clicking the second dot scrolls embla and moves the active flag onto slide 2.
    await user.click(screen.getByRole('button', { name: '公告 2' }));

    const activeSlide = screen
      .getAllByTestId('dashboard-notice-slide')
      .find((slide) => slide.getAttribute('data-active') === 'true')!;
    expect(activeSlide).toHaveTextContent('Notice B');
    const activeDot = screen
      .getAllByTestId('dashboard-notice-dot')
      .find((dot) => dot.getAttribute('data-active') === 'true')!;
    expect(activeDot).toHaveAttribute('aria-current', 'true');
    expect(activeDot).toHaveAttribute('aria-label', '公告 2');

    // The active slide's card opens its own notice dialog.
    await user.click(within(activeSlide).getByTestId('dashboard-notice-card'));
    expect(screen.getByText('Second notice')).toBeInTheDocument();
  });

  it('confirms reset package purchase and navigates to the generated order', async () => {
    const { user } = renderWithProviders(<DashboardPage />);

    // The label appears both in the traffic alert and on the plan card; either
    // opens the same confirm dialog.
    await user.click(screen.getAllByRole('button', { name: '购买流量重置包' })[0]!);
    expect(screen.getByText('确定重置当前已用流量？')).toBeInTheDocument();

    await user.click(screen.getByTestId('dashboard-confirm-primary'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123'));
    expect(mocks.saveOrder).toHaveBeenCalledWith({
      kind: 'plan',
      period: 'reset_price',
      plan_id: 1,
    });
  });

  it('confirms opening a new traffic period and refetches subscription data', async () => {
    mocks.subscribe = baseSubscribe({ allow_new_period: true, d: 1000, u: 0 });
    const { user } = renderWithProviders(<DashboardPage />);

    await user.click(screen.getByRole('button', { name: '提前开启流量周期' }));
    expect(screen.getByText('确定开启下一个流量周期？')).toBeInTheDocument();

    await user.click(screen.getByTestId('dashboard-confirm-primary'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
    expect(mocks.newPeriodMutateAsync).toHaveBeenCalled();
    expect(mocks.refetchSubscribe).toHaveBeenCalled();
    expect(mocks.toastSuccess).toHaveBeenCalledWith('提前开启流量周期成功');
  });

  it('routes alert and shortcut actions through React Router', async () => {
    mocks.stat = { pending_order_count: 1, pending_ticket_count: 1, invited_user_count: 0 };
    const { user } = renderWithProviders(<DashboardPage />);

    await user.click(screen.getByRole('link', { name: '立即支付' }));
    await user.click(screen.getByRole('link', { name: '立即查看' }));
    await user.click(screen.getByRole('link', { name: /查看教程/ }));

    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    expect(mocks.navigate).toHaveBeenCalledWith('/ticket');
    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
  });
});
