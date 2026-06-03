import {
  cloneElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  type HTMLAttributes,
  type ReactElement,
} from 'react';
import { Button, Input, Modal, Select, Switch, Table, Tooltip } from 'antd';
import type { TableProps } from 'antd';
import { MenuOutlined, PlusOutlined, QuestionCircleOutlined } from '@ant-design/icons';
import { admin } from '@v2board/api-client';
import type { AdminPayment, PaymentFormDefinition } from '@v2board/types';
import { apiClient } from '@/lib/api';
import {
  useAdminPayments,
  useDropPaymentMutation,
  useSavePaymentMutation,
  useShowPaymentMutation,
  useSortPaymentMutation,
} from '@/lib/queries';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';

type SavePaymentPayload = Parameters<typeof admin.savePayment>[1];

function PaymentEditor({
  record,
  fetchLoading,
  children,
  onSave,
  onSaved,
}: {
  record?: AdminPayment;
  fetchLoading: boolean;
  children: ReactElement<{ onClick?: () => void }>;
  onSave: (payload: SavePaymentPayload) => Promise<unknown>;
  onSaved: () => void;
}) {
  const [submit, setSubmit] = useState<Record<string, unknown>>(() => ({ ...(record ?? {}) }));
  const [visible, setVisible] = useState(false);
  const [paymentMethods, setPaymentMethods] = useState<string[]>([]);
  const [selectPaymentMethod, setSelectPaymentMethod] = useState<string | undefined>(
    record?.payment,
  );
  const [form, setForm] = useState<PaymentFormDefinition>({});
  const [config, setConfig] = useState<Record<string, unknown>>(() => ({
    ...(record?.config ?? {}),
  }));

  const submitOnChange = (key: string, value: unknown) => {
    setSubmit((current) => ({ ...current, [key]: value }));
  };

  const configOnChange = (key: string, value: unknown) => {
    setConfig((current) => ({ ...current, [key]: value }));
  };

  const onSelectPaymentMethod = async (payment: string | undefined) => {
    const nextForm = await admin.paymentForm(apiClient, payment, record?.id);
    setForm(nextForm);
    setSelectPaymentMethod(payment);
  };

  const show = async () => {
    const methods = await admin.paymentMethods(apiClient);
    const selected = record?.payment || methods[0];
    setPaymentMethods(methods);
    setSelectPaymentMethod(selected);
    setVisible(true);
    await onSelectPaymentMethod(selected);
  };

  const save = async () => {
    await onSave({
      ...submit,
      payment: selectPaymentMethod,
      config,
    } as SavePaymentPayload);
    setVisible(false);
    onSaved();
  };

  return (
    <>
      {cloneElement(children, { onClick: show })}
      <Modal
        title={submit.id ? '编辑支付方式' : '添加支付方式'}
        open={visible}
        onCancel={() => setVisible(false)}
        onOk={save}
        okText={submit.id ? '保存' : '添加'}
        okButtonProps={{ loading: fetchLoading }}
        cancelText="取消"
      >
        <div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">显示名称</label>
            <Input
              placeholder="用于前端显示使用"
              defaultValue={submit.name as string | undefined}
              onChange={(event) => submitOnChange('name', event.target.value)}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">图标URL(选填)</label>
            <Input
              placeholder="用于前端显示使用(https://x.com/icon.svg)"
              defaultValue={submit.icon as string | undefined}
              onChange={(event) => submitOnChange('icon', event.target.value)}
            />
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">自定义通知域名(选填)</label>
            <Input
              placeholder="网关的通知将会发送到该域名(https://x.com)"
              defaultValue={submit.notify_domain as string | undefined}
              onChange={(event) => submitOnChange('notify_domain', event.target.value)}
            />
          </div>
          <div className="row">
            <div className="col-6">
              <div className="form-group">
                <label htmlFor="example-text-input-alt">百分比手续费(选填)</label>
                <Input
                  suffix="%"
                  type="number"
                  placeholder="在订单金额基础上附加手续费"
                  defaultValue={submit.handling_fee_percent as string | number | undefined}
                  onChange={(event) => submitOnChange('handling_fee_percent', event.target.value)}
                />
              </div>
            </div>
            <div className="col-6">
              <div className="form-group">
                <label htmlFor="example-text-input-alt">固定手续费(选填)</label>
                <Input
                  type="number"
                  placeholder="在订单金额基础上附加手续费"
                  defaultValue={(submit.handling_fee_fixed as number) / 100}
                  onChange={(event) =>
                    submitOnChange('handling_fee_fixed', 100 * (event.target.value as unknown as number))
                  }
                />
              </div>
            </div>
          </div>
          <div className="form-group">
            <label htmlFor="example-text-input-alt">接口文件</label>
            <div>
              <Select
                style={{ width: '100%' }}
                defaultValue={selectPaymentMethod}
                onChange={(value) => {
                  void onSelectPaymentMethod(value);
                }}
              >
                {paymentMethods.map((method) => (
                  <Select.Option value={method}>
                    {method}
                  </Select.Option>
                ))}
              </Select>
            </div>
          </div>
          {Object.keys(form).map((key) => {
            const field = form[key];
            if (!field) return null;
            const inputType = field.type;
            const showInput =
              inputType === 'input' || inputType === 'text' || inputType === 'string' || !inputType;

            return (
              <div className="form-group">
                <label htmlFor="example-text-input-alt">{field.label}</label>
                {showInput ? (
                  <Input
                    placeholder={field.description}
                    defaultValue={(config[key] || field.value) as string | undefined}
                    onChange={(event) => configOnChange(key, event.target.value)}
                  />
                ) : null}
              </div>
            );
          })}
          {selectPaymentMethod === 'MGate' ? (
            <div className="alert alert-warning mb-0" role="alert">
              <p className="mb-0">MGate TG@nulledsan</p>
            </div>
          ) : null}
        </div>
      </Modal>
    </>
  );
}

export default function PaymentsPage() {
  const payments = useAdminPayments();
  const save = useSavePaymentMutation();
  const show = useShowPaymentMutation();
  const drop = useDropPaymentMutation();
  const sort = useSortPaymentMutation();
  const [orderedPayments, setOrderedPayments] = useState<AdminPayment[]>(() => payments.data ?? []);
  const [legacySortLoading, setLegacySortLoading] = useState(false);
  const orderRef = useRef(orderedPayments);
  const dragIndex = useRef<number | null>(null);

  useEffect(() => {
    if (payments.data) setOrderedPayments(payments.data);
  }, [payments.data]);

  orderRef.current = orderedPayments;

  const components = useMemo(
    () => ({
      body: {
        row: (
          props: HTMLAttributes<HTMLTableRowElement> & { 'data-sort-index'?: number },
        ) => {
          const onDrop = () => {
            const from = dragIndex.current;
            const to = Number(props['data-sort-index']);
            dragIndex.current = null;
            if (from == null || !Number.isFinite(to) || from === to) return;

            const next = [...orderRef.current];
            const moved = next[from];
            if (!moved) return;
            if (from < to) {
              next.splice(to + 1, 0, moved);
              next.splice(from, 1);
            } else {
              next.splice(to, 0, moved);
              next.splice(from + 1, 1);
            }
            setOrderedPayments(next);
            setLegacySortLoading(true);
            sort.mutate(next.map((payment) => payment.id), {
              onSuccess: () => {
                void payments.refetch().finally(() => {
                  setLegacySortLoading(false);
                });
              },
            });
          };

          return <tr {...props} onDragOver={(event) => event.preventDefault()} onDrop={onDrop} />;
        },
      },
    }),
    [payments, sort],
  );

  const columns: TableProps<AdminPayment>['columns'] = [
    {
      title: 'ID',
      dataIndex: 'id',
      key: 'id',
      render: (id: number, _row, index) => (
        <>
          <MenuOutlined
            draggable
            onDragStart={() => {
              dragIndex.current = index;
            }}
            style={{ cursor: 'move' }}
          />{' '}
          {id}
        </>
      ),
    },
    {
      title: '启用',
      dataIndex: 'enable',
      key: 'enable',
      render: (enable: 0 | 1 | string, row) => (
        <Switch
          checked={parseInt(String(enable), 10) as unknown as boolean}
          size="small"
          onChange={() =>
            show.mutate(row.id, {
              onSuccess: () => {
                void payments.refetch();
              },
            })
          }
        />
      ),
    },
    {
      title: '显示名称',
      dataIndex: 'name',
      key: 'name',
    },
    {
      title: '支付接口',
      dataIndex: 'payment',
      key: 'payment',
    },
    {
      title: (
        <span>
          通知地址{' '}
          <Tooltip
            placement="top"
            title="支付网关将会把数据通知到本地地址，请通过防火墙放行本地地址。"
          >
            <QuestionCircleOutlined />
          </Tooltip>
        </span>
      ),
      dataIndex: 'notify_url',
      key: 'notify_url',
    },
    {
      title: '操作',
      dataIndex: 'action',
      key: 'action',
      align: 'right',
      fixed: 'right',
      render: (_value, row) => (
        <>
          <PaymentEditor
            key={row.id}
            record={row}
            fetchLoading={payments.isFetching}
            onSave={(payload) => save.mutateAsync(payload)}
            onSaved={() => {
              void payments.refetch();
            }}
          >
            <a ref={legacyHref()}>编辑</a>
          </PaymentEditor>
          <div className="ant-divider ant-divider-vertical" />
          <a
            ref={legacyHref('javascript:void(0)')}
            onClick={() => {
              Modal.confirm({
                title: '警告',
                content: '确定要删除该条项目吗？',
                onOk: () => {
                  void drop.mutateAsync(row.id).then(() => {
                    void payments.refetch();
                  });
                },
                okText: '确定',
                cancelText: '取消',
              });
            }}
          >
            删除
          </a>
        </>
      ),
    },
  ];

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={payments.isFetching || legacySortLoading}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <PaymentEditor
                key={0}
                fetchLoading={payments.isFetching}
                onSave={(payload) => save.mutateAsync(payload)}
                onSaved={() => {
                  void payments.refetch();
                }}
              >
                <Button>
                  <PlusOutlined /> 添加支付方式
                </Button>
              </PaymentEditor>
            </div>
            <Table<AdminPayment>
              tableLayout="auto"
              dataSource={orderedPayments}
              columns={columns}
              components={components}
              pagination={false}
              onRow={(_record, index) =>
                ({ 'data-sort-index': index } as HTMLAttributes<HTMLElement>)
              }
              scroll={{ x: 1300 }}
            />
          </div>
        </div>
      </LegacySpin>
    </>
  );
}
