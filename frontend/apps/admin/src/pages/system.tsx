import { Card, Descriptions, Table, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { useQueueStats, useSystemLog, useSystemStatus } from '@/lib/queries';

export default function SystemPage() {
  const { t } = useTranslation();
  const status = useSystemStatus();
  const queue = useQueueStats();
  const log = useSystemLog();

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.system')}</Typography.Title>
      <Card title={t('admin.system.status')} loading={status.isLoading}>
        {status.data && (
          <Descriptions column={2} bordered size="small">
            <Descriptions.Item label={t('admin.system.schedule')}>
              <Tag color={status.data.schedule ? 'green' : 'red'}>
                {status.data.schedule ? t('common.enable') : t('common.disable')}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.horizon')}>
              <Tag color={status.data.horizon ? 'green' : 'red'}>
                {status.data.horizon ? t('common.enable') : t('common.disable')}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.log_channel')}>
              {status.data.logChannel}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.log_level')}>
              {status.data.logLevel}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.cache_driver')}>
              {status.data.cacheDriver}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.backend_version')}>
              {status.data.backendVersion}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.frontend_version')} span={2}>
              {status.data.frontendVersion}
            </Descriptions.Item>
          </Descriptions>
        )}
      </Card>

      <Card title={t('admin.system.queue')} loading={queue.isLoading}>
        {queue.data && (
          <Descriptions column={3} bordered size="small">
            <Descriptions.Item label={t('admin.system.queue_status')}>
              <Tag color={queue.data.status === 'running' ? 'green' : 'orange'}>
                {queue.data.status}
              </Tag>
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.queue_failed')}>
              {queue.data.failedJobs}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.queue_jpm')}>
              {queue.data.jobsPerMinute}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.queue_recent')}>
              {queue.data.recentJobs}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.queue_processes')}>
              {queue.data.processes}
            </Descriptions.Item>
            <Descriptions.Item label={t('admin.system.queue_paused')}>
              {queue.data.pausedMasters}
            </Descriptions.Item>
          </Descriptions>
        )}
      </Card>

      <Card title={t('admin.system.log')} loading={log.isLoading}>
        <Table
          rowKey={(_, idx) => String(idx)}
          dataSource={(log.data ?? []).map((line, idx) => ({ idx, line }))}
          columns={[
            { title: '#', dataIndex: 'idx', width: 60 },
            { title: t('admin.system.log_line'), dataIndex: 'line', ellipsis: true },
          ]}
          pagination={{ pageSize: 30 }}
        />
      </Card>
    </div>
  );
}
