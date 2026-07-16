import { Suspense, useState, type ComponentType, type SVGProps } from 'react';
import { Link, ScrollRestoration, useLocation } from 'react-router';
import { useSuspenseQuery } from '@tanstack/react-query';
import {
  BarChart3,
  BookOpen,
  CreditCard,
  Gauge,
  Gift,
  Layers,
  List,
  MessageSquare,
  Monitor,
  Moon,
  Route as RouteIcon,
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
  title: string;
  icon: ShellIcon;
}

interface NavGroup {
  title?: string;
  items: NavItem[];
}

const NAV: NavGroup[] = [
  {
    items: [{ to: '/dashboard', title: '仪表盘', icon: Gauge }],
  },
  {
    title: '设置',
    items: [
      { to: '/config/system', title: '系统配置', icon: SlidersHorizontal },
      { to: '/config/payment', title: '支付配置', icon: CreditCard },
    ],
  },
  {
    title: '服务器',
    items: [
      { to: '/server/manage', title: '节点管理', icon: Layers },
      { to: '/server/group', title: '权限组管理', icon: Wrench },
      { to: '/server/route', title: '路由管理', icon: RouteIcon },
    ],
  },
  {
    title: '财务',
    items: [
      { to: '/plan', title: '订阅管理', icon: ShoppingBag },
      { to: '/order', title: '订单管理', icon: List },
      { to: '/coupon', title: '优惠券管理', icon: Gift },
      { to: '/giftcard', title: '礼品卡管理', icon: Star },
    ],
  },
  {
    title: '用户',
    items: [
      { to: '/user', title: '用户管理', icon: Users },
      { to: '/notice', title: '公告管理', icon: MessageSquare },
      { to: '/ticket', title: '工单管理', icon: Ticket },
      { to: '/knowledge', title: '知识库管理', icon: BookOpen },
    ],
  },
  {
    title: '指标',
    items: [{ to: '/queue', title: '队列监控', icon: BarChart3 }],
  },
];

const SIDEBAR_STATE_COOKIE = 'sidebar_state';

function readSidebarDefaultOpen(): boolean {
  if (typeof document === 'undefined') return true;
  return readCookie(SIDEBAR_STATE_COOKIE) !== 'false';
}

function findActiveTitle(pathname: string): string {
  for (const group of NAV) {
    for (const item of group.items) {
      if (pathname === item.to || pathname.startsWith(item.to + '/')) return item.title;
    }
  }
  return '';
}

// The sidebar lives inside SidebarProvider, so it owns the router hooks and the
// mobile-sheet close-on-navigate that AdminLayout (which renders the provider)
// cannot reach through useSidebar.
function AdminSidebar({ siteTitle, email }: { siteTitle: string; email: string }) {
  const location = useLocation();
  const { setOpenMobile } = useSidebar();

  const closeMobile = () => setOpenMobile(false);

  return (
    <Sidebar
      id="sidebar"
      variant="sidebar"
      collapsible="icon"
      sheetTitle="导航"
      sheetDescription="管理中心导航"
    >
      <SidebarHeader>
        <div className="flex items-center justify-between gap-1">
          <Link
            to="/dashboard"
            onClick={closeMobile}
            className="min-w-0 truncate rounded-md px-2 py-1 text-left text-base font-semibold text-sidebar-foreground outline-hidden focus-visible:ring-2 focus-visible:ring-sidebar-ring group-data-[collapsible=icon]:hidden"
          >
            {siteTitle}
          </Link>
          <SidebarTrigger className="size-8 shrink-0" aria-label="切换导航" />
        </div>
      </SidebarHeader>

      <SidebarContent role="navigation" aria-label="主导航">
        {NAV.map((group, groupIndex) => (
          <SidebarGroup key={group.title ?? `group-${groupIndex}`}>
            {group.title ? (
              <SidebarGroupLabel className="group-data-[collapsible=icon]:mt-0">
                {group.title}
              </SidebarGroupLabel>
            ) : null}
            <SidebarGroupContent>
              <SidebarMenu>
                {group.items.map((item) => {
                  const active =
                    location.pathname === item.to || location.pathname.startsWith(item.to + '/');
                  return (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton asChild isActive={active} tooltip={item.title}>
                        <Link
                          to={item.to}
                          aria-current={active ? 'page' : undefined}
                          onClick={closeMobile}
                        >
                          <item.icon />
                          <span>{item.title}</span>
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

function AdminLayoutContent() {
  const location = useLocation();
  const darkMode = useDarkMode();
  const themePreference = useThemePreference();
  const [sidebarDefaultOpen] = useState(readSidebarDefaultOpen);
  const { data: userInfo } = useSuspenseQuery({
    ...adminSessionQueryOptions.userInfo(),
    refetchOnMount: false,
  });
  const siteTitle = getAdminTitle();
  const title = findActiveTitle(location.pathname);

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
            <SidebarTrigger className="-ml-1 md:hidden" aria-label="切换导航" />
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
                    aria-label="切换主题"
                    title="切换主题"
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
                      跟随系统
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem
                      value="light"
                      data-theme-option="light"
                      className="gap-2"
                    >
                      <Sun className="size-4" />
                      浅色
                    </DropdownMenuRadioItem>
                    <DropdownMenuRadioItem value="dark" data-theme-option="dark" className="gap-2">
                      <Moon className="size-4" />
                      深色
                    </DropdownMenuRadioItem>
                  </DropdownMenuRadioGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </header>

        <div id="main-container" className="flex-1">
          <div className="mx-auto w-full max-w-7xl px-4 py-4 sm:px-6 md:py-6">
            <RouteBoundaryOutlet />
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

function AdminLayoutFallback() {
  return (
    <div
      role="status"
      className="flex min-h-screen items-center justify-center bg-background"
    >
      <Spinner className="size-6" />
      <span className="sr-only">正在加载</span>
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
