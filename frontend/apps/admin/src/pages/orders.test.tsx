import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import dayjs from 'dayjs';
import { describe, expect, it, vi } from 'vitest';
import OrdersPage from './orders';

const ordersSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'orders.tsx'),
  'utf8',
);
const legacyFilterDrawerSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../components/legacy-filter-drawer.tsx'),
  'utf8',
);
const adminQueriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({
    removeQueries: vi.fn(),
  }),
}));

vi.mock('@/lib/queries', () => ({
  useAdminOrders: () => ({
    isLoading: false,
    isFetching: false,
    data: {
      data: [
        {
          id: 1,
          trade_no: '202601010001',
          callback_no: null,
          plan_id: 1,
          period: 'month_price',
          type: 1,
          total_amount: 1200,
          handling_amount: null,
          discount_amount: 0,
          surplus_amount: 0,
          refund_amount: 0,
          balance_amount: 0,
          surplus_order_ids: null,
          status: 0,
          commission_status: 0,
          commission_balance: 0,
          payment_id: null,
          invite_user_id: null,
          paid_at: null,
          created_at: 1700000000,
          updated_at: 1700000000,
          user_id: 1,
          plan_name: '基础套餐',
        },
        {
          id: 2,
          trade_no: '202601010002',
          callback_no: 'cb-1',
          plan_id: 1,
          period: 'year_price',
          type: 2,
          total_amount: 8800,
          handling_amount: null,
          discount_amount: 0,
          surplus_amount: 0,
          refund_amount: 0,
          balance_amount: 0,
          surplus_order_ids: null,
          status: 3,
          commission_status: 1,
          commission_balance: 1200,
          payment_id: null,
          invite_user_id: 3,
          paid_at: null,
          created_at: 1700086400,
          updated_at: 1700086400,
          user_id: 2,
          plan_name: '年度套餐',
        },
      ],
      total: 2,
    },
  }),
  useAdminPlans: () => ({
    data: [{ id: 1, name: '基础套餐' }],
  }),
  useAdminOrderDetail: () => ({
    data: undefined,
  }),
  useAdminUserInfo: () => ({
    data: undefined,
  }),
  useAssignOrderMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useMarkOrderPaidMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useCancelOrderMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useUpdateOrderMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

describe('OrdersPage legacy order manager', () => {
  it('renders the original order table shell, columns, status controls, and assign action', () => {
    const html = renderToStaticMarkup(<OrdersPage />);

    expect(html).toContain('class="d-flex justify-content-between align-items-center"');
    expect(html).toContain('class="block block-rounded"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('class="ant-btn-group"');
    expect(html).toContain('class="ant-btn"');
    expect(html).toContain('aria-label="图标: filter"');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-table-fixed" style="width:1050px"');
    expect(html).toContain('class="ant-table-align-center" style="text-align:center"');
    expect(html).toContain('class="ant-table-align-right" style="text-align:right"');
    expect(html).toContain(
      'class="ant-table-align-right ant-table-row-cell-last" style="text-align:right"',
    );
    expect(html).toContain('class="ant-pagination ant-table-pagination mini"');
    expect(html).toContain('过滤器');
    expect(html).toContain('添加订单');
    expect(html).toContain('# 订单号');
    expect(html).toContain('类型');
    expect(html).toContain('订阅计划');
    expect(html).toContain('周期');
    expect(html).toContain('支付金额');
    expect(html).toContain('订单状态');
    expect(html).toContain('佣金金额');
    expect(html).toContain('佣金状态');
    expect(html).toContain('创建时间');
    expect(html).toContain('202...001');
    expect(html).toContain('新购');
    expect(html).toContain('月付');
    expect(html).toContain('12.00');
    expect(html).toContain('待支付');
    expect(html).toContain('标记为');
    expect(html).toContain('class="ant-tag"');
    expect(html).toContain('class="ant-badge ant-badge-status ant-badge-not-a-wrapper"');
    expect(html).toContain('续费');
    expect(html).toContain('年付');
    expect(html).toContain('88.00');
    expect(html).toContain('已完成');
    expect(html).toContain('12.00');
    expect(html).toContain('发放中');
    expect(html).toContain(dayjs(1700000000 * 1000).format('YYYY/MM/DD HH:mm'));
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('css-dev-only');
    expect(html).not.toContain('ant-typography');
    expect(html).not.toContain('ant-descriptions');
  });

  it('uses the original drawer-style multi-condition filter with select status values', () => {
    expect(ordersSource).not.toContain('function LegacyFilterButton');
    expect(ordersSource).toContain('<LegacyFilterDrawer');
    expect(ordersSource).toContain('function filterButtonClassName(active: boolean)');
    expect(ordersSource).toContain("return `ant-btn${active ? ' ant-btn-primary' : ''}`;");
    expect(ordersSource).toContain('className={filterButtonClassName(query.filter.length > 0)}');
    expect(ordersSource).toContain('<div className="ant-btn-group">');
    expect(ordersSource).toContain('<LegacyFilterIcon />');
    expect(ordersSource).toContain('<span> 过滤器</span>');
    expect(ordersSource).not.toContain("type={query.filter.length > 0 ? 'primary' : 'default'}");
    expect(ordersSource).toContain("key: 'status'");
    expect(ordersSource).toContain("type: 'select'");
    expect(ordersSource).toContain("{ key: '未支付', value: 0 }");
    expect(ordersSource).toContain("{ key: '已支付', value: 1 }");
    expect(ordersSource).toContain("{ key: '无效', value: 3 }");
    expect(legacyFilterDrawerSource).toContain('className="v2board-filter-drawer"');
    expect(legacyFilterDrawerSource).toContain('添加条件');
    expect(legacyFilterDrawerSource).toContain('欲检索内容');
    expect(legacyFilterDrawerSource).toContain(
      'defaultValue={(filter.value || undefined) as LegacySelectValue | undefined}',
    );
    expect(legacyFilterDrawerSource).not.toContain(
      'value={(filter.value || undefined) as LegacySelectValue | undefined}',
    );
    expect(legacyFilterDrawerSource).toContain('v2board-drawer-action');
    expect(legacyFilterDrawerSource).toContain('检索');
  });

  it('submits the legacy assigned-order modal state without default replacements', () => {
    expect(ordersSource).toContain('interface AssignOrderSubmit');
    expect(ordersSource).toContain('function assignOrderSubmit(): AssignOrderSubmit');
    expect(ordersSource).toContain('email: undefined');
    expect(ordersSource).toContain('plan_id: undefined');
    expect(ordersSource).toContain('period: undefined');
    expect(ordersSource).toContain('total_amount: undefined');
    expect(ordersSource).toContain('useState<AssignOrderSubmit>(() => assignOrderSubmit())');
    expect(ordersSource).toContain('setSubmit(assignOrderSubmit());');
    expect(ordersSource).not.toContain('useState<AssignOrderSubmit>({})');
    expect(ordersSource).not.toContain('setSubmit({});');
    expect(ordersSource).toContain('.mutateAsync(submit)');
    expect(ordersSource).toContain('await onAssigned();\n      close();');
    expect(ordersSource).not.toContain('      onAssigned();\n      close();');
    expect(ordersSource).toContain('return orders.refetch();');
    expect(ordersSource).toContain(
      'onAssigned: () => void | Promise<unknown>;',
    );
    expect(ordersSource).not.toContain('void orders.refetch();\n                }}');
    expect(ordersSource).not.toContain("period: submit.period ?? 'month_price'");
    expect(ordersSource).not.toContain('total_amount: Number(submit.total_amount ?? 0)');

    const assignHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useAssignOrderMutation()'),
      adminQueriesSource.indexOf('export function useReplyTicketMutation()'),
    );
    expect(assignHook).not.toContain('onSuccess');
    expect(assignHook).not.toContain("queryKey: ['admin', 'orders']");
  });

  it('keeps the original assigned-order modal loading text behavior', () => {
    const start = ordersSource.indexOf('function AssignOrderButton({');
    const end = ordersSource.indexOf('function OrderDetailModal(', start);
    const block = ordersSource.slice(start, end);

    expect(block).toContain('<LegacyModal');
    expect(block).toContain('title="订单分配"');
    expect(block).toContain('visible={open}');
    expect(block).toContain("okText={assign.isPending ? <LegacyLoadingIcon /> : '确定'}");
    expect(block).toContain('cancelText="取消"');
    expect(block).not.toContain('<Modal');
    expect(block).not.toContain('open={open}');
    expect(ordersSource).not.toContain('Menu, Modal, Row');
    expect(ordersSource).not.toContain('LoadingOutlined');
    expect(ordersSource).not.toContain('@ant-design/icons');
    expect(ordersSource).not.toContain('okButtonProps={{ loading: assign.isPending }}');
  });

  it('keeps the original assigned-order label targets without generated input ids', () => {
    expect(ordersSource).toContain('<label htmlFor="example-text-input-alt">用户邮箱</label>');
    expect(ordersSource).toContain('<label htmlFor="example-text-input-alt">请选择订阅</label>');
    expect(ordersSource).toContain('<label htmlFor="example-text-input-alt">请选择周期</label>');
    expect(ordersSource).toContain('<label htmlFor="example-text-input-alt">支付金额</label>');
    expect(ordersSource).not.toContain('assign-email');
    expect(ordersSource).not.toContain('assign-plan');
    expect(ordersSource).not.toContain('assign-period');
    expect(ordersSource).not.toContain('assign-amount');
  });

  it('uses the legacy assigned-order input shells', () => {
    expect(ordersSource).toContain(
      "import { LegacyInput, LegacyInputGroup } from '@/components/legacy-input';",
    );
    expect(ordersSource).toContain('<LegacyInput');
    expect(ordersSource).toContain('className="ant-input"');
    expect(ordersSource).toContain('placeholder="请输入用户邮箱"');
    expect(ordersSource).toContain('<LegacyInputGroup');
    expect(ordersSource).toContain('placeholder="请输入需要支付的金额"');
    expect(ordersSource).toContain('addonAfter="¥"');
    expect(ordersSource).not.toContain("Input, Row } from 'antd'");
    expect(ordersSource).not.toContain('<Input');
  });

  it('uses the legacy assigned-order select shell without antd Select options', () => {
    expect(ordersSource).toContain("} from '@/components/legacy-select';");
    expect(ordersSource).toContain('LegacySelect,');
    expect(ordersSource).toContain('type LegacySelectOption,');
    expect(ordersSource).toContain('type LegacySelectValue,');
    expect(ordersSource).toContain(
      'function planSelectOptions(plans: Plan[]): LegacySelectOption[]',
    );
    expect(ordersSource).toContain('options={planSelectOptions(plans)}');
    expect(ordersSource).toContain(
      'const PERIOD_OPTIONS: LegacySelectOption[] = Object.keys(PERIOD_TEXT).map',
    );
    expect(ordersSource).toContain('options={PERIOD_OPTIONS}');
    expect(ordersSource).not.toContain('<Select');
    expect(ordersSource).not.toContain('Select.Option');
    expect(ordersSource).not.toContain('Select, Tooltip } from');
  });

  it('uses the original outer fetchLoading spin wrapper for order refetches', () => {
    expect(ordersSource).toContain("import { LegacySpin } from '@/components/legacy-spin';");
    expect(ordersSource).toContain(
      '<LegacySpin loading={legacyFetchLoading(orders.isFetching, orders.error)}>',
    );
    expect(ordersSource).not.toContain('className="spinner-grow text-primary"');
    expect(ordersSource).not.toContain('              loading={orders.isFetching}');
    expect(ordersSource).not.toContain('loading={orders.isLoading}');
  });

  it('empties the cached order list on page unmount like the original order model', () => {
    expect(ordersSource).toContain("import { useQueryClient } from '@tanstack/react-query';");
    expect(ordersSource).toContain('const queryClient = useQueryClient();');
    expect(ordersSource).toContain("queryClient.removeQueries({ queryKey: ['admin', 'orders'] });");
    expect(ordersSource).not.toContain(
      'queryClient.removeQueries({ queryKey: adminKeys.orders(query) });',
    );
  });

  it('keeps the original direct badge status mapping and order action menu keys', () => {
    expect(ordersSource).toContain("import { LegacyBadge } from '@/components/legacy-badge';");
    expect(ordersSource).toContain('<LegacyBadge status={ORDER_STATUS_BADGE[value]} />');
    expect(ordersSource).toContain('<LegacyBadge status={COMMISSION_STATUS_BADGE[value]} />');
    expect(ordersSource).not.toContain("ORDER_STATUS_BADGE[value] ?? 'default'");
    expect(ordersSource).not.toContain("COMMISSION_STATUS_BADGE[value] ?? 'default'");
    expect(ordersSource).not.toContain("Badge } from 'antd'");
    expect(ordersSource).toContain('LegacyDropdownMenu,');
    expect(ordersSource).toContain('LegacyDropdownMenuItem,');
    expect(ordersSource).not.toContain("import type { DropdownProps } from 'antd';");
    expect(ordersSource).not.toContain('popupRender={() => overlay}');
    expect(ordersSource).not.toContain('<Menu>');
    expect(ordersSource).toContain('trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}');
    expect(ordersSource).toContain('disabled={value !== 0}');
    expect(ordersSource).toContain('overlay={');
    expect(ordersSource).toContain(
      '<LegacyDropdownMenuItem key="1" onClick={() => updateOrderStatus(row.trade_no, \'1\')}>',
    );
    expect(ordersSource).toContain(
      '<LegacyDropdownMenuItem key="2" onClick={() => updateOrderStatus(row.trade_no, \'2\')}>',
    );
    expect(ordersSource).toContain("updateOrderStatus(row.trade_no, '1')");
    expect(ordersSource).not.toContain('menu={{');
    expect(ordersSource).not.toContain("{ key: '1', label: '已支付' }");
    expect(ordersSource).not.toContain("{ key: '2', label: '取消' }");
    expect(ordersSource).not.toContain("key === '1'");
    expect(ordersSource).not.toContain("key === 'paid'");
    expect(ordersSource).not.toContain('<Badge');
    expect(ordersSource).not.toContain('<CaretDownOutlined');
  });

  it('keeps order action refetches after successful mutation requests', () => {
    const paidStart = ordersSource.indexOf("status === '1' ? paid.mutateAsync(tradeNo)");
    const cancelStart = ordersSource.indexOf(': cancel.mutateAsync(tradeNo);');
    const sharedRefetch = ordersSource.indexOf('void orders.refetch();', paidStart);
    const updateStart = ordersSource.indexOf('updateOrder\n      .mutateAsync({');
    const updateRefetch = ordersSource.indexOf('void orders.refetch();', updateStart);

    expect(paidStart).toBeGreaterThan(-1);
    expect(cancelStart).toBeGreaterThan(paidStart);
    expect(sharedRefetch).toBeGreaterThan(cancelStart);
    expect(updateStart).toBeGreaterThan(-1);
    expect(updateRefetch).toBeGreaterThan(updateStart);

    const paidHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useMarkOrderPaidMutation()'),
      adminQueriesSource.indexOf('export function useCancelOrderMutation()'),
    );
    const cancelHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useCancelOrderMutation()'),
      adminQueriesSource.indexOf('export function useUpdateOrderMutation()'),
    );
    const updateHook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useUpdateOrderMutation()'),
      adminQueriesSource.indexOf('export function useAssignOrderMutation()'),
    );
    expect(paidHook).not.toContain('onSuccess');
    expect(cancelHook).not.toContain('onSuccess');
    expect(updateHook).not.toContain('onSuccess');
    expect(paidHook).not.toContain("queryKey: ['admin', 'orders']");
    expect(cancelHook).not.toContain("queryKey: ['admin', 'orders']");
    expect(updateHook).not.toContain("queryKey: ['admin', 'orders']");
  });

  it('keeps the original commission display text separate from menu text', () => {
    expect(ordersSource).toContain("3: '已驳回'");
    expect(ordersSource).toContain('key="3"');
    expect(ordersSource).toContain('disabled={value === 3}');
    expect(ordersSource).toContain("onClick={() => updateCommissionStatus(row.trade_no, '0')}");
    expect(ordersSource).toContain("onClick={() => updateCommissionStatus(row.trade_no, '1')}");
    expect(ordersSource).toContain("onClick={() => updateCommissionStatus(row.trade_no, '3')}");
    expect(ordersSource).toContain('无效');
    expect(ordersSource).not.toContain("3: '无效'");
  });

  it('keeps the original direct amount arithmetic without zero fallback', () => {
    expect(ordersSource).toContain('return ((value as number) / 100).toFixed(2);');
    expect(ordersSource).not.toContain('value ?? 0');
  });

  it('keeps the original short order number substr slicing', () => {
    expect(ordersSource).toContain('return `${value.substr(0, 3)}...${value.substr(-3)}`;');
    expect(ordersSource).not.toContain('value.substring(0, 3)');
    expect(ordersSource).not.toContain('value.substring(value.length - 3)');
  });

  it('keeps the original wrapper click target for opening order details', () => {
    expect(ordersSource).toContain('<div onClick={() => setDetailId(row.id)}>');
    expect(ordersSource).toContain('<a ref={legacyHref()}>{shortTradeNo(row.trade_no)}</a>');
    expect(ordersSource).not.toContain(
      '<a ref={legacyHref()} onClick={() => setDetailId(row.id)}>',
    );
  });

  it('keeps the original top placement for order status tooltips', () => {
    expect(ordersSource).toContain("import { LegacyTooltip } from '@/components/legacy-tooltip';");
    expect(ordersSource).toContain('<LegacyTooltip title="查看TA邀请的人">');
    expect(ordersSource).toContain(
      '<LegacyTooltip placement="top" title="标记为[已支付]后将会由系统进行开通后并完成">',
    );
    expect(ordersSource).toContain(
      '<LegacyTooltip placement="top" title="标记为[有效]后将会由系统处理后发放到用户并完成">',
    );
    expect(ordersSource).not.toContain("Tooltip } from 'antd'");
    expect(ordersSource).not.toContain('<Tooltip');
  });

  it('keeps the original first-fetch pagination and addFilter jump page values', () => {
    expect(ordersSource).toContain('const storedFilter = readStoredOrderFilter();');
    expect(ordersSource).toContain('current: storedFilter.filter.length > 0 ? 1 : 0,');
    expect(ordersSource).toContain('total: storedFilter.total,');
    expect(ordersSource).toContain('filter: storedFilter.filter,');
    expect(ordersSource).toContain('current: query.current || 1,');
    expect(ordersSource).toContain('setQuery((state) => ({ ...state, current: 1, filter }))');
  });

  it('keeps the legacy stored order filter total and array compatibility', () => {
    expect(ordersSource).toContain("function readStoredOrderFilter(): Pick<QueryState, 'filter' | 'total'>");
    expect(ordersSource).toContain("if (Array.isArray(parsed)) return { filter: parsed };");
    expect(ordersSource).toContain("total: typeof parsed.total === 'number' ? parsed.total : undefined,");
  });

  it('keeps pagination updates on the table onChange path only', () => {
    expect(ordersSource).toContain('type LegacyTablePaginationChange');
    expect(ordersSource).toContain(
      'const updateTablePagination = (pagination: LegacyTablePaginationChange) =>',
    );
    expect(ordersSource).toContain('<LegacyTablePagination');
    expect(ordersSource).toContain('onChange={updateTablePagination}');
    expect(ordersSource).toContain('...pagination,');
    expect(ordersSource).not.toContain('current: pagination.current ?? state.current');
    expect(ordersSource).not.toContain('pageSize: pagination.pageSize ?? state.pageSize');
    expect(ordersSource).not.toContain('total: pagination.total');
    expect(ordersSource).not.toContain('onChange={(pagination: TablePaginationConfig) =>');
    expect(ordersSource).not.toContain('onChange: (current: number, pageSize: number) =>');
  });

  it('keeps the bundled order pagination total as the direct response field', () => {
    expect(ordersSource).toContain('total: orders.data?.total,');
    expect(ordersSource).not.toContain('total: orders.data?.total ?? 0');
  });

  it('waits for invite user details before showing invited order detail content', () => {
    expect(ordersSource).toContain(
      'const loaded = Boolean(detail && user.data?.email && (!detail.invite_user_id || inviteUser.data));',
    );
    expect(ordersSource).not.toContain('const loaded = Boolean(detail && user.data?.email);');
    expect(ordersSource).not.toContain('inviteUser.data?.email));');
  });

  it('keeps the original false footer on the order detail modal using the old modal shell', () => {
    const start = ordersSource.indexOf('function OrderDetailModal(');
    const end = ordersSource.indexOf('export default function OrdersPage()', start);
    const block = ordersSource.slice(start, end);

    expect(ordersSource).toContain("import { App } from 'antd';");
    expect(ordersSource).not.toContain("import { App, Col, Divider, Row } from 'antd';");
    expect(ordersSource).toContain("import { LegacyModal } from '@/components/legacy-modal';");
    expect(ordersSource).toContain('function OrderDetailRow');
    expect(ordersSource).toContain('className="ant-row"');
    expect(ordersSource).toContain('className="ant-col ant-col-6"');
    expect(ordersSource).toContain('className="ant-col ant-col-18"');
    expect(ordersSource).toContain("import { LegacyDivider } from '@/components/legacy-divider';");
    expect(ordersSource.match(/<LegacyDivider \/>/g)).toHaveLength(3);
    expect(ordersSource).not.toContain('function OrderDetailDivider()');
    expect(ordersSource).not.toContain('className="ant-divider ant-divider-horizontal"');
    expect(ordersSource).not.toContain('<Row');
    expect(ordersSource).not.toContain('<Col');
    expect(ordersSource).not.toContain('<Divider');
    expect(block).toContain(
      '<LegacyModal visible={open} title="订单信息" onCancel={onClose} footer={false}>',
    );
    expect(block).not.toContain(
      '<Modal open={open} title="订单信息" onCancel={onClose} footer={false}>',
    );
    expect(block).not.toContain('footer={null}');
  });

  it('keeps the original table row identity and detail fallback behavior', () => {
    expect(ordersSource).toContain('<LegacyStandaloneTable');
    expect(ordersSource).toContain('scrollX={1050}');
    expect(ordersSource).toContain('scrollPositionRight={false}');
    expect(ordersSource).toContain('{...legacyTableRowKey(index)}');
    expect(ordersSource).toContain(
      '<td className="ant-table-align-center" style={{ textAlign: \'center\' }}>',
    );
    expect(ordersSource.match(/className="ant-table-align-right"/g)).toHaveLength(2);
    expect(ordersSource).toContain(
      'className="ant-table-align-right ant-table-row-cell-last"',
    );
    expect(ordersSource).toContain("import { LegacyTag } from '@/components/legacy-tag';");
    expect(ordersSource).not.toContain('function LegacyTag');
    expect(ordersSource).not.toContain('<Table<AdminOrderRow>');
    expect(ordersSource).not.toContain('rowKey="id"');
    expect(ordersSource).not.toContain('tableLayout="auto"');
    expect(ordersSource).not.toContain('dataSource={orders.data?.data ?? []}');
    expect(ordersSource).not.toContain('<Tag>');
    expect(ordersSource).toContain(
      'const planName = plans.find((plan) => plan.id === detail?.plan_id)?.name;',
    );
    expect(ordersSource).not.toContain('detail?.plan_name');
    expect(ordersSource).not.toContain('PERIOD_TEXT[detail.period] ?? detail.period');
    expect(ordersSource).not.toContain('PERIOD_TEXT[value] ?? value');
    expect(ordersSource).not.toContain('{planName ??');
  });
});
