import { lazy, Suspense, type ComponentType, type ReactNode, type SVGProps } from 'react';
import { useTranslation } from 'react-i18next';
import type { SelectorParam } from 'i18next';
import { Link, useNavigate } from 'react-router';
import {
  AlertTriangle,
  List,
  ShoppingBag,
  SlidersHorizontal,
  TrendingUp,
  UserPlus,
  Users,
} from 'lucide-react';
import type { AdminFilter } from '@v2board/api-client';
import type { ServerRankItem, UserRankItem } from '@v2board/types';
import {
  useConfig,
  useQueueStats,
  useStat,
  useStatOrder,
  useStatServerLast,
  useStatServerToday,
  useStatUserLast,
  useStatUserToday,
} from '@/lib/queries';
import { Alert, AlertDescription } from '@v2board/ui/alert';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@v2board/ui/card';
import { PageShell } from '@v2board/ui/page';
import { ErrorState } from '@v2board/ui/error-state';
import type { RankingChartDatum } from '@/components/admin-chart';

const AdminChart = lazy(() => import('@/components/admin-chart'));

function formatCent(value?: number) {
  return value ? (value / 100).toFixed(2) : '0.00';
}

function buildRankingData<T extends ServerRankItem | UserRankItem>(
  data: readonly T[],
  getName: (item: T) => string,
): RankingChartDatum[] {
  return data.map((item) => ({ name: getName(item), total: item.total }));
}

const PENDING_COMMISSION_ORDER_FILTER: AdminFilter[] = [
  { key: 'status', condition: '=', value: '3' },
  { key: 'commission_status', condition: '=', value: '0' },
  { key: 'commission_balance', condition: '>', value: '0' },
];

interface DashboardShortcut {
  titleKey: SelectorParam;
  to: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}

const SHORTCUTS: DashboardShortcut[] = [
  {
    titleKey: ($) => $.admin.dashboard.shortcut_system_settings,
    to: '/config/system',
    icon: SlidersHorizontal,
  },
  { titleKey: ($) => $.admin.dashboard.shortcut_orders, to: '/order', icon: List },
  { titleKey: ($) => $.admin.dashboard.shortcut_plans, to: '/plan', icon: ShoppingBag },
  { titleKey: ($) => $.admin.dashboard.shortcut_users, to: '/user', icon: Users },
];

export default function DashboardPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queueStatus = useQueueStats();
  const stat = useStat();
  const order = useStatOrder();
  const serverLast = useStatServerLast();
  const serverToday = useStatServerToday();
  const userToday = useStatUserToday();
  const userLast = useStatUserLast();
  const config = useConfig('site');
  const currency = config.data?.site?.currency;
  const data = stat.data;
  // §6.1 (W9): queue-stats `status` is a real JSON boolean on the wire.
  const queueHealthy = queueStatus.data?.status === true;
  const dashboardQueries = [
    queueStatus,
    stat,
    order,
    serverLast,
    serverToday,
    userToday,
    userLast,
    config,
  ];
  const dashboardFailed = dashboardQueries.some((query) => query.isError);

  const goPendingCommissionOrders = () => {
    window.sessionStorage.setItem(
      'v2board-admin-order-filter',
      JSON.stringify(PENDING_COMMISSION_ORDER_FILTER),
    );
    void navigate('/order');
  };

  // §6.8 (W14): server_name is nullable when a rank row's node was deleted;
  // fall back to the stable id so the bar still renders identifiably.
  const serverRankName = (item: ServerRankItem) => item.server_name ?? `#${item.server_id}`;
  const serverTodayChart = buildRankingData(serverToday.data ?? [], serverRankName);
  const serverLastChart = buildRankingData(serverLast.data ?? [], serverRankName);
  const userTodayChart = buildRankingData(userToday.data ?? [], (item) => item.email);
  const userLastChart = buildRankingData(userLast.data ?? [], (item) => item.email);

  const rankCharts = [
    { title: t(($) => $.admin.dashboard.server_today_rank), data: serverTodayChart },
    { title: t(($) => $.admin.dashboard.server_last_rank), data: serverLastChart },
    { title: t(($) => $.admin.dashboard.user_today_rank), data: userTodayChart },
    { title: t(($) => $.admin.dashboard.user_last_rank), data: userLastChart },
  ];

  return (
    <PageShell data-testid="dashboard-page">
      {dashboardFailed ? (
        <ErrorState
          message={t(($) => $.admin.dashboard.load_failed)}
          onRetry={() => {
            for (const query of dashboardQueries) void query.refetch();
          }}
        />
      ) : null}
      {queueStatus.data && !queueHealthy ? (
        <Alert variant="destructive" data-testid="dashboard-queue-alert">
          <AlertTriangle className="size-4" />
          <AlertDescription>{t(($) => $.admin.dashboard.queue_alert)}</AlertDescription>
        </Alert>
      ) : null}

      {data?.ticket_pending_total ? (
        <Alert variant="destructive" data-testid="dashboard-ticket-alert">
          <AlertTriangle className="size-4" />
          <AlertDescription className="flex flex-wrap items-center gap-1">
            <span>
              {t(($) => $.admin.dashboard.ticket_pending, { count: data.ticket_pending_total })}
            </span>
            <Button asChild variant="link" className="h-auto p-0 text-sm text-destructive">
              <Link to="/ticket">{t(($) => $.admin.dashboard.handle_now)}</Link>
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}

      {data?.commission_pending_total ? (
        <Alert variant="destructive" data-testid="dashboard-commission-alert">
          <AlertTriangle className="size-4" />
          <AlertDescription className="flex flex-wrap items-center gap-1">
            <span>
              {t(($) => $.admin.dashboard.commission_pending, {
                count: data.commission_pending_total,
              })}
            </span>
            <Button
              variant="link"
              className="h-auto p-0 text-sm text-destructive"
              onClick={goPendingCommissionOrders}
              data-testid="dashboard-commission-action"
            >
              {t(($) => $.admin.dashboard.handle_now)}
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}

      <div className="grid gap-4 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
        {SHORTCUTS.map((shortcut) => (
          <Button
            asChild
            key={shortcut.to}
            variant="outline"
            className="h-auto w-full flex-col gap-3 rounded-xl bg-card px-6 py-6 text-card-foreground hover:bg-accent hover:text-accent-foreground"
          >
            <Link to={shortcut.to}>
              <shortcut.icon className="size-6 text-primary" />
              <span className="text-sm font-medium">{t(shortcut.titleKey)}</span>
            </Link>
          </Button>
        ))}
      </div>

      <div className="grid gap-4 @2xl/main:grid-cols-3">
        <StatCard
          icon={<Users className="size-5" />}
          label={t(($) => $.admin.dashboard.online_users)}
          value={data?.online_user || '0'}
        />
        <StatCard
          icon={<TrendingUp className="size-5" />}
          label={t(($) => $.admin.dashboard.day_income)}
          value={
            <>
              {formatCent(data?.day_income)}
              <span className="ml-1 text-sm font-medium text-muted-foreground">{currency}</span>
            </>
          }
        />
        <StatCard
          icon={<UserPlus className="size-5" />}
          label={t(($) => $.admin.dashboard.day_register)}
          value={data?.day_register_total || '0'}
        />
      </div>

      <Card>
        <CardContent className="grid gap-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
          <MiniStat
            label={t(($) => $.admin.dashboard.month_income)}
            value={`${formatCent(data?.month_income)} ${currency ?? ''}`}
          />
          <MiniStat
            label={t(($) => $.admin.dashboard.last_month_income)}
            value={`${formatCent(data?.last_month_income)} ${currency ?? ''}`}
          />
          <MiniStat
            label={t(($) => $.admin.dashboard.last_month_commission_payout)}
            value={`${formatCent(data?.commission_last_month_payout)} ${currency ?? ''}`}
          />
          <MiniStat
            label={t(($) => $.admin.dashboard.month_register)}
            value={data?.month_register_total || '-'}
          />
        </CardContent>
      </Card>

      <Card className="min-w-0 overflow-hidden">
        <CardHeader>
          <CardTitle>{t(($) => $.admin.dashboard.order_stat)}</CardTitle>
        </CardHeader>
        <CardContent className="min-w-0">
          <ChartSuspense label={t(($) => $.admin.dashboard.order_chart_label)}>
            <AdminChart
              kind="order"
              data={order.data ?? []}
              label={t(($) => $.admin.dashboard.order_chart_label)}
              className="h-[360px] w-full min-w-0"
            />
          </ChartSuspense>
        </CardContent>
      </Card>

      <div className="grid gap-4 @4xl/main:grid-cols-2">
        {rankCharts.map((chart) => (
          <Card key={chart.title} className="min-w-0 overflow-hidden">
            <CardHeader>
              <CardTitle>{chart.title}</CardTitle>
            </CardHeader>
            <CardContent className="min-w-0">
              <ChartSuspense label={chart.title}>
                <AdminChart
                  kind="ranking"
                  data={chart.data}
                  label={chart.title}
                  className="h-[360px] w-full min-w-0"
                />
              </ChartSuspense>
            </CardContent>
          </Card>
        ))}
      </div>
    </PageShell>
  );
}

function ChartSuspense({ label, children }: { label: string; children: ReactNode }) {
  const { t } = useTranslation();
  return (
    <Suspense
      fallback={
        <div
          className="h-[360px] w-full animate-pulse rounded-md bg-muted motion-reduce:animate-none"
          role="status"
          aria-label={t(($) => $.admin.dashboard.chart_loading, { label })}
        />
      }
    >
      {children}
    </Suspense>
  );
}

function StatCard({ icon, label, value }: { icon: ReactNode; label: string; value: ReactNode }) {
  return (
    <Card>
      <CardContent className="flex items-start justify-between gap-3">
        <div className="space-y-2">
          <div className="text-sm text-muted-foreground">{label}</div>
          <div className="text-3xl font-semibold tracking-tight text-foreground">{value}</div>
        </div>
        <div className="flex size-10 items-center justify-center rounded-md bg-muted text-muted-foreground">
          {icon}
        </div>
      </CardContent>
    </Card>
  );
}

function MiniStat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="space-y-1">
      <div className="text-xl font-semibold tracking-tight text-foreground">{value}</div>
      <div className="text-sm text-muted-foreground">{label}</div>
    </div>
  );
}
