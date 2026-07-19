import { Suspense, useState, type ComponentType, type SVGProps } from 'react';
import { Link, ScrollRestoration, useLocation } from 'react-router';
import { useTranslation } from 'react-i18next';
import type { SelectorParam } from 'i18next';
import { useSuspenseQuery } from '@tanstack/react-query';
import {
  BarChart3,
  BookOpen,
  ClipboardList,
  CreditCard,
  Gauge,
  Gift,
  Layers,
  List,
  MessageSquare,
  Monitor,
  Moon,
  Route as RouteIcon,
  ShieldAlert,
  ShoppingBag,
  SlidersHorizontal,
  Star,
  Sun,
  Ticket,
  Users,
  Wrench,
} from 'lucide-react';
import { readCookie } from '@v2board/i18n';
import { adminSessionQueryOptions } from '@/lib/session-queries';
import { useAccountMfa } from '@/lib/queries';
import { MfaDialog } from '@/components/mfa-dialog';
import { Card, CardContent } from '@/components/ui/card';
import { getAdminTitle } from '@/lib/runtime-config';
import {
  setThemePreference,
  useDarkMode,
  useThemePreference,
  type ThemePreference,
} from '@/lib/dark-mode';
import { RouteBoundaryOutlet } from '@/components/route-error-boundary';
import { AdminNavUser } from './admin-nav-user';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
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

type ShellIcon = ComponentType<SVGProps<SVGSVGElement>>;

interface NavItem {
  to: string;
  titleKey: SelectorParam;
  icon: ShellIcon;
}

interface NavGroup {
  id: string;
  titleKey?: SelectorParam;
  items: NavItem[];
}

const NAV: NavGroup[] = [
  {
    id: 'primary',
    items: [{ to: '/dashboard', titleKey: ($) => $.admin.nav.dashboard, icon: Gauge }],
  },
  {
    id: 'settings',
    titleKey: ($) => $.admin.nav.group_settings,
    items: [
      { to: '/config/system', titleKey: ($) => $.admin.nav.system_config, icon: SlidersHorizontal },
      { to: '/config/payment', titleKey: ($) => $.admin.nav.payment_config, icon: CreditCard },
    ],
  },
  {
    id: 'server',
    titleKey: ($) => $.admin.nav.group_server,
    items: [
      { to: '/server/manage', titleKey: ($) => $.admin.nav.server_manage, icon: Layers },
      { to: '/server/group', titleKey: ($) => $.admin.nav.server_group, icon: Wrench },
      { to: '/server/route', titleKey: ($) => $.admin.nav.server_route, icon: RouteIcon },
    ],
  },
  {
    id: 'finance',
    titleKey: ($) => $.admin.nav.group_finance,
    items: [
      { to: '/plan', titleKey: ($) => $.admin.nav.plans, icon: ShoppingBag },
      { to: '/order', titleKey: ($) => $.admin.nav.orders, icon: List },
      { to: '/coupon', titleKey: ($) => $.admin.nav.coupons, icon: Gift },
      { to: '/giftcard', titleKey: ($) => $.admin.nav.giftcards, icon: Star },
    ],
  },
  {
    id: 'user',
    titleKey: ($) => $.admin.nav.group_user,
    items: [
      { to: '/user', titleKey: ($) => $.admin.nav.users, icon: Users },
      { to: '/notice', titleKey: ($) => $.admin.nav.notices, icon: MessageSquare },
      { to: '/ticket', titleKey: ($) => $.admin.nav.tickets, icon: Ticket },
      { to: '/knowledge', titleKey: ($) => $.admin.nav.knowledge, icon: BookOpen },
    ],
  },
  {
    id: 'metrics',
    titleKey: ($) => $.admin.nav.group_metrics,
    items: [
      { to: '/queue', titleKey: ($) => $.admin.nav.queue, icon: BarChart3 },
      { to: '/audit', titleKey: ($) => $.admin.nav.audit, icon: ClipboardList },
    ],
  },
];

const SIDEBAR_STATE_COOKIE = 'sidebar_state';

function readSidebarDefaultOpen(): boolean {
  if (typeof document === 'undefined') return true;
  return readCookie(SIDEBAR_STATE_COOKIE) !== 'false';
}

function findActiveTitleKey(pathname: string): SelectorParam | undefined {
  for (const group of NAV) {
    for (const item of group.items) {
      if (pathname === item.to || pathname.startsWith(item.to + '/')) return item.titleKey;
    }
  }
  return undefined;
}

// The sidebar lives inside SidebarProvider, so it owns the router hooks and the
// mobile-sheet close-on-navigate that AdminLayout (which renders the provider)
// cannot reach through useSidebar.
function AdminSidebar({ siteTitle, email }: { siteTitle: string; email: string }) {
  const { t } = useTranslation();
  const location = useLocation();
  const { setOpenMobile } = useSidebar();

  const closeMobile = () => setOpenMobile(false);

  return (
    <Sidebar
      id="sidebar"
      variant="sidebar"
      collapsible="icon"
      sheetTitle={t(($) => $.admin.nav.sheet_title)}
      sheetDescription={t(($) => $.admin.nav.sheet_description)}
    >
      <SidebarHeader>
        <div className="flex items-center justify-between gap-1">
          <Link
            to="/dashboard"
            onClick={closeMobile}
            className="min-w-0 truncate rounded-md px-2 py-1 text-left text-base font-semibold text-sidebar-foreground outline-hidden group-data-[collapsible=icon]:hidden focus-visible:ring-2 focus-visible:ring-sidebar-ring"
          >
            {siteTitle}
          </Link>
          <SidebarTrigger
            className="size-8 shrink-0"
            aria-label={t(($) => $.admin.nav.toggle_nav)}
          />
        </div>
      </SidebarHeader>

      <SidebarContent role="navigation" aria-label={t(($) => $.admin.nav.primary_nav)}>
        {NAV.map((group) => (
          <SidebarGroup key={group.id}>
            {group.titleKey ? (
              <SidebarGroupLabel className="group-data-[collapsible=icon]:mt-0">
                {t(group.titleKey)}
              </SidebarGroupLabel>
            ) : null}
            <SidebarGroupContent>
              <SidebarMenu>
                {group.items.map((item) => {
                  const active =
                    location.pathname === item.to || location.pathname.startsWith(item.to + '/');
                  return (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton asChild isActive={active} tooltip={t(item.titleKey)}>
                        <Link
                          to={item.to}
                          aria-current={active ? 'page' : undefined}
                          onClick={closeMobile}
                        >
                          <item.icon />
                          <span>{t(item.titleKey)}</span>
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
        <AdminNavUser email={email} />
      </SidebarFooter>
    </Sidebar>
  );
}

/**
 * §6.10 `admin_mfa_force`: while the deployment demands an enabled factor and
 * this account has none, the backend answers every route outside the
 * `account/mfa` family with 403 `mfa_enrollment_required` — so instead of a
 * wall of failed requests, the shell swaps the routed page for this
 * enrollment prompt until the factor is confirmed.
 */
function MfaEnrollmentGate() {
  const { t } = useTranslation();
  const [dialogOpen, setDialogOpen] = useState(false);
  return (
    <Card className="mx-auto max-w-lg" data-testid="mfa-enrollment-gate">
      <CardContent className="flex flex-col items-center gap-4 py-10 text-center">
        <ShieldAlert className="size-10 text-destructive" aria-hidden />
        <div className="space-y-1">
          <h2 className="text-lg font-semibold">{t(($) => $.admin.auth.mfa_required_title)}</h2>
          <p className="text-sm text-muted-foreground">
            {t(($) => $.admin.auth.mfa_required_description)}
          </p>
        </div>
        <Button type="button" onClick={() => setDialogOpen(true)}>
          {t(($) => $.admin.auth.mfa_setup_now)}
        </Button>
      </CardContent>
      <MfaDialog open={dialogOpen} onOpenChange={setDialogOpen} />
    </Card>
  );
}

function AdminLayoutContent() {
  const { t } = useTranslation();
  const location = useLocation();
  const darkMode = useDarkMode();
  const themePreference = useThemePreference();
  const [sidebarDefaultOpen] = useState(readSidebarDefaultOpen);
  const { data: userInfo } = useSuspenseQuery({
    ...adminSessionQueryOptions.userInfo(),
    refetchOnMount: false,
  });
  const accountMfa = useAccountMfa();
  const mfaBlocked =
    accountMfa.data?.totp_required === true && accountMfa.data.totp_enabled === false;
  const siteTitle = getAdminTitle();
  const titleKey = findActiveTitleKey(location.pathname);

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
      <AdminSidebar siteTitle={siteTitle} email={userInfo.email} />

      <SidebarInset>
        <header
          id="page-header"
          className="flex h-12 shrink-0 items-center gap-2 border-b border-border"
        >
          <div className="flex w-full items-center gap-1 px-4 sm:px-6 lg:gap-2">
            <SidebarTrigger
              className="-ml-1 md:hidden"
              aria-label={t(($) => $.admin.nav.toggle_nav)}
            />
            <Separator
              orientation="vertical"
              className="mx-2 data-[orientation=vertical]:h-4 md:hidden"
            />
            <h1
              data-slot="page-title"
              className="min-w-0 flex-1 truncate text-base font-medium text-foreground"
            >
              {titleKey ? t(titleKey) : null}
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
                    aria-label={t(($) => $.common.toggle_theme)}
                    title={t(($) => $.common.toggle_theme)}
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

        {/* The named container lets page grids column-switch on the width
            they actually get (content area minus sidebar), not the viewport. */}
        <div id="main-container" className="@container/main flex-1">
          <div className="mx-auto w-full max-w-7xl px-4 py-4 sm:px-6 md:py-6">
            {mfaBlocked ? <MfaEnrollmentGate /> : <RouteBoundaryOutlet />}
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

function AdminLayoutFallback() {
  const { t } = useTranslation();
  return (
    <div role="status" className="flex min-h-screen items-center justify-center bg-background">
      <Spinner className="size-6" />
      <span className="sr-only">{t(($) => $.admin.nav.loading)}</span>
    </div>
  );
}

export function AdminLayout() {
  return (
    <Suspense fallback={<AdminLayoutFallback />}>
      <AdminLayoutContent />
    </Suspense>
  );
}
