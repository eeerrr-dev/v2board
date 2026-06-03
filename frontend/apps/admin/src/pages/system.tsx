import { Spin, Table } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import { useEffect, type ReactNode } from 'react';
import type { QueueWorkloadItem } from '@v2board/types';
import { useQueueStats, useQueueWorkload } from '@/lib/queries';

const QUEUE_NAMES: Record<string, string> = {
  order_handle: '订单队列',
  send_email: '邮件队列',
  send_email_mass: '邮件群发队列',
  send_telegram: 'Telegram消息队列',
  stat: '统计队列',
  traffic_fetch: '流量消费队列',
};

const columns: ColumnsType<QueueWorkloadItem> = [
  {
    title: '队列名称',
    dataIndex: 'name',
    key: 'name',
    render: (value: string) => QUEUE_NAMES[value],
  },
  {
    title: '作业量',
    dataIndex: 'processes',
    key: 'processes',
  },
  {
    title: '任务量',
    dataIndex: 'length',
    key: 'length',
  },
  {
    title: '占用时间',
    dataIndex: 'wait',
    key: 'wait',
    align: 'right',
    render: (value: number) => `${value}s`,
  },
];

function LegacySpin({ loading, children }: { loading: boolean; children: ReactNode }) {
  return (
    <Spin spinning={loading} indicator={<div className="spinner-grow text-primary" />}>
      {children}
    </Spin>
  );
}

export function startLegacyQueuePolling(
  refetchStats: () => unknown,
  refetchWorkload: () => unknown,
) {
  let timer: number | undefined;
  const getData = () => {
    void refetchStats();
    void refetchWorkload();
    timer = window.setTimeout(() => {
      getData();
    }, 3000);
  };
  getData();
  return () => {
    if (timer !== undefined) window.clearTimeout(timer);
  };
}

export default function SystemPage() {
  const queueStats = useQueueStats();
  const queueWorkload = useQueueWorkload();
  const stats = queueStats.data;
  const workload = queueWorkload.data?.filter((item) => item.name !== 'default');

  useEffect(
    () => startLegacyQueuePolling(queueStats.refetch, queueWorkload.refetch),
    [],
  );

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
                  <div className="mt-4 font-size-h3">{stats ? (stats.status ? '运行中' : '未启动') : null}</div>
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
            <Table<QueueWorkloadItem>
              columns={columns}
              dataSource={workload}
              pagination={false}
            />
          </div>
        </div>
      </LegacySpin>
    </>
  );
}
