import { Suspense, useState, type ComponentType, type SVGProps } from 'react';
import { Link, ScrollRestoration, useLocation, useNavigation } from 'react-router';
import { useSuspenseQuery } from '@tanstack/react-query';
import type { SelectorParam } from 'i18next';
import { useTranslation } from 'react-i18next';
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
import {
  setThemePreference,
  useDarkMode,
  useThemePreference,
  type ThemePreference,
} from '@/lib/dark-mode';
import { getSiteTitle } from '@/lib/runtime-config';
import { readCookie } from '@v2board/i18n';
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
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { Skeleton } from '@/components/ui/skeleton';
import { Spinner } from '@/components/ui/spinner';

type ShellIcon = ComponentType<SVGProps<SVGSVGElement>>;

interface NavItem {
  to: string;
  labelKey: SelectorParam;
  icon: ShellIcon;
}

interface NavGroup {
  id: string;
  labelKey?: SelectorParam;
  items: NavItem[];
}

interface AppLayoutProps {
  loading?: boolean;
  title?: string;
}

const NAV: NavGroup[] = [
  {
    id: 'primary',
    items: [
      { to: '/dashboard', labelKey: ($) => $.nav.dashboard, icon: Gauge },
      { to: '/knowledge', labelKey: ($) => $.nav.knowledge, icon: BookOpen },
    ],
  },
  {
    id: 'subscribe',
    labelKey: ($) => $.nav.group_subscribe,
    items: [
      { to: '/plan', labelKey: ($) => $.nav.buy_subscribe, icon: ShoppingBag },
      { to: '/node', labelKey: ($) => $.nav.node, icon: Server },
    ],
  },
  {
    id: 'finance',
    labelKey: ($) => $.nav.group_finance,
    items: [
      { to: '/order', labelKey: ($) => $.nav.orders, icon: ReceiptText },
      { to: '/invite', labelKey: ($) => $.nav.invite, icon: UsersRound },
    ],
  },
  {
    id: 'user',
    labelKey: ($) => $.nav.group_user,
    items: [
      { to: '/profile', labelKey: ($) => $.nav.profile, icon: UserRound },
      { to: '/ticket', labelKey: ($) => $.nav.tickets, icon: Headphones },
      { to: '/traffic', labelKey: ($) => $.nav.traffic, icon: Activity },
    ],
  },
];

const DETAIL_LABELS: { match: RegExp; labelKey: SelectorParam }[] = [
  { match: /^\/order\/[^/]+$/, labelKey: ($) => $.order.detail },
  { match: /^\/plan\/[^/]+$/, labelKey: ($) => $.plan.checkout_title },
];

const SIDEBAR_STATE_COOKIE = 'sidebar_state';

// The Sidebar primitive writes this cookie on every desktop expand/collapse;
// reading it back into defaultOpen makes the choice survive reloads (a
// frontend-only Tier-2 nicety, same pattern as the dark_mode cookie).
function readSidebarDefaultOpen(): boolean {
  if (typeof document === 'undefined') return true;
  // Reuse the shared cookie reader (same parser dark-mode.ts uses) instead of a
  // second hand-rolled tokenizer. Absent cookie -> '' -> open by default.
  return readCookie(SIDEBAR_STATE_COOKIE) !== 'false';
}

function findActiveLabel(pathname: string): SelectorParam | undefined {
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
      sheetTitle={t(($) => $.nav.primary_nav)}
      sheetDescription={t(($) => $.nav.mobile_nav_description)}
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
            className="min-w-0 truncate rounded-md px-2 py-1 text-base font-semibold text-sidebar-foreground outline-hidden group-data-[collapsible=icon]:hidden focus-visible:ring-2 focus-visible:ring-sidebar-ring"
          >
            {siteTitle}
          </Link>
          <SidebarTrigger className="size-8 shrink-0" aria-label={t(($) => $.nav.toggle_nav)} />
        </div>
      </SidebarHeader>

      <SidebarContent role="navigation" aria-label={t(($) => $.nav.primary_nav)}>
        {NAV.map((group) => (
          <SidebarGroup key={group.id}>
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
                    location.pathname === item.to || location.pathname.startsWith(item.to + '/');

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
  const { t } = useTranslation();
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
  const siteTitle = getSiteTitle();
  const title = titleProp ?? (activeLabel ? t(activeLabel) : '');
  // Static: the trigger opens a three-option theme menu, so a state-dependent
  // enable/disable label would misdescribe it.
  const themeControlLabel = t(($) => $.common.toggle_theme);

  return (
    <SidebarProvider
      id="page-container"
      defaultOpen={sidebarDefaultOpen}
      className="text-foreground"
    >
      {/* Router-driven scroll management: new navigations start at the top
          (what the old scrollTo effect did) and back/forward restores the
          previous position instead of losing it. */}
      <ScrollRestoration />
      {navPending ? (
        <div
          data-testid="route-pending-bar"
          aria-hidden
          className="pointer-events-none fixed inset-x-0 top-0 z-50 h-0.5 overflow-hidden"
        >
          <div className="h-full w-full animate-pulse bg-primary motion-reduce:animate-none" />
        </div>
      ) : null}

      <AppSidebar siteTitle={siteTitle} email={user.email} />

      <SidebarInset>
        <header
          id="page-header"
          className="flex h-12 shrink-0 items-center gap-2 border-b border-border"
        >
          {/* Same mx-auto/max-w cap as the content wrapper below so the page
              title and the page body share a left edge at every width. */}
          <div className="mx-auto flex w-full max-w-6xl items-center gap-1 px-4 sm:px-6 lg:gap-2">
            {/* The desktop collapse control lives in the sidebar header; this
                trigger only opens the mobile drawer. */}
            <SidebarTrigger className="-ml-1 md:hidden" aria-label={t(($) => $.nav.toggle_nav)} />

            <Separator
              orientation="vertical"
              className="mx-2 data-[orientation=vertical]:h-4 md:hidden"
            />

            <h1
              data-slot="page-title"
              className="min-w-0 flex-1 truncate text-base font-medium text-foreground"
            >
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
                    <DropdownMenuRadioItem
                      value="system"
                      data-theme-option="system"
                      className="gap-2"
                    >
                      <Monitor className="size-4" />
                      {t(($) => $.common.theme_system)}
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem
                      value="light"
                      data-theme-option="light"
                      className="gap-2"
                    >
                      <Sun className="size-4" />
                      {t(($) => $.common.theme_light)}
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem value="dark" data-theme-option="dark" className="gap-2">
                      <Moon className="size-4" />
                      {t(($) => $.common.theme_dark)}
                    </DropdownMenuRadioItem>
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </header>

        {/* The named container lets page grids column-switch on the width they
            actually get (content area minus sidebar), not the viewport. */}
        {loading ? (
          <div id="main-container" className="@container/main flex-1">
            <LoadingState className="mx-auto max-w-6xl space-y-4 px-4 py-4 sm:px-6 md:py-6">
              <Skeleton className="h-7 w-44" aria-hidden />
              <SkeletonRows rows={4} />
            </LoadingState>
          </div>
        ) : (
          <div id="main-container" className="@container/main flex-1">
            <div className="mx-auto w-full max-w-6xl px-4 py-4 sm:px-6 md:py-6">
              <RouteBoundaryOutlet />
            </div>
          </div>
        )}
      </SidebarInset>
    </SidebarProvider>
  );
}

function AppLayoutFallback() {
  const { t } = useTranslation();
  // Keep the same stable surface hook as the hydrated shell.
  return (
    <div role="status" className="flex min-h-screen items-center justify-center bg-background">
      <Spinner className="size-6" />
      <span className="sr-only">{t(($) => $.common.loading)}</span>
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
