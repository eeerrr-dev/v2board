import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import PaymentsPage from './payments';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'payments.tsx'), 'utf8');
const legacyDragSortSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../components/legacy-drag-sort.tsx'),
  'utf8',
);
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

vi.mock('@/lib/queries', () => ({
  useAdminPayments: () => ({
    isLoading: false,
    isFetching: false,
    data: [
      {
        id: 1,
        uuid: 'uuid',
        name: 'Alipay',
        payment: 'AlipayF2F',
        icon: null,
        handling_fee_fixed: 0,
        handling_fee_percent: 0,
        config: {},
        notify_domain: null,
        notify_url: 'https://example.com/api/v1/guest/payment/notify/uuid',
        enable: 1,
        sort: 1,
        created_at: 1,
        updated_at: 1,
      },
    ],
  }),
  useSavePaymentMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useShowPaymentMutation: () => ({
    mutate: vi.fn(),
  }),
  useDropPaymentMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useSortPaymentMutation: () => ({
    mutate: vi.fn(),
  }),
}));

describe('PaymentsPage legacy payment config', () => {
  it('renders the original payment config table shell and actions', () => {
    const html = renderToStaticMarkup(<PaymentsPage />);

    expect(html).toContain('class="d-flex justify-content-between align-items-center"');
    expect(html).toContain('class="block block-rounded"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('<button type="button" class="ant-btn">');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('添加支付方式');
    expect(html).toContain('class="ant-switch-small ant-switch ant-switch-checked"');
    expect(html).toContain('class="ant-switch-inner"');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-spin-nested-loading"');
    expect(html).toContain('class="ant-table ant-table-default ant-table-scroll-position-left"');
    expect(html).toContain('class="ant-table-scroll"');
    expect(html).toContain('tabindex="-1" class="ant-table-body" style="overflow-x:scroll"');
    expect(html).toContain('class="ant-table-fixed" style="width:1300px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain(
      'class="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"',
    );
    expect(html).toContain('data-row-key="0"');
    expect(html).toContain('ID');
    expect(html).toContain('启用');
    expect(html).toContain('显示名称');
    expect(html).toContain('支付接口');
    expect(html).toContain('通知地址');
    expect(html).toContain('操作');
    expect(html).toContain('anticon-question-circle');
    expect(html).toContain('anticon-menu');
    expect(html).toContain('Alipay');
    expect(html).toContain('AlipayF2F');
    expect(html).toContain('https://example.com/api/v1/guest/payment/notify/uuid');
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-typography');
    expect(html).not.toContain('ant-table-cell');
  });

  it('keeps the bundled notification-address tooltip copy', () => {
    expect(source).toContain("import { LegacyTooltip } from '@/components/legacy-tooltip';");
    expect(source).toContain('<LegacyTooltip');
    expect(source).toContain('placement="top"');
    expect(source).toContain(
      'title="支付网关将会把数据通知到本地址，请通过防火墙放行本地址。"',
    );
    expect(source).not.toContain("Tooltip } from 'antd'");
    expect(source).not.toContain('<Tooltip');
    expect(source).not.toContain('支付网关将会把数据通知到本地地址，请通过防火墙放行本地地址。');
  });

  it('uses the legacy falsy fallback for dynamic payment config defaults', () => {
    expect(source).toContain('defaultValue={(config[key] || field.value) as string | undefined}');
    expect(source).not.toContain('config[key] ?? field.value');
  });

  it('dereferences dynamic payment form fields directly like the legacy renderer', () => {
    const dynamicFormBlock = source.slice(
      source.indexOf('{Object.keys(form).map((key) => {'),
      source.indexOf('{selectPaymentMethod ===', source.indexOf('{Object.keys(form).map')),
    );

    expect(dynamicFormBlock).toContain(
      'const field = form[key] as PaymentFormDefinition[string];',
    );
    expect(dynamicFormBlock).toContain('const inputType = field.type;');
    expect(dynamicFormBlock).toContain(
      '<label htmlFor="example-text-input-alt">{field.label}</label>',
    );
    expect(dynamicFormBlock).not.toContain('if (!field) return null;');
  });

  it('does not force-remount dynamic config inputs when switching payment methods', () => {
    expect(source).not.toContain('key={`${selectPaymentMethod}-${key}`}');
    expect(source).not.toContain('<div className="form-group" key={key}>');
  });

  it('uses the legacy fixed handling fee default conversion', () => {
    expect(source).toContain('defaultValue={(submit.handling_fee_fixed as number) / 100}');
    expect(source).toContain('submitOnChange(');
    expect(source).toContain("'handling_fee_fixed',");
    expect(source).toContain('100 * (event.target.value as unknown as number),');
    expect(source).not.toContain('Number(submit.handling_fee_fixed)');
    expect(source).not.toContain('Number(event.target.value)');
    expect(source).not.toContain('submit.handling_fee_fixed == null');
  });

  it('uses the legacy payment editor input shells', () => {
    expect(source).toContain("import { LegacyInput } from '@/components/legacy-input';");
    expect(source).toContain('<LegacyInput');
    expect(source).toContain('className="ant-input"');
    expect(source).toContain('placeholder="用于前端显示使用"');
    expect(source).toContain('placeholder="用于前端显示使用(https://x.com/icon.svg)"');
    expect(source).toContain('placeholder="网关的通知将会发送到该域名(https://x.com)"');
    expect(source).toContain('suffix="%"');
    expect(source).toContain('type="number"');
    expect(source).toContain('placeholder="在订单金额基础上附加手续费"');
    expect(source).toContain('defaultValue={(config[key] || field.value) as string | undefined}');
    expect(source).not.toContain("import { Input } from 'antd';");
    expect(source).not.toContain('<Input');
  });

  it('uses the old Ant Design modal shell for the payment editor', () => {
    const editorBlock = source.slice(
      source.indexOf('function PaymentEditor({'),
      source.indexOf('export default function PaymentsPage()'),
    );

    expect(source).toContain("import { LegacyModal } from '@/components/legacy-modal';");
    expect(editorBlock).toContain('<LegacyModal');
    expect(editorBlock).toContain("title={submit.id ? '编辑支付方式' : '添加支付方式'}");
    expect(editorBlock).toContain('visible={visible}');
    expect(editorBlock).toContain("okText={submit.id ? '保存' : '添加'}");
    expect(editorBlock).toContain('cancelText="取消"');
    expect(editorBlock).not.toContain('<Modal');
    expect(editorBlock).not.toContain('open={visible}');
  });

  it('keeps the original parseInt switch checked value without boolean normalization', () => {
    expect(source).toContain("import { LegacySwitch } from '@/components/legacy-switch';");
    expect(source).toContain('<LegacySwitch');
    expect(source).toContain('checked={parseInt(String(enable), 10) as unknown as boolean}');
    expect(source).not.toContain('Boolean(parseInt(String(enable), 10))');
    expect(source).not.toContain('<Switch');
    expect(source).not.toContain('Switch, Tooltip');
  });

  it('uses the legacy payment method select while preserving fetch-first selection updates', () => {
    expect(source).toContain(
      'const [selectPaymentMethod, setSelectPaymentMethod] = useState<string | undefined>(undefined);',
    );
    expect(source).not.toContain('useState<string | undefined>(\n    record?.payment,\n  )');
    expect(source).toContain('setSelectPaymentMethod(selected);');
    expect(source).toContain(
      "import { LegacySelect, type LegacySelectOption } from '@/components/legacy-select';",
    );
    expect(source).toContain(
      'function paymentMethodOptions(methods: string[]): LegacySelectOption[]',
    );
    expect(source).toContain('<LegacySelect');
    expect(source).toContain('defaultValue={selectPaymentMethod}');
    expect(source).toContain('options={paymentMethodOptions(paymentMethods)}');
    expect(source).not.toContain('value={selectPaymentMethod}');
    expect(source).not.toContain('<Select');
    expect(source).not.toContain('Select.Option');
    expect(source).not.toContain('Modal, Select');
  });

  it('updates the selected payment method only after fetching its form', () => {
    const block = source.slice(
      source.indexOf('const onSelectPaymentMethod = async'),
      source.indexOf('const show = async', source.indexOf('const onSelectPaymentMethod = async')),
    );

    expect(block).toContain(
      'const nextForm = await admin.paymentForm(apiClient, payment, record?.id);',
    );
    expect(block).toContain('setForm(nextForm);');
    expect(block).toContain('setSelectPaymentMethod(payment);');
    expect(block.indexOf('setForm(nextForm);')).toBeLessThan(
      block.indexOf('setSelectPaymentMethod(payment);'),
    );
    expect(block).not.toContain('if (!payment)');
  });

  it('keeps the original add-payment editor component key', () => {
    expect(source).toContain('<PaymentEditor\n                key={0}');
    expect(source).toContain('key={row.id}');
  });

  it('keeps the original page loading state during payment refetches and sorting', () => {
    expect(source).toContain('const [legacySortLoading, setLegacySortLoading] = useState(false);');
    expect(source).toContain('setLegacySortLoading(true);');
    expect(source).toContain('loading={payments.isFetching || legacySortLoading}');
    expect(source).not.toContain('loading={payments.isFetching || sort.isPending}');
    expect(source).not.toContain('loading={payments.isLoading}');
  });

  it('keeps the legacy payment table without an explicit rowKey', () => {
    expect(source).toContain('<LegacyStandaloneTable');
    expect(source).toContain('headers={headers}');
    expect(source).toContain('isEmpty={orderedPayments.length === 0}');
    expect(source).toContain('scrollX={1300}');
    expect(source).toContain('scrollPositionRight={false}');
    expect(source).toContain('fixedRightChildren={orderedPayments.map((row, index) => (');
    expect(source).toContain('{...legacyTableRowKey(index)}');
    expect(source).toContain('<LegacyDragSort');
    expect(source).not.toContain('<Table<AdminPayment>');
    expect(source).not.toContain('tableLayout="auto"');
    expect(source).not.toContain('pagination={false}');
    expect(source).not.toContain('data-row-key');
    expect(source).not.toContain('data-sort-index');
    expect(source).not.toContain('rowKey="id"');
  });

  it('does not wait for the payment refetch before resolving save, matching complete-before-fetch', () => {
    const saveBlock = source.slice(
      source.indexOf('const save = async () => {'),
      source.indexOf('return (', source.indexOf('const save = async () => {')),
    );

    expect(saveBlock).toContain('setVisible(false);\n    onSaved();');
    expect(saveBlock.indexOf('setVisible(false);')).toBeLessThan(saveBlock.indexOf('onSaved();'));
    expect(
      source.match(/onSaved=\{\(\) => \{\n\s+void payments\.refetch\(\);\n\s+\}\}/g),
    ).toHaveLength(2);
  });

  it('keeps the original vertical divider markup in the payment action column', () => {
    expect(source).toContain("import { LegacyDivider } from '@/components/legacy-divider';");
    expect(source).toContain('<LegacyDivider type="vertical" />');
    expect(source).not.toContain(
      '<div className="ant-divider ant-divider-vertical" role="separator" />',
    );
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
  });

  it('keeps the legacy delete confirm from returning a modal-loading promise', () => {
    expect(source).toContain("import { legacyConfirm } from '@/components/legacy-confirm';");
    expect(source).toContain("import { LegacyInput } from '@/components/legacy-input';");
    expect(source).not.toContain("import { Input } from 'antd';");
    expect(source).not.toContain("import { Input, Modal, Tooltip } from 'antd';");
    expect(source).not.toContain("import { Input, Tooltip } from 'antd';");
    expect(source).not.toContain('Modal.confirm({');
    expect(source).toContain('void legacyConfirm({');
    const confirmStart = source.indexOf('onOk: () => {');
    const dropStart = source.indexOf('void drop.mutateAsync(row.id).then(() => {', confirmStart);

    expect(confirmStart).toBeGreaterThan(-1);
    expect(dropStart).toBeGreaterThan(confirmStart);
    expect(source).toContain('void payments.refetch();');
    expect(source).not.toContain('onOk: () => drop.mutateAsync(row.id)');
  });

  it('uses the original drag-sort wrapper instead of business row keys', () => {
    expect(source).toContain(
      "import { LegacyDragSort, LegacyMenuIcon } from '@/components/legacy-drag-sort';",
    );
    expect(source).toContain('<LegacyDragSort');
    expect(source).toContain('<LegacyMenuIcon />');
    expect(source).not.toContain('function LegacyPaymentDragSort({');
    expect(legacyDragSortSource).toContain('export function LegacyDragSort({');
    expect(legacyDragSortSource).toContain('LEGACY_DRAG_LINE_STYLE');
    expect(legacyDragSortSource).toContain(
      '<div role="presentation" onMouseDown={onMouseDown} ref={dragList}>',
    );
    expect(source).toContain('nodeSelector="tr"');
    expect(source).toContain('handleSelector="i"');
    expect(legacyDragSortSource).toContain("handle.setAttribute('draggable', 'false');");
    expect(legacyDragSortSource).toContain("dragNode.setAttribute('draggable', 'true');");
    expect(legacyDragSortSource).toContain(
      '<i aria-label="图标: menu" {...props} className="anticon anticon-menu">',
    );
    expect(source).not.toContain('<MenuOutlined');
    expect(source).not.toContain('dragIndex.current');
    expect(source).not.toContain('onDrop={onDrop}');
    expect(source).toContain('if (fromIndex < toIndex) {');
    expect(source).toContain('next.splice(toIndex + 1, 0, moved);');
    expect(source).toContain('next.splice(fromIndex, 1);');
    expect(source).toContain('next.splice(toIndex, 0, moved);');
    expect(source).toContain('next.splice(fromIndex + 1, 1);');
    expect(source).toContain('sort.mutate(');
    expect(source).toContain('next.map((payment) => payment.id),');
    expect(source).toContain(
      'void payments.refetch().finally(() => {\n            setLegacySortLoading(false);\n          });',
    );
  });

  it('keeps payment mutations fetching from the page after successful requests', () => {
    const showStart = source.indexOf('show.mutate(row.id, {');
    const showRefetch = source.indexOf('void payments.refetch();', showStart);
    const sortStart = source.indexOf('sort.mutate(');
    const sortRefetch = source.indexOf('void payments.refetch().finally', sortStart);
    const dropStart = source.indexOf('drop.mutateAsync(row.id).then');
    const dropRefetch = source.indexOf('void payments.refetch();', dropStart);

    expect(showStart).toBeGreaterThan(-1);
    expect(showRefetch).toBeGreaterThan(showStart);
    expect(sortStart).toBeGreaterThan(-1);
    expect(sortRefetch).toBeGreaterThan(sortStart);
    expect(dropStart).toBeGreaterThan(-1);
    expect(dropRefetch).toBeGreaterThan(dropStart);

    for (const [start, end] of [
      ['export function useSavePaymentMutation()', 'export function useShowPaymentMutation()'],
      ['export function useShowPaymentMutation()', 'export function useSortPaymentMutation()'],
      ['export function useSortPaymentMutation()', 'export function useDropPaymentMutation()'],
      ['export function useDropPaymentMutation()', 'export function useGenerateCouponMutation()'],
    ] as const) {
      const hook = queriesSource.slice(queriesSource.indexOf(start), queriesSource.indexOf(end));
      expect(hook).not.toContain('onSuccess');
      expect(hook).not.toContain('adminKeys.payments');
    }
  });
});
