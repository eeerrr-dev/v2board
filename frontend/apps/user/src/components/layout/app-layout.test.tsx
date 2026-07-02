import { act, type ComponentProps } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppLayout } from './app-layout';

const mocks = vi.hoisted(() => ({
  darkMode: false,
  themePreference: 'system' as 'system' | 'light' | 'dark',
  labels: {
    'common.cancel': '取消',
    'common.dark_mode_disable': 'Disable dark mode',
    'common.dark_mode_enable': 'Enable dark mode',
    'common.theme_system': 'System',
    'common.theme_light': 'Light',
    'common.theme_dark': 'Dark',
    'common.language': 'Language',
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
    'nav.toggle_nav': 'Toggle navigation',
    'nav.primary_nav': 'Primary navigation',
    'nav.mobile_nav_description': 'Displays the primary navigation.',
    'order.detail': '订单详情',
    'plan.checkout_title': '确认订单',
  } as Record<string, string>,
  locale: 'zh-CN',
  location: { pathname: '/dashboard', search: '' },
  navigationState: 'idle' as 'idle' | 'loading' | 'submitting',
  changeLanguage: vi.fn(),
  logout: vi.fn(),
  navigate: vi.fn(),
  selectLocale: vi.fn(),
  setThemePreference: vi.fn(),
  darkListeners: new Set<() => void>(),
  title: 'V2Board',
  user: { email: 'user@example.com' },
}));

vi.mock('react-router', () => ({
  Outlet: () => <div data-outlet="true">Outlet content</div>,
  // Nav items render as real <a> elements through SidebarMenuButton asChild;
  // the mock mirrors Link's real contract: the user onClick runs first on an
  // un-prevented event, and navigation is skipped entirely when that handler
  // calls preventDefault — so a handler that kills navigation fails the tests.
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
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
  useNavigation: () => ({ state: mocks.navigationState }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: mocks.locale, changeLanguage: mocks.changeLanguage },
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

// The account menu's Language submenu renders the shared locale items; pin the
// enabled-locale list so the test asserts the submenu wiring, not settings.
vi.mock('@/lib/locale-menu', () => ({
  getCurrentLocaleLabel: () => '简体中文',
  getEnabledLocales: () => [
    { code: 'en-US', label: 'English' },
    { code: 'zh-CN', label: '简体中文' },
  ],
  selectLocale: (code: string) => mocks.selectLocale(code),
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
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.darkMode = false;
  mocks.themePreference = 'system';
  mocks.locale = 'zh-CN';
  mocks.location = { pathname: '/dashboard', search: '' };
  mocks.navigationState = 'idle';
  mocks.changeLanguage.mockReset();
  mocks.logout.mockReset();
  mocks.navigate.mockReset();
  mocks.selectLocale.mockReset();
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
    // Nav items are real links (asChild), and exactly one of them — the
    // /dashboard item matching the mocked location — carries aria-current.
    expect(html).toContain('href="/knowledge"');
    expect(html.match(/aria-current="page"/g)).toHaveLength(1);
    const dashboardAnchors = html.match(/<a [^>]*href="\/dashboard"[^>]*>/g) ?? [];
    expect(dashboardAnchors.some((anchor) => anchor.includes('aria-current="page"'))).toBe(true);
    expect(html).toContain('V2Board');
    expect(html).toContain('仪表盘');
    expect(html).toContain('使用文档');
    expect(html).toContain('购买订阅');
    expect(html).toContain('节点状态');
    expect(html).toContain('我的订单');
    expect(html).toContain('个人中心');
    expect(html).toContain('id="page-header"');
    expect(html).toContain('v2board-container-title');
    // The language switcher moved into the account menu's submenu; the header
    // must no longer carry a standalone trigger for it.
    expect(html).not.toContain('data-testid="app-language-trigger"');
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
    // The primitive persists expanded/collapsed in the sidebar_state cookie;
    // clear it so a toggle in one test cannot leak into the next.
    document.cookie = 'sidebar_state=; path=/; max-age=0';
    // Pin a desktop viewport by default so useIsMobile resolves to the
    // collapsible rail; the mobile-sheet test overrides innerWidth locally.
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 1024,
    });
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

  it('scrolls on route changes and toggles the collapsible desktop rail', async () => {
    await renderLayout();

    expect(scrollTo).toHaveBeenCalledWith(0, 0);
    // The desktop rail keeps the #sidebar hook and renders no mobile Sheet.
    expect(container.querySelector('#sidebar')).not.toBeNull();
    expect(
      document.body.querySelector('[data-slot="sidebar"][data-mobile="true"]'),
    ).toBeNull();

    const rail = container.querySelector<HTMLElement>('[data-slot="sidebar"]')!;
    expect(rail.getAttribute('data-state')).toBe('expanded');

    // On desktop the collapse control lives inside the sidebar header itself
    // and collapses the icon rail (backed by the sidebar_state cookie).
    const trigger = container.querySelector<HTMLButtonElement>('#sidebar [data-sidebar="trigger"]')!;
    expect(trigger.getAttribute('aria-label')).toBe('Toggle navigation');
    await act(async () => {
      trigger.click();
      await Promise.resolve();
    });
    expect(rail.getAttribute('data-state')).toBe('collapsed');
    // The collapse is persisted under the same cookie name the restore path
    // reads back — the write half of the sidebar_state round trip.
    expect(document.cookie).toContain('sidebar_state=false');

    // Nav items stay reachable in the rail as real links (middle-click/a11y)
    // inside the navigation landmark, and route through the router.
    const knowledge = Array.from(
      container.querySelectorAll<HTMLAnchorElement>('[data-sidebar="content"] a'),
    ).find((link) => link.textContent?.includes('使用文档'))!;
    expect(knowledge.getAttribute('href')).toBe('/knowledge');
    await act(async () => {
      knowledge.click();
      await Promise.resolve();
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
  });

  it('restores the collapsed rail from the sidebar_state cookie', async () => {
    document.cookie = 'sidebar_state=false; path=/';
    await renderLayout();

    const rail = container.querySelector<HTMLElement>('[data-slot="sidebar"]')!;
    expect(rail.getAttribute('data-state')).toBe('collapsed');
  });

  it('opens the mobile nav sheet and closes it after navigation', async () => {
    // Below the 768px breakpoint useIsMobile swaps the rail for a Radix Sheet.
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 500,
    });
    await renderLayout();

    const sheet = () =>
      document.body.querySelector('[data-slot="sidebar"][data-mobile="true"]');
    expect(sheet()).toBeNull();

    // On mobile the drawer opener is the header trigger (the sidebar and its
    // own trigger are inside the not-yet-open Sheet).
    const openButton = container.querySelector<HTMLButtonElement>(
      '#page-header [data-sidebar="trigger"]',
    )!;
    await act(async () => {
      openButton.click();
      await Promise.resolve();
    });

    // The mobile drawer is a Radix Sheet portaled to the document body
    // (focus trap, Esc-dismiss, aria-modal) instead of a hand-rolled transform.
    expect(sheet()).not.toBeNull();

    const knowledge = Array.from(
      document.body.querySelectorAll<HTMLAnchorElement>(
        '[data-slot="sidebar"][data-mobile="true"] [data-sidebar="content"] a',
      ),
    ).find((link) => link.textContent?.includes('使用文档'))!;
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

    // On desktop the account menu pops upward from the sidebar-footer card,
    // never sideways into the content area.
    const menu = document.body.querySelector<HTMLElement>('[data-testid="app-avatar-menu"]')!;
    expect(menu.getAttribute('data-side')).toBe('top');

    // Both entries must be actual menu items — the sidebar nav also renders
    // 个人中心, so body-text alone cannot pin the dropdown's contents.
    const menuItems = Array.from(document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'));
    expect(menuItems.some((item) => item.textContent?.includes('个人中心'))).toBe(true);

    const logoutItem = menuItems.find((item) => item.textContent?.includes('登出'))!;
    await act(async () => {
      logoutItem.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      logoutItem.click();
      await Promise.resolve();
    });

    expect(mocks.logout).toHaveBeenCalled();
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });

  it('switches the language through the user-menu submenu', async () => {
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

    // The language switcher lives inside the account menu as a Radix submenu.
    const subTrigger = document.body.querySelector<HTMLElement>(
      '[data-testid="app-language-trigger"]',
    )!;
    expect(subTrigger.getAttribute('data-slot')).toBe('dropdown-menu-sub-trigger');
    await act(async () => {
      subTrigger.click();
      await Promise.resolve();
    });

    const submenu = document.body.querySelector<HTMLElement>(
      '[data-testid="app-language-menu"]',
    )!;
    expect(submenu).not.toBeNull();
    const english = Array.from(
      submenu.querySelectorAll<HTMLElement>('[role="menuitem"]'),
    ).find((item) => item.textContent?.includes('English'))!;
    await act(async () => {
      english.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      english.click();
      await Promise.resolve();
    });

    // Selecting persists the locale and switches i18n — the language
    // persistence contract, now routed through the profile menu.
    expect(mocks.selectLocale).toHaveBeenCalledWith('en-US');
    expect(mocks.changeLanguage).toHaveBeenCalledWith('en-US');
  });

  it('navigates to the profile page from the user menu', async () => {
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

    const profileItem = Array.from(
      document.body.querySelectorAll<HTMLElement>('[role="menuitem"]'),
    ).find((item) => item.textContent?.includes('个人中心'))!;
    await act(async () => {
      profileItem.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      profileItem.click();
      await Promise.resolve();
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/profile');
  });
});
