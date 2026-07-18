import { lazy, Suspense, type ReactNode } from 'react';
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
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
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

const SHORTCUTS = [
  { title: '系统设置', to: '/config/system', icon: SlidersHorizontal },
  { title: '订单管理', to: '/order', icon: List },
  { title: '订阅管理', to: '/plan', icon: ShoppingBag },
  { title: '用户管理', to: '/user', icon: Users },
];

export default function DashboardPage() {
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
    { title: '今日节点流量排行', data: serverTodayChart },
    { title: '昨日节点流量排行', data: serverLastChart },
    { title: '今日用户流量排行', data: userTodayChart },
    { title: '昨日用户流量排行', data: userLastChart },
  ];

  return (
    <PageShell data-testid="dashboard-page">
      {dashboardFailed ? (
        <ErrorState
          message="仪表盘数据加载失败"
          onRetry={() => {
            for (const query of dashboardQueries) void query.refetch();
          }}
        />
      ) : null}
      {queueStatus.data && !queueHealthy ? (
        <Alert variant="destructive" data-testid="dashboard-queue-alert">
          <AlertTriangle className="size-4" />
          <AlertDescription>当前队列服务运行异常，可能会导致业务无法使用。</AlertDescription>
        </Alert>
      ) : null}

      {data?.ticket_pending_total ? (
        <Alert variant="destructive" data-testid="dashboard-ticket-alert">
          <AlertTriangle className="size-4" />
          <AlertDescription className="flex flex-wrap items-center gap-1">
            <span>有 {data.ticket_pending_total} 条工单等待处理</span>
            <Button asChild variant="link" className="h-auto p-0 text-sm text-destructive">
              <Link to="/ticket">立即处理</Link>
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}

      {data?.commission_pending_total ? (
        <Alert variant="destructive" data-testid="dashboard-commission-alert">
          <AlertTriangle className="size-4" />
          <AlertDescription className="flex flex-wrap items-center gap-1">
            <span>有 {data.commission_pending_total} 笔佣金等待确认</span>
            <Button
              variant="link"
              className="h-auto p-0 text-sm text-destructive"
              onClick={goPendingCommissionOrders}
              data-testid="dashboard-commission-action"
            >
              立即处理
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
              <span className="text-sm font-medium">{shortcut.title}</span>
            </Link>
          </Button>
        ))}
      </div>

      <div className="grid gap-4 @2xl/main:grid-cols-3">
        <StatCard
          icon={<Users className="size-5" />}
          label="在线人数"
          value={data?.online_user || '0'}
        />
        <StatCard
          icon={<TrendingUp className="size-5" />}
          label="今日收入"
          value={
            <>
              {formatCent(data?.day_income)}
              <span className="ml-1 text-sm font-medium text-muted-foreground">{currency}</span>
            </>
          }
        />
        <StatCard
          icon={<UserPlus className="size-5" />}
          label="实时注册"
          value={data?.day_register_total || '0'}
        />
      </div>

      <Card>
        <CardContent className="grid gap-6 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
          <MiniStat
            label="本月收入"
            value={`${formatCent(data?.month_income)} ${currency ?? ''}`}
          />
          <MiniStat
            label="上月收入"
            value={`${formatCent(data?.last_month_income)} ${currency ?? ''}`}
          />
          <MiniStat
            label="上月佣金支出"
            value={`${formatCent(data?.commission_last_month_payout)} ${currency ?? ''}`}
          />
          <MiniStat label="本月新增用户" value={data?.month_register_total || '-'} />
        </CardContent>
      </Card>

      <Card className="min-w-0 overflow-hidden">
        <CardHeader>
          <CardTitle>订单统计</CardTitle>
        </CardHeader>
        <CardContent className="min-w-0">
          <ChartSuspense label="订单统计折线图">
            <AdminChart
              kind="order"
              data={order.data ?? []}
              label="订单统计折线图"
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
  return (
    <Suspense
      fallback={
        <div
          className="h-[360px] w-full animate-pulse rounded-md bg-muted motion-reduce:animate-none"
          role="status"
          aria-label={`${label}加载中`}
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
