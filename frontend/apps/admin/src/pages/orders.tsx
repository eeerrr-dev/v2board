import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router';
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
import { LegacyFilterDrawer, type LegacyFilterKey } from '@/components/legacy-filter-drawer';
import { LegacySpin } from '@/components/legacy-spin';
import { legacyHref } from '@/lib/legacy-href';
import { legacyFetchLoading } from '@/lib/legacy-fetch-loading';
import { LegacyButton } from '@/components/legacy-button';
import {
  LegacyCaretDownIcon,
  LegacyFilterIcon,
  LegacyLoadingIcon,
  LegacyPlusIcon,
  LegacyQuestionCircleIcon,
} from '@/components/legacy-ant-icon';
import {
  LegacyStandaloneTable,
  legacyTableRowKey,
  LegacyTablePagination,
  type LegacyStandaloneTableHeader,
  type LegacyTablePaginationChange,
} from '@/components/legacy-standalone-table';
import { LegacyModal } from '@/components/legacy-modal';
import {
  LegacySelect,
  type LegacySelectOption,
  type LegacySelectValue,
} from '@/components/legacy-select';
import {
  LegacyDropdown,
  LegacyDropdownMenu,
  LegacyDropdownMenuItem,
  LEGACY_DROPDOWN_CLICK_TRIGGER,
} from '@/components/legacy-dropdown';
import { LegacyTooltip } from '@/components/legacy-tooltip';
import { LegacyInput, LegacyInputGroup } from '@/components/legacy-input';
import { LegacyBadge } from '@/components/legacy-badge';
import { LegacyTag } from '@/components/legacy-tag';
import { LegacyDivider } from '@/components/legacy-divider';

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

const PERIOD_OPTIONS: LegacySelectOption[] = Object.keys(PERIOD_TEXT).map((period) => ({
  value: period,
  label: PERIOD_TEXT[period] ?? period,
}));

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
  total?: number;
  filter: AdminFilter[];
}

function readStoredOrderFilter(): Pick<QueryState, 'filter' | 'total'> {
  if (typeof window === 'undefined') return { filter: [] };
  const stored = window.sessionStorage.getItem('v2board-admin-order-filter');
  if (!stored) return { filter: [] };
  window.sessionStorage.removeItem('v2board-admin-order-filter');
  try {
    const parsed = JSON.parse(stored) as AdminFilter[] | Pick<QueryState, 'filter' | 'total'>;
    if (Array.isArray(parsed)) return { filter: parsed };
    return {
      filter: Array.isArray(parsed.filter) ? parsed.filter : [],
      total: typeof parsed.total === 'number' ? parsed.total : undefined,
    };
  } catch {
    return { filter: [] };
  }
}

const detailRowStyle = { marginLeft: -8, marginRight: -8, marginBottom: 0 };
const detailColStyle = { paddingLeft: 8, paddingRight: 8 };

function cents(value?: number | null) {
  return ((value as number) / 100).toFixed(2);
}

function shortTradeNo(value: string) {
  // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
  return `${value.substr(0, 3)}...${value.substr(-3)}`;
}

function filterButtonClassName(active: boolean) {
  return `ant-btn${active ? ' ant-btn-primary' : ''}`;
}

function legacyOrderTableRows<T>(rows: T[], current: number, pageSize: number) {
  if (rows.length <= pageSize) return rows;
  const page = Math.max(current || 1, 1);
  return rows.slice((page - 1) * pageSize, page * pageSize);
}

interface AssignOrderSubmit {
  email?: string;
  plan_id?: number;
  period?: PlanPeriod;
  total_amount?: string;
}

function assignOrderSubmit(): AssignOrderSubmit {
  return {
    email: undefined,
    plan_id: undefined,
    period: undefined,
    total_amount: undefined,
  };
}

function planSelectOptions(plans: Plan[]): LegacySelectOption[] {
  return plans.map((plan) => ({ value: plan.id, label: plan.name }));
}

function OrderDetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="ant-row" style={detailRowStyle}>
      <div className="ant-col ant-col-6" style={detailColStyle}>
        {label}
      </div>
      <div className="ant-col ant-col-18" style={detailColStyle}>
        {children}
      </div>
    </div>
  );
}

function AssignOrderButton({
  plans,
  onAssigned,
}: {
  plans: Plan[];
  onAssigned: () => void | Promise<unknown>;
}) {
  const assign = useAssignOrderMutation();
  const [open, setOpen] = useState(false);
  const [submit, setSubmit] = useState<AssignOrderSubmit>(() => assignOrderSubmit());

  const close = () => {
    setOpen(false);
    setSubmit(assignOrderSubmit());
  };

  const assignOrder = async () => {
    try {
      await assign.mutateAsync(submit);
      await onAssigned();
      close();
    } catch {
      // Errors are surfaced by the global onError handler (legacy parity); keep the dialog open.
    }
  };

  return (
    <>
      <LegacyButton className="ant-btn" style={{ marginLeft: 10 }} onClick={() => setOpen(true)}>
        <LegacyPlusIcon />
        <span> 添加订单</span>
      </LegacyButton>
      <LegacyModal
        title="订单分配"
        visible={open}
        okText={assign.isPending ? <LegacyLoadingIcon /> : '确定'}
        cancelText="取消"
        onCancel={close}
        onOk={() => {
          void assignOrder();
        }}
      >
        <div className="form-group">
          <label htmlFor="example-text-input-alt">用户邮箱</label>
          <LegacyInput
            className="ant-input"
            placeholder="请输入用户邮箱"
            value={submit.email}
            onChange={(event) => setSubmit((state) => ({ ...state, email: event.target.value }))}
          />
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">请选择订阅</label>
          <div>
            <LegacySelect
              value={submit.plan_id as LegacySelectValue | undefined}
              style={{ width: '100%' }}
              placeholder="请选择订阅"
              options={planSelectOptions(plans)}
              onChange={(plan_id) =>
                setSubmit((state) => ({
                  ...state,
                  plan_id: plan_id as AssignOrderSubmit['plan_id'],
                }))
              }
            />
          </div>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">请选择周期</label>
          <div>
            <LegacySelect
              value={submit.period as LegacySelectValue | undefined}
              style={{ width: '100%' }}
              placeholder="请选择周期"
              options={PERIOD_OPTIONS}
              onChange={(period) =>
                setSubmit((state) => ({ ...state, period: period as AssignOrderSubmit['period'] }))
              }
            />
          </div>
        </div>
        <div className="form-group">
          <label htmlFor="example-text-input-alt">支付金额</label>
          <LegacyInputGroup
            placeholder="请输入需要支付的金额"
            addonAfter="¥"
            value={submit.total_amount}
            onChange={(event) =>
              setSubmit((state) => ({ ...state, total_amount: event.target.value }))
            }
          />
        </div>
      </LegacyModal>
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
  const loaded = Boolean(detail && user.data?.email && (!detail.invite_user_id || inviteUser.data));

  return (
    <LegacyModal visible={open} title="订单信息" onCancel={onClose} footer={false}>
      {loaded && detail ? (
        <div>
          <OrderDetailRow label="邮箱">
            <a
              ref={legacyHref()}
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
          <LegacyDivider />
          <OrderDetailRow label="支付金额">{cents(detail.total_amount)}</OrderDetailRow>
          <OrderDetailRow label="余额支付">{cents(detail.balance_amount)}</OrderDetailRow>
          <OrderDetailRow label="优惠金额">{cents(detail.discount_amount)}</OrderDetailRow>
          <OrderDetailRow label="退回金额">{cents(detail.refund_amount)}</OrderDetailRow>
          <OrderDetailRow label="折抵金额">{cents(detail.surplus_amount)}</OrderDetailRow>
          <LegacyDivider />
          <OrderDetailRow label="创建时间">{formatDateTime(detail.created_at)}</OrderDetailRow>
          <OrderDetailRow label="更新时间">{formatDateTime(detail.updated_at)}</OrderDetailRow>
          {detail.invite_user_id && detail.status === 3 ? (
            <div>
              <LegacyDivider />
              <OrderDetailRow label="邀请人">
                <LegacyTooltip title="查看TA邀请的人">
                  <a
                    ref={legacyHref()}
                    onClick={() =>
                      inviteUser.data &&
                      onUserFilter('invite_by_email', '模糊', inviteUser.data.email)
                    }
                  >
                    {inviteUser.data?.email}
                  </a>
                </LegacyTooltip>
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
        <LegacyLoadingIcon style={{ fontSize: 24, color: '#415A94' }} />
      )}
    </LegacyModal>
  );
}

export default function OrdersPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState<QueryState>(() => {
    const storedFilter = readStoredOrderFilter();
    return {
      current: storedFilter.filter.length > 0 ? 1 : 0,
      pageSize: 10,
      total: storedFilter.total,
      filter: storedFilter.filter,
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

  const updateOrderStatus = (tradeNo: string, status: '1' | '2') => {
    const request = status === '1' ? paid.mutateAsync(tradeNo) : cancel.mutateAsync(tradeNo);
    request
      .then(() => {
        void orders.refetch();
      })
      .catch(() => undefined);
  };

  const updateCommissionStatus = (tradeNo: string, value: string) => {
    updateOrder
      .mutateAsync({
        tradeNo,
        key: 'commission_status',
        value,
      })
      .then(() => {
        void orders.refetch();
      })
      .catch(() => undefined);
  };

  const userFilter = (key: string, condition: string, value: string | number) => {
    window.sessionStorage.setItem(
      'v2board-admin-user-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    navigate('/user');
  };

  const headers: LegacyStandaloneTableHeader[] = [
    { title: '# 订单号' },
    { title: '类型' },
    { title: '订阅计划' },
    { title: '周期', alignCenter: true },
    { title: '支付金额', alignRight: true },
    {
      title: (
        <span>
          <LegacyTooltip placement="top" title="标记为[已支付]后将会由系统进行开通后并完成">
            <span>
              订单状态 <LegacyQuestionCircleIcon />
            </span>
          </LegacyTooltip>
        </span>
      ),
    },
    { title: '佣金金额', alignRight: true },
    {
      title: (
        <span>
          佣金状态{' '}
          <LegacyTooltip placement="top" title="标记为[有效]后将会由系统处理后发放到用户并完成">
            <LegacyQuestionCircleIcon />
          </LegacyTooltip>
        </span>
      ),
    },
    { title: '创建时间', alignRight: true },
  ];

  const renderOrderStatus = (row: AdminOrderRow) => {
    const value = row.status;
    return (
      <div>
        <LegacyDropdown
          disabled={value !== 0}
          trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
          overlay={
            <LegacyDropdownMenu>
              <LegacyDropdownMenuItem key="1" onClick={() => updateOrderStatus(row.trade_no, '1')}>
                已支付
              </LegacyDropdownMenuItem>
              <LegacyDropdownMenuItem key="2" onClick={() => updateOrderStatus(row.trade_no, '2')}>
                取消
              </LegacyDropdownMenuItem>
            </LegacyDropdownMenu>
          }
        >
          <div>
            <LegacyBadge status={ORDER_STATUS_BADGE[value]} />
            <span>{ORDER_STATUS_TEXT[value]} </span>
            {value === 0 ? (
              <a ref={legacyHref()}>
                标记为 <LegacyCaretDownIcon />
              </a>
            ) : null}
          </div>
        </LegacyDropdown>
      </div>
    );
  };

  const renderCommissionStatus = (row: AdminOrderRow) => {
    const value = row.commission_status;
    if (row.status === 0 || row.status === 2 || !row.commission_balance) return '-';
    if (row.commission_status === 2) {
      return (
        <div>
          <LegacyBadge status={COMMISSION_STATUS_BADGE[value]} />
          <span>{COMMISSION_STATUS_TEXT[value]} </span>
        </div>
      );
    }
    return (
      <div>
        <LegacyDropdown
          trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}
          overlay={
            <LegacyDropdownMenu>
              <LegacyDropdownMenuItem
                key="0"
                disabled={value === 0}
                onClick={() => updateCommissionStatus(row.trade_no, '0')}
              >
                待确认
              </LegacyDropdownMenuItem>
              <LegacyDropdownMenuItem
                key="1"
                disabled={value === 1}
                onClick={() => updateCommissionStatus(row.trade_no, '1')}
              >
                有效
              </LegacyDropdownMenuItem>
              <LegacyDropdownMenuItem
                key="3"
                disabled={value === 3}
                onClick={() => updateCommissionStatus(row.trade_no, '3')}
              >
                无效
              </LegacyDropdownMenuItem>
            </LegacyDropdownMenu>
          }
        >
          <div>
            <LegacyBadge status={COMMISSION_STATUS_BADGE[value]} />
            <span>{COMMISSION_STATUS_TEXT[value]} </span>
            <a ref={legacyHref()}>
              标记为 <LegacyCaretDownIcon />
            </a>
          </div>
        </LegacyDropdown>
      </div>
    );
  };

  const tableData = orders.data?.data ?? [];
  const tablePagination = {
    current: query.current || 1,
    pageSize: query.pageSize,
    total: orders.data?.total,
  };
  const updateTablePagination = (pagination: LegacyTablePaginationChange) =>
    setQuery((state) => ({
      ...state,
      ...pagination,
    }));
  const visibleRows = legacyOrderTableRows(
    tableData,
    tablePagination.current,
    tablePagination.pageSize,
  );

  return (
    <>
      <div className="d-flex justify-content-between align-items-center" />
      <LegacySpin loading={legacyFetchLoading(orders.isFetching, orders.error)}>
        <div className="block block-rounded">
          <div className="bg-white">
            <div style={{ padding: 15 }}>
              <div className="ant-btn-group">
                <LegacyFilterDrawer
                  value={query.filter}
                  keys={ORDER_FILTER_KEYS}
                  onChange={setFilter}
                >
                  <LegacyButton className={filterButtonClassName(query.filter.length > 0)}>
                    <LegacyFilterIcon />
                    <span> 过滤器</span>
                  </LegacyButton>
                </LegacyFilterDrawer>
              </div>
              <AssignOrderButton
                plans={plans.data ?? []}
                onAssigned={() => {
                  return orders.refetch();
                }}
              />
            </div>
            <LegacyStandaloneTable
              headers={headers}
              isEmpty={tableData.length === 0}
              scrollX={1050}
              scrollPositionRight={false}
              pagination={
                <LegacyTablePagination
                  current={tablePagination.current}
                  pageSize={tablePagination.pageSize}
                  total={tablePagination.total}
                  onChange={updateTablePagination}
                />
              }
            >
              {visibleRows.map((row, index) => (
                <tr
                  key={index}
                  className="ant-table-row ant-table-row-level-0"
                  {...legacyTableRowKey(index)}
                >
                  <td className="">
                    <div>
                      <div onClick={() => setDetailId(row.id)}>
                        <a ref={legacyHref()}>{shortTradeNo(row.trade_no)}</a>
                      </div>
                    </div>
                  </td>
                  <td className="">{ORDER_TYPE_TEXT[row.type]}</td>
                  <td className="">{row.plan_name}</td>
                  <td className="ant-table-align-center" style={{ textAlign: 'center' }}>
                    <LegacyTag>{PERIOD_TEXT[row.period]}</LegacyTag>
                  </td>
                  <td className="ant-table-align-right" style={{ textAlign: 'right' }}>
                    {cents(row.total_amount)}
                  </td>
                  <td className="">{renderOrderStatus(row)}</td>
                  <td className="ant-table-align-right" style={{ textAlign: 'right' }}>
                    {row.status === 0 || row.status === 2
                      ? '-'
                      : row.commission_balance
                        ? cents(row.commission_balance)
                        : '-'}
                  </td>
                  <td className="">{renderCommissionStatus(row)}</td>
                  <td
                    className="ant-table-align-right ant-table-row-cell-last"
                    style={{ textAlign: 'right' }}
                  >
                    {formatDateMinuteSlash(row.created_at)}
                  </td>
                </tr>
              ))}
            </LegacyStandaloneTable>
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
