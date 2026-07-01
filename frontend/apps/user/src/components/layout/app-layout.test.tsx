import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppLayout } from './app-layout';

const mocks = vi.hoisted(() => ({
  darkMode: false,
  themePreference: 'system' as 'system' | 'light' | 'dark',
  labels: {
    'common.cancel': '取消',
    'common.toggle_theme': 'Toggle theme',
    'common.theme_system': 'System',
    'common.theme_light': 'Light',
    'common.theme_dark': 'Dark',
    'common.logout': '登出',
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
    'nav.open_nav': 'Open navigation',
    'nav.primary_nav': 'Primary navigation',
    'order.detail': '订单详情',
    'plan.checkout_title': '确认订单',
  } as Record<string, string>,
  locale: 'zh-CN',
  location: { pathname: '/dashboard', search: '' },
  navigationState: 'idle' as 'idle' | 'loading' | 'submitting',
  logout: vi.fn(),
  navigate: vi.fn(),
  setThemePreference: vi.fn(),
  darkListeners: new Set<() => void>(),
  title: 'V2Board',
  version: '9.9.9',
  user: { email: 'user@example.com' },
}));

vi.mock('react-router', () => ({
  Outlet: () => <div data-outlet="true">Outlet content</div>,
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
  useNavigation: () => ({ state: mocks.navigationState }),
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

vi.mock('@/lib/dark-mode', async () => {
  const { useEffect, useState } = await import('react');
  const useStore = <T,>(read: () => T) => {
    const [value, setValue] = useState(read);
    useEffect(() => {
      const listener = () => setValue(read());
      mocks.darkListeners.add(listener);
      return () => {
        mocks.darkListeners.delete(listener);
      };
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);
    return value;
  };
  return {
    setThemePreference: (preference: 'system' | 'light' | 'dark') => {
      mocks.setThemePreference(preference);
      mocks.themePreference = preference;
      mocks.darkMode = preference === 'dark';
      mocks.darkListeners.forEach((listener) => listener());
    },
    useDarkMode: () => useStore(() => mocks.darkMode),
    useThemePreference: () => useStore(() => mocks.themePreference),
  };
});

vi.mock('@/lib/legacy-settings', () => ({
  getLegacyTitle: () => mocks.title,
  getLegacySettings: () => ({ title: mocks.title, version: mocks.version }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.darkMode = false;
  mocks.themePreference = 'system';
  mocks.locale = 'zh-CN';
  mocks.location = { pathname: '/dashboard', search: '' };
  mocks.navigationState = 'idle';
  mocks.logout.mockReset();
  mocks.navigate.mockReset();
  mocks.setThemePreference.mockReset();
  mocks.darkListeners.clear();
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

  it('shows the route pending bar only while the router navigation is in flight', () => {
    expect(renderToStaticMarkup(<AppLayout />)).not.toContain('data-testid="route-pending-bar"');

    mocks.navigationState = 'loading';
    expect(renderToStaticMarkup(<AppLayout />)).toContain('data-testid="route-pending-bar"');
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

  it('scrolls on route changes, opens the mobile nav sheet, and closes it after navigation', async () => {
    await renderLayout();

    expect(scrollTo).toHaveBeenCalledWith(0, 0);
    // The persistent desktop rail keeps the #sidebar hook and the nav for every viewport.
    expect(container.querySelector('#sidebar')).not.toBeNull();

    const sheet = () => document.body.querySelector('[data-slot="sheet-content"]');
    expect(sheet()).toBeNull();

    const openButton = container.querySelector<HTMLButtonElement>('button[aria-label="Open navigation"]')!;
    await act(async () => {
      openButton.click();
      await Promise.resolve();
    });

    // The mobile drawer is now a Radix Sheet portaled to the document body
    // (focus trap, Esc-dismiss, aria-modal) instead of a hand-rolled transform.
    expect(sheet()).not.toBeNull();

    const knowledge = Array.from(
      document.body.querySelectorAll<HTMLButtonElement>('[data-slot="sheet-content"] nav button'),
    ).find((button) => button.textContent?.includes('使用文档'))!;
    await act(async () => {
      knowledge.click();
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
    // Navigating closes the controlled Sheet.
    expect(sheet()).toBeNull();
  });

  it('changes the theme through the header menu', async () => {
    await renderLayout();

    const trigger = container.querySelector<HTMLButtonElement>('button[data-dark-mode-trigger]')!;
    expect(trigger.querySelector('svg')).not.toBeNull();
    await act(async () => {
      trigger.dispatchEvent(
        new PointerEvent('pointerdown', { bubbles: true, button: 0, ctrlKey: false }),
      );
      await Promise.resolve();
    });

    const darkOption = document.body.querySelector<HTMLElement>('[data-theme-option="dark"]')!;
    expect(darkOption).not.toBeNull();
    await act(async () => {
      darkOption.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      darkOption.click();
      await Promise.resolve();
    });

    expect(mocks.setThemePreference).toHaveBeenCalledWith('dark');
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

  it('shows the deployed version from settings instead of a hardcoded string', async () => {
    await renderLayout();

    expect(container.textContent).toContain('V2Board v9.9.9');
    expect(container.innerHTML).not.toContain('v1.7.4');
  });
});
