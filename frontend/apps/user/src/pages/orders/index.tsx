import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useOrders, useCancelOrderMutation } from '@/lib/queries';
import { formatDateMinuteSlash, formatDateTime } from '@v2board/config/format';
import { legacyConfirm } from '@/components/legacy-confirm';
import { LegacyEmpty } from '@/components/legacy-empty';
import { isLegacyMobile } from '@/lib/legacy-settings';

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

export default function OrdersPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isFetching } = useOrders();
  const cancel = useCancelOrderMutation();
  const orders = data ?? [];
  const mobile = isLegacyMobile();
  const desktopTableClassName = [
    'ant-table',
    'ant-table-default',
    orders.length ? '' : 'ant-table-empty',
    'ant-table-scroll-position-left',
  ].filter(Boolean).join(' ');

  const onCancelOrder = async (tradeNo: string) => {
    const ok = await legacyConfirm({
      title: t('common.attention'),
      content: t('order.cancel_confirm'),
      okText: t('order.cancel'),
      cancelText: t('common.cancel'),
    });
    if (!ok) return;
    try {
      await cancel.mutateAsync(tradeNo);
    } catch {}
  };

  return (
    <div className={`block block-rounded ${isFetching ? 'block-mode-loading' : ''}`}>
      <div className="bg-white">
        {mobile ? (
          <div className="am-list">
            <div className="am-list-body">
              {orders.map((order) => {
                const status = STATUS_LABEL[order.status] ?? STATUS_LABEL[0]!;
                return (
                  <div
                    className="am-list-item am-list-item-middle"
                    key={order.trade_no}
                    onClick={() => navigate(`/order/${order.trade_no}`)}
                  >
                    <div className="am-list-line am-list-line-multiple">
                      <div className="am-list-content">
                        {order.plan?.name}{' '}
                        <div className="am-list-brief">{formatDateTime(order.created_at)}</div>
                      </div>
                      <div className="am-list-extra">
                        <div>{(order.total_amount / 100).toFixed(2)}</div>
                        <div>
                          <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                            <span className={`ant-badge-status-dot ant-badge-status-${status.status}`} />
                          </span>
                          {t(status.key)}
                        </div>
                      </div>
                      <div className="am-list-arrow am-list-arrow-horizontal" aria-hidden />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        ) : (
          <div className="ant-table-wrapper">
            <div className={desktopTableClassName}>
              <div className="ant-table-content">
                <div className="ant-table-scroll">
                  <div className="ant-table-body" style={{ overflowX: 'auto' }}>
                    <table style={{ minWidth: 900, width: '100%', tableLayout: 'auto' }}>
                      <thead className="ant-table-thead">
                        <tr>
                          <th className="ant-table-cell">{t('order.trade_no_col')}</th>
                          <th className="ant-table-cell ant-table-cell-align-center">{t('order.period')}</th>
                          <th className="ant-table-cell ant-table-cell-align-right">{t('order.amount')}</th>
                          <th className="ant-table-cell">{t('order.status')}</th>
                          <th className="ant-table-cell">{t('order.created_at')}</th>
                          <th className="ant-table-cell ant-table-cell-align-right ant-table-fixed-columns-in-body">
                            {t('order.action_col')}
                          </th>
                        </tr>
                      </thead>
                      <tbody className="ant-table-tbody">
                        {orders.length ? (
                          orders.map((order) => {
                            const status = STATUS_LABEL[order.status] ?? STATUS_LABEL[0]!;
                            const periodKey = order.period ? PERIOD_LABEL[order.period] : null;
                            return (
                              <tr className="ant-table-row ant-table-row-level-0" key={order.trade_no}>
                                <td className="ant-table-cell">
                                  <a
                                    href="javascript:void(0);"
                                    onClick={() => navigate(`/order/${order.trade_no}`)}
                                  >
                                    {order.trade_no}
                                  </a>
                                </td>
                                <td className="ant-table-cell ant-table-cell-align-center">
                                  <span className="ant-tag">
                                    {periodKey ? t(periodKey) : ''}
                                  </span>
                                </td>
                                <td className="ant-table-cell ant-table-cell-align-right">
                                  {(order.total_amount / 100).toFixed(2)}
                                </td>
                                <td className="ant-table-cell">
                                  <div>
                                    <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                      <span className={`ant-badge-status-dot ant-badge-status-${status.status}`} />
                                    </span>
                                    {t(status.key)}
                                  </div>
                                </td>
                                <td className="ant-table-cell">{formatDateMinuteSlash(order.created_at)}</td>
                                <td className="ant-table-cell ant-table-cell-align-right ant-table-fixed-columns-in-body">
                                  <div>
                                    <a
                                      href="javascript:void(0);"
                                      {...(order.status === 2 ? { disabled: true } : {})}
                                      onClick={() => navigate(`/order/${order.trade_no}`)}
                                    >
                                      {t('order.return')}
                                    </a>
                                    <span className="ant-divider ant-divider-vertical" role="separator" />
                                    <a
                                      href="javascript:void(0);"
                                      {...(order.status !== 0 ? { disabled: true } : {})}
                                      onClick={() => void onCancelOrder(order.trade_no)}
                                    >
                                      {t('common.cancel')}
                                    </a>
                                  </div>
                                </td>
                              </tr>
                            );
                          })
                        ) : (
                          <tr className="ant-table-placeholder">
                            <td className="ant-table-cell" colSpan={6}>
                              <LegacyEmpty />
                            </td>
                          </tr>
                        )}
                      </tbody>
                    </table>
                  </div>
                </div>
                {orders.length > 0 && (
                  <div className="ant-table-fixed-right">
                    <div className="ant-table-header">
                      <table className="ant-table-fixed" style={{ width: 170 }}>
                        <thead className="ant-table-thead">
                          <tr>
                            <th className="ant-table-cell ant-table-cell-align-right">
                              {t('order.action_col')}
                            </th>
                          </tr>
                        </thead>
                      </table>
                    </div>
                    <div className="ant-table-body-outer">
                      <div className="ant-table-body-inner">
                        <table className="ant-table-fixed" style={{ width: 170 }}>
                          <tbody className="ant-table-tbody">
                            {orders.map((order) => (
                              <tr className="ant-table-row ant-table-row-level-0" key={order.trade_no}>
                                <td className="ant-table-cell ant-table-cell-align-right">
                                  <div>
                                    <a
                                      href="javascript:void(0);"
                                      {...(order.status === 2 ? { disabled: true } : {})}
                                      onClick={() => navigate(`/order/${order.trade_no}`)}
                                    >
                                      {t('order.return')}
                                    </a>
                                    <span className="ant-divider ant-divider-vertical" role="separator" />
                                    <a
                                      href="javascript:void(0);"
                                      {...(order.status !== 0 ? { disabled: true } : {})}
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
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
