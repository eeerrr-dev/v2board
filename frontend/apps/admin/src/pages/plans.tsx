import { useEffect, useMemo, useRef, useState } from 'react';
import {
  App,
  Button,
  Card,
  Dropdown,
  Form,
  Input,
  InputNumber,
  Modal,
  Space,
  Switch,
  Table,
  Tag,
  Typography,
} from 'antd';
import type { TableProps } from 'antd';
import { CaretDownOutlined, MenuOutlined, UserOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { Plan } from '@v2board/types';
import {
  useAdminPlans,
  useDropPlanMutation,
  useSavePlanMutation,
  useServerGroups,
  useSortPlansMutation,
  useUpdatePlanMutation,
} from '@/lib/queries';
import { formatMoney } from '@v2board/config/format';
import { i18nGet } from '@/lib/errors';

const PRICE_KEYS: (keyof Plan)[] = [
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
];

const PRICE_COLUMNS: { key: string; dataIndex: keyof Plan }[] = [
  { key: 'plan.monthly', dataIndex: 'month_price' },
  { key: 'plan.quarterly', dataIndex: 'quarter_price' },
  { key: 'plan.half_year', dataIndex: 'half_year_price' },
  { key: 'plan.yearly', dataIndex: 'year_price' },
  { key: 'plan.two_year', dataIndex: 'two_year_price' },
  { key: 'plan.three_year', dataIndex: 'three_year_price' },
  { key: 'plan.onetime', dataIndex: 'onetime_price' },
  { key: 'admin.plan.reset_price', dataIndex: 'reset_price' },
];

export default function PlansPage() {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const save = useSavePlanMutation();
  const drop = useDropPlanMutation();
  const update = useUpdatePlanMutation();
  const sort = useSortPlansMutation();
  const [editing, setEditing] = useState<Plan | null>(null);
  const [creating, setCreating] = useState(false);

  // Local mirror of the plan order so the drag handle can reorder optimistically.
  const [order, setOrder] = useState<Plan[]>([]);
  useEffect(() => {
    if (plans.data) setOrder(plans.data);
  }, [plans.data]);
  const orderRef = useRef(order);
  orderRef.current = order;
  const dragIndex = useRef<number | null>(null);

  const groupMap = useMemo(() => {
    const map = new Map<number, string>();
    for (const g of groups.data ?? []) map.set(g.id, g.name);
    return map;
  }, [groups.data]);

  // Reorder rows by dragging the 排序 handle, then persist the new id order.
  const components = useMemo(
    () => ({
      body: {
        row: (props: React.HTMLAttributes<HTMLTableRowElement> & { 'data-row-key'?: number }) => {
          const onDrop = () => {
            const from = dragIndex.current;
            const current = orderRef.current;
            const to = current.findIndex((p) => p.id === props['data-row-key']);
            dragIndex.current = null;
            if (from == null || to < 0 || from === to) return;
            const next = [...current];
            const [moved] = next.splice(from, 1);
            if (!moved) return;
            next.splice(to, 0, moved);
            setOrder(next);
            sort.mutate(next.map((p) => p.id));
          };
          return <tr {...props} onDragOver={(e) => e.preventDefault()} onDrop={onDrop} />;
        },
      },
    }),
    [sort],
  );

  const doDelete = (row: Plan) =>
    modal.confirm({
      title: t('common.delete'),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await drop.mutateAsync(row.id);
          message.success(t('common.success'));
        } catch (e) {
          if (e instanceof Error) message.error(i18nGet(e.message));
        }
      },
    });

  const toggle = async (id: number, field: 'show' | 'renew', next: boolean) => {
    try {
      await update.mutateAsync({ id, [field]: next ? 1 : 0 });
      message.success(t('common.success'));
    } catch (e) {
      if (e instanceof Error) message.error(i18nGet(e.message));
    }
  };

  const columns: TableProps<Plan>['columns'] = [
    {
      title: t('admin.plan.order'),
      width: 60,
      render: (_: unknown, __: Plan, index: number) => (
        <span
          draggable
          onDragStart={() => {
            dragIndex.current = index;
          }}
          style={{ cursor: 'move' }}
        >
          <MenuOutlined />
        </span>
      ),
    },
    {
      title: t('admin.plan.sale_status'),
      dataIndex: 'show',
      width: 90,
      render: (v: 0 | 1, row) => (
        <Switch size="small" checked={v === 1} onChange={(next) => toggle(row.id, 'show', next)} />
      ),
    },
    {
      title: t('admin.plan.renew'),
      dataIndex: 'renew',
      width: 70,
      render: (v: 0 | 1, row) => (
        <Switch size="small" checked={v === 1} onChange={(next) => toggle(row.id, 'renew', next)} />
      ),
    },
    { title: t('admin.common.name'), dataIndex: 'name' },
    {
      title: t('admin.plan.stat'),
      dataIndex: 'count',
      width: 80,
      render: (v: number | undefined) => (
        <span>
          <UserOutlined /> {v ?? 0}
        </span>
      ),
    },
    {
      title: t('admin.plan.traffic'),
      dataIndex: 'transfer_enable',
      render: (v: number) => `${v} GB`,
    },
    {
      title: t('admin.plan.device_limit'),
      dataIndex: 'device_limit',
      render: (v: number | null) => v ?? '-',
    },
    ...PRICE_COLUMNS.map((c) => ({
      title: t(c.key),
      dataIndex: c.dataIndex,
      render: (v: number | null) => (v ? formatMoney(v, '') : '-'),
    })),
    {
      title: t('admin.user.group'),
      dataIndex: 'group_id',
      render: (v: number) => <Tag>{groupMap.get(v) ?? '-'}</Tag>,
    },
    {
      title: t('common.operation'),
      fixed: 'right',
      width: 100,
      render: (_: unknown, row) => (
        <Dropdown
          trigger={['click']}
          menu={{
            items: [
              { key: 'edit', label: t('common.edit') },
              { key: 'delete', danger: true, label: t('common.delete') },
            ],
            onClick: ({ key }) => {
              if (key === 'edit') setEditing(row);
              else doDelete(row);
            },
          }}
        >
          <a onClick={(e) => e.preventDefault()}>
            <Space size={4}>
              {t('common.operation')}
              <CaretDownOutlined />
            </Space>
          </a>
        </Dropdown>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.plans')}</Typography.Title>
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('admin.plan.new')}
        </Button>
      </Card>
      <Card>
        <Table<Plan>
          loading={plans.isLoading}
          rowKey="id"
          dataSource={order}
          columns={columns}
          components={components}
          pagination={false}
          scroll={{ x: 'max-content' }}
        />
      </Card>
      <PlanModal
        open={Boolean(editing) || creating}
        plan={editing}
        onClose={() => {
          setEditing(null);
          setCreating(false);
        }}
        onSubmit={async (values) => {
          try {
            await save.mutateAsync(values);
            message.success(t('common.success'));
            setEditing(null);
            setCreating(false);
          } catch (e) {
            if (e instanceof Error) message.error(i18nGet(e.message));
          }
        }}
      />
    </div>
  );
}

function PlanModal({
  open,
  plan,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plan: Plan | null;
  onClose: () => void;
  onSubmit: (values: Partial<Plan> & { force_update?: 0 | 1 }) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal
      open={open}
      title={plan ? t('admin.plan.edit') : t('admin.plan.new')}
      onCancel={onClose}
      onOk={() => form.submit()}
      destroyOnClose
      width={720}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={
          plan ?? { show: 1, renew: 1, transfer_enable: 100, group_id: 1, sort: 1 }
        }
        onFinish={(values) => onSubmit({ ...values, id: plan?.id })}
      >
        <Form.Item name="name" label={t('plan.detail')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="content" label="Content">
          <Input.TextArea rows={3} />
        </Form.Item>
        <div className="grid grid-cols-2 gap-4">
          <Form.Item name="group_id" label="Group ID" rules={[{ required: true }]}>
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="transfer_enable" label="GB / cycle" rules={[{ required: true }]}>
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="device_limit" label={t('plan.device_limit')}>
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="speed_limit" label={t('plan.speed_limit')}>
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="capacity_limit" label="Capacity">
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
          <Form.Item name="sort" label="Sort">
            <InputNumber style={{ width: '100%' }} />
          </Form.Item>
          {PRICE_KEYS.map((k) => (
            <Form.Item key={k} name={k} label={k}>
              <InputNumber style={{ width: '100%' }} />
            </Form.Item>
          ))}
        </div>
        <Form.Item name="show" label={t('admin.plan.show')}>
          <InputNumber min={0} max={1} />
        </Form.Item>
        <Form.Item name="renew" label={t('admin.plan.allow_renew')}>
          <InputNumber min={0} max={1} />
        </Form.Item>
        <Form.Item name="force_update" label="Sync existing subscribers">
          <InputNumber min={0} max={1} />
        </Form.Item>
        {plan?.count != null && plan.count > 0 && (
          <Tag color="orange">{plan.count} active users</Tag>
        )}
      </Form>
    </Modal>
  );
}
