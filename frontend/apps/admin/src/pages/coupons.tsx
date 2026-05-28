import { useState } from 'react';
import {
  App,
  Button,
  Card,
  DatePicker,
  Form,
  Input,
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Space,
  Switch,
  Table,
  Tabs,
  Tag,
  Typography,
} from 'antd';
import type { TableProps } from 'antd';
import dayjs from 'dayjs';
import { useTranslation } from 'react-i18next';
import type { Coupon, Giftcard } from '@v2board/types';
import {
  useAdminCoupons,
  useAdminGiftcards,
  useAdminPlans,
  useDropCouponMutation,
  useDropGiftcardMutation,
  useGenerateCouponMutation,
  useGenerateGiftcardMutation,
  useShowCouponMutation,
} from '@/lib/queries';
import { formatDateTime, formatMoney } from '@v2board/config/format';
import { i18nGet } from '@/lib/errors';

export default function CouponsPage() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.coupons')}</Typography.Title>
      <Tabs
        items={[
          { key: 'coupon', label: t('admin.coupon.coupons'), children: <CouponsTab /> },
          { key: 'giftcard', label: t('admin.coupon.giftcards'), children: <GiftcardsTab /> },
        ]}
      />
    </div>
  );
}

function CouponsTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [query, setQuery] = useState({ current: 1, pageSize: 20 });
  const coupons = useAdminCoupons(query);
  const plans = useAdminPlans();
  const generate = useGenerateCouponMutation();
  const drop = useDropCouponMutation();
  const show = useShowCouponMutation();
  const [creating, setCreating] = useState(false);

  const columns: TableProps<Coupon>['columns'] = [
    { title: '#', dataIndex: 'id', width: 70 },
    {
      title: t('common.enable'),
      dataIndex: 'show',
      width: 80,
      render: (v: 0 | 1, row) => (
        <Switch
          size="small"
          checked={v === 1}
          onChange={async (next) => {
            try {
              await show.mutateAsync({ id: row.id, show: next ? 1 : 0 });
              message.success(t('common.success'));
            } catch (e) {
              if (e instanceof Error) message.error(i18nGet(e.message));
            }
          }}
        />
      ),
    },
    { title: t('admin.coupon.coupon_name'), dataIndex: 'name' },
    {
      title: t('admin.coupon.type'),
      dataIndex: 'type',
      render: (v: 1 | 2, row) => (
        <Tag color={v === 1 ? 'blue' : 'orange'}>
          {v === 1
            ? `${t('admin.coupon.amount')} ${formatMoney(row.value)}`
            : `${t('admin.coupon.percent')} ${row.value}%`}
        </Tag>
      ),
    },
    { title: t('admin.coupon.code'), dataIndex: 'code', ellipsis: true },
    {
      title: t('admin.coupon.remaining'),
      dataIndex: 'limit_use',
      render: (v: number | null) => v ?? '∞',
    },
    {
      title: t('admin.coupon.valid_period'),
      dataIndex: 'started_at',
      render: (_: unknown, row) => `${formatDateTime(row.started_at)} ~ ${formatDateTime(row.ended_at)}`,
    },
    {
      title: t('common.operation'),
      render: (_: unknown, row) => (
        <Popconfirm
          title={t('common.delete')}
          onConfirm={async () => {
            try {
              await drop.mutateAsync(row.id);
              message.success(t('common.success'));
            } catch (e) {
              if (e instanceof Error) message.error(i18nGet(e.message));
            }
          }}
        >
          <Button size="small" danger>
            {t('common.delete')}
          </Button>
        </Popconfirm>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('admin.coupon.generate')}
        </Button>
      </Card>
      <Card>
        <Table<Coupon>
          loading={coupons.isLoading}
          rowKey="id"
          dataSource={coupons.data?.data ?? []}
          columns={columns}
          pagination={{
            current: query.current,
            pageSize: query.pageSize,
            total: coupons.data?.total ?? 0,
            onChange: (current, pageSize) => setQuery({ current, pageSize }),
          }}
        />
      </Card>
      <GenerateCouponModal
        open={creating}
        plans={plans.data?.map((p) => ({ label: p.name, value: p.id })) ?? []}
        onClose={() => setCreating(false)}
        onSubmit={async (values) => {
          try {
            await generate.mutateAsync(values);
            message.success(t('common.success'));
            setCreating(false);
          } catch (e) {
            if (e instanceof Error) message.error(i18nGet(e.message));
          }
        }}
      />
    </div>
  );
}

function GenerateCouponModal({
  open,
  plans,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plans: { label: string; value: number }[];
  onClose: () => void;
  onSubmit: (values: Partial<Coupon> & { generate_count?: number }) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal
      open={open}
      title={t('admin.coupon.generate')}
      onCancel={onClose}
      destroyOnClose
      onOk={() => form.submit()}
      width={640}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={{ type: 1, show: 1 }}
        onFinish={(raw) => {
          const values = {
            ...raw,
            started_at: raw.started_at ? dayjs(raw.started_at).unix() : undefined,
            ended_at: raw.ended_at ? dayjs(raw.ended_at).unix() : undefined,
          };
          return onSubmit(values);
        }}
      >
        <Form.Item name="name" label={t('admin.coupon.name')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="code" label={t('admin.coupon.code')}>
          <Input placeholder="leave empty to auto-generate" />
        </Form.Item>
        <Form.Item name="generate_count" label={t('admin.coupon.generate_count')}>
          <InputNumber min={1} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="type" label={t('admin.coupon.type')} rules={[{ required: true }]}>
          <Select
            options={[
              { value: 1, label: t('admin.coupon.amount') },
              { value: 2, label: t('admin.coupon.percent') },
            ]}
          />
        </Form.Item>
        <Form.Item name="value" label={t('admin.coupon.value')} rules={[{ required: true }]}>
          <InputNumber min={0} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="limit_use" label={t('admin.coupon.limit_use')}>
          <InputNumber min={0} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="limit_use_with_user" label={t('admin.coupon.limit_use_with_user')}>
          <InputNumber min={0} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="limit_plan_ids" label={t('admin.coupon.limit_plan_ids')}>
          <Select mode="multiple" options={plans} />
        </Form.Item>
        <Form.Item name="limit_period" label={t('admin.coupon.limit_period')}>
          <Select
            mode="multiple"
            options={[
              { value: 'month_price' },
              { value: 'quarter_price' },
              { value: 'half_year_price' },
              { value: 'year_price' },
              { value: 'two_year_price' },
              { value: 'three_year_price' },
              { value: 'onetime_price' },
              { value: 'reset_price' },
            ]}
          />
        </Form.Item>
        <Space size="middle" style={{ width: '100%' }}>
          <Form.Item name="started_at" label={t('admin.coupon.started_at')} rules={[{ required: true }]}>
            <DatePicker showTime style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="ended_at" label={t('admin.coupon.ended_at')} rules={[{ required: true }]}>
            <DatePicker showTime style={{ width: '100%' }} />
          </Form.Item>
        </Space>
      </Form>
    </Modal>
  );
}

function GiftcardsTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const cards = useAdminGiftcards();
  const plans = useAdminPlans();
  const generate = useGenerateGiftcardMutation();
  const drop = useDropGiftcardMutation();
  const [creating, setCreating] = useState(false);

  const GIFTCARD_TYPES: Record<number, string> = {
    1: t('admin.giftcard.type_balance'),
    2: t('admin.giftcard.type_plan'),
    3: t('admin.giftcard.type_traffic'),
    4: t('admin.giftcard.type_expire'),
    5: t('admin.giftcard.type_reset'),
  };

  const columns: TableProps<Giftcard>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70 },
    { title: t('admin.giftcard.code'), dataIndex: 'code', ellipsis: true },
    {
      title: t('admin.giftcard.type'),
      dataIndex: 'type',
      render: (v: number) => <Tag>{GIFTCARD_TYPES[v] ?? v}</Tag>,
    },
    { title: t('admin.giftcard.value'), dataIndex: 'value' },
    {
      title: t('admin.giftcard.started_at'),
      dataIndex: 'started_at',
      render: (v: number | null) => (v ? formatDateTime(v) : '-'),
    },
    {
      title: t('admin.giftcard.ended_at'),
      dataIndex: 'ended_at',
      render: (v: number | null) => (v ? formatDateTime(v) : '-'),
    },
    {
      title: t('common.operation'),
      render: (_: unknown, row) => (
        <Popconfirm
          title={t('common.delete')}
          onConfirm={async () => {
            try {
              await drop.mutateAsync(row.id);
              message.success(t('common.success'));
            } catch (e) {
              if (e instanceof Error) message.error(i18nGet(e.message));
            }
          }}
        >
          <Button size="small" danger>
            {t('common.delete')}
          </Button>
        </Popconfirm>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('admin.giftcard.generate')}
        </Button>
      </Card>
      <Card>
        <Table<Giftcard>
          loading={cards.isLoading}
          rowKey="id"
          dataSource={cards.data ?? []}
          columns={columns}
        />
      </Card>
      <GenerateGiftcardModal
        open={creating}
        plans={plans.data?.map((p) => ({ label: p.name, value: p.id })) ?? []}
        onClose={() => setCreating(false)}
        onSubmit={async (values) => {
          try {
            await generate.mutateAsync(values);
            message.success(t('common.success'));
            setCreating(false);
          } catch (e) {
            if (e instanceof Error) message.error(i18nGet(e.message));
          }
        }}
      />
    </div>
  );
}

function GenerateGiftcardModal({
  open,
  plans,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plans: { label: string; value: number }[];
  onClose: () => void;
  onSubmit: (values: Partial<Giftcard> & { generate_count?: number }) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal
      open={open}
      title={t('admin.giftcard.generate')}
      onCancel={onClose}
      destroyOnClose
      onOk={() => form.submit()}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={{ type: 1 }}
        onFinish={(raw) => {
          const values = {
            ...raw,
            started_at: raw.started_at ? dayjs(raw.started_at).unix() : undefined,
            ended_at: raw.ended_at ? dayjs(raw.ended_at).unix() : undefined,
          };
          return onSubmit(values);
        }}
      >
        <Form.Item name="type" label={t('admin.giftcard.type')} rules={[{ required: true }]}>
          <Select
            options={[
              { value: 1, label: t('admin.giftcard.type_balance') },
              { value: 2, label: t('admin.giftcard.type_plan') },
              { value: 3, label: t('admin.giftcard.type_traffic') },
              { value: 4, label: t('admin.giftcard.type_expire') },
              { value: 5, label: t('admin.giftcard.type_reset') },
            ]}
          />
        </Form.Item>
        <Form.Item name="value" label={t('admin.giftcard.value')} rules={[{ required: true }]}>
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="plan_id" label={t('order.plan')}>
          <Select allowClear options={plans} />
        </Form.Item>
        <Form.Item name="limit_use" label={t('admin.coupon.limit_use')}>
          <InputNumber min={0} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="generate_count" label={t('admin.coupon.generate_count')}>
          <InputNumber min={1} style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="started_at" label={t('admin.coupon.started_at')}>
          <DatePicker showTime style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="ended_at" label={t('admin.coupon.ended_at')}>
          <DatePicker showTime style={{ width: '100%' }} />
        </Form.Item>
      </Form>
    </Modal>
  );
}
