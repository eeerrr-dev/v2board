import { useMemo, useState } from 'react';
import {
  App,
  Badge,
  Button,
  Card,
  Dropdown,
  Form,
  Input,
  Modal,
  Space,
  Table,
  Tag,
  Typography,
} from 'antd';
import type { TableProps } from 'antd';
import { CaretDownOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import {
  useAdminPlans,
  useAdminUsers,
  useDeleteUserMutation,
  useGenerateUserMutation,
  useResetUserSecretMutation,
  useServerGroups,
  useUpdateUserMutation,
} from '@/lib/queries';
import { BYTE_GB, formatDateMinuteSlash, formatMoney } from '@v2board/config/format';
import type { AdminFilter } from '@v2board/api-client';
import type { AdminUserRow, AdminUserUpdatePayload } from '@v2board/types';
import { i18nGet } from '@/lib/errors';

interface QueryState {
  current: number;
  pageSize: number;
  filter: AdminFilter[];
}

export default function UsersPage() {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 20, filter: [] });
  const users = useAdminUsers(query);
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const update = useUpdateUserMutation();
  const remove = useDeleteUserMutation();
  const resetSecret = useResetUserSecretMutation();
  const generate = useGenerateUserMutation();

  const [editing, setEditing] = useState<AdminUserRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [filterEmail, setFilterEmail] = useState('');

  const planOptions = useMemo(
    () =>
      plans.data?.map((p) => ({ label: p.name, value: p.id })) ?? [],
    [plans.data],
  );

  const groupMap = useMemo(() => {
    const map = new Map<number, string>();
    for (const g of groups.data ?? []) map.set(g.id, g.name);
    return map;
  }, [groups.data]);

  const onSearch = () => {
    const filter: AdminFilter[] = filterEmail
      ? [{ key: 'email', condition: '模糊', value: filterEmail }]
      : [];
    setQuery({ current: 1, pageSize: query.pageSize, filter });
  };

  const doReset = (row: AdminUserRow) =>
    modal.confirm({
      title: t('admin.user.reset_secret'),
      onOk: async () => {
        try {
          await resetSecret.mutateAsync(row.id);
          message.success(t('common.success'));
        } catch (error) {
          if (error instanceof Error) message.error(i18nGet(error.message));
        }
      },
    });

  const doDelete = (row: AdminUserRow) =>
    modal.confirm({
      title: t('admin.user.mass_delete'),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await remove.mutateAsync(row.id);
          message.success(t('common.success'));
        } catch (error) {
          if (error instanceof Error) message.error(i18nGet(error.message));
        }
      },
    });

  const columns: TableProps<AdminUserRow>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 70, sorter: true },
    {
      title: t('admin.user.email'),
      dataIndex: 'email',
      ellipsis: true,
      render: (v: string, row) => (
        <span>
          <Badge status={row.alive_ip > 0 ? 'success' : 'default'} />
          {v}
        </span>
      ),
    },
    {
      title: t('admin.user.status'),
      dataIndex: 'banned',
      render: (v: 0 | 1) =>
        v === 1 ? (
          <Tag color="red">{t('admin.user.status_banned')}</Tag>
        ) : (
          <Tag color="green">{t('admin.user.status_normal')}</Tag>
        ),
    },
    { title: t('admin.user.subscription'), dataIndex: 'plan_name', render: (v) => v ?? '-' },
    {
      title: t('admin.user.group'),
      dataIndex: 'group_id',
      render: (v: number | null) => (v != null ? groupMap.get(v) ?? '-' : '-'),
    },
    {
      title: t('admin.user.used_g'),
      dataIndex: 'total_used',
      render: (v: number) => <Tag color="green">{(v / BYTE_GB).toFixed(2)}</Tag>,
    },
    {
      title: t('admin.user.transfer_g'),
      dataIndex: 'transfer_enable',
      render: (v: number) => (v / BYTE_GB).toFixed(2),
    },
    {
      title: t('admin.user.device_count'),
      render: (_: unknown, row) => `${row.alive_ip} / ${row.device_limit ?? '∞'}`,
    },
    {
      title: t('dashboard.valid_until'),
      dataIndex: 'expired_at',
      render: (v: number | null) => {
        if (v == null) return <Tag color="red">-</Tag>;
        const expired = v * 1000 < Date.now();
        return <Tag color={expired ? 'red' : 'green'}>{formatDateMinuteSlash(v)}</Tag>;
      },
    },
    {
      title: t('admin.user.balance'),
      dataIndex: 'balance',
      render: (v: number) => formatMoney(v, ''),
    },
    {
      title: t('admin.user.commission'),
      dataIndex: 'commission_balance',
      render: (v: number) => formatMoney(v, ''),
    },
    {
      title: t('admin.user.created_at'),
      dataIndex: 'created_at',
      render: (v: number) => formatDateMinuteSlash(v),
    },
    {
      title: t('common.operation'),
      fixed: 'right',
      width: 100,
      render: (_: unknown, row: AdminUserRow) => (
        <Dropdown
          trigger={['click']}
          menu={{
            items: [
              { key: 'edit', label: t('common.edit') },
              { key: 'reset', label: t('admin.user.reset_secret') },
              { key: 'delete', danger: true, label: t('common.delete') },
            ],
            onClick: ({ key }) => {
              if (key === 'edit') setEditing(row);
              else if (key === 'reset') doReset(row);
              else if (key === 'delete') doDelete(row);
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
      <Typography.Title level={3}>{t('admin.nav.users')}</Typography.Title>
      <Card>
        <Space wrap>
          <Input.Search
            placeholder="email"
            value={filterEmail}
            onChange={(e) => setFilterEmail(e.target.value)}
            onSearch={onSearch}
            allowClear
            style={{ width: 280 }}
          />
          <Button type="primary" onClick={() => setCreating(true)}>
            {t('admin.user.generate')}
          </Button>
        </Space>
      </Card>
      <Card>
        <Table<AdminUserRow>
          loading={users.isLoading}
          rowKey="id"
          dataSource={users.data?.data ?? []}
          columns={columns}
          scroll={{ x: 'max-content' }}
          pagination={{
            current: query.current,
            pageSize: query.pageSize,
            total: users.data?.total ?? 0,
            showSizeChanger: true,
            onChange: (current, pageSize) => setQuery((q) => ({ ...q, current, pageSize })),
          }}
        />
      </Card>

      <UserEditModal
        open={Boolean(editing)}
        user={editing}
        plans={planOptions}
        onClose={() => setEditing(null)}
        onSubmit={async (values) => {
          try {
            await update.mutateAsync(values as unknown as AdminUserUpdatePayload);
            message.success(t('common.success'));
            setEditing(null);
          } catch (error) {
            if (error instanceof Error) message.error(i18nGet(error.message));
          }
        }}
      />

      <GenerateUserModal
        open={creating}
        plans={planOptions}
        onClose={() => setCreating(false)}
        onSubmit={async (values) => {
          try {
            await generate.mutateAsync(values as Parameters<typeof generate.mutateAsync>[0]);
            message.success(t('common.success'));
            setCreating(false);
          } catch (error) {
            if (error instanceof Error) message.error(i18nGet(error.message));
          }
        }}
      />
    </div>
  );
}

interface PlanOption {
  label: string;
  value: number;
}

function UserEditModal({
  open,
  user,
  plans,
  onClose,
  onSubmit,
}: {
  open: boolean;
  user: AdminUserRow | null;
  plans: PlanOption[];
  onClose: () => void;
  onSubmit: (values: Record<string, unknown>) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();

  return (
    <Modal
      open={open}
      title={`Edit user #${user?.id ?? ''}`}
      onCancel={onClose}
      destroyOnClose
      onOk={() => form.submit()}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={user ?? {}}
        onFinish={(values) => onSubmit({ ...values, id: user?.id })}
      >
        <Form.Item name="email" label="Email" rules={[{ required: true, type: 'email' }]}>
          <Input />
        </Form.Item>
        <Form.Item name="password" label="Password">
          <Input.Password placeholder="leave empty to keep" />
        </Form.Item>
        <Form.Item name="plan_id" label={t('order.plan')}>
          <select className="ant-select-selector" style={{ width: '100%', height: 32 }}>
            <option value="">-</option>
            {plans.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </Form.Item>
        <Form.Item name="balance" label={t('dashboard.balance')}>
          <Input type="number" />
        </Form.Item>
        <Form.Item name="commission_balance" label={t('dashboard.commission_balance')}>
          <Input type="number" />
        </Form.Item>
        <Form.Item name="banned" label="Banned">
          <Input type="number" min={0} max={1} />
        </Form.Item>
        <Form.Item name="is_admin" label="Admin">
          <Input type="number" min={0} max={1} />
        </Form.Item>
      </Form>
    </Modal>
  );
}

function GenerateUserModal({
  open,
  plans,
  onClose,
  onSubmit,
}: {
  open: boolean;
  plans: PlanOption[];
  onClose: () => void;
  onSubmit: (values: Record<string, unknown>) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal open={open} onCancel={onClose} title={t('admin.user.generate')} onOk={() => form.submit()}>
      <Form layout="vertical" form={form} onFinish={onSubmit}>
        <Form.Item name="email_prefix" label="Prefix">
          <Input placeholder="leave empty for batch" />
        </Form.Item>
        <Form.Item name="email_suffix" label="Suffix" rules={[{ required: true }]}>
          <Input placeholder="example.com" />
        </Form.Item>
        <Form.Item name="password" label="Password">
          <Input.Password />
        </Form.Item>
        <Form.Item name="plan_id" label={t('order.plan')}>
          <select className="ant-select-selector" style={{ width: '100%', height: 32 }}>
            <option value="">-</option>
            {plans.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </Form.Item>
        <Form.Item name="generate_count" label="Batch count">
          <Input type="number" min={1} />
        </Form.Item>
        <Form.Item name="expired_at" label="Expired at (timestamp)">
          <Input type="number" />
        </Form.Item>
      </Form>
    </Modal>
  );
}
