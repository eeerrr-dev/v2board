import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useServers, useSubscribe } from '@/lib/queries';

export default function NodePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isFetching } = useServers({ refetchOnMount: 'always' });
  const subscribe = useSubscribe({ refetchOnMount: 'always' });
  const servers = data ?? [];

  const to = subscribe.data?.plan_id ? `/plan/${subscribe.data.plan_id}` : '/plan';

  return (
    <div className="row mb-3 mb-md-0">
      <div className="col-md-12">
        {isFetching ? (
          <div className="spinner-grow text-primary" role="status">
            <span className="sr-only">Loading...</span>
          </div>
        ) : servers.length > 0 ? (
          <div className="block block-rounded js-appear-enabled">
            <div className="block-content p-0">
              <div className="ant-table-wrapper">
                <div className="ant-table ant-table-default ant-table-scroll-position-left">
                  <div className="ant-table-content">
                    <div className="ant-table-scroll">
                      <div className="ant-table-body" style={{ overflowX: 'auto' }}>
                        <table style={{ minWidth: 900, width: '100%', tableLayout: 'auto' }}>
                          <thead className="ant-table-thead">
                            <tr>
                              <th className="ant-table-cell">{t('node.simple_name')}</th>
                              <th className="ant-table-cell ant-table-cell-align-center">
                                <span
                                  className="v2board-ant-tooltip-trigger"
                                  data-title={t('node.status_tip')}
                                >
                                  {t('node.status')} <i className="anticon anticon-question-circle" />
                                </span>
                              </th>
                              <th className="ant-table-cell ant-table-cell-align-center">
                                <span
                                  className="v2board-ant-tooltip-trigger"
                                  data-title={t('node.rate_tip')}
                                >
                                  {t('node.rate')} <i className="anticon anticon-question-circle" />
                                </span>
                              </th>
                              <th className="ant-table-cell">{t('node.tags')}</th>
                            </tr>
                          </thead>
                          <tbody className="ant-table-tbody">
                            {servers.map((s) => {
                              const tags = (s as { tags?: string[] | null }).tags;
                              return (
                                <tr className="ant-table-row ant-table-row-level-0" key={s.id}>
                                  <td className="ant-table-cell">{s.name}</td>
                                  <td className="ant-table-cell ant-table-cell-align-center">
                                    <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                      <span
                                        className={`ant-badge-status-dot ant-badge-status-${
                                          Number.parseInt(String(s.is_online), 10) ? 'processing' : 'error'
                                        }`}
                                      />
                                    </span>
                                  </td>
                                  <td className="ant-table-cell ant-table-cell-align-center">
                                    <span className="ant-tag" style={{ minWidth: 60 }}>
                                      {s.rate} x
                                    </span>
                                  </td>
                                  <td className="ant-table-cell">
                                    {tags
                                      ? tags.map((tag) => (
                                          <span className="ant-tag" key={tag}>
                                            {tag}
                                          </span>
                                        ))
                                      : '-'}
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
        ) : (
          <div className="alert alert-dark" role="alert">
            <p className="mb-0">
              {t('dashboard.no_subscription')}{' '}
              <a
                className="alert-link"
                href="javascript:void(0);"
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
