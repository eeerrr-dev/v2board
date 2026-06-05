import {
  cloneElement,
  useEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
  type MouseEvent as ReactMouseEvent,
} from 'react';
import { Button, Input, Modal, Select, Switch, Table, Tooltip } from 'antd';
import type { TableProps } from 'antd';
import { PlusOutlined, QuestionCircleOutlined } from '@ant-design/icons';
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

const LEGACY_DRAG_LINE_STYLE =
  'position:fixed;z-index:9999;height:0;margin-top:-1px;border-bottom:dashed 2px rgba(0,0,0,.3);display:none;';

function findClosestWithin(target: EventTarget | null, selector: string, root: HTMLElement | null) {
  let element = target instanceof Element ? target : null;
  while (element && element !== root) {
    if (element.matches(selector)) return element as HTMLElement;
    element = element.parentElement;
  }
  return null;
}

function siblingIndex(element: HTMLElement, ignoreSelector: string) {
  const parent = element.parentElement;
  if (!parent) return -1;
  return Array.from(parent.children)
    .filter((child) => ignoreSelector === '' || !child.matches(ignoreSelector))
    .indexOf(element);
}

function scrollParent(element: HTMLElement | null) {
  let current = element;
  while (current) {
    const overflow = window.getComputedStyle(current).overflow;
    if (
      (overflow === 'auto' || overflow === 'scroll') &&
      (current.offsetWidth < current.scrollWidth || current.offsetHeight < current.scrollHeight)
    ) {
      return current;
    }
    if (current === document.body) return null;
    current = current.parentElement;
  }
  return null;
}

function LegacyPaymentDragSort({
  children,
  onDragEnd,
  nodeSelector = 'tr',
  handleSelector = '',
  ignoreSelector = '',
  enableScroll = true,
  scrollSpeed = 10,
  lineClassName = '',
}: {
  children: ReactNode;
  onDragEnd: (fromIndex: number, toIndex: number) => void;
  nodeSelector?: string;
  handleSelector?: string;
  ignoreSelector?: string;
  enableScroll?: boolean;
  scrollSpeed?: number;
  lineClassName?: string;
}) {
  const dragList = useRef<HTMLDivElement | null>(null);
  const dragLine = useRef<HTMLDivElement | null>(null);
  const cacheDragTarget = useRef<HTMLElement | null>(null);
  const scrollElement = useRef<HTMLElement | null>(null);
  const scrollTimerId = useRef<number | null>(null);
  const fromIndex = useRef(-1);
  const toIndex = useRef(-1);
  const direction = useRef(3);

  const getDragNode = (target: EventTarget | null) =>
    findClosestWithin(target, nodeSelector, dragList.current);
  const getHandleNode = (target: EventTarget | null) =>
    findClosestWithin(target, handleSelector || nodeSelector, dragList.current);

  const getDragLine = () => {
    if (!dragLine.current) {
      dragLine.current = window.document.createElement('div');
      dragLine.current.setAttribute('style', LEGACY_DRAG_LINE_STYLE);
      window.document.body.appendChild(dragLine.current);
    }
    dragLine.current.className = lineClassName;
    return dragLine.current;
  };

  const hideDragLine = () => {
    if (dragLine.current) dragLine.current.style.display = 'none';
  };

  const fixDragLine = (element: HTMLElement | null) => {
    const line = getDragLine();
    if (!element || fromIndex.current < 0 || fromIndex.current === toIndex.current) {
      hideDragLine();
      return;
    }

    const rect = element.getBoundingClientRect();
    const top = toIndex.current < fromIndex.current ? rect.top : rect.top + rect.height;
    if (enableScroll && scrollElement.current) {
      const scrollRect = scrollElement.current.getBoundingClientRect();
      if (top < scrollRect.top - 2 || top > scrollRect.top + scrollRect.height + 2) {
        hideDragLine();
        return;
      }
    }

    line.style.left = `${rect.left}px`;
    line.style.width = `${rect.width}px`;
    line.style.top = `${top}px`;
    line.style.display = 'block';
  };

  const stopAutoScroll = () => {
    if (scrollTimerId.current !== null) {
      window.clearInterval(scrollTimerId.current);
      scrollTimerId.current = null;
    }
    fixDragLine(cacheDragTarget.current);
  };

  const autoScroll = () => {
    if (!scrollElement.current) return;
    const top = scrollElement.current.scrollTop;
    if (direction.current === 3) {
      scrollElement.current.scrollTop = top + scrollSpeed;
      if (top === scrollElement.current.scrollTop) stopAutoScroll();
    } else if (direction.current === 1) {
      scrollElement.current.scrollTop = top - scrollSpeed;
      if (scrollElement.current.scrollTop <= 0) stopAutoScroll();
    } else {
      stopAutoScroll();
    }
  };

  const resolveAutoScroll = (event: DragEvent, element: HTMLElement) => {
    if (!scrollElement.current) return;
    const rect = scrollElement.current.getBoundingClientRect();
    const zone = element.offsetHeight * (2 / 3);
    direction.current = 0;
    if (event.pageY > rect.top + rect.height - zone) direction.current = 3;
    else if (event.pageY < rect.top + zone) direction.current = 1;
    if (direction.current) {
      if (scrollTimerId.current === null) {
        scrollTimerId.current = window.setInterval(autoScroll, 20);
      }
    } else {
      stopAutoScroll();
    }
  };

  const onDragEnter = (event: DragEvent) => {
    const dragNode = getDragNode(event.target);
    if (dragNode) {
      toIndex.current = siblingIndex(dragNode, ignoreSelector);
      if (enableScroll) resolveAutoScroll(event, dragNode);
    } else {
      toIndex.current = -1;
      stopAutoScroll();
    }
    cacheDragTarget.current = dragNode;
    fixDragLine(dragNode);
  };

  const onDragStart = (event: DragEvent) => {
    const dragNode = getDragNode(event.target);
    if (!dragNode) return;
    const parent = dragNode.parentElement;
    if (!parent) return;
    event.dataTransfer?.setData('Text', '');
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
    parent.ondragenter = onDragEnter;
    parent.ondragover = (dragEvent) => {
      dragEvent.preventDefault();
      return true;
    };
    const index = siblingIndex(dragNode, ignoreSelector);
    fromIndex.current = index;
    toIndex.current = index;
    scrollElement.current = scrollParent(parent);
  };

  const onNativeDragEnd = (event: DragEvent) => {
    const dragNode = getDragNode(event.target);
    stopAutoScroll();
    if (dragNode) {
      dragNode.removeAttribute('draggable');
      dragNode.ondragstart = null;
      dragNode.ondragend = null;
      if (dragNode.parentElement) {
        dragNode.parentElement.ondragenter = null;
        dragNode.parentElement.ondragover = null;
      }
      if (fromIndex.current >= 0 && fromIndex.current !== toIndex.current) {
        onDragEnd(fromIndex.current, toIndex.current);
      }
    }
    hideDragLine();
    fromIndex.current = -1;
    toIndex.current = -1;
  };

  const onMouseDown = (event: ReactMouseEvent<HTMLDivElement>) => {
    const handle = getHandleNode(event.target);
    if (!handle) return;
    const dragNode =
      handleSelector && handleSelector !== nodeSelector ? getDragNode(handle) : handle;
    if (!dragNode) return;
    handle.setAttribute('draggable', 'false');
    dragNode.setAttribute('draggable', 'true');
    dragNode.ondragstart = onDragStart;
    dragNode.ondragend = onNativeDragEnd;
  };

  useEffect(
    () => () => {
      if (dragLine.current?.parentNode) dragLine.current.parentNode.removeChild(dragLine.current);
      dragLine.current = null;
      cacheDragTarget.current = null;
    },
    [],
  );

  return (
    <div role="presentation" onMouseDown={onMouseDown} ref={dragList}>
      {children}
    </div>
  );
}

function LegacyMenuIcon() {
  return (
    <i aria-label="icon: menu" className="anticon anticon-menu">
      <svg
        viewBox="64 64 896 896"
        focusable="false"
        data-icon="menu"
        width="1em"
        height="1em"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M904 160H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8zm0 624H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8zm0-312H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8z" />
      </svg>
    </i>
  );
}

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
    sort.mutate(next.map((payment) => payment.id), {
      onSuccess: () => {
        void payments.refetch().finally(() => {
          setLegacySortLoading(false);
        });
      },
    });
  };

  const columns: TableProps<AdminPayment>['columns'] = [
    {
      title: 'ID',
      dataIndex: 'id',
      key: 'id',
      render: (id: number) => (
        <>
          <LegacyMenuIcon />{' '}
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
            <LegacyPaymentDragSort
              onDragEnd={(fromIndex, toIndex) => sortPayment(fromIndex, toIndex)}
              nodeSelector="tr"
              handleSelector="i"
            >
              <Table<AdminPayment>
                tableLayout="auto"
                dataSource={orderedPayments}
                columns={columns}
                pagination={false}
                scroll={{ x: 1300 }}
              />
            </LegacyPaymentDragSort>
          </div>
        </div>
      </LegacySpin>
    </>
  );
}
