import { useState } from 'react';
import {
  App,
  Button,
  Card,
  Form,
  Input,
  Modal,
  Popconfirm,
  Space,
  Switch,
  Table,
  Typography,
} from 'antd';
import { useTranslation } from 'react-i18next';
import type { Notice } from '@v2board/types';
import {
  useAdminNotices,
  useDropNoticeMutation,
  useSaveNoticeMutation,
  useShowNoticeMutation,
  useUpdateNoticeMutation,
} from '@/lib/queries';
import { formatDateTime } from '@v2board/config/format';
import { i18nGet } from '@/lib/errors';

export default function NoticesPage() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [query, setQuery] = useState({ current: 1, pageSize: 20 });
  const notices = useAdminNotices(query);
  const save = useSaveNoticeMutation();
  const update = useUpdateNoticeMutation();
  const drop = useDropNoticeMutation();
  const show = useShowNoticeMutation();
  const [editing, setEditing] = useState<Notice | null>(null);
  const [creating, setCreating] = useState(false);

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.notices')}</Typography.Title>
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('common.add')}
        </Button>
      </Card>
      <Card>
        <Table<Notice>
          loading={notices.isLoading}
          rowKey="id"
          dataSource={notices.data?.data ?? []}
          columns={[
            { title: '#', dataIndex: 'id', width: 60 },
            {
              title: t('admin.plan.show'),
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
            { title: t('admin.common.title'), dataIndex: 'title' },
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
          pagination={{
            current: query.current,
            pageSize: query.pageSize,
            total: notices.data?.total ?? 0,
            onChange: (current, pageSize) => setQuery({ current, pageSize }),
          }}
        />
      </Card>
      <NoticeModal
        open={Boolean(editing) || creating}
        notice={editing}
        onClose={() => {
          setEditing(null);
          setCreating(false);
        }}
        onSubmit={async (values) => {
          try {
            if (editing) {
              await update.mutateAsync({ ...values, id: editing.id });
            } else {
              await save.mutateAsync(values);
            }
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

function NoticeModal({
  open,
  notice,
  onClose,
  onSubmit,
}: {
  open: boolean;
  notice: Notice | null;
  onClose: () => void;
  onSubmit: (values: Partial<Notice>) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal
      open={open}
      title={notice ? t('common.edit') : t('common.add')}
      onCancel={onClose}
      onOk={() => form.submit()}
      destroyOnClose
      width={720}
    >
      <Form layout="vertical" form={form} initialValues={notice ?? { show: 1 }} onFinish={onSubmit}>
        <Form.Item name="title" label="Title" rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="img_url" label="Image URL">
          <Input />
        </Form.Item>
        <Form.Item name="content" label="Content (HTML)" rules={[{ required: true }]}>
          <Input.TextArea rows={10} />
        </Form.Item>
      </Form>
    </Modal>
  );
}
