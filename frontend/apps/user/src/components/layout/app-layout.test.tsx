import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppLayout } from './app-layout';

const mocks = vi.hoisted(() => ({
  darkMode: false,
  labels: {
    'common.cancel': '取消',
    'common.logout': '登出',
    'common.search': '搜索',
    'nav.buy_subscribe': '购买订阅',
    'nav.dashboard': '仪表盘',
    'nav.group_finance': '财务',
    'nav.group_subscribe': '订阅',
    'nav.group_user': '用户',
    'nav.invite': '我的邀请',
    'nav.knowledge': '使用文档',
    'nav.node': '节点状态',
    'nav.orders': '我的订单',
    'nav.profile': '个人中心',
    'nav.tickets': '我的工单',
    'nav.traffic': '流量明细',
    'order.detail': '订单详情',
    'plan.checkout_title': '确认订单',
  } as Record<string, string>,
  locale: 'zh-CN',
  location: { pathname: '/dashboard', search: '' },
  logout: vi.fn(),
  navigate: vi.fn(),
  setDarkMode: vi.fn(),
  title: 'V2Board',
  user: { email: 'user@example.com' },
}));

vi.mock('react-router', () => ({
  Outlet: () => <div data-outlet="true">Outlet content</div>,
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: mocks.locale },
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

vi.mock('./shadcn-language-menu', () => ({
  ShadcnLanguageMenu: () => (
    <button type="button" data-testid="app-language-trigger">
      简体中文
    </button>
  ),
}));

vi.mock('@tanstack/react-query', () => ({
  useSuspenseQuery: () => ({
    data: mocks.user,
  }),
}));

vi.mock('@/lib/queries', () => ({
  userQueryOptions: {
    info: () => ({ queryKey: ['user', 'info'] }),
  },
}));

vi.mock('@/lib/auth', () => ({
  logout: mocks.logout,
}));

vi.mock('@/lib/dark-mode', () => ({
  isDarkModeEnabled: () => mocks.darkMode,
  setDarkMode: mocks.setDarkMode,
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacyTitle: () => mocks.title,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.darkMode = false;
  mocks.locale = 'zh-CN';
  mocks.location = { pathname: '/dashboard', search: '' };
  mocks.logout.mockReset();
  mocks.navigate.mockReset();
  mocks.setDarkMode.mockReset();
  mocks.title = 'V2Board';
  mocks.user = { email: 'user@example.com' };
}

describe('AppLayout shadcn app shell markup', () => {
  beforeEach(resetMocks);

  it('renders the redesigned shell, navigation, header controls, and outlet', () => {
    const html = renderToStaticMarkup(<AppLayout />);

    expect(html).toContain('id="page-container"');
    expect(html).toContain('v2board-app-shell');
    expect(html).toContain('id="sidebar"');
    expect(html).toContain('V2Board');
    expect(html).toContain('仪表盘');
    expect(html).toContain('使用文档');
    expect(html).toContain('购买订阅');
    expect(html).toContain('节点状态');
    expect(html).toContain('我的订单');
    expect(html).toContain('个人中心');
    expect(html).toContain('id="page-header"');
    expect(html).toContain('v2board-container-title');
    expect(html).toContain('data-testid="app-language-trigger"');
    expect(html).toContain('data-testid="app-avatar-trigger"');
    expect(html).toContain('user@example.com');
    expect(html).toContain('id="main-container"');
    expect(html).toContain('v2board-app-main');
    expect(html).toContain('Outlet content');
    expect(html).not.toContain('nav-main-link');
    expect(html).not.toContain('content content-full');
  });

  it('uses detail route titles without marking sidebar structure as legacy nav', () => {
    mocks.location = { pathname: '/order/TRADE123', search: '' };

    const html = renderToStaticMarkup(<AppLayout />);

    expect(html).toContain('订单详情');
    expect(html).not.toContain('nav-main-link active');
  });

  it('renders a centered shadcn loading state', () => {
    const html = renderToStaticMarkup(<AppLayout loading />);

    expect(html).toContain('id="main-container"');
    expect(html).toContain('v2board-app-main');
    expect(html).toContain('animate-spin');
    expect(html).not.toContain('data-outlet="true"');
  });
});

describe('AppLayout shadcn app shell behavior', () => {
  let container: HTMLDivElement;
  let root: Root;
  let scrollTo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    resetMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    scrollTo = vi.fn();
    Object.defineProperty(window, 'scrollTo', {
      configurable: true,
      value: scrollTo,
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderLayout(props?: Parameters<typeof AppLayout>[0]) {
    await act(async () => {
      root.render(<AppLayout {...props} />);
      await Promise.resolve();
    });
  }

  it('scrolls on route changes, opens mobile nav, and closes it after navigation', async () => {
    await renderLayout();

    expect(scrollTo).toHaveBeenCalledWith(0, 0);
    const openButton = container.querySelector<HTMLButtonElement>('button[aria-label="Open navigation"]')!;

    await act(async () => {
      openButton.click();
      await Promise.resolve();
    });

    expect(container.querySelector('#sidebar')?.className).toContain('translate-x-0');

    const knowledge = Array.from(container.querySelectorAll<HTMLButtonElement>('nav button')).find(
      (button) => button.textContent?.includes('使用文档'),
    )!;
    await act(async () => {
      knowledge.click();
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
    expect(container.querySelector('#sidebar')?.className).toContain('max-lg:-translate-x-full');
  });

  it('toggles dark mode through the header button', async () => {
    await renderLayout();

    const toggle = container.querySelector<HTMLButtonElement>('button[data-dark-mode-trigger]')!;
    await act(async () => {
      toggle.click();
      await Promise.resolve();
    });

    expect(mocks.setDarkMode).toHaveBeenCalledWith(true);
    expect(toggle.getAttribute('aria-label')).toBe('Disable dark mode');
    expect(container.querySelector('button[data-dark-mode-trigger] svg')).not.toBeNull();
  });

  it('opens the user menu and logs out through the shadcn dropdown', async () => {
    await renderLayout();

    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="app-avatar-trigger"]',
    )!;
    await act(async () => {
      trigger.dispatchEvent(
        new PointerEvent('pointerdown', { bubbles: true, button: 0, ctrlKey: false }),
      );
      await Promise.resolve();
    });

    expect(document.body.textContent).toContain('个人中心');
    expect(document.body.textContent).toContain('登出');

    const logoutItem = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]')).find(
      (item) => item.textContent?.includes('登出'),
    )!;
    await act(async () => {
      logoutItem.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      logoutItem.click();
      await Promise.resolve();
    });

    expect(mocks.logout).toHaveBeenCalled();
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });

  it('renders the optional search row without legacy overlay markup', async () => {
    const onChange = vi.fn();
    await renderLayout({ search: { placeholder: 'Search...', onChange } });

    const search = Array.from(container.querySelectorAll<HTMLButtonElement>('button')).find(
      (button) => button.textContent === '搜索',
    )!;
    await act(async () => {
      search.click();
      await Promise.resolve();
    });

    const input = container.querySelector<HTMLInputElement>('input[placeholder="Search..."]')!;
    expect(input.closest('.border-t')?.className).not.toMatch(/(^|\s)block(\s|$)/);
    const valueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    valueSetter?.call(input, 'node');
    await act(async () => {
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenCalledWith('node');
    expect(container.innerHTML).not.toContain('overlay-header');
  });
});
