import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useState, type AnchorHTMLAttributes } from 'react';
import { useOrders, useCancelOrderMutation } from '@/lib/queries';
import { formatLegacyDateTime, formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyEmpty } from '@/components/legacy-empty';
import { isLegacyMobile } from '@/lib/legacy-settings';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';
import { useFixedColumnRowHeights } from '@/lib/use-fixed-column-row-heights';
import { legacyHref } from '@/lib/legacy-href';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

const STATUS_LABEL: Record<number, { key: string; status: string }> = {
  0: { key: 'order.status_unpaid', status: 'error' },
  1: { key: 'order.status_processing', status: 'processing' },
  2: { key: 'order.status_cancelled', status: 'default' },
  3: { key: 'order.status_completed', status: 'success' },
  4: { key: 'order.status_credit', status: 'default' },
};

const PERIOD_LABEL: Record<string, string> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
};

function legacyDisabledAnchorProps(disabled: boolean): AnchorHTMLAttributes<HTMLAnchorElement> {
  return { disabled } as AnchorHTMLAttributes<HTMLAnchorElement>;
}

export default function OrdersPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const ordersQuery = useOrders();
  const { data, isFetching } = ordersQuery;
  const loading = useLegacyFetchLoading(isFetching);
  const cancel = useCancelOrderMutation();
  const orders = data ?? [];
  const [hoverKey, setHoverKey] = useState<number | null>(null);
  const [activeMobileKey, setActiveMobileKey] = useState<number | null>(null);
  const mobile = isLegacyMobile();
  const { bodyRef, onScroll, scrollPositionClassName } = useTableScrollPosition(orders.length);
  const { mainTableRef, fixedTableRef } = useFixedColumnRowHeights(orders.length);
  const desktopTableClassName = [
    'ant-table',
    'ant-table-default',
    orders.length ? '' : 'ant-table-empty',
    scrollPositionClassName,
  ].filter(Boolean).join(' ');

  const onCancelOrder = (tradeNo: string) => {
    void legacyConfirm({
      title: t('common.attention'),
      content: t('order.cancel_confirm'),
      okText: t('order.cancel'),
      okButtonProps: { loading: cancel.isPending },
      onOk: () => {
        void cancel.mutateAsync(tradeNo).catch(() => {});
      },
    });
  };

  return (
    // The original builds this as `"block block-rounded  ".concat(...)` — note the
    // DOUBLE space before the loading class (umi.js @2306135), so the rendered class
    // attribute has two spaces; reproduced verbatim.
    <div className={`block block-rounded  ${loading ? 'block-mode-loading' : ''}`}>
      <div className="bg-white">
        {mobile ? (
          <div className="am-list">
            <div className="am-list-body">
              {orders.map((order, index) => {
                const status = STATUS_LABEL[order.status];
                return (
                  <div
                    key={index}
                    className={`am-list-item am-list-item-middle${activeMobileKey === index ? ' am-list-item-active' : ''}`}
                    onTouchStart={() => setActiveMobileKey(index)}
                    onTouchMove={() => setActiveMobileKey(null)}
                    onTouchEnd={() => setActiveMobileKey(null)}
                    onTouchCancel={() => setActiveMobileKey(null)}
                    onMouseDown={() => setActiveMobileKey(index)}
                    onMouseUp={() => setActiveMobileKey(null)}
                    onMouseLeave={() => setActiveMobileKey(null)}
                    onClick={() => navigate(`/order/${order.trade_no}`)}
                  >
                    <div className="am-list-line am-list-line-multiple">
                      <div className="am-list-content">
                        {order.plan?.name}{' '}
                        <div className="am-list-brief">{formatLegacyDateTime(order.created_at)}</div>
                      </div>
                      <div className="am-list-extra">
                        <div>
                          <div>{(order.total_amount / 100).toFixed(2)}</div>
                          <div>
                            <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                              <span className={`ant-badge-status-dot${status ? ` ant-badge-status-${status.status}` : ''}`} />
                              <span className="ant-badge-status-text" />
                            </span>
                            {status ? t(status.key) : ''}
                          </div>
                        </div>
                      </div>
                      <div className="am-list-arrow am-list-arrow-horizontal" aria-hidden />
                    </div>
                    {/* antd-mobile ListItem always renders a trailing ripple div
                        (hidden until a touch triggers the cover animation). */}
                    <div className="am-list-ripple" style={{ display: 'none' }} />
                  </div>
                );
              })}
            </div>
          </div>
        ) : (
          <div className="ant-table-wrapper">
            {/* antd v3 Table always wraps its content in Spin (loading defaults to
                false); with no loading prop the orders table never spins, so the
                spinner div / ant-spin-blur are absent — only the two static wrappers. */}
            <div className="ant-spin-nested-loading">
              <div className="ant-spin-container">
            <div className={desktopTableClassName}>
              <div className="ant-table-content">
                <div className="ant-table-scroll">
                  <div
                    ref={bodyRef}
                    tabIndex={-1}
                    className="ant-table-body"
                    style={{ overflowX: 'scroll', WebkitTransform: 'translate3d (0, 0, 0)' }}
                    onScroll={onScroll}
                  >
                    <table
                      ref={mainTableRef}
                      className="ant-table-fixed"
                      style={{ width: 900 }}
                    >
                      <colgroup>
                        <col />
                        <col />
                        <col />
                        <col />
                        <col />
                        <col />
                      </colgroup>
                      <thead className="ant-table-thead">
                        <tr>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('order.trade_no_col')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="ant-table-align-center" style={{ textAlign: 'center' }}>
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('order.period')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="ant-table-align-right" style={{ textAlign: 'right' }}>
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('order.amount')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('order.status')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('order.created_at')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th
                            className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"
                            style={{ textAlign: 'right' }}
                          >
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('order.action_col')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                        </tr>
                      </thead>
                      <tbody className="ant-table-tbody">
                        {orders.map((order, index) => {
                          const status = STATUS_LABEL[order.status];
                          const periodLabelKey = order.period ? PERIOD_LABEL[order.period] : undefined;
                          const periodLabel = periodLabelKey ? t(periodLabelKey) : undefined;
                          return (
                            <tr
                              className={`ant-table-row ant-table-row-level-0${hoverKey === index ? ' ant-table-row-hover' : ''}`}
                              data-row-key={index}
                              key={index}
                              onMouseEnter={() => setHoverKey(index)}
                              onMouseLeave={() => setHoverKey(null)}
                            >
                              <td className="">
                                <a
                                  ref={legacyHref()}
                                  onClick={() => navigate(`/order/${order.trade_no}`)}
                                >
                                  {order.trade_no}
                                </a>
                              </td>
                              <td className="" style={{ textAlign: 'center' }}>
                                <span className="ant-tag">{periodLabel}</span>
                              </td>
                              <td className="" style={{ textAlign: 'right' }}>
                                {(order.total_amount / 100).toFixed(2)}
                              </td>
                              <td className="">
                                <div>
                                  <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                    <span className={`ant-badge-status-dot${status ? ` ant-badge-status-${status.status}` : ''}`} />
                                    <span className="ant-badge-status-text" />
                                  </span>
                                  {status ? t(status.key) : ''}
                                </div>
                              </td>
                              <td className="">{formatLegacyDateMinuteSlash(order.created_at)}</td>
                              <td
                                className="ant-table-fixed-columns-in-body"
                                style={{ textAlign: 'right' }}
                              >
                                <div>
                                  <a
                                    ref={legacyHref()}
                                    {...legacyDisabledAnchorProps(order.status === 2)}
                                    onClick={() => navigate(`/order/${order.trade_no}`)}
                                  >
                                    {t('order.return')}
                                  </a>
                                  <div className="ant-divider ant-divider-vertical" role="separator" />
                                  <a
                                    ref={legacyHref()}
                                    {...legacyDisabledAnchorProps(order.status !== 0)}
                                    onClick={() => void onCancelOrder(order.trade_no)}
                                  >
                                    {t('common.cancel')}
                                  </a>
                                </div>
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                  {orders.length === 0 && (
                    <div className="ant-table-placeholder">
                      <LegacyEmpty />
                    </div>
                  )}
                </div>
                <div className="ant-table-fixed-right">
                  <div
                    className="ant-table-body-outer"
                    style={{ WebkitTransform: 'translate3d (0, 0, 0)' }}
                  >
                    <div className="ant-table-body-inner">
                      <table ref={fixedTableRef} className="ant-table-fixed">
                        <colgroup>
                          <col />
                        </colgroup>
                        <thead className="ant-table-thead">
                          <tr>
                            <th
                              className="ant-table-align-right ant-table-row-cell-last"
                              style={{ textAlign: 'right' }}
                            >
                              <span className="ant-table-header-column">
                                <div>
                                  <span className="ant-table-column-title">{t('order.action_col')}</span>
                                  <span className="ant-table-column-sorter" />
                                </div>
                              </span>
                            </th>
                          </tr>
                        </thead>
                        <tbody className="ant-table-tbody">
                          {orders.map((order, index) => (
                            <tr
                              className={`ant-table-row ant-table-row-level-0${hoverKey === index ? ' ant-table-row-hover' : ''}`}
                              data-row-key={index}
                              key={index}
                              onMouseEnter={() => setHoverKey(index)}
                              onMouseLeave={() => setHoverKey(null)}
                            >
                              <td className="" style={{ textAlign: 'right' }}>
                                <div>
                                  <a
                                    ref={legacyHref()}
                                    {...legacyDisabledAnchorProps(order.status === 2)}
                                    onClick={() => navigate(`/order/${order.trade_no}`)}
                                  >
                                    {t('order.return')}
                                  </a>
                                  <div className="ant-divider ant-divider-vertical" role="separator" />
                                  <a
                                    ref={legacyHref()}
                                    {...legacyDisabledAnchorProps(order.status !== 0)}
                                    onClick={() => void onCancelOrder(order.trade_no)}
                                  >
                                    {t('common.cancel')}
                                  </a>
                                </div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                </div>
              </div>
            </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
