import { type ComponentProps } from 'react';
import { fireEvent, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { AppLayout } from './app-layout';

const mocks = vi.hoisted(() => ({
  darkMode: false,
  themePreference: 'system' as 'system' | 'light' | 'dark',
  labels: {
    'common.cancel': '取消',
    'common.close_dialog': 'Close dialog',
    'common.toggle_theme': 'Toggle theme',
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

// The Sidebar primitive persists expanded/collapsed in the sidebar_state
// cookie, which survives across tests within this file; clear it and pin a
// desktop viewport so a toggle in one test cannot leak into the next. The
// mobile-sheet test overrides innerWidth locally.
function resetShellEnvironment() {
  resetMocks();
  document.cookie = 'sidebar_state=; path=/; max-age=0';
  Object.defineProperty(window, 'innerWidth', {
    configurable: true,
    writable: true,
    value: 1024,
  });
}

describe('AppLayout shadcn app shell structure', () => {
  beforeEach(resetShellEnvironment);

  it('renders the redesigned shell, navigation, header controls, and outlet', () => {
    const { container } = renderWithProviders(<AppLayout />);

    // Parity-harness hooks: #page-container/#main-container are ready
    // selectors, #sidebar and #page-header anchor interaction scenarios.
    expect(container.querySelector('#page-container')).not.toBeNull();
    expect(container.querySelector('#sidebar')).not.toBeNull();
    expect(container.querySelector('#page-header')).not.toBeNull();
    expect(container.querySelector('#main-container')).not.toBeNull();

    // The sidebar exposes a labelled navigation landmark of real links.
    const nav = screen.getByRole('navigation', { name: 'Primary navigation' });
    expect(within(nav).getByRole('link', { name: '仪表盘' })).toHaveAttribute(
      'href',
      '/dashboard',
    );
    expect(within(nav).getByRole('link', { name: '使用文档' })).toHaveAttribute(
      'href',
      '/knowledge',
    );
    for (const name of ['购买订阅', '节点状态', '我的订单', '个人中心']) {
      expect(within(nav).getByRole('link', { name })).toBeInTheDocument();
    }
    // Exactly one item — the one matching the mocked /dashboard location —
    // carries aria-current.
    expect(within(nav).getByRole('link', { name: '仪表盘' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(container.querySelectorAll('[aria-current="page"]')).toHaveLength(1);

    // Brand wordmark links back to the dashboard.
    expect(screen.getByRole('link', { name: 'V2Board' })).toHaveAttribute('href', '/dashboard');

    // Header: the page title (visual-parity reads .v2board-container-title),
    // and the account chip with the user's email. The language switcher moved
    // into the account menu's submenu, so the header must no longer carry a
    // standalone trigger for it.
    const title = screen.getByRole('heading', { level: 1 });
    expect(title).toHaveTextContent('仪表盘');
    expect(title).toHaveClass('v2board-container-title');
    expect(screen.queryByTestId('app-language-trigger')).not.toBeInTheDocument();
    expect(screen.getByTestId('app-avatar-trigger')).toHaveTextContent('user@example.com');

    // The routed page renders inside the shell.
    expect(screen.getByText('Outlet content')).toBeInTheDocument();
  });

  it('uses detail route titles while keeping the parent nav item current', () => {
    mocks.location = { pathname: '/order/TRADE123', search: '' };

    renderWithProviders(<AppLayout />);

    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('订单详情');
    expect(screen.getByRole('link', { name: '我的订单' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('renders a centered loading state instead of the outlet', () => {
    const { container } = renderWithProviders(<AppLayout loading />);

    expect(container.querySelector('#main-container')).not.toBeNull();
    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByText('Outlet content')).not.toBeInTheDocument();
  });

  it('shows the route pending bar only while the router navigation is in flight', () => {
    const { rerender } = renderWithProviders(<AppLayout />);
    expect(screen.queryByTestId('route-pending-bar')).not.toBeInTheDocument();

    mocks.navigationState = 'loading';
    rerender(<AppLayout />);
    expect(screen.getByTestId('route-pending-bar')).toBeInTheDocument();
  });
});

describe('AppLayout shadcn app shell behavior', () => {
  let scrollTo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    resetShellEnvironment();
    scrollTo = vi.fn();
    Object.defineProperty(window, 'scrollTo', {
      configurable: true,
      value: scrollTo,
    });
  });

  it('scrolls on route changes and toggles the collapsible desktop rail', async () => {
    const { container, user } = renderWithProviders(<AppLayout />);

    expect(scrollTo).toHaveBeenCalledWith(0, 0);
    // The desktop rail keeps the #sidebar hook and renders no mobile Sheet.
    expect(container.querySelector('#sidebar')).not.toBeNull();
    expect(
      document.body.querySelector('[data-slot="sidebar"][data-mobile="true"]'),
    ).toBeNull();

    const rail = container.querySelector<HTMLElement>('[data-slot="sidebar"]')!;
    expect(rail).toHaveAttribute('data-state', 'expanded');

    // On desktop the collapse control lives inside the sidebar header itself
    // and collapses the icon rail (backed by the sidebar_state cookie).
    const trigger = container.querySelector<HTMLButtonElement>(
      '#sidebar [data-sidebar="trigger"]',
    )!;
    expect(trigger).toHaveAttribute('aria-label', 'Toggle navigation');
    await user.click(trigger);
    expect(rail).toHaveAttribute('data-state', 'collapsed');
    // The collapse is persisted under the same cookie name the restore path
    // reads back — the write half of the sidebar_state round trip.
    expect(document.cookie).toContain('sidebar_state=false');

    // Nav items stay reachable in the rail as real links (middle-click/a11y)
    // inside the navigation landmark, and route through the router.
    const nav = screen.getByRole('navigation', { name: 'Primary navigation' });
    const knowledge = within(nav).getByRole('link', { name: '使用文档' });
    expect(knowledge).toHaveAttribute('href', '/knowledge');
    await user.click(knowledge);
    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
  });

  it('restores the collapsed rail from the sidebar_state cookie', () => {
    document.cookie = 'sidebar_state=false; path=/';

    const { container } = renderWithProviders(<AppLayout />);

    expect(container.querySelector('[data-slot="sidebar"]')).toHaveAttribute(
      'data-state',
      'collapsed',
    );
  });

  it('opens the mobile nav sheet and closes it after navigation', async () => {
    // Below the 768px breakpoint useIsMobile swaps the rail for a Radix Sheet.
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 500,
    });
    const { container, user } = renderWithProviders(<AppLayout />);

    const sheet = () =>
      document.body.querySelector<HTMLElement>('[data-slot="sidebar"][data-mobile="true"]');
    expect(sheet()).toBeNull();

    // On mobile the drawer opener is the header trigger (the same
    // '#page-header [data-sidebar="trigger"]' selector visual-parity clicks;
    // the sidebar and its own trigger are inside the not-yet-open Sheet).
    const openButton = container.querySelector<HTMLButtonElement>(
      '#page-header [data-sidebar="trigger"]',
    )!;
    await user.click(openButton);

    // The mobile drawer is a Radix Sheet portaled to the document body
    // (focus trap, Esc-dismiss, aria-modal) instead of a hand-rolled transform.
    expect(sheet()).not.toBeNull();

    const knowledge = within(sheet()!).getByRole('link', { name: '使用文档' });
    await user.click(knowledge);

    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
    // Navigating closes the controlled Sheet.
    expect(sheet()).toBeNull();
  });

  it('changes the theme through the header menu', async () => {
    const { user } = renderWithProviders(<AppLayout />);

    const trigger = screen.getByRole('button', { name: 'Toggle theme' });
    expect(trigger).toHaveAttribute('data-dark-mode-trigger');
    expect(trigger.querySelector('svg')).not.toBeNull();
    await user.click(trigger);

    // The menu exposes the three preferences as radio items; picking one
    // routes through the dark-mode store.
    await user.click(screen.getByRole('menuitemradio', { name: 'Dark' }));

    expect(mocks.setThemePreference).toHaveBeenCalledWith('dark');
  });

  it('opens the user menu and logs out through the shadcn dropdown', async () => {
    const { user } = renderWithProviders(<AppLayout />);

    await user.click(screen.getByTestId('app-avatar-trigger'));

    // On desktop the account menu pops upward from the sidebar-footer card,
    // never sideways into the content area.
    expect(screen.getByTestId('app-avatar-menu')).toHaveAttribute('data-side', 'top');

    // Both entries must be actual menu items — the sidebar nav also renders
    // 个人中心 as a link, so body-text alone cannot pin the dropdown's contents.
    expect(screen.getByRole('menuitem', { name: '个人中心' })).toBeInTheDocument();

    await user.click(screen.getByRole('menuitem', { name: '登出' }));

    expect(mocks.logout).toHaveBeenCalled();
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });

  it('switches the language through the user-menu submenu', async () => {
    const { user } = renderWithProviders(<AppLayout />);

    await user.click(screen.getByTestId('app-avatar-trigger'));

    // The language switcher lives inside the account menu as a Radix submenu.
    const subTrigger = screen.getByTestId('app-language-trigger');
    expect(subTrigger).toHaveAttribute('data-slot', 'dropdown-menu-sub-trigger');
    await user.click(subTrigger);

    const submenu = await screen.findByTestId('app-language-menu');
    // The sub-content must be portaled out of the parent menu: rendered
    // inline, the parent's overflow-hidden clips the whole panel away.
    expect(screen.getByTestId('app-avatar-menu').contains(submenu)).toBe(false);

    // Locale items expose radio semantics so SRs announce the active locale.
    // fireEvent, not user.click: userEvent's pointer-leave on the sub-trigger
    // hits Radix's grace-area math, which under happy-dom's zero-size rects
    // closes the submenu before the click would land.
    fireEvent.click(within(submenu).getByRole('menuitemradio', { name: 'English' }));

    // Selecting persists the locale and switches i18n — the language
    // persistence contract, now routed through the profile menu.
    expect(mocks.selectLocale).toHaveBeenCalledWith('en-US');
    expect(mocks.changeLanguage).toHaveBeenCalledWith('en-US');
  });

  it('navigates to the profile page from the user menu', async () => {
    const { user } = renderWithProviders(<AppLayout />);

    await user.click(screen.getByTestId('app-avatar-trigger'));
    await user.click(screen.getByRole('menuitem', { name: '个人中心' }));

    expect(mocks.navigate).toHaveBeenCalledWith('/profile');
  });
});
