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
  CalendarClock,
  CheckCircle2,
  Copy,
  CreditCard,
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
import { Spinner } from '@/components/ui/spinner';
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
    <div className="oneClickSubscribe___2t9Xg grid gap-1 p-2">
      <button
        type="button"
        className="item___yrtOv subsrcibe-for-link flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
        onClick={copyUrl}
      >
        <Copy className="size-4 text-muted-foreground" />
        <span>{t('dashboard.copy_subscribe')}</span>
      </button>
      <button
        type="button"
        className="item___yrtOv subscribe-for-qrcode flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
        onClick={() => setQrOpen(true)}
      >
        <QrCode className="size-4 text-muted-foreground" />
        <span>{t('dashboard.scan_qrcode_subscribe')}</span>
      </button>
      {subscribeTargets.map((target) => (
        <button
          type="button"
          key={target.title}
          className={cn(
            'item___yrtOv flex min-h-11 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground',
            target.title.replace(' ', '-').toLowerCase(),
          )}
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
          className="ant-btn w-full"
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
      className="v2board-notice-card block w-full overflow-hidden rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-colors hover:bg-accent/40 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={() => openNotice(notice)}
    >
      <div
        className="min-h-36 p-5 sm:min-h-40"
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
    <div className="v2board-dashboard-page space-y-6">
      <div className="grid gap-3">
        {pendingOrderCount > 0 && (
          <Alert
            className="alert alert-danger border-destructive/25 bg-destructive/5 text-foreground"
            role="alert"
          >
            <AlertCircle className="size-4 text-destructive" />
            <AlertDescription>
              <span>{t('dashboard.alert_pending_order')}</span>
              <button
                type="button"
                className="alert-link font-medium text-foreground underline-offset-4 hover:underline"
                onClick={() => navigate('/order')}
              >
                {t('order.pay_now')}
              </button>
            </AlertDescription>
          </Alert>
        )}
        {openTicketCount > 0 && (
          <Alert className="alert alert-warning border-amber-200 bg-amber-50 text-foreground" role="alert">
            <Bell className="size-4 text-amber-600" />
            <AlertDescription>
              <span>
                <strong>{openTicketCount}</strong> {t('dashboard.alert_open_ticket_suffix')}
              </span>
              <button
                type="button"
                className="alert-link font-medium text-foreground underline-offset-4 hover:underline"
                onClick={() => navigate('/ticket')}
              >
                {t('dashboard.alert_view')}
              </button>
            </AlertDescription>
          </Alert>
        )}
        {shouldShowTrafficAlert && (
          <Alert className="alert alert-info border-sky-200 bg-sky-50 text-foreground" role="alert">
            <AlertCircle className="size-4 text-sky-600" />
            <AlertDescription>
              <span>{t('dashboard.alert_traffic_rate', { rate: usedPctRounded })}</span>
              {trafficAlertResetAvailable ? (
                <button
                  type="button"
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
        <section className="space-y-3">
          <div className="slick-slider">
            <div className="slick-slide slick-active">{renderNoticeCard(activeNoticeCard)}</div>
            {noticeList.length > 1 ? (
              <ul className="slick-dots slick-dots-bottom mt-3 flex justify-center gap-1">
                {noticeList.map((notice, index) => (
                  <li
                    key={notice.id}
                    className={cn(index === activeNoticeIndex && 'slick-active')}
                  >
                    <button
                      type="button"
                      className={cn(
                        'h-1.5 w-6 rounded-full bg-border text-[0px] transition-colors',
                        index === activeNoticeIndex && 'bg-primary',
                      )}
                      onClick={() => setActiveNoticeIndex(index)}
                    >
                      {index + 1}
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </section>
      ) : null}

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1.4fr)_minmax(320px,0.8fr)]">
        <Card className="v2board-dashboard-card">
          <CardHeader className="flex flex-row items-center justify-between gap-4 space-y-0">
            <div className="space-y-1">
              <CardTitle className="block-title text-xl">{t('dashboard.plan')}</CardTitle>
              {hasPlan && hasSubscribeData ? (
                <p className="text-sm text-muted-foreground">{legacySub.plan?.name}</p>
              ) : null}
            </div>
            <Package className="size-5 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {subscribe.isLoading || !hasSubscribeData ? (
              <div className="flex min-h-36 items-center justify-center">
                <Spinner className="size-6" />
              </div>
            ) : hasPlan ? (
              <div className="space-y-5">
                <div className="space-y-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="text-2xl font-semibold tracking-normal">
                      {legacySub.plan!.name}
                    </h2>
                    {expired ? (
                      <span className="text-danger rounded-md border border-destructive/25 bg-destructive/5 px-2 py-1 text-xs font-medium text-destructive">
                        {t('dashboard.expired_label')}
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 rounded-md border border-emerald-200 bg-emerald-50 px-2 py-1 text-xs font-medium text-emerald-700">
                        <CheckCircle2 className="size-3" />
                        {legacySub.expired_at === null ? t('dashboard.long_term') : t('dashboard.plan')}
                      </span>
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

                <div className="space-y-2">
                  <div className="progress h-2 overflow-hidden rounded-full bg-muted">
                    <div
                      className={cn(
                        'progress-bar h-full rounded-full transition-all',
                        usedPctRounded >= 100
                          ? 'bg-danger bg-destructive'
                          : usedPctRounded >= 80
                            ? 'bg-warning bg-amber-500'
                            : 'bg-success bg-emerald-500',
                      )}
                      role="progressbar"
                      style={{ width: `${usedPctClamped}%` }}
                    />
                  </div>
                  <div className="flex flex-wrap gap-x-4 gap-y-1 text-sm font-medium">
                    <span>
                      {t('dashboard.used_traffic', {
                        used: formatBytes(used),
                        total: formatBytes(legacySub.transfer_enable),
                      })}
                    </span>
                    <span className="text-muted-foreground">
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
                className="flex min-h-40 w-full flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border bg-muted/30 text-center transition-colors hover:bg-accent"
                onClick={() => navigate('/plan')}
              >
                <Plus className="size-8 text-muted-foreground" />
                <i className="fa fa-plus sr-only" aria-hidden="true" />
                <span className="text-sm font-medium">{t('dashboard.shortcut_buy')}</span>
              </button>
            )}
          </CardContent>
        </Card>

        <Card className="v2board-dashboard-card">
          <CardHeader className="flex flex-row items-center justify-between gap-4 space-y-0">
            <CardTitle className="block-title text-xl">{t('dashboard.shortcuts')}</CardTitle>
            <Smartphone className="size-5 text-muted-foreground" />
          </CardHeader>
          <CardContent className="grid gap-2">
            {shortcuts.map((shortcut) => {
              const Icon = shortcut.icon;
              return (
                <button
                  type="button"
                  key={shortcut.titleKey}
                  className="v2board-shortcuts-item flex min-h-16 items-center gap-3 rounded-lg border border-border bg-background px-3 text-left transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  onClick={shortcut.onClick ?? (() => navigate(shortcut.to))}
                >
                  <span className="flex size-9 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
                    <Icon className="size-4" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm font-medium">{t(shortcut.titleKey)}</span>
                    <span className="description block truncate text-sm text-muted-foreground">
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
        <DialogContent className="v2board-dashboard-dialog">
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
        <DialogContent className="v2board-dashboard-dialog p-0 sm:max-w-sm">
          <DialogHeader className="px-5 pt-5">
            <DialogTitle>{t('dashboard.shortcut_one_click')}</DialogTitle>
          </DialogHeader>
          {renderSubscribeBox()}
        </DialogContent>
      </Dialog>

      <Dialog open={qrOpen} onOpenChange={setQrOpen}>
        <DialogContent className="v2board-dashboard-dialog sm:max-w-xs">
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
        <DialogContent className="v2board-dashboard-dialog sm:max-w-md">
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
              className="ant-btn ant-btn-primary"
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
    </div>
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

function legacyDaysUntil(timestamp: number | string | null | undefined) {
  return ((Number(timestamp) - Math.floor(Date.now() / 1000)) / 86400).toFixed(0);
}

function isLegacyRenewable(subscribe: ReturnType<typeof useSubscribe>['data']) {
  if (!subscribe?.plan?.renew) return false;
  return Boolean(subscribe.plan.show || !isLegacyExpired(subscribe.expired_at));
}
