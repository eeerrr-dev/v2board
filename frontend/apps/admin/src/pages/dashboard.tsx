import { Card, Col, Row, Statistic, Table, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { useStat, useStatOrder, useStatServerToday, useStatUserToday } from '@/lib/queries';
import { formatMoney } from '@v2board/config/format';
import { Column } from '@ant-design/charts';

export default function DashboardPage() {
  const { t } = useTranslation();
  const stat = useStat();
  const order = useStatOrder();
  const userToday = useStatUserToday();
  const serverToday = useStatServerToday();

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.dashboard')}</Typography.Title>
      <Row gutter={[16, 16]}>
        <Col xs={12} md={6}>
          <Card loading={stat.isLoading}>
            <Statistic
              title={t('admin.dashboard.month_income')}
              value={stat.data ? formatMoney(stat.data.month_income) : '-'}
            />
          </Card>
        </Col>
        <Col xs={12} md={6}>
          <Card loading={stat.isLoading}>
            <Statistic
              title={t('admin.dashboard.month_register_total')}
              value={stat.data?.month_register_total ?? 0}
            />
          </Card>
        </Col>
        <Col xs={12} md={6}>
          <Card loading={stat.isLoading}>
            <Statistic
              title={t('admin.dashboard.ticket_pending_total')}
              value={stat.data?.ticket_pending_total ?? 0}
            />
          </Card>
        </Col>
        <Col xs={12} md={6}>
          <Card loading={stat.isLoading}>
            <Statistic
              title={t('admin.dashboard.commission_pending_total')}
              value={stat.data?.commission_pending_total ?? 0}
            />
          </Card>
        </Col>
      </Row>
      <Row gutter={[16, 16]}>
        <Col xs={24}>
          <Card title={t('admin.dashboard.order_trend')} loading={order.isLoading}>
            {order.data && order.data.length > 0 ? (
              <Column
                height={260}
                data={order.data}
                xField="date"
                yField="value"
                seriesField="type"
                isGroup
              />
            ) : (
              <span style={{ color: '#999' }}>{t('common.empty')}</span>
            )}
          </Card>
        </Col>
      </Row>
      <Row gutter={[16, 16]}>
        <Col xs={24} xl={12}>
          <Card title={t('admin.dashboard.user_today_rank')} loading={userToday.isLoading}>
            <Table
              size="small"
              pagination={false}
              rowKey="user_id"
              dataSource={userToday.data ?? []}
              columns={[
                { title: t('admin.user.email'), dataIndex: 'email' },
                { title: t('common.total'), dataIndex: 'total', align: 'right' },
              ]}
            />
          </Card>
        </Col>
        <Col xs={24} xl={12}>
          <Card title={t('admin.dashboard.server_today_rank')} loading={serverToday.isLoading}>
            <Table
              size="small"
              pagination={false}
              rowKey="server_id"
              dataSource={serverToday.data ?? []}
              columns={[
                { title: t('admin.server.name'), dataIndex: 'server_name' },
                { title: t('common.total'), dataIndex: 'total', align: 'right' },
              ]}
            />
          </Card>
        </Col>
      </Row>
    </div>
  );
}
