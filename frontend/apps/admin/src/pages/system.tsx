import { useEffect } from 'react';
import type { QueueWorkloadItem } from '@v2board/types';
import { useQueueStats, useQueueWorkload } from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';

const QUEUE_NAMES: Record<string, string> = {
  order_handle: '订单队列',
  send_email: '邮件队列',
  send_email_mass: '邮件群发队列',
  send_telegram: 'Telegram消息队列',
  stat: '统计队列',
  traffic_fetch: '流量消费队列',
};

const headers: LegacyStandaloneTableHeader[] = [
  { title: '队列名称' },
  { title: '作业量' },
  { title: '任务量' },
  { title: '占用时间', alignRight: true },
];

export function startLegacyQueuePolling(
  refetchStats: () => unknown,
  refetchWorkload: () => unknown,
) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const getData = () => {
    void refetchStats();
    void refetchWorkload();
    timer = setTimeout(() => {
      getData();
    }, 3000);
  };
  getData();
  return () => {
    if (timer !== undefined) clearTimeout(timer);
  };
}

export default function SystemPage() {
  const queueStats = useQueueStats();
  const queueWorkload = useQueueWorkload();
  const stats = queueStats.data;
  const workload = queueWorkload.data?.filter((item) => item.name !== 'default');

  useEffect(() => startLegacyQueuePolling(queueStats.refetch, queueWorkload.refetch), []);

  return (
    <>
      <LegacySpin loading={!stats}>
        <div className="block block-rounded ">
          <div className="block-header block-header-default">
            <h3 className="block-title">总览</h3>
          </div>
          <div className="block-content p-0">
            <div className="row no-gutters">
              <div className="col-lg-6 col-xl-3 border-right p-4 border-bottom">
                <div>
                  <div>当前作业量</div>
                  <div className="mt-4 font-size-h3">{stats?.jobsPerMinute || '0'}</div>
                </div>
              </div>
              <div className="col-lg-6 col-xl-3 border-right p-4 border-bottom">
                <div>
                  <div>近一小时处理量</div>
                  <div className="mt-4 font-size-h3">{stats?.recentJobs || '0'}</div>
                </div>
              </div>
              <div className="col-lg-6 col-xl-3 border-right p-4 border-bottom">
                <div>
                  <div>7日内报错数量</div>
                  <div className="mt-4 font-size-h3">{stats?.failedJobs || '0'}</div>
                </div>
              </div>
              <div className="col-lg-6 col-xl-3 p-4 border-bottom overflow-hidden">
                <div>
                  <div>状态</div>
                  <div className="mt-4 font-size-h3">
                    {stats ? (stats.status ? '运行中' : '未启动') : null}
                  </div>
                  {stats ? (
                    stats.status ? (
                      <i
                        className="si si-check text-success"
                        style={{ position: 'absolute', fontSize: 100, right: -20, bottom: -20 }}
                      />
                    ) : (
                      <i
                        className="si si-close text-danger"
                        style={{ position: 'absolute', fontSize: 100, right: -20, bottom: -20 }}
                      />
                    )
                  ) : null}
                </div>
              </div>
            </div>
          </div>
        </div>
      </LegacySpin>

      <LegacySpin loading={!workload}>
        <div className="block block-rounded ">
          <div className="block-header block-header-default">
            <h3 className="block-title">当前作业详情</h3>
          </div>
          <div className="block-content p-0">
            <LegacyStandaloneTable headers={headers} isEmpty={(workload ?? []).length === 0}>
              {(workload ?? []).map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyTableRowKey(index)}
                >
                  <td className="">{QUEUE_NAMES[row.name]}</td>
                  <td className="">{row.processes}</td>
                  <td className="">{row.length}</td>
                  <td className="" style={{ textAlign: 'right' }}>
                    {row.wait}s
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
          </div>
        </div>
      </LegacySpin>
    </>
  );
}
