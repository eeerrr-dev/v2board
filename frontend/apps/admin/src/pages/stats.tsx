import { Card, Typography } from 'antd';
import { Column } from '@ant-design/charts';
import { useTranslation } from 'react-i18next';
import { useStatOrder } from '@/lib/queries';

export default function StatsPage() {
  const { t } = useTranslation();
  const order = useStatOrder();

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.stat')}</Typography.Title>
      <Card title={t('admin.dashboard.order_trend')} loading={order.isLoading}>
        {order.data && order.data.length > 0 ? (
          <Column
            height={360}
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
    </div>
  );
}
