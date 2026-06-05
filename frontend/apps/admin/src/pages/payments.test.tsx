import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import PaymentsPage from './payments';

const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'payments.tsx'), 'utf8');
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
    expect(html).toContain('添加支付方式');
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
  });

  it('keeps the bundled notification-address tooltip copy', () => {
    expect(source).toContain('title="支付网关将会把数据通知到本地地址，请通过防火墙放行本地地址。"');
    expect(source).not.toContain('支付网关将会把数据通知到本地址，请通过防火墙放行本地址。');
  });

  it('uses the legacy falsy fallback for dynamic payment config defaults', () => {
    expect(source).toContain('defaultValue={(config[key] || field.value) as string | undefined}');
    expect(source).not.toContain('config[key] ?? field.value');
  });

  it('does not force-remount dynamic config inputs when switching payment methods', () => {
    expect(source).not.toContain('key={`${selectPaymentMethod}-${key}`}');
    expect(source).not.toContain('<div className="form-group" key={key}>');
  });

  it('uses the legacy fixed handling fee default conversion', () => {
    expect(source).toContain('defaultValue={(submit.handling_fee_fixed as number) / 100}');
    expect(source).toContain(
      "submitOnChange('handling_fee_fixed', 100 * (event.target.value as unknown as number))",
    );
    expect(source).not.toContain('Number(submit.handling_fee_fixed)');
    expect(source).not.toContain('Number(event.target.value)');
    expect(source).not.toContain('submit.handling_fee_fixed == null');
  });

  it('keeps the original parseInt switch checked value without boolean normalization', () => {
    expect(source).toContain('checked={parseInt(String(enable), 10) as unknown as boolean}');
    expect(source).not.toContain('Boolean(parseInt(String(enable), 10))');
  });

  it('keeps the original uncontrolled payment method select', () => {
    expect(source).toContain(
      'const [selectPaymentMethod, setSelectPaymentMethod] = useState<string | undefined>(undefined);',
    );
    expect(source).not.toContain('useState<string | undefined>(\n    record?.payment,\n  )');
    expect(source).toContain('setSelectPaymentMethod(selected);');
    expect(source).toContain('defaultValue={selectPaymentMethod}');
    expect(source).toContain('<Select.Option value={method}>');
    expect(source).not.toContain('<Select.Option key={method} value={method}>');
    expect(source).not.toContain('value={selectPaymentMethod}');
  });

  it('updates the selected payment method only after fetching its form', () => {
    const block = source.slice(
      source.indexOf('const onSelectPaymentMethod = async'),
      source.indexOf('const show = async', source.indexOf('const onSelectPaymentMethod = async')),
    );

    expect(block).toContain('const nextForm = await admin.paymentForm(apiClient, payment, record?.id);');
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
    expect(source).toContain('tableLayout="auto"');
    expect(source).toContain('pagination={false}');
    expect(source).toContain('<LegacyPaymentDragSort');
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
    expect(source).toContain('onSaved={() => {\n              void payments.refetch();\n            }}');
    expect(source).toContain('onSaved={() => {\n                  void payments.refetch();\n                }}');
  });

  it('keeps the original vertical divider markup in the payment action column', () => {
    expect(source).toContain('<div className="ant-divider ant-divider-vertical" />');
    expect(source).not.toContain('<span className="ant-divider ant-divider-vertical"');
    expect(source).not.toContain('role="separator"');
  });

  it('keeps the legacy delete confirm from returning a modal-loading promise', () => {
    expect(source).toContain('onOk: () => {\n                  void drop.mutateAsync(row.id).then(() => {');
    expect(source).toContain('void payments.refetch();');
    expect(source).not.toContain('onOk: () => drop.mutateAsync(row.id)');
  });

  it('uses the original drag-sort wrapper instead of business row keys', () => {
    expect(source).toContain('function LegacyPaymentDragSort({');
    expect(source).toContain('LEGACY_DRAG_LINE_STYLE');
    expect(source).toContain('<div role="presentation" onMouseDown={onMouseDown} ref={dragList}>');
    expect(source).toContain('nodeSelector="tr"');
    expect(source).toContain('handleSelector="i"');
    expect(source).toContain('handle.setAttribute(\'draggable\', \'false\');');
    expect(source).toContain('dragNode.setAttribute(\'draggable\', \'true\');');
    expect(source).toContain('<i aria-label="icon: menu" className="anticon anticon-menu">');
    expect(source).not.toContain('<MenuOutlined');
    expect(source).not.toContain('dragIndex.current');
    expect(source).not.toContain('onDrop={onDrop}');
    expect(source).toContain('if (fromIndex < toIndex) {');
    expect(source).toContain('next.splice(toIndex + 1, 0, moved);');
    expect(source).toContain('next.splice(fromIndex, 1);');
    expect(source).toContain('next.splice(toIndex, 0, moved);');
    expect(source).toContain('next.splice(fromIndex + 1, 1);');
    expect(source).toContain('sort.mutate(next.map((payment) => payment.id),');
    expect(source).toContain(
      'onSuccess: () => {\n        void payments.refetch().finally(() => {\n          setLegacySortLoading(false);\n        });\n      },',
    );
  });

  it('keeps payment mutations fetching from the page after successful requests', () => {
    const showStart = source.indexOf('show.mutate(row.id, {');
    const showRefetch = source.indexOf('void payments.refetch();', showStart);
    const sortStart = source.indexOf('sort.mutate(next.map((payment) => payment.id),');
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
