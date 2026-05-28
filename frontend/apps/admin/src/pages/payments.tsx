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
  Space,
  Switch,
  Table,
  Typography,
} from 'antd';
import { useTranslation } from 'react-i18next';
import {
  useAdminPayments,
  useDropPaymentMutation,
  usePaymentMethods,
  useSavePaymentMutation,
  useShowPaymentMutation,
} from '@/lib/queries';
import { admin } from '@v2board/api-client';
import { apiClient } from '@/lib/api';
import type { AdminPayment } from '@v2board/types';
import { i18nGet } from '@/lib/errors';

export default function PaymentsPage() {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const payments = useAdminPayments();
  const methods = usePaymentMethods();
  const save = useSavePaymentMutation();
  const drop = useDropPaymentMutation();
  const show = useShowPaymentMutation();
  const [editing, setEditing] = useState<AdminPayment | null>(null);
  const [creating, setCreating] = useState(false);

  return (
    <div className="space-y-4">
      <Typography.Title level={3}>{t('admin.nav.payments')}</Typography.Title>
      <Card>
        <Button type="primary" onClick={() => setCreating(true)}>
          {t('common.add')}
        </Button>
      </Card>
      <Card>
        <Table<AdminPayment>
          loading={payments.isLoading}
          rowKey="id"
          dataSource={payments.data ?? []}
          columns={[
            { title: 'ID', dataIndex: 'id', width: 60 },
            { title: t('admin.common.name'), dataIndex: 'name' },
            { title: t('admin.common.payment_method'), dataIndex: 'payment' },
            {
              title: t('common.enable'),
              dataIndex: 'enable',
              render: (v: 0 | 1, row) => (
                <Switch
                  checked={v === 1}
                  onChange={async (next) => {
                    try {
                      await show.mutateAsync({ id: row.id, enable: next ? 1 : 0 });
                      message.success(t('common.success'));
                    } catch (e) {
                      if (e instanceof Error) message.error(i18nGet(e.message));
                    }
                  }}
                />
              ),
            },
            {
              title: t('common.operation'),
              render: (_, row) => (
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
      <PaymentModal
        open={Boolean(editing) || creating}
        payment={editing}
        methods={methods.data ?? []}
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

function PaymentModal({
  open,
  payment,
  methods,
  onClose,
  onSubmit,
}: {
  open: boolean;
  payment: AdminPayment | null;
  methods: string[];
  onClose: () => void;
  onSubmit: (values: Partial<AdminPayment>) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [paymentType, setPaymentType] = useState(payment?.payment ?? methods[0] ?? '');
  const formDef = useFormDef(paymentType);

  return (
    <Modal
      open={open}
      title={payment ? t('common.edit') : t('common.add')}
      onCancel={onClose}
      onOk={() => form.submit()}
      destroyOnClose
    >
      <Form
        layout="vertical"
        form={form}
        initialValues={payment ?? { enable: 1, payment: methods[0] }}
        onFinish={(values) => onSubmit({ ...values, id: payment?.id })}
      >
        <Form.Item name="name" label="Name" rules={[{ required: true }]}>
          <Input />
        </Form.Item>
        <Form.Item name="payment" label="Payment driver" rules={[{ required: true }]}>
          <select
            value={paymentType}
            onChange={(e) => {
              setPaymentType(e.target.value);
              form.setFieldValue('payment', e.target.value);
            }}
            style={{ width: '100%', height: 32, padding: '0 8px' }}
          >
            {methods.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </Form.Item>
        <Form.Item name="icon" label="Icon URL">
          <Input />
        </Form.Item>
        <Form.Item name="handling_fee_fixed" label="Handling fee (fixed, cents)">
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        <Form.Item name="handling_fee_percent" label="Handling fee (percent)">
          <InputNumber style={{ width: '100%' }} />
        </Form.Item>
        {formDef &&
          Object.entries(formDef).map(([key, def]) => (
            <Form.Item key={key} name={['config', key]} label={def.label} tooltip={def.description}>
              <Input />
            </Form.Item>
          ))}
      </Form>
    </Modal>
  );
}

function useFormDef(paymentType: string) {
  const [formDef, setFormDef] = useState<Record<string, { label: string; description?: string }> | null>(
    null,
  );
  if (paymentType && formDef == null) {
    void admin.paymentForm(apiClient, paymentType).then(setFormDef).catch(() => setFormDef({}));
  }
  return formDef;
}
