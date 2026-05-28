import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { QRCodeCanvas } from '@rc-component/qrcode';
import { user } from '@v2board/api-client';
import type { Notice } from '@v2board/types';
import { apiClient } from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import {
  useCommConfig,
  useNewPeriodMutation,
  useNotices,
  useSubscribe,
  useUserStat,
} from '@/lib/queries';
import { daysUntil, formatBytes, formatDate } from '@v2board/config/format';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { isLegacyMobile, legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';

interface Shortcut {
  to: string;
  iconClass: string;
  titleKey: string;
  descKey: string;
  onClick?: () => void;
}

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
  const [noticeIndex, setNoticeIndex] = useState(0);
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const [qrOpen, setQrOpen] = useState(false);
  const [subscribeHeight, setSubscribeHeight] = useState<number>();
  const subscribeBoxRef = useRef<HTMLDivElement>(null);

  const pendingOrderCount = stat.data?.pending_orders ?? 0;
  const openTicketCount = stat.data?.pending_tickets ?? 0;
  const sub = subscribe.data;

  useEffect(() => {
    const popup = notices.data?.data.find((notice) => notice.tags?.includes('弹窗'));
    if (!popup) return;
    setActiveNotice(popup);
    setNoticeOpen(true);
  }, [notices.data]);

  useEffect(() => {
    const count = notices.data?.data.length ?? 0;
    if (count <= 1) return;
    const id = window.setInterval(() => {
      setNoticeIndex((idx) => (idx + 1) % count);
    }, 3000);
    return () => window.clearInterval(id);
  }, [notices.data?.data.length]);

  useEffect(() => {
    if (!subscribeOpen) return;
    const id = window.setTimeout(() => {
      setSubscribeHeight(subscribeBoxRef.current?.offsetHeight);
    }, 100);
    return () => window.clearTimeout(id);
  }, [subscribeOpen, sub?.subscribe_url]);

  const hasSubscribeData = Boolean(sub?.email);
  const hasPlan = Boolean(sub?.plan_id);
  const used = sub ? sub.u + sub.d : 0;
  const usedPct = sub ? (used / sub.transfer_enable) * 100 : 0;
  const daysLeft = daysUntil(sub?.expired_at ?? null);
  const expired = isLegacyExpired(sub?.expired_at ?? null);
  const canRenew = isLegacyRenewable(sub);
  const resetAvailable = Boolean(
    hasPlan &&
      sub?.plan?.reset_price &&
      usedPct >= 80 &&
      !expired,
  );
  const shouldShowTrafficAlert = Boolean(hasPlan && usedPct >= 80 && usedPct < 100 && !expired);
  const canNewPeriod = Boolean(
    hasPlan &&
      sub?.allow_new_period &&
      usedPct >= 100 &&
      !expired,
  );
  const noticeList = notices.data?.data ?? [];
  const activeNoticeIndex = noticeList.length > 0 ? noticeIndex % noticeList.length : 0;
  const subscribeUrl = sub?.subscribe_url as string;

  const copyUrl = () => {
    legacyCopyText(subscribeUrl);
    toast.success(t('dashboard.copy_success'));
  };

  const shortcuts: Shortcut[] = [
    {
      to: '/knowledge',
      iconClass: 'si si-book-open',
      titleKey: 'dashboard.shortcut_tutorial',
      descKey: 'dashboard.shortcut_tutorial_desc',
    },
    {
      to: '#',
      iconClass: 'si si-feed',
      titleKey: 'dashboard.shortcut_one_click',
      descKey: 'dashboard.shortcut_one_click_desc',
      onClick: () => setSubscribeOpen(true),
    },
    {
      to: canRenew && sub?.plan_id ? `/plan/${sub.plan_id}` : '/plan',
      iconClass: canRenew ? 'si si-clock' : 'si si-bag',
      titleKey: canRenew ? 'dashboard.renew_subscribe' : 'dashboard.shortcut_buy',
      descKey: canRenew ? 'dashboard.shortcut_renew_desc' : 'dashboard.shortcut_buy_desc',
    },
    {
      to: '/ticket',
      iconClass: 'si si-support',
      titleKey: 'dashboard.shortcut_problem',
      descKey: 'dashboard.shortcut_problem_desc',
    },
  ];

  const saveResetPackage = async () => {
    if (!sub?.plan_id) return;
    const ok = await legacyConfirm({
      title: t('dashboard.reset_package_confirm_title'),
      content: t('dashboard.reset_package_confirm_content'),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      maskClosable: true,
    });
    if (!ok) return;
    try {
      const tradeNo = await user.saveOrder(apiClient, {
        period: 'reset_price',
        plan_id: sub.plan_id,
      });
      navigate(`/order/${tradeNo}`);
    } catch {}
  };

  const openNewPeriod = async () => {
    const ok = await legacyConfirm({
      title: t('dashboard.new_period_confirm_title'),
      content: t('dashboard.new_period_confirm_content'),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      maskClosable: true,
    });
    if (!ok) return;
    try {
      await newPeriod.mutateAsync();
      toast.success('提前开启流量周期成功');
      navigate('/dashboard');
    } catch {}
  };

  const renderSubscribeBox = () => (
    <div className="oneClickSubscribe___2t9Xg v2board-one-click-subscribe" ref={subscribeBoxRef}>
      <div
        className="item___yrtOv v2board-one-click-item subsrcibe-for-link"
        onClick={copyUrl}
      >
        <div>
          <i className="fa fa-copy mr-2" aria-hidden />
        </div>
        <div>{t('dashboard.copy_subscribe')}</div>
      </div>
      <div
        className="item___yrtOv v2board-one-click-item subscribe-for-qrcode"
        onClick={() => setQrOpen(true)}
      >
        <div>
          <i className="fa fa-qrcode mr-2" aria-hidden />
        </div>
        <div>{t('dashboard.scan_qrcode_subscribe')}</div>
      </div>
      {getSubscribeTargets(subscribeUrl).map((target) => (
        <div
          key={target.title}
          className={`item___yrtOv v2board-one-click-item ${target.title.replace(' ', '-').toLowerCase()}`}
          onClick={() => {
            window.location.href = target.href;
          }}
        >
          <div>
            <img
              src={`${window.settings?.assets_path ?? ''}/./images/icon/${target.title}.png`}
            />
          </div>
          <div>{t('dashboard.import_to')} {target.title}</div>
        </div>
      ))}
      <div style={{ padding: 10 }}>
        <button
          type="button"
          className="ant-btn ant-btn-primary ant-btn-lg ant-btn-block"
          onClick={() => navigate('/knowledge')}
        >
          <span>{t('dashboard.use_tutorial')}</span>
        </button>
      </div>
    </div>
  );

  const renderNoticeCard = (notice: Notice) => (
    <a
      className="block block-rounded bg-image mb-0 v2board-bg-pixels"
      href="javascript:void(0)"
      style={
        notice.img_url
          ? { backgroundImage: `url(${notice.img_url})`, backgroundSize: 'cover' }
          : undefined
      }
      onClick={() => {
        setActiveNotice(notice);
        setNoticeOpen(true);
      }}
    >
      <div className="block-content bg-black-50">
        <div className="mb-5 mb-sm-7 d-sm-flex justify-content-sm-between align-items-sm-center">
          <p>
            <span className="badge badge-danger p-2 text-uppercase">{t('notice.title')}</span>
          </p>
        </div>
        <p className="font-size-lg text-white mb-1">{notice.title}</p>
        <p className="font-w600 text-white-75">{formatDate(notice.created_at)}</p>
      </div>
    </a>
  );

  const mobileSubscribe = isLegacyMobile();

  return (
    <>
      {pendingOrderCount > 0 && (
        <div className="alert alert-danger" role="alert">
          <p className="mb-0">
            {t('dashboard.alert_pending_order')}{' '}
            <a
              className="alert-link"
              href="javascript:void(0)"
              onClick={() => navigate('/order')}
            >
              {t('order.pay_now')}
            </a>
          </p>
        </div>
      )}
      {openTicketCount > 0 && (
        <div className="alert alert-warning" role="alert">
          <p className="mb-0">
            <strong>{openTicketCount}</strong> {t('dashboard.alert_open_ticket_suffix')}{' '}
            <a
              className="alert-link"
              href="javascript:void(0)"
              onClick={() => navigate('/ticket')}
            >
              {t('dashboard.alert_view')}
            </a>
          </p>
        </div>
      )}
      {shouldShowTrafficAlert && (
        <div className="alert alert-info" role="alert">
          <p className="mb-0">
            {t('dashboard.alert_traffic_rate', { rate: Math.round(usedPct * 100) / 100 })}{' '}
            {resetAvailable && (
              <a href="javascript:void(0)" onClick={saveResetPackage}>
                <strong>{t('dashboard.buy_reset_package')}</strong>
              </a>
            )}
          </p>
        </div>
      )}
      {noticeList.length > 0 && (
        <div className="row mb-3 mb-md-0">
          <div className="col-12 mb-sm-4">
            {noticeList.length > 1 ? (
              <div className="ant-carousel v2board-notice-carousel">
                <div className="slick-slider slick-initialized">
                  <div className="slick-list">
                    <div
                      className="slick-track"
                      style={{ transform: `translate3d(-${activeNoticeIndex * 100}%, 0, 0)` }}
                    >
                      {noticeList.map((notice, index) => (
                        <div
                          className={`slick-slide${
                            index === activeNoticeIndex ? ' slick-active slick-current' : ''
                          }`}
                          key={notice.id}
                        >
                          {renderNoticeCard(notice)}
                        </div>
                      ))}
                    </div>
                  </div>
                  <ul className="slick-dots slick-dots-bottom">
                    {noticeList.map((notice, index) => (
                      <li
                        className={index === activeNoticeIndex ? 'slick-active' : ''}
                        key={notice.id}
                      >
                        <button type="button" onClick={() => setNoticeIndex(index)}>
                          {index + 1}
                        </button>
                      </li>
                    ))}
                  </ul>
                </div>
              </div>
            ) : (
              renderNoticeCard(noticeList[0]!)
            )}
          </div>
        </div>
      )}

      <div className="row mb-3 mb-md-0">
        <div className="col-xl-12">
          <div className="block block-rounded js-appear-enabled">
            <div className="block-header block-header-default">
              <h3 className="block-title">{t('dashboard.plan')}</h3>
            </div>
            <div className="block-content">
              {subscribe.isLoading || !hasSubscribeData ? (
                <LegacyLoadingIcon className="font-size-h3 mb-3" />
              ) : hasPlan && sub?.plan ? (
                <div>
                  <div>
                    <div className="justify-content-md-between align-items-md-center">
                      <div>
                        <h3 className="h4 mb-3">{sub.plan!.name}</h3>
                        {sub.expired_at == null ? (
                          <p className="font-size-sm text-muted">{t('dashboard.long_term')}</p>
                        ) : expired ? (
                          <p className="font-size-sm text-muted">
                            <a className="font-w600 text-danger" href="javascript:void(0);">
                              {t('dashboard.expired_label')}
                            </a>
                          </p>
                        ) : (
                          <p className="font-size-sm text-muted">
                            <span>
                              {t('dashboard.expires_in', {
                                date: formatDate(sub.expired_at).replaceAll('-', '/'),
                                day: daysLeft ?? 0,
                              })}
                              {sub.reset_day != null
                                ? sub.reset_day === 0
                                  ? t('dashboard.reset_today')
                                  : t('dashboard.reset_in_days', { days: sub.reset_day })
                                : ''}
                            </span>
                          </p>
                        )}
                        <div className="mb-0">
                          <div className="progress mb-1" style={{ height: 6 }}>
                            <div
                              className={`progress-bar progress-bar-striped progress-bar-animated ${
                                usedPct >= 100
                                  ? 'bg-danger'
                                  : usedPct >= 80
                                    ? 'bg-warning'
                                    : 'bg-success'
                              }`}
                              role="progressbar"
                              style={{ width: `${usedPct}%` }}
                            />
                          </div>
                          <p className="font-size-sm font-w600 mb-3">
                            <span className="font-w700">
                              {t('dashboard.used_traffic', {
                                used: formatBytes(used),
                                total: formatBytes(sub.transfer_enable),
                              })}
                            </span>
                            <span className="font-w700"> </span>
                            <span className="font-w700">
                              {t('dashboard.devices_online', {
                                alive_ip: sub.alive_ip,
                                device_limit: sub.device_limit ?? '∞',
                              })}
                            </span>
                          </p>
                          {resetAvailable && (
                            <div className="mb-4">
                              <button
                                type="button"
                                className="ant-btn ant-btn-primary"
                                onClick={saveResetPackage}
                              >
                                <span>{t('dashboard.buy_reset_package')}</span>
                              </button>
                            </div>
                          )}
                          {canNewPeriod && (
                            <div className="mb-4">
                              <button
                                type="button"
                                className="ant-btn ant-btn-primary"
                                onClick={openNewPeriod}
                              >
                                <span>{t('dashboard.new_period')}</span>
                              </button>
                            </div>
                          )}
                          {expired && (
                            <div className="mb-4">
                              <button
                                type="button"
                                className="ant-btn ant-btn-primary"
                                onClick={() =>
                                  navigate(canRenew && sub.plan_id ? `/plan/${sub.plan_id}` : '/plan')
                                }
                              >
                                <span>
                                  {canRenew
                                    ? t('dashboard.renew_subscribe')
                                    : t('dashboard.buy_subscribe')}
                                </span>
                              </button>
                            </div>
                          )}
                          <div />
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              ) : (
                <a onClick={() => navigate('/plan')}>
                  <div>
                    <div className="text-center">
                      <div>
                        <i className="fa fa-plus fa-2x" aria-hidden />
                      </div>
                      <div className="font-size-sm text-uppercase text-muted pt-2 pb-3">
                        {t('dashboard.shortcut_buy')}
                      </div>
                    </div>
                  </div>
                </a>
              )}
            </div>
          </div>
        </div>
      </div>

      <div className="row mb-3 mb-md-0">
        <div className="col-xl-12">
          <div className="block block-rounded js-appear-enabled">
            <div className="block-header block-header-default">
              <h3 className="block-title">{t('dashboard.shortcuts')}</h3>
            </div>
            <div className="block-content p-0">
              <div className="justify-content-md-between align-items-md-center">
                <div className="mb-3">
                  {shortcuts.map((s) => {
                    return (
                      <div
                        key={s.titleKey}
                        className="v2board-shortcuts-item"
                        onClick={s.onClick ?? (() => navigate(s.to))}
                      >
                        <div>{t(s.titleKey)}</div>
                        <div className="description">
                          {t(s.descKey, { title: window.settings?.title })}
                        </div>
                        <i
                          style={{ float: 'right' }}
                          className={`nav-main-link-icon ${s.iconClass}`}
                          aria-hidden
                        />
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
      <Dialog open={noticeOpen} onOpenChange={setNoticeOpen}>
        <DialogContent className="v2board-ant-modal">
          <div className="ant-modal-header">
            <div className="ant-modal-title">{activeNotice?.title}</div>
          </div>
          {activeNotice?.content && (
            <div className="ant-modal-body">
              <div
                className="notice-content"
                dangerouslySetInnerHTML={{ __html: activeNotice.content }}
              />
            </div>
          )}
        </DialogContent>
      </Dialog>
      {mobileSubscribe ? (
        subscribeOpen && (
          <div className="ant-drawer ant-drawer-bottom v2board-subscribe-drawer">
            <div
              className="ant-drawer-mask"
              onClick={() => setSubscribeOpen(false)}
            />
            <div
              className="ant-drawer-content-wrapper"
              style={subscribeHeight ? { height: subscribeHeight } : undefined}
            >
              <div className="ant-drawer-content">
                <div className="ant-drawer-wrapper-body">
                  <div className="ant-drawer-body">
                    {renderSubscribeBox()}
                  </div>
                </div>
              </div>
            </div>
          </div>
        )
      ) : (
        <Dialog open={subscribeOpen} onOpenChange={setSubscribeOpen}>
          <DialogContent className="v2board-subscribe-dialog" showClose={false} centered>
            <div className="ant-modal-body">{renderSubscribeBox()}</div>
          </DialogContent>
        </Dialog>
      )}
      <Dialog open={qrOpen} onOpenChange={setQrOpen}>
        <DialogContent
          className="v2board-qrcode-dialog"
          showClose={false}
          centered
          zIndex={2000}
        >
          <div className="ant-modal-body">
            <QRCodeCanvas value={subscribeUrl} size={128} />
            <div style={{ marginTop: 10 }}>
              {t('dashboard.qrcode_client_tip')}
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

function getSubscribeTargets(url: string) {
  const title = window.settings?.title;
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

function isLegacyRenewable(subscribe: ReturnType<typeof useSubscribe>['data']) {
  if (!subscribe?.plan?.renew) return false;
  return Boolean(subscribe.plan.show || !isLegacyExpired(subscribe.expired_at));
}
