import { useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import {
  App,
  Badge,
  Button,
  Col,
  Divider,
  Dropdown,
  Input,
  Modal,
  Row,
  Select,
  Spin,
  Table,
  Tag,
  Tooltip,
} from 'antd';
import type { ButtonProps, TablePaginationConfig, TableProps } from 'antd';
import {
  CaretDownOutlined,
  FilterOutlined,
  LoadingOutlined,
  PlusOutlined,
  QuestionCircleOutlined,
} from '@ant-design/icons';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import type { AdminFilter } from '@v2board/api-client';
import type { AdminOrderRow, Plan, PlanPeriod } from '@v2board/types';
import { formatDateMinuteSlash, formatDateTime } from '@v2board/config/format';
import {
  useAdminOrderDetail,
  useAdminOrders,
  useAdminPlans,
  useAdminUserInfo,
  useAssignOrderMutation,
  useCancelOrderMutation,
  useMarkOrderPaidMutation,
  useUpdateOrderMutation,
} from '@/lib/queries';
import { i18nGet } from '@/lib/errors';
import { LegacyFilterDrawer, type LegacyFilterKey } from '@/components/legacy-filter-drawer';

const PERIOD_TEXT: Record<string, string> = {
  month_price: '月付',
  quarter_price: '季付',
  half_year_price: '半年付',
  year_price: '年付',
  two_year_price: '两年付',
  three_year_price: '三年付',
  onetime_price: '一次性',
  reset_price: '流量重置包',
};

const ORDER_TYPE_TEXT: Record<number, string> = {
  1: '新购',
  2: '续费',
  3: '变更',
  4: '流量包',
  9: '充值',
};

const ORDER_STATUS_TEXT: Record<number, string> = {
  0: '待支付',
  1: '开通中',
  2: '已取消',
  3: '已完成',
  4: '已折抵',
};

const COMMISSION_STATUS_TEXT: Record<number, string> = {
  0: '待确认',
  1: '发放中',
  2: '已发放',
  3: '已驳回',
};

const ORDER_STATUS_BADGE = ['error', 'processing', 'default', 'success', 'default'] as const;
const COMMISSION_STATUS_BADGE = ['default', 'processing', 'success', 'error'] as const;

const ORDER_FILTER_KEYS: LegacyFilterKey[] = [
  { key: 'trade_no', title: '订单号', condition: ['模糊', '='] },
  {
    key: 'status',
    title: '订单状态',
    type: 'select',
    condition: ['='],
    options: [
      { key: '未支付', value: 0 },
      { key: '已支付', value: 1 },
      { key: '已取消', value: 2 },
      { key: '已完成', value: 3 },
      { key: '已折抵', value: 4 },
    ],
  },
  {
    key: 'commission_status',
    title: '佣金状态',
    type: 'select',
    condition: ['='],
    options: [
      { key: '待确认', value: 0 },
      { key: '发放中', value: 1 },
      { key: '已发放', value: 2 },
      { key: '无效', value: 3 },
    ],
  },
  { key: 'user_id', title: '用户ID', condition: ['='] },
  { key: 'invite_user_id', title: '邀请人ID', condition: ['=', '!='] },
  { key: 'callback_no', title: '回调单号', condition: ['模糊'] },
  { key: 'commission_balance', title: '佣金金额', condition: ['>', '<', '=', '!=', '>=', '<='] },
];

interface QueryState {
  current: number;
  pageSize: number;
  filter: AdminFilter[];
}

function readStoredOrderFilter(): AdminFilter[] {
  if (typeof window === 'undefined') return [];
  const stored = window.sessionStorage.getItem('v2board-admin-order-filter');
  if (!stored) return [];
  window.sessionStorage.removeItem('v2board-admin-order-filter');
  try {
    return JSON.parse(stored) as AdminFilter[];
  } catch {
    return [];
  }
}

const detailRowStyle = { marginBottom: 0 };

function LegacySpin({ loading, children }: { loading: boolean; children: ReactNode }) {
  return (
    <Spin spinning={loading} indicator={<div className="spinner-grow text-primary" />}>
      {children}
    </Spin>
  );
}

function cents(value?: number | null) {
  return ((value as number) / 100).toFixed(2);
}

function shortTradeNo(value: string) {
  return `${value.substr(0, 3)}...${value.substr(-3)}`;
}

function filterButtonType(active: boolean): ButtonProps['type'] {
  return active ? 'primary' : ('' as ButtonProps['type']);
}

function showError(message: ReturnType<typeof App.useApp>['message'], error: unknown) {
  if (error instanceof Error) message.error(i18nGet(error.message));
}

function OrderDetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Row gutter={[16, 16]} style={detailRowStyle}>
      <Col span={6}>{label}</Col>
      <Col span={18}>{children}</Col>
    </Row>
  );
}

function AssignOrderButton({
  plans,
  onAssigned,
}: {
  plans: Plan[];
  onAssigned: () => void;
}) {
  const { message } = App.useApp();
  const assign = useAssignOrderMutation();
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<{
    email?: string;
    plan_id?: number;
    period?: PlanPeriod;
    total_amount?: string;
  }>({});

  const close = () => {
    setOpen(false);
    setSubmit({});
  };

  return (
    <>
      <Button style={{ marginLeft: 10 }} onClick={() => setOpen(true)}>
        <PlusOutlined /> 添加订单
      </Button>
      <Modal
        title="订单分配"
        open={open}
        okText={assign.isPending ? <LoadingOutlined /> : '确定'}
        cancelText="取消"
        onCancel={close}
        onOk={() => {
          assign
            .mutateAsync(submit)
            .then(() => {
              onAssigned();
              close();
            })
            .catch((error) => showError(message, error));
        }}
      >
        <div className="form-group">
          <label htmlFor="example-text-input-alt">用户邮箱</label>
          <Input
            placeholder="请输入用户邮箱"
            value={submit.email}
            onChange={(event) => setSubmit((state) => ({ ...state, email: event.target.value }))}
          />
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">请选择订阅</label>
          <div>
            <Select
              value={submit.plan_id}
              style={{ width: '100%' }}
              placeholder="请选择订阅"
              onChange={(plan_id) => setSubmit((state) => ({ ...state, plan_id }))}
            >
              {plans.map((plan) => (
                <Select.Option key={Math.random()} value={plan.id}>
                  {plan.name}
                </Select.Option>
              ))}
            </Select>
          </div>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">请选择周期</label>
          <div>
            <Select
              value={submit.period}
              style={{ width: '100%' }}
              placeholder="请选择周期"
              onChange={(period) => setSubmit((state) => ({ ...state, period }))}
            >
              {Object.keys(PERIOD_TEXT).map((period) => (
                <Select.Option key={Math.random()} value={period}>
                  {PERIOD_TEXT[period]}
                </Select.Option>
              ))}
            </Select>
          </div>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">支付金额</label>
          <Input
            placeholder="请输入需要支付的金额"
            addonAfter="¥"
            value={submit.total_amount}
            onChange={(event) =>
              setSubmit((state) => ({ ...state, total_amount: event.target.value }))
            }
          />
        </div>
      </Modal>
    </>
  );
}

function OrderDetailModal({
  id,
  open,
  onClose,
  plans,
  onUserFilter,
}: {
  id?: number;
  open: boolean;
  onClose: () => void;
  plans: Plan[];
  onUserFilter: (key: string, condition: string, value: string | number) => void;
}) {
  const order = useAdminOrderDetail(id);
  const user = useAdminUserInfo(order.data?.user_id);
  const inviteUser = useAdminUserInfo(order.data?.invite_user_id);
  const detail = order.data;
  const planName = plans.find((plan) => plan.id === detail?.plan_id)?.name;
  const loaded = Boolean(detail && user.data?.email && (!detail.invite_user_id || inviteUser.data?.email));

  return (
    <Modal open={open} title="订单信息" onCancel={onClose} footer={null}>
      {loaded && detail ? (
        <div>
          <OrderDetailRow label="邮箱">
            <a
              href="javascript:void(0);"
              onClick={() => user.data && onUserFilter('email', '模糊', user.data.email)}
            >
              {user.data?.email}
            </a>
          </OrderDetailRow>
          <OrderDetailRow label="订单号">{detail.trade_no}</OrderDetailRow>
          <OrderDetailRow label="订单周期">{PERIOD_TEXT[detail.period]}</OrderDetailRow>
          <OrderDetailRow label="订单状态">{ORDER_STATUS_TEXT[detail.status]}</OrderDetailRow>
          <OrderDetailRow label="订阅计划">{planName}</OrderDetailRow>
          <OrderDetailRow label="回调单号">{detail.callback_no || '-'}</OrderDetailRow>
          <Divider />
          <OrderDetailRow label="支付金额">{cents(detail.total_amount)}</OrderDetailRow>
          <OrderDetailRow label="余额支付">{cents(detail.balance_amount)}</OrderDetailRow>
          <OrderDetailRow label="优惠金额">{cents(detail.discount_amount)}</OrderDetailRow>
          <OrderDetailRow label="退回金额">{cents(detail.refund_amount)}</OrderDetailRow>
          <OrderDetailRow label="折抵金额">{cents(detail.surplus_amount)}</OrderDetailRow>
          <Divider />
          <OrderDetailRow label="创建时间">{formatDateTime(detail.created_at)}</OrderDetailRow>
          <OrderDetailRow label="更新时间">{formatDateTime(detail.updated_at)}</OrderDetailRow>
          {detail.invite_user_id && detail.status === 3 ? (
            <div>
              <Divider />
              <OrderDetailRow label="邀请人">
                <Tooltip title="查看TA邀请的人">
                  <a
                    href="javascript:void(0);"
                    onClick={() =>
                      inviteUser.data &&
                      onUserFilter('invite_by_email', '模糊', inviteUser.data.email)
                    }
                  >
                    {inviteUser.data?.email}
                  </a>
                </Tooltip>
              </OrderDetailRow>
              <OrderDetailRow label="佣金金额">{cents(detail.commission_balance)}</OrderDetailRow>
              {detail.actual_commission_balance ? (
                <OrderDetailRow label="实际发放">
                  {cents(detail.actual_commission_balance)}
                </OrderDetailRow>
              ) : null}
              <OrderDetailRow label="佣金状态">
                {COMMISSION_STATUS_TEXT[detail.commission_status]}
              </OrderDetailRow>
            </div>
          ) : null}
        </div>
      ) : (
        <LoadingOutlined style={{ fontSize: 24, color: '#415A94' }} />
      )}
    </Modal>
  );
}

export default function OrdersPage() {
  const { message } = App.useApp();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState<QueryState>(() => {
    const storedFilter = readStoredOrderFilter();
    return {
      current: storedFilter.length > 0 ? 1 : 0,
      pageSize: 10,
      filter: storedFilter,
    };
  });
  const [detailId, setDetailId] = useState<number>();
  const orders = useAdminOrders(query);
  const plans = useAdminPlans();
  const paid = useMarkOrderPaidMutation();
  const cancel = useCancelOrderMutation();
  const updateOrder = useUpdateOrderMutation();

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: ['admin', 'orders'] });
    },
    [queryClient],
  );

  const setFilter = (filter: AdminFilter[]) =>
    setQuery((state) => ({ ...state, current: 1, filter }));

  const userFilter = (key: string, condition: string, value: string | number) => {
    window.sessionStorage.setItem(
      'v2board-admin-user-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    navigate('/user');
  };

  const columns = useMemo<TableProps<AdminOrderRow>['columns']>(
    () => [
      {
        title: '# 订单号',
        dataIndex: 'trade_no',
        key: 'trade_no',
        render: (value: string, row) => (
          <a href="javascript:void(0);" onClick={() => setDetailId(row.id)}>
            {shortTradeNo(value)}
          </a>
        ),
      },
      {
        title: '类型',
        dataIndex: 'type',
        key: 'type',
        render: (value: number) => ORDER_TYPE_TEXT[value],
      },
      {
        title: '订阅计划',
        dataIndex: 'plan_name',
        key: 'plan_name',
      },
      {
        title: '周期',
        dataIndex: 'period',
        key: 'period',
        align: 'center',
        render: (value: string) => <Tag>{PERIOD_TEXT[value]}</Tag>,
      },
      {
        title: '支付金额',
        dataIndex: 'total_amount',
        key: 'total_amount',
        align: 'right',
        render: (value: number) => cents(value),
      },
      {
        title: (
          <span>
            <Tooltip title="标记为[已支付]后将会由系统进行开通后并完成">
              订单状态 <QuestionCircleOutlined />
            </Tooltip>
          </span>
        ),
        dataIndex: 'status',
        key: 'status',
        render: (value: number, row) => (
          <div>
            <Dropdown
              disabled={value !== 0}
              trigger={['click']}
              menu={{
                items: [
                  { key: '1', label: '已支付' },
                  { key: '2', label: '取消' },
                ],
                onClick: ({ key }) => {
                  const request =
                    key === '1'
                      ? paid.mutateAsync(row.trade_no)
                      : cancel.mutateAsync(row.trade_no);
                  request
                    .then(() => {
                      void orders.refetch();
                    })
                    .catch((error) => showError(message, error));
                },
              }}
            >
              <div>
                <Badge status={ORDER_STATUS_BADGE[value]} />
                <span>{ORDER_STATUS_TEXT[value]} </span>
                {value === 0 ? (
                  <a href="javascript:void(0);">
                    标记为 <CaretDownOutlined />
                  </a>
                ) : null}
              </div>
            </Dropdown>
          </div>
        ),
      },
      {
        title: '佣金金额',
        dataIndex: 'commission_balance',
        key: 'commission_balance',
        align: 'right',
        render: (value: number, row) =>
          row.status === 0 || row.status === 2 ? '-' : value ? cents(value) : '-',
      },
      {
        title: (
          <span>
            佣金状态{' '}
            <Tooltip title="标记为[有效]后将会由系统处理后发放到用户并完成">
              <QuestionCircleOutlined />
            </Tooltip>
          </span>
        ),
        dataIndex: 'commission_status',
        key: 'commission_status',
        render: (value: number, row) => {
          if (row.status === 0 || row.status === 2 || !row.commission_balance) return '-';
          if (row.commission_status === 2) {
            return (
              <div>
                <Badge status={COMMISSION_STATUS_BADGE[value]} />
                <span>{COMMISSION_STATUS_TEXT[value]} </span>
              </div>
            );
          }
          return (
            <div>
              <Dropdown
                trigger={['click']}
                menu={{
                  items: [
                    { key: '0', label: '待确认', disabled: value === 0 },
                    { key: '1', label: '有效', disabled: value === 1 },
                    { key: '3', label: '无效', disabled: value === 3 },
                  ],
                  onClick: ({ key }) =>
                    updateOrder
                      .mutateAsync({
                        tradeNo: row.trade_no,
                        key: 'commission_status',
                        value: key,
                      })
                      .then(() => {
                        void orders.refetch();
                      })
                      .catch((error) => showError(message, error)),
                }}
              >
                <div>
                  <Badge status={COMMISSION_STATUS_BADGE[value]} />
                  <span>{COMMISSION_STATUS_TEXT[value]} </span>
                  <a href="javascript:void(0);">
                    标记为 <CaretDownOutlined />
                  </a>
                </div>
              </Dropdown>
            </div>
          );
        },
      },
      {
        title: '创建时间',
        dataIndex: 'created_at',
        key: 'created_at',
        align: 'right',
        render: (value: number) => formatDateMinuteSlash(value),
      },
    ],
    [cancel, message, orders, paid, updateOrder],
  );

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={orders.isFetching}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <LegacyFilterDrawer
                value={query.filter}
                keys={ORDER_FILTER_KEYS}
                onChange={setFilter}
              >
                <Button type={filterButtonType(query.filter.length > 0)}>
                  <FilterOutlined /> 过滤器
                </Button>
              </LegacyFilterDrawer>
              <AssignOrderButton
                plans={plans.data ?? []}
                onAssigned={() => {
                  void orders.refetch();
                }}
              />
            </div>
            <Table<AdminOrderRow>
              tableLayout="auto"
              dataSource={orders.data?.data ?? []}
              pagination={{
                current: query.current || 1,
                pageSize: query.pageSize,
                total: orders.data?.total,
                size: 'small',
              }}
              columns={columns}
              scroll={{ x: 1050 }}
              onChange={(pagination: TablePaginationConfig) =>
                setQuery((state) => ({
                  ...state,
                  current: pagination.current ?? state.current,
                  pageSize: pagination.pageSize ?? state.pageSize,
                }))
              }
            />
          </div>
        </div>
      </LegacySpin>
      <OrderDetailModal
        id={detailId}
        open={detailId != null}
        onClose={() => setDetailId(undefined)}
        plans={plans.data ?? []}
        onUserFilter={userFilter}
      />
    </>
  );
}
