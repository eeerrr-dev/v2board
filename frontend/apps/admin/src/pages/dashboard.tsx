import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import * as echarts from 'echarts';
import type { ECharts, EChartsOption } from 'echarts';
import { AlertTriangle, List, ShoppingBag, SlidersHorizontal, TrendingUp, UserPlus, Users } from 'lucide-react';
import type { AdminFilter } from '@v2board/api-client';
import type { OrderStatPoint, ServerRankItem, UserRankItem } from '@v2board/types';
import {
  useConfig,
  useStat,
  useStatOrder,
  useStatServerLast,
  useStatServerToday,
  useStatUserLast,
  useStatUserToday,
} from '@/lib/queries';
import { getAdminApiBaseUrl } from '@/lib/legacy-settings';
import { useDarkMode } from '@/lib/dark-mode';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';

function formatCent(value?: number) {
  return value ? (value / 100).toFixed(2) : '0.00';
}

// Token-aware echarts host. The legacy darkreader canvas baseline and manual
// window-resize wiring are retired: dark mode re-inits with the built-in dark
// theme (background kept transparent so the Card fill shows through) and a
// ResizeObserver keeps the chart sized to its Card.
function EChart({ option, className }: { option: EChartsOption; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ECharts | undefined>(undefined);
  const dark = useDarkMode();

  useEffect(() => {
    if (!ref.current) return undefined;
    const chart = echarts.init(ref.current, dark ? 'dark' : undefined, { renderer: 'svg' });
    chartRef.current = chart;
    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => chart.resize());
      observer.observe(ref.current);
    }
    return () => {
      observer?.disconnect();
      chart.dispose();
      chartRef.current = undefined;
    };
  }, [dark]);

  useEffect(() => {
    chartRef.current?.setOption({ backgroundColor: 'transparent', ...option }, true);
  }, [option]);

  return <div ref={ref} className={className} />;
}

function buildOrderOption(data: OrderStatPoint[]): EChartsOption {
  const legend: string[] = [];
  const xAxis: string[] = [];
  const series: { name: string; type: 'line'; smooth: boolean; data: number[] }[] = [];
  data.forEach((point) => {
    if (!legend.includes(point.type)) legend.push(point.type);
    if (!xAxis.includes(point.date)) xAxis.push(point.date);
    const existing = series.find((item) => item.name === point.type);
    if (existing) existing.data.push(point.value);
    else series.push({ name: point.type, type: 'line', smooth: true, data: [point.value] });
  });
  return {
    tooltip: { trigger: 'axis' },
    legend: { data: legend, left: '0', z: 4 },
    grid: { left: '1%', right: '1%', bottom: '3%', containLabel: true },
    xAxis: { type: 'category', boundaryGap: false, data: xAxis },
    yAxis: { type: 'value' },
    series,
  };
}

function buildRankOption<T extends ServerRankItem | UserRankItem>(
  data: T[],
  getName: (item: T) => string,
): EChartsOption {
  const names: string[] = [];
  const totals: number[] = [];
  [...data].reverse().forEach((item) => {
    names.push(getName(item));
    totals.push(item.total);
  });
  return {
    tooltip: {
      trigger: 'axis',
      formatter: (params) => `${(params as Array<{ value: unknown }>)[0]!.value} GB`,
    },
    grid: { top: '1%', left: '1%', right: '1%', bottom: '3%', containLabel: true },
    xAxis: { type: 'value' },
    yAxis: { type: 'category', data: names },
    series: [{ data: totals, type: 'bar' }],
  };
}

function useQueueStatus() {
  const [queueStatus, setQueueStatus] = useState<string>();

  useEffect(() => {
    const origin = new URL(getAdminApiBaseUrl()).origin;
    fetch(`${origin}/monitor/api/stats`)
      .then((response) => response.json())
      .then((data) => setQueueStatus(data?.status));
  }, []);

  return queueStatus;
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
  const queueStatus = useQueueStatus();
  const stat = useStat();
  const order = useStatOrder();
  const serverLast = useStatServerLast();
  const serverToday = useStatServerToday();
  const userToday = useStatUserToday();
  const userLast = useStatUserLast();
  const config = useConfig('site');
  const currency = config.data?.site?.currency;
  const data = stat.data;

  const goPendingCommissionOrders = () => {
    window.sessionStorage.setItem(
      'v2board-admin-order-filter',
      JSON.stringify(PENDING_COMMISSION_ORDER_FILTER),
    );
    navigate('/order');
  };

  const orderOption = useMemo(() => buildOrderOption(order.data ?? []), [order.data]);
  const serverTodayOption = useMemo(
    () => buildRankOption(serverToday.data ?? [], (item) => item.server_name),
    [serverToday.data],
  );
  const serverLastOption = useMemo(
    () => buildRankOption(serverLast.data ?? [], (item) => item.server_name),
    [serverLast.data],
  );
  const userTodayOption = useMemo(
    () => buildRankOption(userToday.data ?? [], (item) => item.email),
    [userToday.data],
  );
  const userLastOption = useMemo(
    () => buildRankOption(userLast.data ?? [], (item) => item.email),
    [userLast.data],
  );

  const rankCharts = [
    { title: '今日节点流量排行', option: serverTodayOption },
    { title: '昨日节点流量排行', option: serverLastOption },
    { title: '今日用户流量排行', option: userTodayOption },
    { title: '昨日用户流量排行', option: userLastOption },
  ];

  return (
    <PageShell data-testid="dashboard-page">
      {queueStatus && queueStatus !== 'running' ? (
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
            <Button
              variant="link"
              className="h-auto p-0 text-sm text-destructive"
              onClick={() => navigate('/ticket')}
            >
              立即处理
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

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {SHORTCUTS.map((shortcut) => (
          <button
            key={shortcut.to}
            type="button"
            onClick={() => navigate(shortcut.to)}
            className="flex flex-col items-center gap-3 rounded-xl border border-border bg-card px-6 py-6 text-card-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            <shortcut.icon className="size-6 text-primary" />
            <span className="text-sm font-medium">{shortcut.title}</span>
          </button>
        ))}
      </div>

      <div className="grid gap-4 sm:grid-cols-3">
        <StatCard icon={<Users className="size-5" />} label="在线人数" value={data?.online_user || '0'} />
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
        <CardContent className="grid gap-6 sm:grid-cols-2 xl:grid-cols-4">
          <MiniStat label="本月收入" value={`${formatCent(data?.month_income)} ${currency ?? ''}`} />
          <MiniStat label="上月收入" value={`${formatCent(data?.last_month_income)} ${currency ?? ''}`} />
          <MiniStat
            label="上月佣金支出"
            value={`${formatCent(data?.commission_last_month_payout)} ${currency ?? ''}`}
          />
          <MiniStat label="本月新增用户" value={data?.month_register_total || '-'} />
        </CardContent>
      </Card>

      <Card className="overflow-hidden">
        <CardHeader>
          <CardTitle>订单统计</CardTitle>
        </CardHeader>
        <CardContent>
          <EChart option={orderOption} className="h-[360px] w-full" />
        </CardContent>
      </Card>

      <div className="grid gap-4 xl:grid-cols-2">
        {rankCharts.map((chart) => (
          <Card key={chart.title} className="overflow-hidden">
            <CardHeader>
              <CardTitle>{chart.title}</CardTitle>
            </CardHeader>
            <CardContent>
              <EChart option={chart.option} className="h-[360px] w-full" />
            </CardContent>
          </Card>
        ))}
      </div>
    </PageShell>
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
