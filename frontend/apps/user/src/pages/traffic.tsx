import { useTranslation } from 'react-i18next';
import { useTrafficLog } from '@/lib/queries';
import { LegacyEmpty } from '@/components/legacy-empty';
import { formatBytes, formatDate } from '@v2board/config/format';

export default function TrafficPage() {
  const { t } = useTranslation();
  const { data, isFetching } = useTrafficLog();
  const rows = data ?? [];
  const tableClassName = [
    'ant-table',
    'ant-table-default',
    rows.length ? '' : 'ant-table-empty',
    'ant-table-scroll-position-left',
  ].filter(Boolean).join(' ');

  return (
    <div className={`block block-rounded ${isFetching ? 'block-mode-loading' : ''}`}>
      <div className="bg-white">
        <div className="row p-3">
          <div className="col-lg-12">
            <div className="alert alert-info mb-0" role="alert">
              <p className="mb-0">{t('traffic.notice')}</p>
            </div>
          </div>
        </div>
        <div className="ant-table-wrapper" style={{ borderTop: '1px solid #e8e8e8' }}>
          <div className={tableClassName}>
            <div className="ant-table-content">
              <div className="ant-table-scroll">
                <div className="ant-table-body" style={{ overflowX: 'auto' }}>
                  <table style={{ minWidth: 800, width: '100%', tableLayout: 'auto' }}>
                    <thead className="ant-table-thead">
                      <tr>
                        <th className="ant-table-cell">{t('traffic.record_at')}</th>
                        <th className="ant-table-cell ant-table-cell-align-right">
                          {t('traffic.actual_upload')}
                        </th>
                        <th className="ant-table-cell ant-table-cell-align-right">
                          {t('traffic.actual_download')}
                        </th>
                        <th className="ant-table-cell ant-table-cell-align-center">
                          {t('traffic.deduct_rate')}
                        </th>
                        <th className="ant-table-cell ant-table-cell-align-right ant-table-fixed-columns-in-body">
                          <span
                            className="v2board-ant-tooltip-trigger v2board-ant-tooltip-top-right"
                            data-title={t('traffic.total_formula')}
                          >
                            {t('common.total')} <i className="anticon anticon-question-circle" />
                          </span>
                        </th>
                      </tr>
                    </thead>
                    <tbody className="ant-table-tbody">
                      {rows.length > 0 ? (
                        rows.map((row) => {
                          const rate = Number.parseFloat(row.server_rate);
                          const upload = Number.parseInt(String(row.u), 10);
                          const download = Number.parseInt(String(row.d), 10);
                          const charged = (upload + download) * Number(row.server_rate);
                          return (
                            <tr
                              className="ant-table-row ant-table-row-level-0"
                              key={`${row.record_at}-${row.server_rate}-${row.u}-${row.d}`}
                            >
                              <td className="ant-table-cell">
                                {formatDate(row.record_at).replaceAll('-', '/')}
                              </td>
                              <td className="ant-table-cell ant-table-cell-align-right">
                                {row.server_rate ? formatBytes(upload) : 0}
                              </td>
                              <td className="ant-table-cell ant-table-cell-align-right">
                                {row.server_rate ? formatBytes(download) : 0}
                              </td>
                              <td className="ant-table-cell ant-table-cell-align-center">
                                <span className="ant-tag" style={{ minWidth: 60 }}>
                                  {rate ? `${rate.toFixed(2)} x` : '-'}
                                </span>
                              </td>
                              <td className="ant-table-cell ant-table-cell-align-right ant-table-fixed-columns-in-body">
                                {formatBytes(charged)}
                              </td>
                            </tr>
                          );
                        })
                      ) : (
                        <tr className="ant-table-placeholder">
                          <td className="ant-table-cell" colSpan={5}>
                            <LegacyEmpty />
                          </td>
                        </tr>
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
              {rows.length > 0 && (
                <div className="ant-table-fixed-right">
                  <div className="ant-table-header">
                    <table className="ant-table-fixed" style={{ width: 170 }}>
                      <thead className="ant-table-thead">
                        <tr>
                          <th className="ant-table-cell ant-table-cell-align-right">
                            <span
                              className="v2board-ant-tooltip-trigger v2board-ant-tooltip-top-right"
                              data-title={t('traffic.total_formula')}
                            >
                              {t('common.total')} <i className="anticon anticon-question-circle" />
                            </span>
                          </th>
                        </tr>
                      </thead>
                    </table>
                  </div>
                  <div className="ant-table-body-outer">
                    <div className="ant-table-body-inner">
                      <table className="ant-table-fixed" style={{ width: 170 }}>
                        <tbody className="ant-table-tbody">
                          {rows.map((row) => {
                            const charged =
                              (Number.parseInt(String(row.u), 10) +
                                Number.parseInt(String(row.d), 10)) *
                              Number(row.server_rate);
                            return (
                              <tr
                                className="ant-table-row ant-table-row-level-0"
                                key={`${row.record_at}-${row.server_rate}-${row.u}-${row.d}`}
                              >
                                <td className="ant-table-cell ant-table-cell-align-right">
                                  {formatBytes(charged)}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
