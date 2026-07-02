import {
  Suspense,
  useEffect,
  useState,
  type ComponentType,
  type SVGProps,
} from 'react';
import { Link, useLocation, useNavigation } from 'react-router';
import { useSuspenseQuery } from '@tanstack/react-query';
import type { ParseKeys } from 'i18next';
import { useTranslation } from 'react-i18next';
import { getLegacyLocaleClassName } from '@v2board/i18n';
import {
  Activity,
  BookOpen,
  Gauge,
  Headphones,
  Monitor,
  Moon,
  ReceiptText,
  Server,
  ShoppingBag,
  Sun,
  UserRound,
  UsersRound,
} from 'lucide-react';
import { NavUser } from './nav-user';
import { userQueryOptions } from '@/lib/queries';
import { cn } from '@/lib/cn';
import {
  setThemePreference,
  useDarkMode,
  useThemePreference,
  type ThemePreference,
} from '@/lib/dark-mode';
import { getLegacyTitle } from '@/lib/legacy-settings';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Separator } from '@/components/ui/separator';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
  useSidebar,
} from '@/components/ui/sidebar';
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

const SIDEBAR_STATE_COOKIE = 'sidebar_state';

// The Sidebar primitive writes this cookie on every desktop expand/collapse;
// reading it back into defaultOpen makes the choice survive reloads (a
// frontend-only Tier-2 nicety, same pattern as the dark_mode cookie).
function readSidebarDefaultOpen(): boolean {
  if (typeof document === 'undefined') return true;
  const value = document.cookie
    .split('; ')
    .find((part) => part.startsWith(`${SIDEBAR_STATE_COOKIE}=`))
    ?.slice(SIDEBAR_STATE_COOKIE.length + 1);
  return value !== 'false';
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

// The sidebar lives inside SidebarProvider, so it owns the router hooks and the
// mobile-sheet close-on-navigate (setOpenMobile) that AppLayoutContent — which
// renders the provider — cannot reach through useSidebar.
function AppSidebar({ siteTitle, email }: { siteTitle: string; email: string }) {
  const { t } = useTranslation();
  const location = useLocation();
  const { setOpenMobile } = useSidebar();

  const closeMobile = () => setOpenMobile(false);

  return (
    <Sidebar
      id="sidebar"
      variant="sidebar"
      collapsible="icon"
      sheetTitle={t('nav.primary_nav')}
      sheetDescription={t('nav.mobile_nav_description')}
    >
      {/* Wordmark-only brand (no logo chip) with the collapse trigger living in
          the sidebar itself. Collapsed, the wordmark hides and the size-8
          trigger left-anchors onto the same column as the nav icons, so the
          rail reads as one stable icon column. */}
      <SidebarHeader>
        <div className="flex items-center justify-between gap-1">
          <Link
            to="/dashboard"
            onClick={closeMobile}
            className="min-w-0 truncate rounded-md px-2 py-1 text-base font-semibold text-sidebar-foreground outline-hidden focus-visible:ring-2 focus-visible:ring-sidebar-ring group-data-[collapsible=icon]:hidden"
          >
            {siteTitle}
          </Link>
          <SidebarTrigger className="size-8 shrink-0" aria-label={t('nav.toggle_nav')} />
        </div>
      </SidebarHeader>

      <SidebarContent role="navigation" aria-label={t('nav.primary_nav')}>
        {NAV.map((group, groupIndex) => (
          <SidebarGroup key={group.labelKey ?? `group-${groupIndex}`}>
            {group.labelKey ? (
              // mt-0 overrides the primitive's -mt-8 retraction so the label
              // row keeps its height when collapsed (text still fades): the
              // icons below never shift vertically during expand/collapse.
              <SidebarGroupLabel className="group-data-[collapsible=icon]:mt-0">
                {t(group.labelKey)}
              </SidebarGroupLabel>
            ) : null}
            <SidebarGroupContent>
              <SidebarMenu>
                {group.items.map((item) => {
                  const active =
                    location.pathname === item.to ||
                    location.pathname.startsWith(item.to + '/');

                  return (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton asChild isActive={active} tooltip={t(item.labelKey)}>
                        <Link
                          to={item.to}
                          aria-current={active ? 'page' : undefined}
                          onClick={closeMobile}
                        >
                          <item.icon />
                          <span>{t(item.labelKey)}</span>
                        </Link>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  );
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
      </SidebarContent>

      <SidebarFooter>
        <NavUser email={email} />
      </SidebarFooter>
    </Sidebar>
  );
}

function AppLayoutContent({ loading, title: titleProp }: AppLayoutProps = {}) {
  const { i18n, t } = useTranslation();
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
  const darkMode = useDarkMode();
  const themePreference = useThemePreference();
  const [sidebarDefaultOpen] = useState(readSidebarDefaultOpen);
  const activeLabel = findActiveLabel(location.pathname);
  const siteTitle = getLegacyTitle();
  const title = titleProp ?? (activeLabel ? t(activeLabel) : '');
  const localeClass = getLegacyLocaleClassName(i18n.language);
  const themeControlLabel = darkMode
    ? t('common.dark_mode_disable')
    : t('common.dark_mode_enable');

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [location.pathname]);

  return (
    <SidebarProvider
      id="page-container"
      defaultOpen={sidebarDefaultOpen}
      className={cn('v2board-island v2board-app-shell text-foreground', localeClass)}
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

      <AppSidebar siteTitle={siteTitle} email={user.email} />

      <SidebarInset>
        <header
          id="page-header"
          className="flex h-12 shrink-0 items-center gap-2 border-b border-border"
        >
          <div className="flex w-full items-center gap-1 px-4 sm:px-6 lg:gap-2">
            {/* The desktop collapse control lives in the sidebar header; this
                trigger only opens the mobile drawer. */}
            <SidebarTrigger className="-ml-1 md:hidden" aria-label={t('nav.toggle_nav')} />

            <Separator
              orientation="vertical"
              className="mx-2 data-[orientation=vertical]:h-4 md:hidden"
            />

            <h1 className="v2board-container-title min-w-0 flex-1 truncate text-base font-medium text-foreground">
              {title}
            </h1>

            <div className="ml-auto flex shrink-0 items-center gap-1">
              <DropdownMenu modal={false}>
                <DropdownMenuTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-8 text-muted-foreground hover:text-foreground data-[state=open]:bg-accent data-[state=open]:text-accent-foreground"
                    data-dark-mode-trigger
                    aria-label={themeControlLabel}
                    title={themeControlLabel}
                  >
                    {darkMode ? <Moon /> : <Sun />}
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-40">
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
            </div>
          </div>
        </header>

        {loading ? (
          <div id="main-container" className="v2board-app-main flex flex-1">
            <div className="flex w-full flex-1 items-center justify-center px-4 py-10">
              <Spinner className="size-6" />
            </div>
          </div>
        ) : (
          <div id="main-container" className="v2board-app-main flex-1">
            <div className="mx-auto w-full max-w-7xl px-4 py-4 sm:px-6 md:py-6">
              <RouteBoundaryOutlet />
            </div>
          </div>
        )}
      </SidebarInset>
    </SidebarProvider>
  );
}

function AppLayoutFallback() {
  // Renders before the shell (and its island wrapper) exists, so it must carry
  // v2board-island itself for the token background to resolve.
  return (
    <div className="v2board-island flex min-h-screen items-center justify-center bg-background">
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
