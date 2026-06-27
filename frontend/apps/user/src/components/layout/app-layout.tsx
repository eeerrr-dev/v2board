import { Fragment, type ComponentType, type SVGProps, useEffect, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { getLegacyLocaleClassName } from '@v2board/i18n';
import {
  Activity,
  BookOpen,
  CircleUserRound,
  Gauge,
  Headphones,
  LogOut,
  Menu,
  Moon,
  ReceiptText,
  Server,
  ShoppingBag,
  Sun,
  UserRound,
  UsersRound,
  X,
} from 'lucide-react';
import { ShadcnLanguageMenu } from './shadcn-language-menu';
import { useUserInfo } from '@/lib/queries';
import { logout } from '@/lib/auth';
import { cn } from '@/lib/cn';
import { isDarkModeEnabled, setDarkMode } from '@/lib/dark-mode';
import { getLegacyTitle } from '@/lib/legacy-settings';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';

type ShellIcon = ComponentType<SVGProps<SVGSVGElement>>;

interface NavItem {
  to: string;
  labelKey: string;
  icon: ShellIcon;
}

interface NavGroup {
  labelKey?: string;
  items: NavItem[];
}

interface LegacyLayoutSearch {
  placeholder?: string;
  defaultValue?: string;
  onChange: (value: string) => void;
}

interface AppLayoutProps {
  loading?: boolean;
  search?: LegacyLayoutSearch;
  title?: string;
}

const NAV: NavGroup[] = [
  {
    items: [
      { to: '/dashboard', labelKey: 'nav.dashboard', icon: Gauge },
      { to: '/knowledge', labelKey: 'nav.knowledge', icon: BookOpen },
    ],
  },
  {
    labelKey: 'nav.group_subscribe',
    items: [
      { to: '/plan', labelKey: 'nav.buy_subscribe', icon: ShoppingBag },
      { to: '/node', labelKey: 'nav.node', icon: Server },
    ],
  },
  {
    labelKey: 'nav.group_finance',
    items: [
      { to: '/order', labelKey: 'nav.orders', icon: ReceiptText },
      { to: '/invite', labelKey: 'nav.invite', icon: UsersRound },
    ],
  },
  {
    labelKey: 'nav.group_user',
    items: [
      { to: '/profile', labelKey: 'nav.profile', icon: UserRound },
      { to: '/ticket', labelKey: 'nav.tickets', icon: Headphones },
      { to: '/traffic', labelKey: 'nav.traffic', icon: Activity },
    ],
  },
];

const DETAIL_LABELS: { match: RegExp; labelKey: string }[] = [
  { match: /^\/order\/[^/]+$/, labelKey: 'order.detail' },
  { match: /^\/plan\/[^/]+$/, labelKey: 'plan.checkout_title' },
];

function findActiveLabel(pathname: string): string | undefined {
  for (const d of DETAIL_LABELS) {
    if (d.match.test(pathname)) return d.labelKey;
  }
  for (const group of NAV) {
    for (const item of group.items) {
      if (pathname === item.to || pathname.startsWith(item.to + '/')) {
        return item.labelKey;
      }
    }
  }
  return undefined;
}

export function AppLayout({ loading, search, title: titleProp }: AppLayoutProps = {}) {
  const { i18n, t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const { data: user } = useUserInfo({ refetchOnMount: false });
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [showSearchBar, setShowSearchBar] = useState(false);
  const [darkMode, setDarkModeState] = useState(() => isDarkModeEnabled());
  const activeLabel = findActiveLabel(location.pathname);
  const siteTitle = getLegacyTitle();
  const title = titleProp ?? (activeLabel ? t(activeLabel) : '');
  const localeClass = getLegacyLocaleClassName(i18n.language);

  const go = (to: string) => {
    navigate(to);
    setSidebarOpen(false);
  };

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname]);

  const sidebar = (
    <aside
      id="sidebar"
      className={cn(
        'fixed inset-y-0 left-0 z-40 flex w-72 flex-col border-r border-border bg-card/95 text-card-foreground shadow-sm backdrop-blur transition-transform duration-200 ease-out lg:translate-x-0',
        sidebarOpen ? 'translate-x-0' : 'max-lg:-translate-x-full',
      )}
    >
      <div className="flex h-16 items-center justify-between border-b border-border px-5">
        <button
          type="button"
          className="rounded-md text-lg font-semibold tracking-normal text-foreground outline-none transition-colors hover:text-primary focus-visible:ring-[3px] focus-visible:ring-ring/50"
          onClick={() => go('/dashboard')}
        >
          {siteTitle}
        </button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="lg:hidden"
          onClick={() => setSidebarOpen(false)}
          aria-label="Close navigation"
        >
          <X className="size-4" />
        </Button>
      </div>

      <nav className="flex-1 overflow-y-auto px-3 py-4" aria-label="Primary navigation">
        {NAV.map((group, groupIndex) => (
          <Fragment key={group.labelKey ?? `group-${groupIndex}`}>
            {group.labelKey ? (
              <div className="px-3 pb-2 pt-4 text-xs font-medium uppercase tracking-normal text-muted-foreground first:pt-0">
                {t(group.labelKey)}
              </div>
            ) : null}
            <div className="space-y-1">
              {group.items.map((item) => {
                const Icon = item.icon;
                const active =
                  location.pathname === item.to || location.pathname.startsWith(item.to + '/');

                return (
                  <button
                    type="button"
                    key={item.to}
                    className={cn(
                      'flex h-9 w-full items-center gap-2.5 rounded-md px-3 text-left text-sm font-medium text-muted-foreground transition-all hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
                      active && 'bg-primary text-primary-foreground shadow-xs hover:bg-primary/90 hover:text-primary-foreground',
                    )}
                    onClick={() => go(item.to)}
                  >
                    <Icon className="size-4" />
                    <span>{t(item.labelKey)}</span>
                  </button>
                );
              })}
            </div>
          </Fragment>
        ))}
      </nav>

      <div className="border-t border-border px-5 py-4 text-xs text-muted-foreground">
        {siteTitle} v1.7.4
      </div>
    </aside>
  );

  return (
    <div id="page-container" className={cn('v2board-app-shell min-h-screen', localeClass)}>
      <div
        className={cn(
          'fixed inset-0 z-30 bg-black/40 transition-opacity lg:hidden',
          sidebarOpen ? 'opacity-100' : 'pointer-events-none opacity-0',
        )}
        onClick={() => setSidebarOpen(false)}
      />

      {sidebar}

      <div className="min-h-screen bg-muted/40 lg:pl-72">
        <header
          id="page-header"
          className="sticky top-0 z-30 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80"
        >
          <div className="flex h-16 items-center gap-3 px-4 sm:px-6 lg:px-8">
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="lg:hidden"
              onClick={() => setSidebarOpen(true)}
              aria-label="Open navigation"
            >
              <Menu className="size-4" />
            </Button>

            <div className="min-w-0 flex-1">
              <div className="v2board-container-title truncate text-base font-semibold text-foreground">
                {title}
              </div>
            </div>

            {search ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => setShowSearchBar(true)}
              >
                {t('common.search')}
              </Button>
            ) : null}

            <Button
              type="button"
              variant="ghost"
              size="icon"
              data-dark-mode-trigger
              aria-label={darkMode ? 'Disable dark mode' : 'Enable dark mode'}
              onClick={() => {
                const next = !darkMode;
                setDarkMode(next);
                setDarkModeState(next);
              }}
            >
              {darkMode ? <Moon className="size-4" /> : <Sun className="size-4" />}
            </Button>

            <ShadcnLanguageMenu />

            <DropdownMenu modal={false}>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  className="h-9 max-w-[220px] gap-2 px-2.5"
                  data-testid="app-avatar-trigger"
                >
                  <CircleUserRound className="size-4" />
                  <span className="hidden truncate text-sm font-medium lg:inline">
                    {user?.email || 'Loading...'}
                  </span>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent
                align="end"
                sideOffset={8}
                className="v2board-app-shell-menu-content w-56"
                data-testid="app-avatar-menu"
              >
                <DropdownMenuLabel className="truncate font-normal text-muted-foreground">
                  {user?.email || 'Loading...'}
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={() => navigate('/profile')}>
                  <UserRound className="size-4" />
                  {t('nav.profile')}
                </DropdownMenuItem>
                <DropdownMenuItem
                  onSelect={() => {
                    logout();
                    navigate('/login');
                  }}
                >
                  <LogOut className="size-4" />
                  {t('common.logout')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>

          {search ? (
            <div
              className={cn(
                'border-t border-border bg-background px-4 py-3 sm:px-6 lg:px-8',
                !showSearchBar && 'hidden',
              )}
            >
              <div className="flex gap-2">
                <Input
                  placeholder={search.placeholder}
                  onChange={(event) => search.onChange(event.target.value)}
                  defaultValue={search.defaultValue}
                />
                <Button type="button" variant="outline" onClick={() => setShowSearchBar(false)}>
                  {t('common.cancel')}
                </Button>
              </div>
            </div>
          ) : null}
        </header>

        {loading ? (
          <main id="main-container" className="v2board-app-main">
            <div className="mx-auto flex min-h-[calc(100vh-4rem)] w-full max-w-7xl items-center justify-center px-4 py-10 sm:px-6 lg:px-8">
              <Spinner className="size-6" />
            </div>
          </main>
        ) : (
          <main id="main-container" className="v2board-app-main">
            <div className="mx-auto w-full max-w-7xl px-4 py-6 sm:px-6 lg:px-8 lg:py-8">
              <RouteBoundaryOutlet />
            </div>
          </main>
        )}
      </div>
    </div>
  );
}
