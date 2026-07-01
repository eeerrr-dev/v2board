import {
  Fragment,
  Suspense,
  type ComponentType,
  type SVGProps,
  useEffect,
  useState,
} from 'react';
import { useLocation, useNavigate, useNavigation } from 'react-router';
import { useSuspenseQuery } from '@tanstack/react-query';
import type { ParseKeys } from 'i18next';
import { useTranslation } from 'react-i18next';
import { getLegacyLocaleClassName } from '@v2board/i18n';
import {
  Activity,
  BookOpen,
  Gauge,
  Headphones,
  LogOut,
  Menu,
  Monitor,
  Moon,
  ReceiptText,
  Server,
  ShoppingBag,
  Sun,
  UserRound,
  UsersRound,
} from 'lucide-react';
import { ShadcnLanguageMenu } from './shadcn-language-menu';
import { userQueryOptions } from '@/lib/queries';
import { logout } from '@/lib/auth';
import { cn } from '@/lib/cn';
import {
  setThemePreference,
  useDarkMode,
  useThemePreference,
  type ThemePreference,
} from '@/lib/dark-mode';
import { getLegacySettings, getLegacyTitle } from '@/lib/legacy-settings';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Sheet, SheetContent, SheetTitle } from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';

type ShellIcon = ComponentType<SVGProps<SVGSVGElement>>;

interface NavItem {
  to: string;
  labelKey: ParseKeys;
  icon: ShellIcon;
}

interface NavGroup {
  labelKey?: ParseKeys;
  items: NavItem[];
}

interface AppLayoutProps {
  loading?: boolean;
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

const DETAIL_LABELS: { match: RegExp; labelKey: ParseKeys }[] = [
  { match: /^\/order\/[^/]+$/, labelKey: 'order.detail' },
  { match: /^\/plan\/[^/]+$/, labelKey: 'plan.checkout_title' },
];

// Derive a compact avatar monogram from the account email so the header shows a
// real avatar instead of the raw, truncated address. Prefer the first letters of
// the first two dot/underscore-separated name parts, else the first two chars.
function getInitials(email: string): string {
  const local = (email.split('@')[0] ?? '').trim();
  const parts = local.split(/[._\-+]+/).filter(Boolean);
  const letters =
    parts.length >= 2
      ? `${parts[0]?.[0] ?? ''}${parts[1]?.[0] ?? ''}`
      : local.slice(0, 2);
  return letters.toUpperCase() || 'U';
}

function findActiveLabel(pathname: string): ParseKeys | undefined {
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

function AppLayoutContent({ loading, title: titleProp }: AppLayoutProps = {}) {
  const { i18n, t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  // The redesigned shell lazy-loads each route's chunk and runs its loader on
  // navigation; surface that interstitial with the data router's own navigation
  // state instead of leaving the previous page frozen (Tier-2 presentation).
  const navigation = useNavigation();
  const navPending = navigation.state !== 'idle';
  // The require-user route loader has already awaited ensureQueryData for the
  // user info, so this read is guaranteed to be satisfied: useSuspenseQuery
  // makes the value non-nullable and lets the old `Loading...` branches go.
  const { data: user } = useSuspenseQuery({
    ...userQueryOptions.info(),
    refetchOnMount: false,
  });
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const darkMode = useDarkMode();
  const themePreference = useThemePreference();
  const activeLabel = findActiveLabel(location.pathname);
  const siteTitle = getLegacyTitle();
  // Read the real deployed version the backend injects into window.settings
  // rather than a hardcoded string that drifts out of date.
  const version = getLegacySettings().version;
  const title = titleProp ?? (activeLabel ? t(activeLabel) : '');
  const localeClass = getLegacyLocaleClassName(i18n.language);
  const initials = getInitials(user.email);

  const go = (to: string) => {
    navigate(to);
    setSidebarOpen(false);
  };

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname]);

  const sidebarBody = (
    <div className="flex h-full flex-col">
      <div className="flex h-16 items-center border-b border-border px-5">
        <button
          type="button"
          className="rounded-md text-lg font-semibold tracking-normal text-foreground outline-none transition-colors hover:text-primary focus-visible:ring-[3px] focus-visible:ring-ring/50"
          onClick={() => go('/dashboard')}
        >
          {siteTitle}
        </button>
      </div>

      <nav className="flex-1 overflow-y-auto px-3 py-4" aria-label={t('nav.primary_nav')}>
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
                    aria-current={active ? 'page' : undefined}
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
        {siteTitle}
        {version ? ` v${version}` : null}
      </div>
    </div>
  );

  return (
    <div
      id="page-container"
      className={cn('v2board-island v2board-app-shell min-h-screen text-foreground', localeClass)}
    >
      {navPending ? (
        <div
          data-testid="route-pending-bar"
          aria-hidden
          className="pointer-events-none fixed inset-x-0 top-0 z-50 h-0.5 overflow-hidden"
        >
          <div className="h-full w-full animate-pulse bg-primary" />
        </div>
      ) : null}

      <aside
        id="sidebar"
        className="fixed inset-y-0 left-0 z-40 hidden w-72 flex-col border-r border-border bg-card/95 text-card-foreground shadow-sm backdrop-blur lg:flex"
      >
        {sidebarBody}
      </aside>

      <Sheet open={sidebarOpen} onOpenChange={setSidebarOpen}>
        <SheetContent
          side="left"
          aria-describedby={undefined}
          className="w-72 gap-0 bg-card/95 p-0 text-card-foreground sm:max-w-72 lg:hidden"
        >
          <SheetTitle className="sr-only">{siteTitle}</SheetTitle>
          {sidebarBody}
        </SheetContent>
      </Sheet>

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
              aria-label={t('nav.open_nav')}
            >
              <Menu className="size-4" />
            </Button>

            <div className="min-w-0 flex-1">
              <h1 className="v2board-container-title truncate text-base font-semibold text-foreground">
                {title}
              </h1>
            </div>

            <div className="flex items-center gap-0.5">
              <DropdownMenu modal={false}>
                <DropdownMenuTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="text-muted-foreground"
                    data-dark-mode-trigger
                    aria-label={t('common.toggle_theme')}
                  >
                    {darkMode ? <Moon className="size-4" /> : <Sun className="size-4" />}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="min-w-36">
                  <DropdownMenuRadioGroup
                    value={themePreference}
                    onValueChange={(value) => setThemePreference(value as ThemePreference)}
                  >
                    <DropdownMenuRadioItem value="system" data-theme-option="system" className="gap-2">
                      <Monitor className="size-4" />
                      {t('common.theme_system')}
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem value="light" data-theme-option="light" className="gap-2">
                      <Sun className="size-4" />
                      {t('common.theme_light')}
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem value="dark" data-theme-option="dark" className="gap-2">
                      <Moon className="size-4" />
                      {t('common.theme_dark')}
                    </DropdownMenuRadioItem>
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>

              <ShadcnLanguageMenu />

              <div className="mx-1.5 h-5 w-px bg-border" aria-hidden="true" />

              <DropdownMenu modal={false}>
                <DropdownMenuTrigger asChild>
                  <button
                    type="button"
                    data-testid="app-avatar-trigger"
                    aria-label={user.email}
                    className="inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-semibold text-foreground ring-1 ring-border outline-none transition-colors hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=open]:ring-[3px] data-[state=open]:ring-ring/50"
                  >
                    <span aria-hidden="true">{initials}</span>
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                  align="end"
                  sideOffset={8}
                  className="v2board-island v2board-app-shell-menu-content w-60"
                  data-testid="app-avatar-menu"
                >
                  <div className="flex items-center gap-2.5 px-2 py-1.5">
                    <span
                      aria-hidden="true"
                      className="inline-flex size-8 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-semibold text-foreground ring-1 ring-border"
                    >
                      {initials}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                      {user.email}
                    </span>
                  </div>
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
          </div>
        </header>

        {loading ? (
          <main id="main-container" className="v2board-app-main bg-muted">
            <div className="mx-auto flex min-h-[calc(100vh-4rem)] w-full max-w-7xl items-center justify-center px-4 py-10 sm:px-6 lg:px-8">
              <Spinner className="size-6" />
            </div>
          </main>
        ) : (
          <main id="main-container" className="v2board-app-main bg-muted">
            <div className="mx-auto w-full max-w-7xl px-4 py-6 sm:px-6 lg:px-8 lg:py-8">
              <RouteBoundaryOutlet />
            </div>
          </main>
        )}
      </div>
    </div>
  );
}

function AppLayoutFallback() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-muted/40">
      <Spinner className="size-6" />
    </div>
  );
}

export function AppLayout(props: AppLayoutProps = {}) {
  // useSuspenseQuery inside AppLayoutContent suspends until the user info is
  // ready and re-throws on failure. The require-user loader preloads the data,
  // so the fallback is only a defensive boundary; the route errorElement above
  // turns a suspense re-throw into the existing error UI rather than a white
  // screen.
  return (
    <Suspense fallback={<AppLayoutFallback />}>
      <AppLayoutContent {...props} />
    </Suspense>
  );
}
