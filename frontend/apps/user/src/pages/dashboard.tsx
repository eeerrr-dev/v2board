import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { TouchEvent as ReactTouchEvent } from 'react';
import { createPortal } from 'react-dom';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import QRCode from 'qrcode.react';
import { user } from '@v2board/api-client';
import type { Notice } from '@v2board/types';
import { apiClient } from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { AntBtn } from '@/components/ant-btn';
import {
  useCommConfig,
  useNewPeriodMutation,
  useNotices,
  useSubscribe,
  useUserStat,
} from '@/lib/queries';
import { formatBytes } from '@v2board/config/format';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyLoadingIcon } from '@/components/legacy-loading-icon';
import { isLegacyMobile, legacyCopyText } from '@/lib/legacy-settings';
import { legacyHref } from '@/lib/legacy-href';
import { toast } from '@/lib/legacy-toast';
import { useTransitionStatus } from '@/lib/use-transition-status';
import { lockLegacyDrawerBodyScroll } from '@/lib/legacy-body-scroll';

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
  // antd's Carousel is react-slick with infinite loop: one clone of the last slide is
  // prepended and a post-clone set is appended, so the real slides occupy track
  // positions 1..n. `slidePos` is that track position (0 = prepended clone, n+1 =
  // appended clone); the snap-on-transition-end below resets onto the matching real slide.
  const [slidePos, setSlidePos] = useState(1);
  const [noticeTransition, setNoticeTransition] = useState(true);
  // Non-null while a horizontal touch-drag is in progress: the px the track follows the
  // finger (react-slick's swipe; mouse-drag stays disabled via draggable:false).
  const [dragOffset, setDragOffset] = useState<number | null>(null);
  const [subscribeOpen, setSubscribeOpen] = useState(false);
  const subscribeDrawerStatus = useTransitionStatus(subscribeOpen, 300);
  const [qrOpen, setQrOpen] = useState(false);
  const [savingResetPackage, setSavingResetPackage] = useState(false);
  const [subscribeHeight, setSubscribeHeight] = useState<number>();
  const subscribeBoxRef = useRef<HTMLDivElement>(null);
  const subscribeDrawerRef = useRef<HTMLDivElement>(null);
  const noticePausedRef = useRef(false);
  const touchRef = useRef<{ x: number; y: number; width: number; dir: 'h' | 'v' | null } | null>(
    null,
  );

  const pendingOrderCount = stat.data?.pending_orders ?? 0;
  const openTicketCount = stat.data?.pending_tickets ?? 0;
  const sub = subscribe.data;

  useEffect(() => {
    // Faithful to the packaged theme's notice handler:
    //   if (t.length) { var n = t.find(e => -1 !== e.tags.indexOf('弹窗')); console.log(n), n && modalVisible(n) }
    // It accesses e.tags.indexOf with no null guard and ships a leftover console.log.
    // The try/catch mirrors the dva saga that swallowed the TypeError thrown when a
    // notice has null tags (v2_notice.tags is `varchar(255) DEFAULT NULL`), so the
    // dashboard stays up — exactly as the original behaved (no popup, no crash).
    const list = notices.data;
    if (!list?.length) return;
    try {
      const popup = list.find((notice) => notice.tags!.indexOf('弹窗') !== -1);
      console.log(popup);
      if (popup) {
        setActiveNotice(popup);
        setNoticeOpen(true);
      }
    } catch {}
  }, [notices.data]);

  useEffect(() => {
    const count = notices.data?.length ?? 0;
    // Reset to the first real slide whenever the notice set changes (re-init parity).
    setSlidePos(1);
    if (count <= 1) return;
    const id = window.setInterval(() => {
      if (noticePausedRef.current) return;
      // Advance forward only; the appended clone + snap below gives a seamless wrap.
      setSlidePos((pos) => pos + 1);
    }, 3000);
    return () => window.clearInterval(id);
  }, [notices.data?.length]);

  // After advancing onto the clone, the transition is disabled for the reset to the
  // real first slide; re-enable it on the next frame so the snap stays invisible.
  useEffect(() => {
    if (noticeTransition) return;
    const id = requestAnimationFrame(() => setNoticeTransition(true));
    return () => cancelAnimationFrame(id);
  }, [noticeTransition]);

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
  const usedPctRounded = Math.round(usedPct * 100) / 100;
  const daysLeft = legacyDaysUntil(sub?.expired_at);
  const expired = isLegacyExpired(sub?.expired_at ?? null);
  const canRenew = isLegacyRenewable(sub);
  const resetAvailable = Boolean(
    hasPlan &&
      sub?.plan?.reset_price &&
      usedPctRounded >= 80 &&
      !expired,
  );
  const shouldShowTrafficAlert = Boolean(usedPctRounded >= 80 && usedPctRounded < 100 && !expired);
  const trafficAlertResetAvailable = Boolean(sub?.plan?.reset_price);
  const canNewPeriod = Boolean(
    hasPlan &&
      sub?.allow_new_period &&
      usedPctRounded >= 100 &&
      !expired,
  );
  const noticeList = notices.data ?? [];
  // Map the track position back to the real slide the dots highlight (positions 0 and
  // n+1 are the clones of the last and first slides respectively).
  const activeDotIndex =
    noticeList.length > 0 ? (slidePos - 1 + noticeList.length) % noticeList.length : 0;
  // react-slick measures the list and sizes every slide to listWidth / slidesToShow
  // (slidesToShow = 1), then widens the track to totalSlides * slideWidth. We reproduce
  // those inline pixel widths; slideWidth = 0 until the list is measured (before paint).
  const noticeListRef = useRef<HTMLDivElement>(null);
  const [slideWidth, setSlideWidth] = useState(0);
  useLayoutEffect(() => {
    if (noticeList.length <= 1) return;
    const measure = () => {
      const list = noticeListRef.current;
      if (list) setSlideWidth(Math.ceil(list.getBoundingClientRect().width));
    };
    measure();
    let timer: number | undefined;
    const onResize = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(measure, 150);
    };
    window.addEventListener('resize', onResize);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener('resize', onResize);
    };
  }, [noticeList.length]);
  // Infinite mode clones the last slide before the reel and every slide after it
  // (getPreClones = 1, getPostClones = slideCount), so the reel holds 2n + 1 slides.
  const trackWidth = slideWidth ? (noticeList.length * 2 + 1) * slideWidth : undefined;
  const subscribeUrl = sub?.subscribe_url as string;
  const legacySub = sub!;

  // react-slick swipe (draggable:false ⇒ touch only). touch-action:pan-y lets the page
  // scroll vertically; we own horizontal gestures, follow the finger, then advance one
  // slide when the drag passes listWidth / touchThreshold (5), else snap back.
  const onNoticeTouchStart = (event: ReactTouchEvent<HTMLDivElement>) => {
    const point = event.touches[0]!;
    touchRef.current = {
      x: point.clientX,
      y: point.clientY,
      width: event.currentTarget.offsetWidth,
      dir: null,
    };
    noticePausedRef.current = true;
  };

  const onNoticeTouchMove = (event: ReactTouchEvent<HTMLDivElement>) => {
    const start = touchRef.current;
    if (!start) return;
    const point = event.touches[0]!;
    const dx = point.clientX - start.x;
    const dy = point.clientY - start.y;
    if (start.dir === null) {
      if (Math.abs(dx) < 5 && Math.abs(dy) < 5) return;
      start.dir = Math.abs(dx) > Math.abs(dy) ? 'h' : 'v';
    }
    if (start.dir !== 'h') return;
    setDragOffset(dx);
  };

  const onNoticeTouchEnd = (event: ReactTouchEvent<HTMLDivElement>) => {
    const start = touchRef.current;
    touchRef.current = null;
    noticePausedRef.current = false;
    setDragOffset(null);
    if (!start || start.dir !== 'h') return;
    const dx = (event.changedTouches[0]?.clientX ?? start.x) - start.x;
    if (Math.abs(dx) > start.width / 5) {
      setSlidePos((pos) => (dx < 0 ? pos + 1 : pos - 1));
    }
  };

  const noticeTrackLeft = -slidePos * slideWidth + (dragOffset ?? 0);
  const noticeTrackTransform = `translate3d(${noticeTrackLeft}px, 0px, 0px)`;
  const noticeTrackTransition =
    noticeTransition && dragOffset == null ? 'transform 500ms ease' : '';
  const noticeTrackWebkitTransition =
    noticeTransition && dragOffset == null ? '-webkit-transform 500ms ease' : '';

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
      // Original: push(renewable(d) ? "/plan/"+d.plan_id : "/plan") — no plan_id guard
      // (umi.js @1165700); a renewable sub always carries a plan_id.
      to: canRenew ? `/plan/${sub?.plan_id}` : '/plan',
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

  const saveResetPackage = () => {
    if (!sub) return;
    void legacyConfirm({
      title: t('dashboard.reset_package_confirm_title'),
      content: t('dashboard.reset_package_confirm_content'),
      okText: savingResetPackage ? <LegacyLoadingIcon /> : t('common.confirm'),
      cancelText: t('common.cancel'),
      maskClosable: true,
      okButtonProps: { disabled: savingResetPackage },
      onOk: () => {
        setSavingResetPackage(true);
        void user
          .saveOrder(apiClient, {
            period: 'reset_price',
            plan_id: sub.plan_id as number,
          })
          .then((tradeNo) => navigate(`/order/${tradeNo}`))
          .catch(() => {})
          .finally(() => setSavingResetPackage(false));
      },
    });
  };

  const openNewPeriod = () => {
    void legacyConfirm({
      title: t('dashboard.new_period_confirm_title'),
      content: t('dashboard.new_period_confirm_content'),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      maskClosable: true,
      onOk: () => {
        void newPeriod
          .mutateAsync()
          .then(() => {
            void subscribe.refetch();
            toast.success('提前开启流量周期成功');
            navigate('/dashboard');
          })
          .catch(() => {});
      },
    });
  };

  const renderSubscribeBox = () => (
    // Original uses only the CSS-module hashes (umi.js @36900): box
    // `oneClickSubscribe___2t9Xg`, items `item___yrtOv …`. globals.css styles those
    // hashes directly; no extra v2board-* class is present in the original DOM.
    <div className="oneClickSubscribe___2t9Xg" ref={subscribeBoxRef}>
      <div className="item___yrtOv subsrcibe-for-link" onClick={copyUrl}>
        <div>
          <i className="fa fa-copy mr-2" />
        </div>
        <div>{t('dashboard.copy_subscribe')}</div>
      </div>
      <div className="item___yrtOv subscribe-for-qrcode" onClick={() => setQrOpen(true)}>
        <div>
          <i className="fa fa-qrcode mr-2" />
        </div>
        <div>{t('dashboard.scan_qrcode_subscribe')}</div>
      </div>
      {getSubscribeTargets(subscribeUrl).map((target) => (
        <div
          key={Math.random()}
          className={`item___yrtOv ${target.title.replace(' ', '-').toLowerCase()}`}
          onClick={() => {
            window.location.href = target.href;
          }}
        >
          <div>
            <img
              src={`${window.settings?.assets_path || ''}/./images/icon/${target.title}.png`}
            />
          </div>
          <div>{t('dashboard.import_to')} {target.title}</div>
        </div>
      ))}
      <div style={{ padding: 10 }}>
        <AntBtn
          type="button"
          className="ant-btn ant-btn-primary ant-btn-lg ant-btn-block"
          onClick={() => navigate('/knowledge')}
        >
          <span>{t('dashboard.use_tutorial')}</span>
        </AntBtn>
      </div>
    </div>
  );

  const renderNoticeCard = (notice: Notice) => (
    <a
      className="block block-rounded bg-image mb-0 v2board-bg-pixels"
      ref={legacyHref('javascript:void(0)')}
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
        <p className="font-w600 text-white-75">{formatLegacyDate(notice.created_at)}</p>
      </div>
    </a>
  );

  // react-slick (via antd Carousel) nests every slide's child under a slide wrapper and an
  // inline-block content div, so each slide is slick-slide > div > div[width:100%] > card.
  const renderNoticeSlide = (notice: Notice) => (
    <div>
      <div tabIndex={-1} style={{ width: '100%', display: 'inline-block' }}>
        {renderNoticeCard(notice)}
      </div>
    </div>
  );

  const mobileSubscribe = isLegacyMobile();

  useEffect(() => {
    if (!mobileSubscribe || !subscribeOpen) return;
    return lockLegacyDrawerBodyScroll();
  }, [mobileSubscribe, subscribeOpen]);

  useEffect(() => {
    if (!mobileSubscribe || !subscribeOpen || subscribeDrawerStatus === 'exited') return;
    subscribeDrawerRef.current?.focus();
  }, [mobileSubscribe, subscribeOpen, subscribeDrawerStatus]);

  return (
    <>
      {pendingOrderCount > 0 && (
        <div className="alert alert-danger" role="alert">
          <p className="mb-0">
            {t('dashboard.alert_pending_order')}{' '}
            <a
              className="alert-link"
              ref={legacyHref('javascript:void(0)')}
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
              ref={legacyHref('javascript:void(0)')}
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
            {trafficAlertResetAvailable && (
              <a onClick={saveResetPackage}>
                <strong>购买流量重置包</strong>
              </a>
            )}
          </p>
        </div>
      )}
      {noticeList.length > 0 && (
        <div className="row mb-3 mb-md-0">
          <div className="col-12 mb-sm-4">
            {noticeList.length > 1 ? (
              <div className="ant-carousel">
                <div className="slick-slider slick-initialized" dir="ltr">
                  <div
                    ref={noticeListRef}
                    className="slick-list"
                    onTouchStart={onNoticeTouchStart}
                    onTouchMove={onNoticeTouchMove}
                    onTouchEnd={onNoticeTouchEnd}
                  >
                    <div
                      className="slick-track"
                      style={{
                        opacity: 1,
                        transition: noticeTrackTransition,
                        WebkitTransition: noticeTrackWebkitTransition,
                        WebkitTransform: noticeTrackTransform,
                        transform: noticeTrackTransform,
                        msTransform: `translateX(${noticeTrackLeft}px)`,
                        width: trackWidth,
                      }}
                      onMouseEnter={() => {
                        noticePausedRef.current = true;
                      }}
                      onMouseOver={() => {
                        noticePausedRef.current = true;
                      }}
                      onMouseLeave={() => {
                        noticePausedRef.current = false;
                      }}
                      onTransitionEnd={(event) => {
                        if (event.target !== event.currentTarget) return;
                        if (slidePos === noticeList.length + 1) {
                          setNoticeTransition(false);
                          setSlidePos(1);
                        } else if (slidePos === 0) {
                          setNoticeTransition(false);
                          setSlidePos(noticeList.length);
                        }
                      }}
                    >
                      {/* react-slick stamps every slide (real + clones) with data-index,
                          tabIndex="-1" and aria-hidden=!slick-active. Infinite mode prepends
                          one clone of the last slide (data-index -1) and appends a clone of
                          every slide (data-index n..2n-1); only the first appended clone is
                          ever reached before the loop snaps back. Real slides also carry
                          inline outline:none; clones do not. */}
                      <div
                        className={`slick-slide slick-cloned${
                          slidePos === 0 ? ' slick-active slick-current' : ''
                        }`}
                        data-index={-1}
                        tabIndex={-1}
                        aria-hidden={slidePos !== 0}
                        style={{ width: slideWidth || undefined }}
                        key="slick-clone-last"
                      >
                        {renderNoticeSlide(noticeList[noticeList.length - 1]!)}
                      </div>
                      {noticeList.map((notice, index) => (
                        <div
                          className={`slick-slide${
                            index + 1 === slidePos ? ' slick-active slick-current' : ''
                          }`}
                          data-index={index}
                          tabIndex={-1}
                          aria-hidden={index + 1 !== slidePos}
                          style={{ outline: 'none', width: slideWidth || undefined }}
                          key={Math.random()}
                        >
                          {renderNoticeSlide(notice)}
                        </div>
                      ))}
                      {noticeList.map((notice, index) => (
                        <div
                          className={`slick-slide slick-cloned${
                            index === 0 && slidePos === noticeList.length + 1
                              ? ' slick-active slick-current'
                              : ''
                          }`}
                          data-index={noticeList.length + index}
                          tabIndex={-1}
                          aria-hidden={!(index === 0 && slidePos === noticeList.length + 1)}
                          style={{ width: slideWidth || undefined }}
                          key={`slick-clone-${notice.id}`}
                        >
                          {renderNoticeSlide(notice)}
                        </div>
                      ))}
                    </div>
                  </div>
                  {/* react-slick's appendDots renders `<ul style="display:block">` (matching
                      the .slick-dots block layout), and its customPaging button has no type
                      attribute. */}
                  <ul className="slick-dots slick-dots-bottom" style={{ display: 'block' }}>
                    {noticeList.map((notice, index) => (
                      <li
                        className={index === activeDotIndex ? 'slick-active' : ''}
                        key={notice.id}
                      >
                        <button onClick={() => setSlidePos(index + 1)}>{index + 1}</button>
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
                // Original's LoadingIcon (umi.js v32e) wraps the antd loading Icon in a
                // `<div className>`: createElement("div",{className},createElement(Icon,{type:"loading"})).
                <div className="font-size-h3 mb-3">
                  <LegacyLoadingIcon />
                </div>
              ) : hasPlan ? (
                <div>
                  <div>
                    <div className="justify-content-md-between align-items-md-center">
                      <div>
                        <h3 className="h4 mb-3">{legacySub.plan!.name}</h3>
                        {legacySub.expired_at === null ? (
                          <p className="font-size-sm text-muted">{t('dashboard.long_term')}</p>
                        ) : expired ? (
                          <p className="font-size-sm text-muted">
                            <a className="font-w600 text-danger" ref={legacyHref()}>
                              {t('dashboard.expired_label')}
                            </a>
                          </p>
                        ) : (
                          <p className="font-size-sm text-muted">
                            <span>
                              {t('dashboard.expires_in', {
                                date: formatLegacyDate(legacySub.expired_at).replaceAll('-', '/'),
                                day: daysLeft,
                              })}
                              {legacySub.reset_day !== null
                                ? legacySub.reset_day === 0
                                  ? t('dashboard.reset_today')
                                  : t('dashboard.reset_in_days', { reset_day: legacySub.reset_day })
                                : ''}
                            </span>
                          </p>
                        )}
                        <div className="mb-0">
                          <div className="progress mb-1" style={{ height: 6 }}>
                            <div
                              className={`progress-bar progress-bar-striped progress-bar-animated ${
                                usedPctRounded >= 100
                                  ? 'bg-danger'
                                  : usedPctRounded >= 80
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
                                total: formatBytes(legacySub.transfer_enable),
                              })}
                            </span>
                            <span className="font-w700">{'  '}</span>
                            <span className="font-w700">
                              {t('dashboard.devices_online', {
                                alive_ip: legacySub.alive_ip,
                                device_limit: legacySub.device_limit ?? '∞',
                              })}
                            </span>
                          </p>
                          {resetAvailable && (
                            <div className="mb-4">
                              <AntBtn
                                type="button"
                                className="ant-btn ant-btn-primary"
                                onClick={saveResetPackage}
                              >
                                {t('dashboard.buy_reset_package')}
                              </AntBtn>
                            </div>
                          )}
                          {canNewPeriod && (
                            <div className="mb-4">
                              <AntBtn
                                type="button"
                                className="ant-btn ant-btn-primary"
                                onClick={openNewPeriod}
                              >
                                {t('dashboard.new_period')}
                              </AntBtn>
                            </div>
                          )}
                          {expired && (
                            <div className="mb-4">
                              <AntBtn
                                type="button"
                                className="ant-btn ant-btn-primary"
                                onClick={() => navigate(canRenew ? `/plan/${legacySub.plan_id}` : '/plan')}
                              >
                                {canRenew
                                  ? t('dashboard.renew_subscribe')
                                  : t('dashboard.buy_subscribe')}
                              </AntBtn>
                            </div>
                          )}
                        </div>
                      </div>
                      <div />
                    </div>
                  </div>
                </div>
              ) : (
                <a onClick={() => navigate('/plan')}>
                  <div>
                    <div className="text-center">
                      <div>
                        <i className="fa fa-plus fa-2x" />
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
                          {t(s.descKey)}
                          {s.descKey === 'dashboard.shortcut_tutorial_desc' ? (
                            <> {window.settings?.title}</>
                          ) : null}
                        </div>
                        <i
                          style={{ float: 'right' }}
                          className={`nav-main-link-icon ${s.iconClass}`}
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
      <Dialog
        open={noticeOpen}
        onOpenChange={(open) => {
          setNoticeOpen(open);
          if (!open) setActiveNotice(null);
        }}
      >
        <DialogContent title={activeNotice?.title} maskClosable footer={false}>
          {activeNotice?.content && (
            <div
              className="notice-content"
              dangerouslySetInnerHTML={{ __html: activeNotice.content }}
            />
          )}
        </DialogContent>
      </Dialog>
      {mobileSubscribe ? (
        subscribeDrawerStatus !== 'exited' && createPortal(
          <div
            ref={subscribeDrawerRef}
            tabIndex={-1}
            className={`ant-drawer ant-drawer-bottom${
              subscribeDrawerStatus === 'entered' ? ' ant-drawer-open' : ''
            }`}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                event.stopPropagation();
                setSubscribeOpen(false);
              }
            }}
          >
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
                  <div className="ant-drawer-body" style={{ padding: 0 }}>
                    {renderSubscribeBox()}
                  </div>
                </div>
              </div>
            </div>
          </div>,
          document.body,
        )
      ) : (
        <Dialog open={subscribeOpen} onOpenChange={setSubscribeOpen}>
          <DialogContent
            closable={false}
            footer={false}
            width={300}
            centered
            bodyStyle={{ padding: 0 }}
          >
            {renderSubscribeBox()}
          </DialogContent>
        </Dialog>
      )}
      <Dialog open={qrOpen} onOpenChange={setQrOpen}>
        <DialogContent
          closable={false}
          footer={false}
          width={300}
          centered
          style={{ textAlign: 'center' }}
          zIndex={2000}
        >
          <QRCode
            value={subscribeUrl}
            renderAs="canvas"
          />
          <div style={{ marginTop: 10 }}>{t('dashboard.qrcode_client_tip')}</div>
        </DialogContent>
      </Dialog>
    </>
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

function formatLegacyDate(timestamp: number | string | null | undefined) {
  const d = new Date(Number(timestamp) * 1000);
  if (Number.isNaN(d.getTime())) return 'Invalid date';
  const pad = (n: number) => `${n}`.padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function legacyDaysUntil(timestamp: number | string | null | undefined) {
  return ((Number(timestamp) - Math.floor(Date.now() / 1000)) / 86400).toFixed(0);
}

function isLegacyRenewable(subscribe: ReturnType<typeof useSubscribe>['data']) {
  if (!subscribe?.plan?.renew) return false;
  return Boolean(!subscribe.plan.show || !isLegacyExpired(subscribe.expired_at));
}
