import { useState } from 'react';
import { Link } from 'react-router';
import { Trans, useTranslation } from 'react-i18next';
import type { SelectorParam } from 'i18next';
import {
  AlertCircle,
  Bell,
  BookOpen,
  CheckCircle2,
  Headphones,
  LinkIcon,
  Package,
  Plus,
  RefreshCcw,
  ShoppingBag,
  Smartphone,
} from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { PageShell } from '@/components/ui/page';
import { Progress } from '@/components/ui/progress';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { DashboardNoticeCarousel } from './dashboard-notice-carousel';
import {
  DashboardConfirmDialog,
  DashboardSubscribeDialog,
  type DashboardConfirmAction,
} from './dashboard-dialogs';
import { deriveDashboardSubscription } from './dashboard-subscription';
import { useCommConfig, useNotices, useSubscribe, useUserStat } from '@/lib/queries';
import { formatBackendDateSlash, formatBytes } from '@v2board/config/format';
import { cn } from '@/lib/cn';
import { getSiteTitle } from '@/lib/runtime-config';

interface ShortcutBase {
  id: string;
  icon: typeof BookOpen;
  titleKey: SelectorParam;
  descKey: SelectorParam;
  appendSiteTitle?: boolean;
}

type Shortcut = ShortcutBase &
  ({ to: string; onClick?: never } | { onClick: () => void; to?: never });

export default function DashboardPage() {
  const { t } = useTranslation();
  const siteTitle = getSiteTitle();
  const subscribe = useSubscribe();
  const stat = useUserStat();
  const notices = useNotices();
  const commConfig = useCommConfig();
  const [confirmAction, setConfirmAction] = useState<DashboardConfirmAction>(null);
  const [subscribeOpen, setSubscribeOpen] = useState(false);

  const pendingOrderCount = stat.data?.pending_orders ?? 0;
  const openTicketCount = stat.data?.pending_tickets ?? 0;
  const sub = subscribe.data;
  const hasSubscribeData = Boolean(sub?.email);
  const hasPlan = Boolean(sub?.plan_id);
  const vm = deriveDashboardSubscription(sub);
  const noticeList = notices.data ?? [];
  const subscribeUrl = typeof sub?.subscribe_url === 'string' ? sub.subscribe_url : '';
  const subscription = sub!;

  const requestResetPackage = () => setConfirmAction('reset-package');
  const requestNewPeriod = () => setConfirmAction('new-period');
  const dashboardDataFailed = stat.isError || notices.isError || commConfig.isError;

  const retryDashboardData = () => {
    if (stat.isError) void stat.refetch();
    if (notices.isError) void notices.refetch();
    if (commConfig.isError) void commConfig.refetch();
  };

  const shortcuts: Shortcut[] = [
    {
      id: 'tutorial',
      to: '/knowledge',
      icon: BookOpen,
      titleKey: $ => $.dashboard.shortcut_tutorial,
      descKey: $ => $.dashboard.shortcut_tutorial_desc,
      appendSiteTitle: true,
    },
    {
      id: 'subscribe',
      icon: LinkIcon,
      titleKey: $ => $.dashboard.shortcut_one_click,
      descKey: $ => $.dashboard.shortcut_one_click_desc,
      onClick: () => setSubscribeOpen(true),
    },
    {
      id: 'buy',
      to: vm.canRenew ? `/plan/${sub?.plan_id}` : '/plan',
      icon: vm.canRenew ? RefreshCcw : ShoppingBag,
      titleKey: vm.canRenew
        ? $ => $.dashboard.renew_subscribe
        : $ => $.dashboard.shortcut_buy,
      descKey: vm.canRenew
        ? $ => $.dashboard.shortcut_renew_desc
        : $ => $.dashboard.shortcut_buy_desc,
    },
    {
      id: 'support',
      to: '/ticket',
      icon: Headphones,
      titleKey: $ => $.dashboard.shortcut_problem,
      descKey: $ => $.dashboard.shortcut_problem_desc,
    },
  ];

  return (
    <PageShell data-testid="dashboard-page">
      {dashboardDataFailed ? (
        <ErrorState onRetry={retryDashboardData} data-testid="dashboard-data-error" />
      ) : null}
      {(pendingOrderCount > 0 || openTicketCount > 0 || vm.shouldShowTrafficAlert) && (
        <div data-testid="dashboard-alerts" className="grid gap-3">
          {pendingOrderCount > 0 && (
            <Alert
              data-testid="dashboard-alert"
              data-alert-kind="danger"
              className="border-destructive/25 bg-destructive/5 text-foreground"
              role="alert"
            >
              <AlertCircle className="size-4 text-destructive" />
              <AlertDescription className="sm:flex sm:flex-row sm:items-center sm:gap-2">
                <span>{t($ => $.dashboard.alert_pending_order)}</span>
                <Link
                  to="/order"
                  data-testid="dashboard-alert-link"
                  className="font-medium text-foreground underline-offset-4 hover:underline"
                >
                  {t($ => $.order.pay_now)}
                </Link>
              </AlertDescription>
            </Alert>
          )}
          {openTicketCount > 0 && (
            <Alert
              data-testid="dashboard-alert"
              data-alert-kind="warning"
              className="border-warning/30 bg-warning/10 text-foreground"
              role="alert"
            >
              <Bell className="size-4 text-warning" />
              <AlertDescription className="sm:flex sm:flex-row sm:items-center sm:gap-2">
                <span>
                  <Trans
                    i18nKey={$ => $.dashboard.alert_open_ticket}
                    values={{ count: openTicketCount }}
                    components={{ strong: <strong /> }}
                  />
                </span>
                <Link
                  to="/ticket"
                  data-testid="dashboard-alert-link"
                  className="font-medium text-foreground underline-offset-4 hover:underline"
                >
                  {t($ => $.dashboard.alert_view)}
                </Link>
              </AlertDescription>
            </Alert>
          )}
          {vm.shouldShowTrafficAlert && (
            <Alert
              data-testid="dashboard-alert"
              data-alert-kind="info"
              className="border-info/30 bg-info/10 text-foreground"
              role="alert"
            >
              <AlertCircle className="size-4 text-info" />
              <AlertDescription className="sm:flex sm:flex-row sm:items-center sm:gap-2">
                <span>{t($ => $.dashboard.alert_traffic_rate, { rate: vm.usedPctRounded })}</span>
                {vm.trafficAlertResetAvailable ? (
                  <button
                    type="button"
                    data-testid="dashboard-alert-link"
                    className="font-medium text-foreground underline-offset-4 hover:underline"
                    onClick={requestResetPackage}
                  >
                    {t($ => $.dashboard.buy_reset_package)}
                  </button>
                ) : null}
              </AlertDescription>
            </Alert>
          )}
        </div>
      )}

      {noticeList.length ? (
        <DashboardNoticeCarousel
          key={noticeList.map((notice) => notice.id).join(',')}
          notices={noticeList}
        />
      ) : null}

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1.45fr)_minmax(320px,0.75fr)]">
        <Card data-testid="dashboard-card" className="overflow-hidden">
          <CardHeader className="flex flex-row items-center justify-between gap-4 space-y-0 border-b border-border pb-5">
            <div className="space-y-1">
              <CardTitle data-testid="dashboard-card-title" className="text-xl">
                {t($ => $.dashboard.plan)}
              </CardTitle>
            </div>
            <span className="flex size-9 items-center justify-center rounded-md border border-border bg-background text-muted-foreground">
              <Package className="size-4" />
            </span>
          </CardHeader>
          <CardContent>
            {subscribe.isError ? (
              // A failed subscribe fetch must not fall through to the spinner
              // (which would spin forever) or the buy-subscribe empty state
              // (which would misrepresent a paying user as having no plan).
              <ErrorState
                onRetry={() => void subscribe.refetch()}
                data-testid="dashboard-plan-error"
              />
            ) : subscribe.isLoading || !hasSubscribeData ? (
              <div className="flex min-h-36 items-center justify-center">
                <Spinner className="size-6" />
              </div>
            ) : hasPlan ? (
              <div className="space-y-6">
                <div className="space-y-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="text-2xl font-semibold tracking-normal text-card-foreground">
                      {subscription.plan!.name}
                    </h2>
                    {vm.expired ? (
                      <StatusBadge data-testid="dashboard-status-expired" tone="destructive">
                        {t($ => $.dashboard.expired_label)}
                      </StatusBadge>
                    ) : (
                      <StatusBadge data-testid="dashboard-status-active" tone="success">
                        <CheckCircle2 className="size-3" />
                        {t($ => $.dashboard.active)}
                      </StatusBadge>
                    )}
                  </div>
                  {subscription.expired_at === null ? (
                    <p className="text-sm text-muted-foreground">{t($ => $.dashboard.long_term)}</p>
                  ) : vm.expired ? (
                    <p className="text-sm text-muted-foreground">{t($ => $.dashboard.expired_label)}</p>
                  ) : (
                    <p className="text-sm leading-6 text-muted-foreground">
                      {t($ => $.dashboard.expires_in, {
                        date: formatBackendDateSlash(subscription.expired_at),
                        day: vm.daysLeft,
                      })}
                      {subscription.reset_day !== null
                        ? subscription.reset_day === 0
                          ? t($ => $.dashboard.reset_today)
                          : t($ => $.dashboard.reset_in_days, { reset_day: subscription.reset_day })
                        : ''}
                    </p>
                  )}
                </div>

                <div className="space-y-3">
                  <Progress
                    data-testid="dashboard-progress"
                    aria-label={t($ => $.dashboard.used_traffic, {
                      used: formatBytes(vm.used),
                      total: formatBytes(subscription.transfer_enable),
                    })}
                    value={vm.usedPctClamped}
                    indicatorClassName={cn(
                      vm.trafficTone === 'danger' && 'bg-destructive',
                      vm.trafficTone === 'warning' && 'bg-warning',
                      // Healthy usage stays neutral (shadcn's default primary bar) rather
                      // than a saturated green, so the card reads calm until usage is high.
                      vm.trafficTone === 'success' && 'bg-primary',
                    )}
                    indicatorProps={{
                      'data-testid': 'dashboard-progress-bar',
                      'data-status': vm.trafficTone,
                    }}
                  />
                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="rounded-lg border border-border bg-muted/30 p-3">
                      <p className="text-sm font-medium">
                        {t($ => $.dashboard.used_traffic, {
                          used: formatBytes(vm.used),
                          total: formatBytes(subscription.transfer_enable),
                        })}
                      </p>
                    </div>
                    <div className="rounded-lg border border-border bg-muted/30 p-3">
                      <p className="text-sm font-medium">
                        {t($ => $.dashboard.devices_online, {
                          alive_ip: subscription.alive_ip,
                          device_limit: subscription.device_limit ?? '∞',
                        })}
                      </p>
                    </div>
                  </div>
                  <div className="sr-only">
                    <span>
                      {t($ => $.dashboard.used_traffic, {
                        used: formatBytes(vm.used),
                        total: formatBytes(subscription.transfer_enable),
                      })}
                    </span>
                    <span>
                      {t($ => $.dashboard.devices_online, {
                        alive_ip: subscription.alive_ip,
                        device_limit: subscription.device_limit ?? '∞',
                      })}
                    </span>
                  </div>
                </div>

                <div className="flex flex-wrap gap-2">
                  {vm.resetAvailable ? (
                    <Button type="button" onClick={requestResetPackage}>
                      {t($ => $.dashboard.buy_reset_package)}
                    </Button>
                  ) : null}
                  {vm.canNewPeriod ? (
                    <Button type="button" onClick={requestNewPeriod}>
                      {t($ => $.dashboard.new_period)}
                    </Button>
                  ) : null}
                  {vm.expired ? (
                    <Button asChild>
                      <Link to={vm.canRenew ? `/plan/${subscription.plan_id}` : '/plan'}>
                        {vm.canRenew
                          ? t($ => $.dashboard.renew_subscribe)
                          : t($ => $.dashboard.buy_subscribe)}
                      </Link>
                    </Button>
                  ) : null}
                </div>
              </div>
            ) : (
              <Link
                to="/plan"
                data-testid="dashboard-empty-plan"
                className="flex min-h-40 w-full flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border bg-muted/30 text-center transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
              >
                <Plus className="size-8 text-muted-foreground" />
                <span className="text-sm font-medium">{t($ => $.dashboard.shortcut_buy)}</span>
              </Link>
            )}
          </CardContent>
        </Card>

        <Card data-testid="dashboard-card" className="overflow-hidden">
          <CardHeader className="flex flex-row items-center justify-between gap-4 space-y-0 border-b border-border pb-5">
            <CardTitle data-testid="dashboard-card-title" className="text-xl">
              {t($ => $.dashboard.shortcuts)}
            </CardTitle>
            <span className="flex size-9 items-center justify-center rounded-md border border-border bg-background text-muted-foreground">
              <Smartphone className="size-4" />
            </span>
          </CardHeader>
          <CardContent className="grid gap-3">
            {shortcuts.map((shortcut) => {
              const Icon = shortcut.icon;
              const content = (
                <>
                  <span className="flex size-9 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground transition-colors group-hover:bg-background group-hover:text-foreground">
                    <Icon className="size-4" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="grid truncate text-sm font-medium">{t(shortcut.titleKey)}</span>
                    <span className="grid truncate text-sm leading-6 text-muted-foreground">
                      {t(shortcut.descKey)}
                      {shortcut.appendSiteTitle ? <> {siteTitle}</> : null}
                    </span>
                  </span>
                </>
              );
              const className =
                'group flex min-h-[4.5rem] min-w-0 items-center gap-3 rounded-lg border border-border bg-background p-4 text-left transition-colors hover:bg-accent/70 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50';

              return shortcut.onClick ? (
                <button
                  type="button"
                  key={shortcut.id}
                  data-testid="dashboard-shortcut"
                  className={className}
                  onClick={shortcut.onClick}
                >
                  {content}
                </button>
              ) : (
                <Link
                  key={shortcut.id}
                  to={shortcut.to}
                  data-testid="dashboard-shortcut"
                  className={className}
                >
                  {content}
                </Link>
              );
            })}
          </CardContent>
        </Card>
      </div>

      <DashboardSubscribeDialog
        open={subscribeOpen}
        onOpenChange={setSubscribeOpen}
        subscribeUrl={subscribeUrl}
      />
      <DashboardConfirmDialog action={confirmAction} onClose={() => setConfirmAction(null)} />
    </PageShell>
  );
}
