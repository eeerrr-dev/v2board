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
  Typography,
} from 'antd';
import type { TableProps } from 'antd';
import { useTranslation } from 'react-i18next';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import {
  useAdminKnowledge,
  useAdminKnowledgeCategories,
  useDropKnowledgeMutation,
  useSaveKnowledgeMutation,
  useShowKnowledgeMutation,
} from '@/lib/queries';
import { admin } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import { formatDateTime } from '@v2board/config/format';
import { i18nGet } from '@/lib/errors';
import { SUPPORTED_LOCALES } from '@v2board/i18n';

const LANGUAGE_OPTIONS = SUPPORTED_LOCALES.map((l) => ({ label: l.label, value: l.code }));

export default function KnowledgePage() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const list = useAdminKnowledge();
  const categories = useAdminKnowledgeCategories();
  const save = useSaveKnowledgeMutation();
  const drop = useDropKnowledgeMutation();
  const show = useShowKnowledgeMutation();
  const [editing, setEditing] = useState<Knowledge | null>(null);
  const [creating, setCreating] = useState(false);

  const onEdit = async (id: number) => {
    try {
      const detail = await admin.knowledgeDetail(apiClient, id);
      setEditing(detail);
    } catch (e) {
      if (e instanceof Error) message.error(i18nGet(e.message));
    }
  };

  const columns: TableProps<KnowledgeSummary>['columns'] = [
    { title: 'ID', dataIndex: 'id', width: 60 },
    { title: t('knowledge.category'), dataIndex: 'category' },
    { title: t('common.title'), dataIndex: 'title' },
    {
      title: t('common.enable'),
      dataIndex: 'show',
      render: (v: 0 | 1 | undefined, row) => (
        <Switch
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
    {
      title: t('order.updated_at'),
      dataIndex: 'updated_at',
      render: (v: number) => formatDateTime(v),
    },
    {
      title: t('common.operation'),
      render: (_: unknown, row) => (
        <Space size="small">
          <Button size="small" onClick={() => onEdit(row.id)}>
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
  ];

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.knowledge')}</Typography.Title>
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('common.add')}
        </Button>
      </Card>
      <Card>
        <Table<KnowledgeSummary>
          loading={list.isLoading}
          rowKey="id"
          dataSource={list.data ?? []}
          columns={columns}
        />
      </Card>
      <KnowledgeModal
        open={Boolean(editing) || creating}
        knowledge={editing}
        categories={categories.data ?? []}
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

function KnowledgeModal({
  open,
  knowledge,
  categories,
  onClose,
  onSubmit,
}: {
  open: boolean;
  knowledge: Knowledge | null;
  categories: string[];
  onClose: () => void;
  onSubmit: (values: Partial<Knowledge>) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  return (
    <Modal
      open={open}
      title={knowledge ? t('common.edit') : t('common.add')}
      onCancel={onClose}
      destroyOnClose
      onOk={() => form.submit()}
      width={840}
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={knowledge ?? { language: 'zh-CN', show: 1, sort: 1 }}
        onFinish={(values) => onSubmit({ ...values, id: knowledge?.id })}
      >
        <Form.Item name="title" label={t('common.title')} rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item
          name="category"
          label={t('knowledge.category')}
          rules={[{ required: true }]}
        >
          <Select
            mode="tags"
            maxCount={1}
            options={categories.map((c) => ({ label: c, value: c }))}
          />
        </Form.Item>
        <Form.Item name="language" label={t('common.language')} rules={[{ required: true }]}>
          <Select options={LANGUAGE_OPTIONS} />
        </Form.Item>
        <Form.Item name="body" label={t('admin.knowledge.body')} rules={[{ required: true }]}>
          <Input.TextArea rows={14} />
        </Form.Item>
        <Form.Item name="sort" label={t('admin.knowledge.sort')}>
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="show" label={t('common.enable')}>
          <InputNumber min={0} max={1} />
        </Form.Item>
      </Form>
    </Modal>
  );
}
