import { cloneElement, useEffect, useRef, useState, type ReactElement } from 'react';
import { Input, Modal, Select, Tooltip } from 'antd';
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
import { LegacyDragSort, LegacyMenuIcon } from '@/components/legacy-drag-sort';
import { LegacyButton } from '@/components/legacy-button';
import { LegacyModal } from '@/components/legacy-modal';
import { LegacyPlusIcon, LegacyQuestionCircleIcon } from '@/components/legacy-ant-icon';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  type LegacyStandaloneTableHeader,
} from '@/components/legacy-standalone-table';
import { LegacySwitch } from '@/components/legacy-switch';

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
  const [selectPaymentMethod, setSelectPaymentMethod] = useState<string | undefined>(undefined);
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
      <LegacyModal
        title={submit.id ? '编辑支付方式' : '添加支付方式'}
        visible={visible}
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
                    submitOnChange(
                      'handling_fee_fixed',
                      100 * (event.target.value as unknown as number),
                    )
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
                  <Select.Option value={method}>{method}</Select.Option>
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
      </LegacyModal>
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

  useEffect(() => {
    if (payments.data) setOrderedPayments(payments.data);
  }, [payments.data]);

  orderRef.current = orderedPayments;

  const sortPayment = (fromIndex: number, toIndex: number) => {
    const next = [...orderRef.current];
    const moved = next[fromIndex];
    if (!moved) return;
    if (fromIndex < toIndex) {
      next.splice(toIndex + 1, 0, moved);
      next.splice(fromIndex, 1);
    } else {
      next.splice(toIndex, 0, moved);
      next.splice(fromIndex + 1, 1);
    }
    setOrderedPayments(next);
    setLegacySortLoading(true);
    sort.mutate(
      next.map((payment) => payment.id),
      {
        onSuccess: () => {
          void payments.refetch().finally(() => {
            setLegacySortLoading(false);
          });
        },
      },
    );
  };

  const headers: LegacyStandaloneTableHeader[] = [
    { title: 'ID' },
    { title: '启用' },
    { title: '显示名称' },
    { title: '支付接口' },
    {
      title: (
        <span>
          通知地址{' '}
          <Tooltip
            placement="top"
            title="支付网关将会把数据通知到本地地址，请通过防火墙放行本地地址。"
          >
            <LegacyQuestionCircleIcon />
          </Tooltip>
        </span>
      ),
    },
    { title: '操作', alignRight: true, fixedRight: true },
  ];

  const renderPaymentEnableSwitch = (enable: 0 | 1 | string, row: AdminPayment) => (
    <LegacySwitch
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
  );

  const renderPaymentActions = (row: AdminPayment) => (
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
  );

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
                <LegacyButton className="ant-btn">
                  <LegacyPlusIcon />
                  <span> 添加支付方式</span>
                </LegacyButton>
              </PaymentEditor>
            </div>
            <LegacyDragSort
              onDragEnd={(fromIndex, toIndex) => sortPayment(fromIndex, toIndex)}
              nodeSelector="tr"
              handleSelector="i"
            >
              <LegacyStandaloneTable
                headers={headers}
                isEmpty={orderedPayments.length === 0}
                scrollX={1300}
                scrollPositionRight={false}
                fixedRightChildren={orderedPayments.map((row, index) => (
                  <tr
                    key={index}
                    className="ant-table-row ant-table-row-level-0"
                    {...legacyTableRowKey(index)}
                  >
                    <td className="ant-table-row-cell-last" style={{ textAlign: 'right' }}>
                      {renderPaymentActions(row)}
                    </td>
                  </tr>
                ))}
              >
                {orderedPayments.map((row, index) => (
                  <tr
                    key={index}
                    className="ant-table-row ant-table-row-level-0"
                    {...legacyTableRowKey(index)}
                  >
                    <td className="">
                      <LegacyMenuIcon /> {row.id}
                    </td>
                    <td className="">{renderPaymentEnableSwitch(row.enable, row)}</td>
                    <td className="">{row.name}</td>
                    <td className="">{row.payment}</td>
                    <td className="">{row.notify_url}</td>
                    <td
                      className="ant-table-fixed-columns-in-body ant-table-row-cell-last"
                      style={{ textAlign: 'right' }}
                    >
                      {renderPaymentActions(row)}
                    </td>
                  </tr>
                ))}
              </LegacyStandaloneTable>
            </LegacyDragSort>
          </div>
        </div>
      </LegacySpin>
    </>
  );
}
