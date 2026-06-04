import { act } from 'react';
import type { ReactNode } from 'react';
import { readFileSync } from 'node:fs';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DashboardPage from './dashboard';

const mocks = vi.hoisted(() => ({
  copyText: vi.fn(),
  isMobile: false,
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
    'dashboard.expires_in': '于 {date} 到期，距离到期还有 {day} 天。',
    'dashboard.import_to': '导入到',
    'dashboard.long_term': '该订阅长期有效',
    'dashboard.new_period': '提前开启流量周期',
    'dashboard.new_period_confirm_content':
      '点击「确定」将会扣除当前流量周期剩余订阅时长（按月重置时扣除本周期剩余订阅时长，每月1号重置时扣除整月时间30天，年周期同理），系统将会重置您的已使用流量。',
    'dashboard.new_period_confirm_title': '确定开启下一个流量周期？',
    'dashboard.plan': '我的订阅',
    'dashboard.qrcode_client_tip': '使用支持扫码的客户端进行订阅',
    'dashboard.renew_subscribe': '续费订阅',
    'dashboard.reset_in_days': '已用流量将在 {reset_day} 日后重置',
    'dashboard.reset_package_confirm_content':
      '点击「确定」将会跳转到收银台，支付订单后系统将会清空您当月已使用流量。',
    'dashboard.reset_package_confirm_title': '确定重置当前已用流量？',
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
  legacyConfirm: vi.fn(),
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

vi.mock('react-router-dom', () => ({
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

vi.mock('@/components/ui/dialog', () => ({
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
    title,
  }: {
    bodyStyle?: Record<string, unknown>;
    centered?: boolean;
    children: ReactNode;
    closable?: boolean;
    footer?: ReactNode;
    style?: Record<string, unknown>;
    title?: ReactNode;
    width?: number;
    zIndex?: number;
  }) => (
    <div data-dialog-content="open">
      {title}
      {children}
    </div>
  ),
}));

vi.mock('@/components/legacy-confirm', () => ({
  legacyConfirm: mocks.legacyConfirm,
}));

vi.mock('@/lib/queries', () => ({
  useCommConfig: () => ({}),
  useNewPeriodMutation: () => ({
    mutateAsync: mocks.newPeriodMutateAsync,
  }),
  useNotices: () => ({
    data: { data: mocks.notices },
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
  isLegacyMobile: () => mocks.isMobile,
  legacyCopyText: mocks.copyText,
}));

vi.mock('@/lib/api', () => ({
  apiClient: { name: 'apiClient' },
}));

vi.mock('@/lib/legacy-toast', () => ({
  toast: {
    success: mocks.toastSuccess,
  },
}));

vi.mock('@/lib/use-transition-status', () => ({
  useTransitionStatus: (open: boolean) => (open ? 'entered' : 'exited'),
}));

vi.mock('@/lib/legacy-body-scroll', () => ({
  lockLegacyDrawerBodyScroll: () => vi.fn(),
}));

vi.mock('@v2board/api-client', () => ({
  user: {
    saveOrder: mocks.saveOrder,
  },
}));

vi.mock('qrcode.react', () => ({
  default: ({ value }: { value?: string }) => <canvas data-qrcode={value} />,
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
  mocks.isMobile = false;
  mocks.legacyConfirm.mockReset();
  mocks.navigate.mockReset();
  mocks.newPeriodMutateAsync.mockReset();
  mocks.newPeriodMutateAsync.mockResolvedValue(true);
  mocks.notices = [];
  mocks.refetchSubscribe.mockReset();
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
    assets_path: '/theme/default/assets',
    title: 'V2Board',
  };
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('DashboardPage bundled-theme markup', () => {
  beforeEach(resetMocks);

  it('renders the original alerts, notice card, subscription card, and shortcuts', () => {
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

    expect(html).toContain('class="alert alert-danger"');
    expect(html).toContain('还有没支付的订单');
    expect(html).toContain('立即支付');
    expect(html).toContain('class="alert alert-warning"');
    expect(html).toContain('<strong>3</strong>');
    expect(html).toContain('条工单正在处理中');
    expect(html).toContain('立即查看');
    expect(html).toContain('class="alert alert-info"');
    expect(html).toContain('当前已使用流量达 85%');
    expect(html).toContain('购买流量重置包');
    expect(html).toContain('block block-rounded bg-image mb-0 v2board-bg-pixels');
    expect(html).toContain('background-image:url(/notice.jpg);background-size:cover');
    expect(html).toContain('badge badge-danger p-2 text-uppercase');
    expect(html).toContain('公告');
    expect(html).toContain('Notice A');
    expect(html).toContain('2023-11-14');
    expect(html).toContain('我的订阅');
    expect(html).toContain('Pro');
    expect(html).toContain('于 2100/01/01 到期，距离到期还有');
    expect(html).toContain('已用流量将在 5 日后重置');
    expect(html).toContain('progress-bar progress-bar-striped progress-bar-animated bg-warning');
    expect(html).toContain('在线设备 2/∞');
    expect(html).toContain('捷径');
    expect(html).toContain('查看教程');
    expect(html).toContain('学习如何使用 V2Board');
    expect(html).toContain('一键订阅');
    expect(html).toContain('续费订阅');
    expect(html).toContain('遇到问题');
  });

  it('renders the original buy-subscribe empty state when there is no active plan', () => {
    mocks.subscribe = baseSubscribe({ plan: null, plan_id: null });

    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('fa fa-plus fa-2x');
    expect(html).toContain('font-size-sm text-uppercase text-muted pt-2 pb-3');
    expect(html).toContain('购买订阅');
  });

  it('keeps the bundled-theme subscription branch keyed only by plan_id', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/dashboard.tsx`, 'utf8');

    expect(source).toContain(') : hasPlan ? (');
    expect(source).not.toContain(') : hasPlan && sub?.plan ? (');
  });

  it('renders the original loading icon when subscribe has not loaded an email', () => {
    mocks.subscribe = {};

    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('class="font-size-h3 mb-3"');
    expect(html).toContain('anticon-loading');
  });
});

describe('DashboardPage bundled-theme actions', () => {
  let container: HTMLDivElement;
  let root: Root;
  let consoleLogSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    resetMocks();
    consoleLogSpy = vi.spyOn(console, 'log').mockImplementation(() => undefined);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    consoleLogSpy.mockRestore();
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderDashboard() {
    await act(async () => {
      root.render(<DashboardPage />);
      await Promise.resolve();
    });
  }

  it('opens the old one-click subscribe box, copies the URL, and opens the QR dialog', async () => {
    await renderDashboard();

    const shortcut = Array.from(container.querySelectorAll('.v2board-shortcuts-item')).find(
      (item) => item.textContent?.includes('一键订阅'),
    )!;

    await act(async () => {
      shortcut.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('oneClickSubscribe___2t9Xg');
    expect(container.innerHTML).toContain('item___yrtOv subsrcibe-for-link');
    expect(container.innerHTML).toContain('复制订阅地址');
    expect(container.innerHTML).toContain('导入到 Hiddify');
    expect(container.innerHTML).toContain('/theme/default/assets/./images/icon/Hiddify.png');

    const copy = Array.from(container.querySelectorAll('.item___yrtOv')).find(
      (item) => item.textContent === '复制订阅地址',
    )!;
    await act(async () => {
      copy.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.copyText).toHaveBeenCalledWith('https://example.test/sub');
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');

    const qr = Array.from(container.querySelectorAll('.item___yrtOv')).find(
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

  it('keeps the bundled-theme random keys for subscribe import entries', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/dashboard.tsx`, 'utf8');

    expect(source).toContain('<div\n          key={Math.random()}');
    expect(source).toContain(
      "src={`${window.settings?.assets_path || ''}/./images/icon/${target.title}.png`}",
    );
    expect(source).not.toContain('key={target.title}');
    expect(source).not.toContain("window.settings?.assets_path ?? ''");
  });

  it('keeps the bundled-theme direct window title append for tutorial shortcut', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/dashboard.tsx`, 'utf8');
    const descStart = source.indexOf("{s.descKey === 'dashboard.shortcut_tutorial_desc'");
    const tutorialShortcutSource = source.slice(
      descStart,
      source.indexOf('</div>', descStart),
    );

    expect(tutorialShortcutSource).toContain('<> {window.settings?.title}</>');
    expect(tutorialShortcutSource).not.toContain("window.settings?.title ?? ''");
  });

  it('keeps the bundled-theme random keys for notice carousel slides', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/dashboard.tsx`, 'utf8');
    const slideSource = source.slice(
      source.indexOf("style={{ outline: 'none', width: slideWidth || undefined }}"),
      source.indexOf('{renderNoticeSlide(notice)}', source.indexOf("style={{ outline: 'none'")),
    );

    expect(source).toContain('noticeList.map((notice, index) => (');
    expect(slideSource).toContain("style={{ outline: 'none', width: slideWidth || undefined }}\n                          key={Math.random()}");
    expect(slideSource).not.toContain('key={notice.id}');
  });

  it('keeps the bundled-theme notice modal mask and false footer props explicit', () => {
    const source = readFileSync(`${process.cwd()}/src/pages/dashboard.tsx`, 'utf8');
    const modalSource = source.slice(
      source.indexOf('<DialogContent title={activeNotice?.title}'),
      source.indexOf('</DialogContent>', source.indexOf('<DialogContent title={activeNotice?.title}')),
    );

    expect(modalSource).toContain(
      '<DialogContent title={activeNotice?.title} maskClosable footer={false}>',
    );
    expect(modalSource).not.toContain('<DialogContent title={activeNotice?.title} footer={null}>');
    expect(source).toContain('footer={false}');
    expect(source).not.toContain('footer={null}');
  });

  it('creates the reset-traffic order with the original confirm options', async () => {
    await renderDashboard();

    const resetButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '购买流量重置包',
    )!;

    await act(async () => {
      resetButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).toHaveBeenCalledTimes(1);
    const options = mocks.legacyConfirm.mock.calls[0]![0] as {
      content: string;
      maskClosable: boolean;
      okText: ReactNode;
      onOk: () => void;
      title: string;
    };
    expect(options.maskClosable).toBe(true);
    expect(options.title).toBe('确定重置当前已用流量？');
    expect(options.content).toBe(
      '点击「确定」将会跳转到收银台，支付订单后系统将会清空您当月已使用流量。',
    );
    expect(options.okText).toBe('确定');

    await act(async () => {
      options.onOk();
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.saveOrder).toHaveBeenCalledWith(
      { name: 'apiClient' },
      { period: 'reset_price', plan_id: 1 },
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/order/ORDER123');
  });

  it('refreshes subscribe once after new-period success without waiting for that refresh', async () => {
    mocks.subscribe = baseSubscribe({ allow_new_period: true, u: 1000 });
    let resolveMutation!: () => void;
    mocks.newPeriodMutateAsync.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveMutation = resolve;
        }),
    );
    mocks.refetchSubscribe.mockImplementation(() => new Promise(() => {}));

    await renderDashboard();

    const newPeriodButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '提前开启流量周期',
    );
    expect(newPeriodButton).toBeTruthy();

    await act(async () => {
      newPeriodButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.legacyConfirm).toHaveBeenCalledTimes(1);
    const confirmOptions = mocks.legacyConfirm.mock.calls[0]![0] as { onOk: () => void };

    act(() => {
      confirmOptions.onOk();
    });

    expect(mocks.newPeriodMutateAsync).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveMutation();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.refetchSubscribe).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('提前开启流量周期成功');
    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
  });
});
