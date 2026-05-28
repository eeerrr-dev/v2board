import { useState } from 'react';
import {
  App,
  Button,
  Card,
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
import { useTranslation } from 'react-i18next';
import {
  useCopyServerMutation,
  useDropServerGroupMutation,
  useDropServerMutation,
  useDropServerRouteMutation,
  useSaveServerGroupMutation,
  useSaveServerRouteMutation,
  useServerGroups,
  useServerNodes,
  useServerRoutes,
  useUpdateServerMutation,
} from '@/lib/queries';
import { admin } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { i18nGet } from '@/lib/errors';
import { formatDateTime } from '@v2board/config/format';

const SERVER_TYPES: admin.ServerTypeName[] = [
  'shadowsocks',
  'vmess',
  'trojan',
  'tuic',
  'vless',
  'hysteria',
  'anytls',
];

export default function ServersPage() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.servers')}</Typography.Title>
      <Tabs
        items={[
          { key: 'nodes', label: t('admin.server.nodes'), children: <NodesTab /> },
          { key: 'groups', label: t('admin.server.groups'), children: <GroupsTab /> },
          { key: 'routes', label: t('admin.server.routes'), children: <RoutesTab /> },
        ]}
      />
    </div>
  );
}

function NodesTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const nodes = useServerNodes();
  const groups = useServerGroups();
  const update = useUpdateServerMutation();
  const drop = useDropServerMutation();
  const copy = useCopyServerMutation();
  const [editing, setEditing] = useState<{ type: admin.ServerTypeName; id?: number } | null>(null);

  const groupName = (ids: number[]) =>
    ids
      .map((id) => groups.data?.find((g) => g.id === id)?.name ?? String(id))
      .join(', ');

  const columns: TableProps<admin.ServerNode>['columns'] = [
    { title: t('admin.server.node_id'), dataIndex: 'id', width: 70 },
    {
      title: t('admin.server.show'),
      dataIndex: 'show',
      width: 80,
      render: (v: 0 | 1, row) => (
        <Switch
          size="small"
          checked={v === 1}
          onChange={async (next) => {
            try {
              await update.mutateAsync({
                type: row.type as admin.ServerTypeName,
                id: row.id,
                show: next ? 1 : 0,
              });
              message.success(t('common.success'));
            } catch (e) {
              if (e instanceof Error) message.error(i18nGet(e.message));
            }
          }}
        />
      ),
    },
    {
      title: t('admin.server.node'),
      dataIndex: 'name',
      render: (v: string, row) => (
        <span>
          {v} <Tag>{row.type}</Tag>
        </span>
      ),
    },
    {
      title: t('admin.server.address'),
      render: (_: unknown, row) => `${row.host}:${row.port}`,
    },
    {
      title: t('admin.server.people'),
      dataIndex: 'online',
      render: (v: number, row) => (
        <Tag color={row.is_online === 1 ? 'green' : 'default'}>{v}</Tag>
      ),
    },
    { title: t('admin.server.rate'), dataIndex: 'rate' },
    {
      title: t('admin.server.groups'),
      dataIndex: 'group_id',
      render: (v: number[]) => groupName(v),
    },
    {
      title: t('common.operation'),
      width: 260,
      render: (_: unknown, row) => (
        <Space size="small">
          <Button
            size="small"
            onClick={() => setEditing({ type: row.type as admin.ServerTypeName, id: row.id })}
          >
            {t('common.edit')}
          </Button>
          <Popconfirm
            title={t('admin.server.copy')}
            onConfirm={async () => {
              try {
                await copy.mutateAsync({ type: row.type as admin.ServerTypeName, id: row.id });
                message.success(t('common.success'));
              } catch (e) {
                if (e instanceof Error) message.error(i18nGet(e.message));
              }
            }}
          >
            <Button size="small">{t('admin.server.copy')}</Button>
          </Popconfirm>
          <Popconfirm
            title={t('common.delete')}
            onConfirm={async () => {
              try {
                await drop.mutateAsync({ type: row.type as admin.ServerTypeName, id: row.id });
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
        </Space>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <Card>
        <Space wrap>
          {SERVER_TYPES.map((type) => (
            <Button key={type} onClick={() => setEditing({ type })}>
              + {type}
            </Button>
          ))}
        </Space>
      </Card>
      <Card>
        <Table<admin.ServerNode>
          loading={nodes.isLoading}
          rowKey="id"
          dataSource={nodes.data ?? []}
          columns={columns}
          scroll={{ x: 'max-content' }}
        />
      </Card>
      <NodeEditModal
        open={editing != null}
        type={editing?.type ?? 'shadowsocks'}
        id={editing?.id}
        groups={groups.data ?? []}
        onClose={() => {
          setEditing(null);
          nodes.refetch();
        }}
      />
    </div>
  );
}

function NodeEditModal({
  open,
  type,
  id,
  groups,
  onClose,
}: {
  open: boolean;
  type: admin.ServerTypeName;
  id?: number;
  groups: admin.ServerGroup[];
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm();

  return (
    <Modal
      open={open}
      title={id ? `${t('common.edit')} ${type}` : `${t('common.add')} ${type}`}
      onCancel={onClose}
      destroyOnClose
      onOk={() => form.submit()}
      width={720}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={{ show: 1, rate: '1', port: 443 }}
        onFinish={async (values) => {
          try {
            await admin.saveServer(apiClient, type, { ...values, id });
            message.success(t('common.success'));
            onClose();
          } catch (e) {
            if (e instanceof Error) message.error(i18nGet(e.message));
          }
        }}
      >
        <Form.Item name="name" label={t('admin.server.name')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="host" label="Host" rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="port" label="Port" rules={[{ required: true }]}>
          <InputNumber style={{ width: '100%' }} min={1} max={65535} />
        </Form.Item>
        <Form.Item name="server_port" label="Server Port">
          <InputNumber style={{ width: '100%' }} min={1} max={65535} />
        </Form.Item>
        <Form.Item name="rate" label={t('admin.server.rate')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item
          name="group_id"
          label={t('admin.server.groups')}
          rules={[{ required: true }]}
        >
          <Select
            mode="multiple"
            options={groups.map((g) => ({ label: g.name, value: g.id }))}
          />
        </Form.Item>
        <Form.Item name="parent_id" label={t('admin.server.parent_id')}>
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        <ServerTypeFields type={type} />
        <Form.Item name="tags" label="Tags">
          <Select mode="tags" />
        </Form.Item>
        <Form.Item name="show" label={t('common.enable')}>
          <InputNumber min={0} max={1} />
        </Form.Item>
      </Form>
    </Modal>
  );
}

function ServerTypeFields({ type }: { type: admin.ServerTypeName }) {
  if (type === 'shadowsocks') {
    return (
      <Form.Item name="cipher" label="Cipher" initialValue="aes-256-gcm">
        <Select
          options={[
            { value: 'aes-256-gcm' },
            { value: 'aes-128-gcm' },
            { value: 'chacha20-ietf-poly1305' },
            { value: '2022-blake3-aes-128-gcm' },
            { value: '2022-blake3-aes-256-gcm' },
          ]}
        />
      </Form.Item>
    );
  }
  if (type === 'vmess') {
    return (
      <>
        <Form.Item name="tls" label="TLS" initialValue={0}>
          <InputNumber min={0} max={2} />
        </Form.Item>
        <Form.Item name="network" label="Network" initialValue="tcp">
          <Select options={[{ value: 'tcp' }, { value: 'ws' }, { value: 'grpc' }, { value: 'h2' }]} />
        </Form.Item>
        <Form.Item name="networkSettings" label="Network settings (JSON)">
          <Input.TextArea rows={3} />
        </Form.Item>
        <Form.Item name="tlsSettings" label="TLS settings (JSON)">
          <Input.TextArea rows={3} />
        </Form.Item>
      </>
    );
  }
  if (type === 'trojan') {
    return (
      <>
        <Form.Item name="server_name" label="SNI">
          <Input />
        </Form.Item>
        <Form.Item name="allow_insecure" label="Allow insecure" initialValue={0}>
          <InputNumber min={0} max={1} />
        </Form.Item>
        <Form.Item name="network" label="Network" initialValue="tcp">
          <Select options={[{ value: 'tcp' }, { value: 'grpc' }, { value: 'ws' }]} />
        </Form.Item>
        <Form.Item name="network_settings" label="Network settings (JSON)">
          <Input.TextArea rows={3} />
        </Form.Item>
      </>
    );
  }
  if (type === 'tuic') {
    return (
      <>
        <Form.Item name="server_name" label="SNI">
          <Input />
        </Form.Item>
        <Form.Item name="alpn" label="ALPN" initialValue="h3">
          <Input />
        </Form.Item>
        <Form.Item name="congestion_control" label="Congestion control" initialValue="bbr">
          <Input />
        </Form.Item>
      </>
    );
  }
  if (type === 'vless') {
    return (
      <>
        <Form.Item name="tls" label="TLS" initialValue={1}>
          <InputNumber min={0} max={2} />
        </Form.Item>
        <Form.Item name="flow" label="Flow">
          <Input />
        </Form.Item>
        <Form.Item name="network" label="Network" initialValue="tcp">
          <Select
            options={[{ value: 'tcp' }, { value: 'ws' }, { value: 'grpc' }, { value: 'h2' }, { value: 'kcp' }]}
          />
        </Form.Item>
        <Form.Item name="network_settings" label="Network settings (JSON)">
          <Input.TextArea rows={3} />
        </Form.Item>
        <Form.Item name="tls_settings" label="TLS settings (JSON)">
          <Input.TextArea rows={3} />
        </Form.Item>
        <Form.Item name="reality_settings" label="Reality settings (JSON)">
          <Input.TextArea rows={3} />
        </Form.Item>
      </>
    );
  }
  if (type === 'hysteria') {
    return (
      <>
        <Form.Item name="version" label="Version" initialValue={2}>
          <InputNumber min={1} max={2} />
        </Form.Item>
        <Form.Item name="up_mbps" label="Up Mbps">
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="down_mbps" label="Down Mbps">
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="obfs" label="Obfs">
          <Input />
        </Form.Item>
        <Form.Item name="obfs_password" label="Obfs password">
          <Input />
        </Form.Item>
        <Form.Item name="server_name" label="SNI">
          <Input />
        </Form.Item>
      </>
    );
  }
  if (type === 'anytls') {
    return (
      <>
        <Form.Item name="server_name" label="SNI">
          <Input />
        </Form.Item>
        <Form.Item name="padding_scheme" label="Padding scheme">
          <Input.TextArea rows={3} />
        </Form.Item>
        <Form.Item name="insecure" label="Insecure" initialValue={0}>
          <InputNumber min={0} max={1} />
        </Form.Item>
      </>
    );
  }
  return null;
}

function GroupsTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const groups = useServerGroups();
  const save = useSaveServerGroupMutation();
  const drop = useDropServerGroupMutation();
  const [editing, setEditing] = useState<admin.ServerGroup | null>(null);
  const [creating, setCreating] = useState(false);

  return (
    <div className="space-y-4">
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('common.add')}
        </Button>
      </Card>
      <Card>
        <Table<admin.ServerGroup>
          loading={groups.isLoading}
          rowKey="id"
          dataSource={groups.data ?? []}
          columns={[
            { title: 'ID', dataIndex: 'id', width: 80 },
            { title: t('admin.server.name'), dataIndex: 'name' },
            {
              title: t('order.created_at'),
              dataIndex: 'created_at',
              render: (v: number) => formatDateTime(v),
            },
            {
              title: t('common.operation'),
              render: (_: unknown, row) => (
                <Space size="small">
                  <Button size="small" onClick={() => setEditing(row)}>
                    {t('common.edit')}
                  </Button>
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
                </Space>
              ),
            },
          ]}
        />
      </Card>
      <Modal
        open={Boolean(editing) || creating}
        title={editing ? t('common.edit') : t('common.add')}
        onCancel={() => {
          setEditing(null);
          setCreating(false);
        }}
        destroyOnClose
        onOk={async () => {
          const name = (document.getElementById('group_name') as HTMLInputElement | null)?.value;
          if (!name) return;
          try {
            await save.mutateAsync({ id: editing?.id, name });
            message.success(t('common.success'));
            setEditing(null);
            setCreating(false);
          } catch (e) {
            if (e instanceof Error) message.error(i18nGet(e.message));
          }
        }}
      >
        <Input
          id="group_name"
          defaultValue={editing?.name ?? ''}
          placeholder={t('admin.server.name')}
        />
      </Modal>
    </div>
  );
}

function RoutesTab() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const routes = useServerRoutes();
  const save = useSaveServerRouteMutation();
  const drop = useDropServerRouteMutation();
  const [editing, setEditing] = useState<admin.ServerRoute | null>(null);
  const [creating, setCreating] = useState(false);

  return (
    <div className="space-y-4">
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('common.add')}
        </Button>
      </Card>
      <Card>
        <Table<admin.ServerRoute>
          loading={routes.isLoading}
          rowKey="id"
          dataSource={routes.data ?? []}
          columns={[
            { title: 'ID', dataIndex: 'id', width: 80 },
            { title: t('admin.server.route_remarks'), dataIndex: 'remarks' },
            {
              title: t('admin.server.route_match'),
              dataIndex: 'match',
              render: (v: string[]) => v.join(', '),
            },
            { title: t('admin.server.route_action'), dataIndex: 'action' },
            { title: t('admin.server.route_action_value'), dataIndex: 'action_value' },
            {
              title: t('common.operation'),
              render: (_: unknown, row) => (
                <Space size="small">
                  <Button size="small" onClick={() => setEditing(row)}>
                    {t('common.edit')}
                  </Button>
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
                </Space>
              ),
            },
          ]}
        />
      </Card>
      <RouteEditModal
        open={Boolean(editing) || creating}
        route={editing}
        onClose={() => {
          setEditing(null);
          setCreating(false);
        }}
        onSubmit={async (values) => {
          try {
            await save.mutateAsync({ ...values, id: editing?.id });
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

function RouteEditModal({
  open,
  route,
  onClose,
  onSubmit,
}: {
  open: boolean;
  route: admin.ServerRoute | null;
  onClose: () => void;
  onSubmit: (values: Partial<admin.ServerRoute>) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal
      open={open}
      title={route ? t('common.edit') : t('common.add')}
      onCancel={onClose}
      destroyOnClose
      onOk={() => form.submit()}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={route ?? { action: 'block' }}
        onFinish={onSubmit}
      >
        <Form.Item name="remarks" label={t('admin.server.route_remarks')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="match" label={t('admin.server.route_match')} rules={[{ required: true }]}>
          <Select mode="tags" />
        </Form.Item>
        <Form.Item name="action" label={t('admin.server.route_action')} rules={[{ required: true }]}>
          <Select
            options={[
              { value: 'block', label: 'block' },
              { value: 'dns', label: 'dns' },
            ]}
          />
        </Form.Item>
        <Form.Item name="action_value" label={t('admin.server.route_action_value')}>
          <Input />
        </Form.Item>
      </Form>
    </Modal>
  );
}
