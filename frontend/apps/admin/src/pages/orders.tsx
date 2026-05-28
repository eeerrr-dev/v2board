import { useState } from 'react';
import { App, Button, Card, Descriptions, Input, Modal, Popconfirm, Space, Table, Tag, Typography } from 'antd';
import type { TableProps } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  useAdminOrders,
  useCancelOrderMutation,
  useMarkOrderPaidMutation,
} from '@/lib/queries';
import type { AdminOrderRow } from '@v2board/types';
import { formatDateTime, formatMoney } from '@v2board/config/format';
import { i18nGet } from '@/lib/errors';

const STATUS_COLOR: Record<number, string> = {
  0: 'orange',
  1: 'blue',
  2: 'default',
  3: 'green',
  4: 'purple',
};

// order.period is a PlanPeriod price key (or 'deposit'); map it to the shared period labels.
const PERIOD_KEY: Record<string, string> = {
  month_price: 'plan.monthly',
  quarter_price: 'plan.quarterly',
  half_year_price: 'plan.half_year',
  year_price: 'plan.yearly',
  two_year_price: 'plan.two_year',
  three_year_price: 'plan.three_year',
  onetime_price: 'plan.onetime',
  reset_price: 'plan.reset',
  deposit: 'admin.order.deposit',
};

export default function OrdersPage() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [keyword, setKeyword] = useState('');
  const [query, setQuery] = useState({
    current: 1,
    pageSize: 20,
    filter: [] as { key: string; condition: string; value: string }[],
  });
  const orders = useAdminOrders(query);
  const paid = useMarkOrderPaidMutation();
  const cancel = useCancelOrderMutation();
  const [detail, setDetail] = useState<AdminOrderRow | null>(null);

  const onSearch = () => {
    setQuery((q) => ({
      ...q,
      current: 1,
      filter: keyword ? [{ key: 'trade_no', condition: '模糊', value: keyword }] : [],
    }));
  };

  const columns: TableProps<AdminOrderRow>['columns'] = [
    { title: t('order.trade_no_col'), dataIndex: 'trade_no', width: 220, ellipsis: true },
    {
      title: t('admin.order.type'),
      dataIndex: 'type',
      render: (v: number) => t(`admin.order.type_${v}`),
    },
    { title: t('admin.order.plan'), dataIndex: 'plan_name', render: (v) => v ?? '-' },
    {
      title: t('order.period'),
      dataIndex: 'period',
      render: (v: string) => {
        const key = PERIOD_KEY[v];
        return key ? t(key) : v;
      },
    },
    {
      title: t('admin.order.amount_paid'),
      dataIndex: 'total_amount',
      render: (v: number) => formatMoney(v),
    },
    {
      title: t('order.status'),
      dataIndex: 'status',
      render: (v: number) => <Tag color={STATUS_COLOR[v] ?? 'default'}>{t(`admin.order.status_${v}`)}</Tag>,
    },
    {
      title: t('admin.order.commission_amount'),
      dataIndex: 'commission_balance',
      render: (v: number) => formatMoney(v),
    },
    {
      title: t('admin.order.commission_status'),
      dataIndex: 'commission_status',
      render: (v: number) => t(`admin.order.commission_status_${v}`),
    },
    {
      title: t('order.created_at'),
      dataIndex: 'created_at',
      render: (v: number) => formatDateTime(v),
    },
  ];

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.orders')}</Typography.Title>
      <Card>
        <Input.Search
          placeholder="trade no"
          value={keyword}
          onChange={(e) => setKeyword(e.target.value)}
          onSearch={onSearch}
          allowClear
          style={{ width: 280 }}
        />
      </Card>
      <Card>
        <Table<AdminOrderRow>
          loading={orders.isLoading}
          rowKey="trade_no"
          dataSource={orders.data?.data ?? []}
          columns={columns}
          scroll={{ x: 'max-content' }}
          onRow={(record) => ({
            onClick: () => setDetail(record),
            style: { cursor: 'pointer' },
          })}
          pagination={{
            current: query.current,
            pageSize: query.pageSize,
            total: orders.data?.total ?? 0,
            showSizeChanger: true,
            onChange: (current, pageSize) => setQuery((q) => ({ ...q, current, pageSize })),
          }}
        />
      </Card>

      <Modal
        open={Boolean(detail)}
        title={detail?.trade_no}
        onCancel={() => setDetail(null)}
        footer={null}
      >
        {detail && (
          <div className="space-y-4">
            <Descriptions column={1} size="small" bordered>
              <Descriptions.Item label={t('admin.order.type')}>
                {t(`admin.order.type_${detail.type}`)}
              </Descriptions.Item>
              <Descriptions.Item label={t('admin.order.plan')}>
                {detail.plan_name ?? '-'}
              </Descriptions.Item>
              <Descriptions.Item label={t('admin.order.amount_paid')}>
                {formatMoney(detail.total_amount)}
              </Descriptions.Item>
              <Descriptions.Item label={t('order.status')}>
                <Tag color={STATUS_COLOR[detail.status] ?? 'default'}>
                  {t(`admin.order.status_${detail.status}`)}
                </Tag>
              </Descriptions.Item>
              <Descriptions.Item label={t('order.created_at')}>
                {formatDateTime(detail.created_at)}
              </Descriptions.Item>
            </Descriptions>
            {detail.status === 0 && (
              <Space size="small">
                <Popconfirm
                  title={t('admin.order.mark_paid')}
                  onConfirm={async () => {
                    try {
                      await paid.mutateAsync(detail.trade_no);
                      message.success(t('common.success'));
                      setDetail(null);
                    } catch (e) {
                      if (e instanceof Error) message.error(i18nGet(e.message));
                    }
                  }}
                >
                  <Button type="primary">{t('admin.order.mark_paid')}</Button>
                </Popconfirm>
                <Popconfirm
                  title={t('admin.order.mark_cancel')}
                  onConfirm={async () => {
                    try {
                      await cancel.mutateAsync(detail.trade_no);
                      message.success(t('common.success'));
                      setDetail(null);
                    } catch (e) {
                      if (e instanceof Error) message.error(i18nGet(e.message));
                    }
                  }}
                >
                  <Button danger>{t('admin.order.mark_cancel')}</Button>
                </Popconfirm>
              </Space>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
