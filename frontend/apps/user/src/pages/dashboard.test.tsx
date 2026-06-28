import { act } from 'react';
import type { ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DashboardPage from './dashboard';

const mocks = vi.hoisted(() => ({
  copyText: vi.fn(),
  labels: {
    'common.cancel': '取消',
    'common.confirm': '确定',
    'dashboard.alert_open_ticket_suffix': '条工单正在处理中',
    'dashboard.alert_pending_order': '还有没支付的订单',
    'dashboard.alert_traffic_rate': '当前已使用流量达 {rate}%',
    'dashboard.alert_view': '立即查看',
    'dashboard.buy_reset_package': '购买流量重置包',
    'dashboard.buy_subscribe': '购买订阅',
    'dashboard.copy_subscribe': '复制订阅地址',
    'dashboard.copy_success': '复制成功',
    'dashboard.devices_online': '在线设备 {alive_ip}/{device_limit}',
    'dashboard.expired_label': '已过期',
    'dashboard.expires_in': '于 {date} 到期，距离到期还有 {day} 天。',
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
    'dashboard.reset_in_days': '已用流量将在 {reset_day} 日后重置',
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
  notices: [] as Array<{
    content?: string;
    created_at: number;
    id: number;
    img_url?: string;
    tags?: string | null;
    title: string;
  }>,
  refetchSubscribe: vi.fn(),
  saveOrder: vi.fn(),
  stat: {
    pending_orders: 0,
    pending_tickets: 0,
  },
  subscribe: {} as Record<string, unknown> | undefined,
  subscribeIsLoading: false,
  toastSuccess: vi.fn(),
}));

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string, values?: Record<string, unknown>) => {
      let label = mocks.labels[key] ?? key;
      Object.entries(values ?? {}).forEach(([name, value]) => {
        label = label.replaceAll(`{{${name}}}`, String(value));
        label = label.replaceAll(`{${name}}`, String(value));
      });
      return label;
    },
  }),
}));

vi.mock('@/components/ui/shadcn-dialog', () => ({
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

vi.mock('@/lib/queries', () => ({
  useCommConfig: () => ({}),
  useNewPeriodMutation: () => ({
    isPending: false,
    mutateAsync: mocks.newPeriodMutateAsync,
  }),
  useSaveOrderMutation: () => ({
    isPending: false,
    mutateAsync: mocks.saveOrder,
  }),
  useNotices: () => ({
    data: mocks.notices,
  }),
  useSubscribe: () => ({
    data: mocks.subscribe,
    isLoading: mocks.subscribeIsLoading,
    refetch: mocks.refetchSubscribe,
  }),
  useUserStat: () => ({
    data: mocks.stat,
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

vi.mock('qrcode.react', () => ({
  QRCodeCanvas: ({ value }: { value?: string }) => <canvas data-qrcode={value} />,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function baseSubscribe(overrides: Record<string, unknown> = {}) {
  return {
    alive_ip: 2,
    allow_new_period: false,
    d: 0,
    device_limit: null,
    email: 'user@example.com',
    expired_at: 4_102_488_000,
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
  mocks.notices = [];
  mocks.refetchSubscribe.mockReset();
  mocks.refetchSubscribe.mockResolvedValue({});
  mocks.saveOrder.mockReset();
  mocks.saveOrder.mockResolvedValue('ORDER123');
  mocks.stat = {
    pending_orders: 0,
    pending_tickets: 0,
  };
  mocks.subscribe = baseSubscribe();
  mocks.subscribeIsLoading = false;
  mocks.toastSuccess.mockReset();
  window.settings = {
    title: 'V2Board',
  };
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('DashboardPage shadcn shell markup', () => {
  beforeEach(resetMocks);

  it('renders alerts, notice, subscription card, and shortcuts with redesigned structure', () => {
    mocks.stat = { pending_orders: 2, pending_tickets: 3 };
    mocks.notices = [
      {
        content: '<p>Notice body</p>',
        created_at: 1_700_000_000,
        id: 1,
        img_url: '/notice.jpg',
        tags: '弹窗',
        title: 'Notice A',
      },
    ];

    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('data-testid="dashboard-page"');
    expect(html).toContain('data-alert-kind="danger"');
    expect(html).toContain('还有没支付的订单');
    expect(html).toContain('立即支付');
    expect(html).toContain('data-alert-kind="warning"');
    expect(html).toContain('<strong>3</strong>');
    expect(html).toContain('条工单正在处理中');
    expect(html).toContain('当前已使用流量达 85%');
    expect(html).toContain('data-testid="dashboard-notice-card"');
    expect(html).toContain('Notice A');
    expect(html).toContain('我的订阅');
    expect(html).toContain('Pro');
    expect(html).toContain('data-testid="dashboard-progress-bar"');
    expect(html).toContain('在线设备 2/∞');
    expect(html).toContain('捷径');
    expect(html).toContain('查看教程');
    expect(html).toContain('学习如何使用 V2Board');
    expect(html).toContain('一键订阅');
    expect(html).toContain('续费订阅');
    expect(html).toContain('遇到问题');
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('oneClickSubscribe___2t9Xg');
    expect(html).not.toContain('slick-slider');
    expect(html).not.toContain('ant-btn-primary');
  });

  it('renders the buy-subscribe empty state when there is no active plan', () => {
    mocks.subscribe = baseSubscribe({ plan: null, plan_id: null });

    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('data-testid="dashboard-empty-plan"');
    expect(html).toContain('购买订阅');
    expect(html).not.toContain('fa fa-plus');
    expect(html).not.toContain('font-size-sm text-uppercase text-muted');
  });

  it('renders a shadcn loading state while subscription data is incomplete', () => {
    mocks.subscribe = {};

    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('animate-spin');
    expect(html).not.toContain('anticon-loading');
  });

  it('keeps dashboard dates behind the user legacy date formatter output', () => {
    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('2100/01/01');
    expect(html).toContain('已用流量将在 5 日后重置');
  });
});

describe('DashboardPage shadcn shell actions', () => {
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

  async function renderDashboard() {
    await act(async () => {
      root.render(<DashboardPage />);
      await Promise.resolve();
    });
  }

  it('opens one-click subscribe, copies the URL, and opens the QR dialog', async () => {
    await renderDashboard();

    const shortcut = Array.from(container.querySelectorAll('[data-testid="dashboard-shortcut"]')).find(
      (item) => item.textContent?.includes('一键订阅'),
    )!;

    await act(async () => {
      shortcut.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('data-testid="dashboard-subscribe-menu"');
    expect(container.innerHTML).toContain('data-testid="dashboard-subscribe-copy"');
    expect(container.innerHTML).toContain('复制订阅地址');
    expect(container.innerHTML).toContain('导入到 Hiddify');
    expect(container.querySelector('img[src*="Hiddify"]')?.getAttribute('src')).toContain(
      'Hiddify.png',
    );
    expect(container.innerHTML).not.toContain('/theme/default/assets/');

    const copy = Array.from(
      container.querySelectorAll('[data-testid^="dashboard-subscribe-"]'),
    ).find(
      (item) => item.textContent === '复制订阅地址',
    )!;
    await act(async () => {
      copy.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.copyText).toHaveBeenCalledWith('https://example.test/sub');
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');

    const qr = Array.from(
      container.querySelectorAll('[data-testid^="dashboard-subscribe-"]'),
    ).find(
      (item) => item.textContent === '扫描二维码订阅',
    )!;
    await act(async () => {
      qr.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.querySelector('canvas')?.getAttribute('data-qrcode')).toBe(
      'https://example.test/sub',
    );
    expect(container.innerHTML).toContain('使用支持扫码的客户端进行订阅');
  });

  it('switches notice dots and opens the notice dialog', async () => {
    mocks.notices = [
      {
        content: '<p>First notice</p>',
        created_at: 1_700_000_000,
        id: 1,
        title: 'Notice A',
      },
      {
        content: '<p>Second notice</p>',
        created_at: 1_700_100_000,
        id: 2,
        title: 'Notice B',
      },
    ];
    await renderDashboard();

    const secondDot = container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="dashboard-notice-dots"] button',
    )[1]!;
    await act(async () => {
      secondDot.click();
      await Promise.resolve();
    });

    expect(
      container.querySelector('[data-testid="dashboard-notice-slide"][data-active="true"]')
        ?.textContent,
    ).toContain('Notice B');

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="dashboard-notice-card"]')!.click();
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('Second notice');
  });

  it('confirms reset package purchase and navigates to the generated order', async () => {
    await renderDashboard();

    const reset = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '购买流量重置包',
    )!;
    await act(async () => {
      reset.click();
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('确定重置当前已用流量？');
    const confirm = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '确定',
    )!;
    await act(async () => {
      confirm.click();
    });
    await flushPromises();

    expect(mocks.saveOrder).toHaveBeenCalledWith({ period: 'reset_price', plan_id: 1 });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');
  });

  it('confirms opening a new traffic period and refetches subscription data', async () => {
    mocks.subscribe = baseSubscribe({ allow_new_period: true, d: 1000, u: 0 });
    await renderDashboard();

    const newPeriod = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '提前开启流量周期',
    )!;
    await act(async () => {
      newPeriod.click();
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('确定开启下一个流量周期？');
    const confirm = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '确定',
    )!;
    await act(async () => {
      confirm.click();
    });
    await flushPromises();

    expect(mocks.newPeriodMutateAsync).toHaveBeenCalled();
    expect(mocks.refetchSubscribe).toHaveBeenCalled();
    expect(mocks.toastSuccess).toHaveBeenCalledWith('提前开启流量周期成功');
    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
  });

  it('routes alert and shortcut actions through React Router', async () => {
    mocks.stat = { pending_orders: 1, pending_tickets: 1 };
    await renderDashboard();

    const payNow = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '立即支付',
    )!;
    const ticket = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '立即查看',
    )!;
    const tutorial = Array.from(
      container.querySelectorAll<HTMLButtonElement>('[data-testid="dashboard-shortcut"]'),
    ).find(
      (button) => button.textContent?.includes('查看教程'),
    )!;

    await act(async () => {
      payNow.click();
      ticket.click();
      tutorial.click();
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    expect(mocks.navigate).toHaveBeenCalledWith('/ticket');
    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
  });
});
