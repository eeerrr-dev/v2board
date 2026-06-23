import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTrafficLog } from '@/lib/queries';
import { QuestionCircleIcon } from '@/components/ant-icon';
import { LegacyEmpty } from '@/components/legacy-empty';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';
import { useFixedColumnRowHeights } from '@/lib/use-fixed-column-row-heights';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import { formatBytes, formatDate } from '@v2board/config/format';

export default function TrafficPage() {
  const { t } = useTranslation();
  const trafficQuery = useTrafficLog();
  const { data, isFetching } = trafficQuery;
  const loading = useLegacyFetchLoading(isFetching, trafficQuery.error);
  const rows = data ?? [];
  const [hoverKey, setHoverKey] = useState<number | null>(null);
  const { bodyRef, onScroll, scrollPositionClassName } = useTableScrollPosition(rows.length);
  const { mainTableRef, fixedTableRef } = useFixedColumnRowHeights(rows.length, {
    bodyRowHeightOffset: 1,
  });
  const tableClassName = [
    'ant-table',
    'ant-table-default',
    rows.length ? '' : 'ant-table-empty',
    scrollPositionClassName,
  ].filter(Boolean).join(' ');

  return (
    // The original builds this as `"block block-rounded  ".concat(...)` — note the
    // DOUBLE space before the loading class (umi.js @1145481), so the rendered class
    // attribute has two spaces; reproduced verbatim.
    <div className={`block block-rounded  ${loading ? 'block-mode-loading' : ''}`}>
      <div className="bg-white">
        <div className="row p-3">
          <div className="col-lg-12">
            <div className="alert alert-info mb-0" role="alert">
              <p className="mb-0">{t('traffic.notice')}</p>
            </div>
          </div>
        </div>
        <div className="ant-table-wrapper" style={{ borderTop: '1px solid #e8e8e8' }}>
          <div className="ant-spin-nested-loading">
            <div className="ant-spin-container">
              <div className={tableClassName}>
                <div className="ant-table-content">
                  <div className="ant-table-scroll">
                    <div
                      ref={bodyRef}
                      tabIndex={-1}
                      className="ant-table-body"
                      style={{ overflowX: 'scroll', WebkitTransform: 'translate3d (0, 0, 0)' }}
                      onScroll={onScroll}
                    >
                      <table ref={mainTableRef} className="ant-table-fixed" style={{ width: 800 }}>
                        <colgroup>
                          <col />
                          <col />
                          <col />
                          <col />
                          <col />
                        </colgroup>
                        {/* antd v3 Table wraps EVERY header cell in
                            span.ant-table-header-column > div > (span.ant-table-column-title,
                            span.ant-table-column-sorter) regardless of sorting; the inner div
                            has no class and the sorter span is empty when no sorter is defined
                            in the legacy oracle output. */}
                        <thead className="ant-table-thead">
                          <tr>
                            <th className="">
                              <span className="ant-table-header-column">
                                <div>
                                  <span className="ant-table-column-title">
                                    {t('traffic.record_at')}
                                  </span>
                                  <span className="ant-table-column-sorter" />
                                </div>
                              </span>
                            </th>
                            <th className="ant-table-align-right" style={{ textAlign: 'right' }}>
                              <span className="ant-table-header-column">
                                <div>
                                  <span className="ant-table-column-title">
                                    {t('traffic.actual_upload')}
                                  </span>
                                  <span className="ant-table-column-sorter" />
                                </div>
                              </span>
                            </th>
                            <th className="ant-table-align-right" style={{ textAlign: 'right' }}>
                              <span className="ant-table-header-column">
                                <div>
                                  <span className="ant-table-column-title">
                                    {t('traffic.actual_download')}
                                  </span>
                                  <span className="ant-table-column-sorter" />
                                </div>
                              </span>
                            </th>
                            <th className="ant-table-align-center" style={{ textAlign: 'center' }}>
                              <span className="ant-table-header-column">
                                <div>
                                  <span className="ant-table-column-title">
                                    {t('traffic.deduct_rate')}
                                  </span>
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
                                  <span className="ant-table-column-title">
                                    <LegacyTooltip title={t('traffic.total_formula')} placement="topRight">
                                      {t('traffic.total_charged')} <QuestionCircleIcon />
                                    </LegacyTooltip>
                                  </span>
                                  <span className="ant-table-column-sorter" />
                                </div>
                              </span>
                            </th>
                          </tr>
                        </thead>
                        <tbody className="ant-table-tbody">
                          {rows.map((row, index) => {
                            const rate = Number.parseFloat(row.server_rate);
                            const upload = parseInt(String(row.u));
                            const download = parseInt(String(row.d));
                            const charged =
                              (upload + download) * (row.server_rate as unknown as number);
                            return (
                              <tr
                                className={`ant-table-row ant-table-row-level-0${hoverKey === index ? ' ant-table-row-hover' : ''}`}
                                data-row-key={index}
                                key={index}
                                onMouseEnter={() => setHoverKey(index)}
                                onMouseLeave={() => setHoverKey(null)}
                              >
                                <td>
                                  {row.record_at ? formatDate(row.record_at).replaceAll('-', '/') : '-'}
                                </td>
                                <td style={{ textAlign: 'right' }}>
                                  {row.server_rate ? formatBytes(upload) : 0}
                                </td>
                                <td style={{ textAlign: 'right' }}>
                                  {row.server_rate ? formatBytes(download) : 0}
                                </td>
                                <td style={{ textAlign: 'center' }}>
                                  <span className="ant-tag" style={{ minWidth: 60 }}>
                                    {rate ? `${rate.toFixed(2)} x` : '-'}
                                  </span>
                                </td>
                                <td
                                  className="ant-table-fixed-columns-in-body"
                                  style={{ textAlign: 'right' }}
                                >
                                  {formatBytes(charged)}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                    {rows.length === 0 && (
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
                                    <span className="ant-table-column-title">
                                      <LegacyTooltip title={t('traffic.total_formula')} placement="topRight">
                                        {t('traffic.total_charged')} <QuestionCircleIcon />
                                      </LegacyTooltip>
                                    </span>
                                    <span className="ant-table-column-sorter" />
                                  </div>
                                </span>
                              </th>
                            </tr>
                          </thead>
                          <tbody className="ant-table-tbody">
                            {rows.map((row, index) => {
                              const charged =
                                (parseInt(String(row.u)) +
                                  parseInt(String(row.d))) *
                                (row.server_rate as unknown as number);
                              return (
                                <tr
                                  className={`ant-table-row ant-table-row-level-0${hoverKey === index ? ' ant-table-row-hover' : ''}`}
                                  data-row-key={index}
                                  key={index}
                                  onMouseEnter={() => setHoverKey(index)}
                                  onMouseLeave={() => setHoverKey(null)}
                                >
                                  <td style={{ textAlign: 'right' }}>
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
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
