import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import QRCode from 'qrcode.react';
import { user } from '@v2board/api-client';
import type { Notice } from '@v2board/types';
import {
  AlertCircle,
  Bell,
  BookOpen,
  CheckCircle2,
  Copy,
  Headphones,
  LinkIcon,
  Package,
  Plus,
  QrCode,
  RefreshCcw,
  ShoppingBag,
  Smartphone,
} from 'lucide-react';
import { apiClient } from '@/lib/api';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { PageShell } from '@/components/ui/page';
import { Progress } from '@/components/ui/progress';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  useCommConfig,
  useNewPeriodMutation,
  useNotices,
  useSubscribe,
  useUserStat,
} from '@/lib/queries';
import { formatBytes } from '@v2board/config/format';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import { formatUserLegacyDate, formatUserLegacyDateSlash } from '@/lib/legacy-date';
import { cn } from '@/lib/cn';
import clashForAndroidIcon from '../assets/images/icon/Clash For Android.png';
import clashForWindowsIcon from '../assets/images/icon/Clash For Windows.png';
import clashMetaForAndroidIcon from '../assets/images/icon/ClashMeta For Android.png';
import clashMetaForWindowsIcon from '../assets/images/icon/ClashMeta For Windows.png';
import clashMetaIcon from '../assets/images/icon/ClashMeta.png';
import clashXIcon from '../assets/images/icon/ClashX.png';
import hiddifyIcon from '../assets/images/icon/Hiddify.png';
import nekoBoxForAndroidIcon from '../assets/images/icon/NekoBox For Android.png';
import quantumultXIcon from '../assets/images/icon/QuantumultX.png';
import shadowrocketIcon from '../assets/images/icon/Shadowrocket.png';
import singBoxIcon from '../assets/images/icon/Sing-box.png';
import stashIcon from '../assets/images/icon/Stash.png';
import surfboardIcon from '../assets/images/icon/Surfboard.png';
import surgeIcon from '../assets/images/icon/Surge.png';

interface Shortcut {
  to: string;
  icon: typeof BookOpen;
  titleKey: string;
  descKey: string;
  onClick?: () => void;
}

type ConfirmAction = 'reset-package' | 'new-period' | null;

const SUBSCRIBE_TARGET_ICONS: Record<string, string> = {
  'Clash For Android': clashForAndroidIcon,
  'Clash For Windows': clashForWindowsIcon,
  'ClashMeta For Android': clashMetaForAndroidIcon,
  'ClashMeta For Windows': clashMetaForWindowsIcon,
  ClashMeta: clashMetaIcon,
  ClashX: clashXIcon,
  Hiddify: hiddifyIcon,
  'NekoBox For Android': nekoBoxForAndroidIcon,
  QuantumultX: quantumultXIcon,
  Shadowrocket: shadowrocketIcon,
  'Sing-box': singBoxIcon,
  Stash: stashIcon,
  Surfboard: surfboardIcon,
  Surge: surgeIcon,
};

export default function DashboardPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const subscribe = useSubscribe();
  const stat = useUserStat();
  const notices = useNotices();
  useCommConfig();
  const newPeriod = useNewPeriodMutation();
  const [activeNotice, setActiveNotice] = useState<Notice | null>(null);
  const [noticeOpen, setNoticeOpen] = useState(false);
  const [activeNoticeIndex, setActiveNoticeIndex] = useState(0);
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [qrOpen, setQrOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<ConfirmAction>(null);
  const [savingResetPackage, setSavingResetPackage] = useState(false);
  const [savingNewPeriod, setSavingNewPeriod] = useState(false);

  const pendingOrderCount = stat.data?.pending_orders ?? 0;
  const openTicketCount = stat.data?.pending_tickets ?? 0;
  const sub = subscribe.data;
  const hasSubscribeData = Boolean(sub?.email);
  const hasPlan = Boolean(sub?.plan_id);
  const used = sub ? sub.u + sub.d : 0;
  const usedPct = sub?.transfer_enable ? (used / sub.transfer_enable) * 100 : 0;
  const usedPctRounded = Math.round(usedPct * 100) / 100;
  const usedPctClamped = Math.max(0, Math.min(100, usedPct));
  const trafficTone = getTrafficTone(usedPctRounded);
  const daysLeft = legacyDaysUntil(sub?.expired_at);
  const expired = isLegacyExpired(sub?.expired_at ?? null);
  const canRenew = isLegacyRenewable(sub);
  const resetAvailable = Boolean(
    hasPlan && sub?.plan?.reset_price && usedPctRounded >= 80 && !expired,
  );
  const shouldShowTrafficAlert = Boolean(usedPctRounded >= 80 && usedPctRounded < 100 && !expired);
  const trafficAlertResetAvailable = Boolean(sub?.plan?.reset_price);
  const canNewPeriod = Boolean(
    hasPlan && sub?.allow_new_period && usedPctRounded >= 100 && !expired,
  );
  const noticeList = notices.data ?? [];
  const activeNoticeCard = noticeList[activeNoticeIndex] ?? noticeList[0];
  const subscribeUrl = typeof sub?.subscribe_url === 'string' ? sub.subscribe_url : '';
  const subscribeTargets = useMemo(
    () => (subscribeUrl ? getSubscribeTargets(subscribeUrl) : []),
    [subscribeUrl],
  );
  const legacySub = sub!;

  useEffect(() => {
    setActiveNoticeIndex(0);
  }, [noticeList.length]);

  useEffect(() => {
    const list = notices.data;
    if (!list?.length) return;
    const popup = list.find((notice) => notice.tags?.includes('弹窗'));
    if (popup) {
      setActiveNotice(popup);
      setNoticeOpen(true);
    }
  }, [notices.data]);

  const copyUrl = () => {
    legacyCopyText(subscribeUrl);
    toast.success(t('dashboard.copy_success'));
  };

  const requestResetPackage = () => {
    if (!sub) return;
    setConfirmAction('reset-package');
  };

  const requestNewPeriod = () => {
    setConfirmAction('new-period');
  };

  const confirmResetPackage = async () => {
    if (!sub) return;
    setSavingResetPackage(true);
    try {
      const tradeNo = await user.saveOrder(apiClient, {
        period: 'reset_price',
        plan_id: sub.plan_id as number,
      });
      setConfirmAction(null);
      navigate(`/order/${tradeNo}`);
    } catch {
    } finally {
      setSavingResetPackage(false);
    }
  };

  const confirmNewPeriod = async () => {
    setSavingNewPeriod(true);
    try {
      await newPeriod.mutateAsync();
      await subscribe.refetch();
      toast.success('提前开启流量周期成功');
      setConfirmAction(null);
      navigate('/dashboard');
    } catch {
    } finally {
      setSavingNewPeriod(false);
    }
  };

  const confirmLoading = savingResetPackage || savingNewPeriod;
  const confirmTitle =
    confirmAction === 'reset-package'
      ? t('dashboard.reset_package_confirm_title')
      : t('dashboard.new_period_confirm_title');
  const confirmContent =
    confirmAction === 'reset-package'
      ? t('dashboard.reset_package_confirm_content')
      : t('dashboard.new_period_confirm_content');

  const shortcuts: Shortcut[] = [
    {
      to: '/knowledge',
      icon: BookOpen,
      titleKey: 'dashboard.shortcut_tutorial',
      descKey: 'dashboard.shortcut_tutorial_desc',
    },
    {
      to: '#',
      icon: LinkIcon,
      titleKey: 'dashboard.shortcut_one_click',
      descKey: 'dashboard.shortcut_one_click_desc',
      onClick: () => setSubscribeOpen(true),
    },
    {
      to: canRenew ? `/plan/${sub?.plan_id}` : '/plan',
      icon: canRenew ? RefreshCcw : ShoppingBag,
      titleKey: canRenew ? 'dashboard.renew_subscribe' : 'dashboard.shortcut_buy',
      descKey: canRenew ? 'dashboard.shortcut_renew_desc' : 'dashboard.shortcut_buy_desc',
    },
    {
      to: '/ticket',
      icon: Headphones,
      titleKey: 'dashboard.shortcut_problem',
      descKey: 'dashboard.shortcut_problem_desc',
    },
  ];

  const renderSubscribeBox = () => (
    <div data-testid="dashboard-subscribe-menu" className="grid gap-1 p-2">
      <button
        type="button"
        data-testid="dashboard-subscribe-copy"
        className="flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
        onClick={copyUrl}
      >
        <Copy className="size-4 text-muted-foreground" />
        <span>{t('dashboard.copy_subscribe')}</span>
      </button>
      <button
        type="button"
        data-testid="dashboard-subscribe-qrcode"
        className="flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
        onClick={() => setQrOpen(true)}
      >
        <QrCode className="size-4 text-muted-foreground" />
        <span>{t('dashboard.scan_qrcode_subscribe')}</span>
      </button>
      {subscribeTargets.map((target) => (
        <button
          type="button"
          key={target.title}
          data-testid="dashboard-subscribe-target"
          data-subscribe-target={subscribeTargetSlug(target.title)}
          className="flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
          onClick={() => {
            window.location.href = target.href;
          }}
        >
          <img className="size-5 rounded-sm" src={SUBSCRIBE_TARGET_ICONS[target.title]} />
          <span>
            {t('dashboard.import_to')} {target.title}
          </span>
        </button>
      ))}
      <div className="px-1 pb-1 pt-2">
        <Button
          type="button"
          data-testid="dashboard-subscribe-tutorial"
          className="w-full"
          onClick={() => navigate('/knowledge')}
        >
          {t('dashboard.use_tutorial')}
        </Button>
      </div>
    </div>
  );

  const openNotice = (notice: Notice) => {
    setActiveNotice(notice);
    setNoticeOpen(true);
  };

  const renderNoticeCard = (notice: Notice) => (
    <button
      type="button"
      data-testid="dashboard-notice-card"
      className="flex w-full flex-col overflow-hidden rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-colors hover:bg-accent/40 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={() => openNotice(notice)}
    >
      <div
        className={cn(
          'min-h-36 p-5 sm:min-h-40',
          !notice.img_url && 'bg-muted/30',
        )}
        style={
          notice.img_url
            ? {
                backgroundImage: `linear-gradient(rgba(0,0,0,.52), rgba(0,0,0,.52)), url(${notice.img_url})`,
                backgroundPosition: 'center',
                backgroundSize: 'cover',
              }
            : undefined
        }
      >
        <span className="inline-flex rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground">
          {t('notice.title')}
        </span>
        <div className={cn('mt-10 space-y-1', notice.img_url && 'text-white')}>
          <div className="line-clamp-2 text-lg font-semibold">{notice.title}</div>
          <div className={cn('text-sm text-muted-foreground', notice.img_url && 'text-white/75')}>
            {formatUserLegacyDate(notice.created_at)}
          </div>
        </div>
      </div>
    </button>
  );

  return (
    <PageShell data-testid="dashboard-page">
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
              <span>{t('dashboard.alert_pending_order')}</span>
              <button
                type="button"
                data-testid="dashboard-alert-link"
                className="font-medium text-foreground underline-offset-4 hover:underline"
                onClick={() => navigate('/order')}
              >
                {t('order.pay_now')}
              </button>
            </AlertDescription>
          </Alert>
        )}
        {openTicketCount > 0 && (
          <Alert
            data-testid="dashboard-alert"
            data-alert-kind="warning"
            className="border-amber-200 bg-amber-50 text-foreground"
            role="alert"
          >
            <Bell className="size-4 text-amber-600" />
            <AlertDescription className="sm:flex sm:flex-row sm:items-center sm:gap-2">
              <span>
                <strong>{openTicketCount}</strong> {t('dashboard.alert_open_ticket_suffix')}
              </span>
              <button
                type="button"
                data-testid="dashboard-alert-link"
                className="font-medium text-foreground underline-offset-4 hover:underline"
                onClick={() => navigate('/ticket')}
              >
                {t('dashboard.alert_view')}
              </button>
            </AlertDescription>
          </Alert>
        )}
        {shouldShowTrafficAlert && (
          <Alert
            data-testid="dashboard-alert"
            data-alert-kind="info"
            className="border-sky-200 bg-sky-50 text-foreground"
            role="alert"
          >
            <AlertCircle className="size-4 text-sky-600" />
            <AlertDescription className="sm:flex sm:flex-row sm:items-center sm:gap-2">
              <span>{t('dashboard.alert_traffic_rate', { rate: usedPctRounded })}</span>
              {trafficAlertResetAvailable ? (
                <button
                  type="button"
                  data-testid="dashboard-alert-link"
                  className="font-medium text-foreground underline-offset-4 hover:underline"
                  onClick={requestResetPackage}
                >
                  {t('dashboard.buy_reset_package')}
                </button>
              ) : null}
            </AlertDescription>
          </Alert>
        )}
      </div>

      {noticeList.length > 0 && activeNoticeCard ? (
        <section data-testid="dashboard-notices" className="space-y-3">
          <Tabs
            data-testid="dashboard-notice-carousel"
            value={String(activeNoticeIndex)}
            onValueChange={(value) => setActiveNoticeIndex(Number(value))}
          >
            {noticeList.map((notice, index) => (
              <TabsContent
                key={notice.id}
                value={String(index)}
                data-testid="dashboard-notice-slide"
                data-active={index === activeNoticeIndex ? 'true' : 'false'}
                className="mt-0 data-[state=inactive]:hidden"
              >
                {renderNoticeCard(notice)}
              </TabsContent>
            ))}
            {noticeList.length > 1 ? (
              <TabsList
                data-testid="dashboard-notice-dots"
                aria-label={t('notice.title')}
                className="mt-3 flex h-auto justify-center gap-1 border-0 bg-transparent p-0 shadow-none"
              >
                {noticeList.map((notice, index) => (
                  <TabsTrigger
                    key={notice.id}
                    value={String(index)}
                    onClick={() => setActiveNoticeIndex(index)}
                    className="h-1.5 w-6 rounded-full bg-border p-0 text-[0px] shadow-none transition-colors hover:bg-muted-foreground/40 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 data-[state=active]:bg-primary data-[state=active]:shadow-none dark:data-[state=active]:bg-primary"
                    data-testid="dashboard-notice-dot"
                    aria-label={`${t('notice.title')} ${index + 1}`}
                  >
                    {index + 1}
                  </TabsTrigger>
                ))}
              </TabsList>
            ) : null}
          </Tabs>
        </section>
      ) : null}

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1.45fr)_minmax(320px,0.75fr)]">
        <Card data-testid="dashboard-card" className="overflow-hidden">
          <CardHeader className="flex flex-row items-center justify-between gap-4 space-y-0 border-b border-border pb-5">
            <div className="space-y-1">
              <CardTitle data-testid="dashboard-card-title" className="text-xl">
                {t('dashboard.plan')}
              </CardTitle>
              {hasPlan && hasSubscribeData ? (
                <p className="text-sm text-muted-foreground">{legacySub.plan?.name}</p>
              ) : null}
            </div>
            <span className="flex size-9 items-center justify-center rounded-md border border-border bg-background text-muted-foreground">
              <Package className="size-4" />
            </span>
          </CardHeader>
          <CardContent className="pt-6">
            {subscribe.isLoading || !hasSubscribeData ? (
              <div className="flex min-h-36 items-center justify-center">
                <Spinner className="size-6" />
              </div>
            ) : hasPlan ? (
              <div className="space-y-6">
                <div className="space-y-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="text-2xl font-semibold tracking-normal">
                      {legacySub.plan!.name}
                    </h2>
                    {expired ? (
                      <StatusBadge
                        data-testid="dashboard-status-expired"
                        tone="destructive"
                      >
                        {t('dashboard.expired_label')}
                      </StatusBadge>
                    ) : (
                      <StatusBadge
                        data-testid="dashboard-status-active"
                        tone="success"
                      >
                        <CheckCircle2 className="size-3" />
                        {legacySub.expired_at === null ? t('dashboard.long_term') : t('dashboard.plan')}
                      </StatusBadge>
                    )}
                  </div>
                  {legacySub.expired_at === null ? (
                    <p className="text-sm text-muted-foreground">{t('dashboard.long_term')}</p>
                  ) : expired ? (
                    <p className="text-sm text-muted-foreground">{t('dashboard.expired_label')}</p>
                  ) : (
                    <p className="text-sm leading-6 text-muted-foreground">
                      {t('dashboard.expires_in', {
                        date: formatUserLegacyDateSlash(legacySub.expired_at),
                        day: daysLeft,
                      })}
                      {legacySub.reset_day !== null
                        ? legacySub.reset_day === 0
                          ? t('dashboard.reset_today')
                          : t('dashboard.reset_in_days', { reset_day: legacySub.reset_day })
                        : ''}
                    </p>
                  )}
                </div>

                <div className="space-y-3">
                  <Progress
                    data-testid="dashboard-progress"
                    value={usedPctClamped}
                    indicatorClassName={cn(
                      trafficTone === 'danger' && 'bg-destructive',
                      trafficTone === 'warning' && 'bg-amber-500',
                      trafficTone === 'success' && 'bg-emerald-500',
                    )}
                    indicatorProps={{
                      'data-testid': 'dashboard-progress-bar',
                      'data-status': trafficTone,
                    }}
                  />
                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="rounded-lg border border-border bg-muted/30 p-3">
                      <p className="text-sm font-medium">
                        {t('dashboard.used_traffic', {
                          used: formatBytes(used),
                          total: formatBytes(legacySub.transfer_enable),
                        })}
                      </p>
                    </div>
                    <div className="rounded-lg border border-border bg-muted/30 p-3">
                      <p className="text-sm font-medium text-muted-foreground">
                        {t('dashboard.devices_online', {
                          alive_ip: legacySub.alive_ip,
                          device_limit: legacySub.device_limit ?? '∞',
                        })}
                      </p>
                    </div>
                  </div>
                  <div className="sr-only">
                    <span>
                      {t('dashboard.used_traffic', {
                        used: formatBytes(used),
                        total: formatBytes(legacySub.transfer_enable),
                      })}
                    </span>
                    <span>
                      {t('dashboard.devices_online', {
                        alive_ip: legacySub.alive_ip,
                        device_limit: legacySub.device_limit ?? '∞',
                      })}
                    </span>
                  </div>
                </div>

                <div className="flex flex-wrap gap-2">
                  {resetAvailable ? (
                    <Button type="button" onClick={requestResetPackage}>
                      {t('dashboard.buy_reset_package')}
                    </Button>
                  ) : null}
                  {canNewPeriod ? (
                    <Button type="button" onClick={requestNewPeriod}>
                      {t('dashboard.new_period')}
                    </Button>
                  ) : null}
                  {expired ? (
                    <Button
                      type="button"
                      onClick={() => navigate(canRenew ? `/plan/${legacySub.plan_id}` : '/plan')}
                    >
                      {canRenew ? t('dashboard.renew_subscribe') : t('dashboard.buy_subscribe')}
                    </Button>
                  ) : null}
                </div>
              </div>
            ) : (
              <button
                type="button"
                data-testid="dashboard-empty-plan"
                className="flex min-h-40 w-full flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border bg-muted/30 text-center transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                onClick={() => navigate('/plan')}
              >
                <Plus className="size-8 text-muted-foreground" />
                <span className="text-sm font-medium">{t('dashboard.shortcut_buy')}</span>
              </button>
            )}
          </CardContent>
        </Card>

        <Card data-testid="dashboard-card" className="overflow-hidden">
          <CardHeader className="flex flex-row items-center justify-between gap-4 space-y-0 border-b border-border pb-5">
            <CardTitle data-testid="dashboard-card-title" className="text-xl">
              {t('dashboard.shortcuts')}
            </CardTitle>
            <span className="flex size-9 items-center justify-center rounded-md border border-border bg-background text-muted-foreground">
              <Smartphone className="size-4" />
            </span>
          </CardHeader>
          <CardContent className="grid gap-3 pt-6">
            {shortcuts.map((shortcut) => {
              const Icon = shortcut.icon;
              return (
                <button
                  type="button"
                  key={shortcut.titleKey}
                  data-testid="dashboard-shortcut"
                  className="group flex min-h-[4.5rem] items-center gap-3 rounded-lg border border-border bg-background p-4 text-left transition-colors hover:bg-accent/70 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  onClick={shortcut.onClick ?? (() => navigate(shortcut.to))}
                >
                  <span className="flex size-9 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground transition-colors group-hover:bg-background group-hover:text-foreground">
                    <Icon className="size-4" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="flex text-sm font-medium">{t(shortcut.titleKey)}</span>
                    <span className="flex overflow-hidden text-ellipsis whitespace-nowrap text-sm leading-6 text-muted-foreground">
                      {t(shortcut.descKey)}
                      {shortcut.descKey === 'dashboard.shortcut_tutorial_desc' ? (
                        <> {window.settings?.title}</>
                      ) : null}
                    </span>
                  </span>
                </button>
              );
            })}
          </CardContent>
        </Card>
      </div>

      <Dialog
        open={noticeOpen}
        onOpenChange={(open) => {
          setNoticeOpen(open);
          if (!open) setActiveNotice(null);
        }}
      >
        <DialogContent data-testid="dashboard-dialog">
          <DialogHeader>
            <DialogTitle>{activeNotice?.title}</DialogTitle>
          </DialogHeader>
          {activeNotice?.content ? (
            <div
              className="notice-content max-h-[60vh] overflow-auto text-sm leading-6"
              dangerouslySetInnerHTML={{ __html: activeNotice.content }}
            />
          ) : null}
        </DialogContent>
      </Dialog>

      <Dialog open={subscribeOpen} onOpenChange={setSubscribeOpen}>
        <DialogContent data-testid="dashboard-dialog" className="p-0 sm:max-w-sm">
          <DialogHeader className="px-5 pt-5">
            <DialogTitle>{t('dashboard.shortcut_one_click')}</DialogTitle>
          </DialogHeader>
          {renderSubscribeBox()}
        </DialogContent>
      </Dialog>

      <Dialog open={qrOpen} onOpenChange={setQrOpen}>
        <DialogContent data-testid="dashboard-dialog" className="sm:max-w-xs">
          <DialogHeader>
            <DialogTitle>{t('dashboard.scan_qrcode_subscribe')}</DialogTitle>
            <DialogDescription>{t('dashboard.qrcode_client_tip')}</DialogDescription>
          </DialogHeader>
          <div className="flex justify-center">
            <QRCode value={subscribeUrl} renderAs="canvas" />
          </div>
        </DialogContent>
      </Dialog>

      <Dialog
        open={confirmAction !== null}
        onOpenChange={(open) => {
          if (!open && !confirmLoading) setConfirmAction(null);
        }}
      >
        <DialogContent data-testid="dashboard-dialog" className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{confirmTitle}</DialogTitle>
            <DialogDescription>{confirmContent}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={confirmLoading}
              onClick={() => setConfirmAction(null)}
            >
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              data-testid="dashboard-confirm-primary"
              loading={confirmLoading}
              onClick={() => {
                void (confirmAction === 'reset-package'
                  ? confirmResetPackage()
                  : confirmNewPeriod());
              }}
            >
              {t('common.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageShell>
  );
}

function getSubscribeTargets(url: string) {
  const title = window.settings!.title;
  const userAgent = window.navigator.userAgent;
  const lowerUserAgent = userAgent.toLowerCase();
  const isAppleMobile =
    lowerUserAgent.includes('iphone') ||
    lowerUserAgent.includes('ipad') ||
    (/Mac/.test(userAgent) && window.navigator.maxTouchPoints > 2);
  const isMac = lowerUserAgent.includes('macintosh');
  const isAndroid = lowerUserAgent.includes('android');
  const isWindows = lowerUserAgent.includes('windows');
  const shadowrocketPayload = window
    .btoa(`${url}&flag=shadowrocket`)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
  const targets = [
    { title: 'Hiddify', href: `hiddify://import/${url}&flag=sing#${title}` },
    {
      title: 'Sing-box',
      href: `sing-box://import-remote-profile?url=${encodeURIComponent(url)}#${title}`,
    },
  ];
  if (isAppleMobile) {
    targets.push(
      {
        title: 'Shadowrocket',
        href: `shadowrocket://add/sub://${shadowrocketPayload}?remark=${title}`,
      },
      {
        title: 'QuantumultX',
        href: `quantumult-x:///update-configuration?remote-resource=${encodeURI(
          JSON.stringify({ server_remote: [`${url}, tag=${title}`] }),
        )}`,
      },
      {
        title: 'Surge',
        href: `surge:///install-config?url=${encodeURIComponent(url)}&name=${title}`,
      },
      {
        title: 'Stash',
        href: `stash://install-config?url=${encodeURIComponent(url)}&name=${title}`,
      },
    );
  }
  if (isMac) {
    targets.push({
      title: 'ClashX',
      href: `clash://install-config?url=${encodeURIComponent(url)}&name=${title}`,
    });
  }
  if (isWindows) {
    targets.push({
      title: 'ClashMeta',
      href: `clash://install-config?url=${encodeURIComponent(`${url}&flag=meta`)}&name=${title}`,
    });
  }
  if (isAndroid) {
    targets.push(
      {
        title: 'NekoBox For Android',
        href: `clash://install-config?url=${encodeURIComponent(`${url}&flag=meta`)}&name=${title}`,
      },
      {
        title: 'ClashMeta For Android',
        href: `clash://install-config?url=${encodeURIComponent(`${url}&flag=meta`)}&name=${title}`,
      },
      {
        title: 'Surfboard',
        href: `surge:///install-config?url=${encodeURIComponent(url)}&name=${title}`,
      },
    );
  }
  return targets;
}

function isLegacyExpired(expiredAt: number | null | undefined) {
  return expiredAt !== null && expiredAt !== undefined && expiredAt < Date.now() / 1000;
}

function getTrafficTone(usedPctRounded: number) {
  if (usedPctRounded >= 100) return 'danger';
  if (usedPctRounded >= 80) return 'warning';
  return 'success';
}

function subscribeTargetSlug(title: string) {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

function legacyDaysUntil(timestamp: number | string | null | undefined) {
  return ((Number(timestamp) - Math.floor(Date.now() / 1000)) / 86400).toFixed(0);
}

function isLegacyRenewable(subscribe: ReturnType<typeof useSubscribe>['data']) {
  if (!subscribe?.plan?.renew) return false;
  return Boolean(subscribe.plan.show || !isLegacyExpired(subscribe.expired_at));
}
