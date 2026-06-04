import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { QuestionCircleIcon } from '@/components/ant-icon';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import { useServers, useSubscribe } from '@/lib/queries';
import { legacyHref } from '@/lib/legacy-href';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

export default function NodePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  // Old componentDidMount dispatches user/getSubscribe before server/fetch.
  const subscribe = useSubscribe({ refetchOnMount: 'always' });
  const { data, isFetching } = useServers({ refetchOnMount: 'always' });
  const loading = useLegacyFetchLoading(isFetching);
  const servers = data ?? [];
  const { bodyRef, onScroll, scrollPositionClassName } = useTableScrollPosition(servers.length, {
    syncOnMount: false,
    syncOnResize: false,
  });

  const to = subscribe.data?.plan_id ? `/plan/${subscribe.data.plan_id}` : '/plan';

  return (
    <div className="row mb-3 mb-md-0">
      <div className="col-md-12">
        {loading ? (
          <div className="spinner-grow text-primary" role="status">
            <span className="sr-only">Loading...</span>
          </div>
        ) : servers.length > 0 ? (
          <div className="block block-rounded js-appear-enabled">
            <div className="block-content p-0">
              <div className="ant-table-wrapper">
                {/* antd v3 Table always wraps its content in Spin (loading defaults to
                    false); with no loading prop the node table never spins, so the
                    spinner div / ant-spin-blur are absent — only the two static wrappers. */}
                <div className="ant-spin-nested-loading">
                  <div className="ant-spin-container">
                    <div className={`ant-table ant-table-default ${scrollPositionClassName}`}>
                      <div className="ant-table-content">
                        <div className="ant-table-scroll">
                          <div
                            ref={bodyRef}
                            className="ant-table-body"
                            tabIndex={-1}
                            style={{
                              overflowX: 'scroll',
                              WebkitTransform: 'translate3d (0, 0, 0)',
                            }}
                            onScroll={onScroll}
                          >
                            <table className="ant-table-fixed" style={{ width: 900, tableLayout: 'auto' }}>
                              <colgroup>
                                <col />
                                <col />
                                <col />
                                <col />
                              </colgroup>
                              <thead className="ant-table-thead">
                                <tr>
                                  <th>
                                    <span className="ant-table-header-column">
                                      <div>
                                        <span className="ant-table-column-title">
                                          {t('node.simple_name')}
                                        </span>
                                        <span className="ant-table-column-sorter" />
                                      </div>
                                    </span>
                                  </th>
                                  <th className="ant-table-align-center" style={{ textAlign: 'center' }}>
                                    <span className="ant-table-header-column">
                                      <div>
                                        <span className="ant-table-column-title">
                                          {/* antd nests the column title's
                                              createElement("span",null,<Tooltip>) inside
                                              ant-table-column-title (umi.js @1217100). */}
                                          <span>
                                            <LegacyTooltip title={t('node.status_tip')}>
                                              {t('node.status')} <QuestionCircleIcon />
                                            </LegacyTooltip>
                                          </span>
                                        </span>
                                        <span className="ant-table-column-sorter" />
                                      </div>
                                    </span>
                                  </th>
                                  <th className="ant-table-align-center" style={{ textAlign: 'center' }}>
                                    <span className="ant-table-header-column">
                                      <div>
                                        <span className="ant-table-column-title">
                                          <span>
                                            <LegacyTooltip title={t('node.rate_tip')}>
                                              {t('node.rate')} <QuestionCircleIcon />
                                            </LegacyTooltip>
                                          </span>
                                        </span>
                                        <span className="ant-table-column-sorter" />
                                      </div>
                                    </span>
                                  </th>
                                  <th className="ant-table-row-cell-last">
                                    <span className="ant-table-header-column">
                                      <div>
                                        <span className="ant-table-column-title">{t('node.tags')}</span>
                                        <span className="ant-table-column-sorter" />
                                      </div>
                                    </span>
                                  </th>
                                </tr>
                              </thead>
                              <tbody className="ant-table-tbody">
                                {servers.map((s, index) => (
                                  <tr className="ant-table-row ant-table-row-level-0" key={index}>
                                    <td>{s.name}</td>
                                    <td style={{ textAlign: 'center' }}>
                                      <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                        <span
                                          className={`ant-badge-status-dot ant-badge-status-${
                                            parseInt(String(s.is_online)) ? 'processing' : 'error'
                                          }`}
                                        />
                                        <span className="ant-badge-status-text" />
                                      </span>
                                    </td>
                                    <td style={{ textAlign: 'center' }}>
                                      <span className="ant-tag" style={{ minWidth: 60 }}>
                                        {String(s.rate)} x
                                      </span>
                                    </td>
                                    <td>
                                      {s.tags
                                        ? s.tags.map((tag) => (
                                            <span className="ant-tag" key={Math.random()}>
                                              {tag}
                                            </span>
                                          ))
                                        : '-'}
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
          </div>
        ) : (
          <div className="alert alert-dark" role="alert">
            <p className="mb-0">
              {t('node.no_available')}{' '}
              <a
                className="alert-link"
                ref={legacyHref()}
                onClick={() => navigate(to)}
              >
                {subscribe.data?.plan_id ? t('node.renew') : t('node.subscribe')}
              </a>
              。
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
