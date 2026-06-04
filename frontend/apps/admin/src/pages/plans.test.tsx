import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import PlansPage from './plans';

const plansSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'plans.tsx'), 'utf8');
const adminQueriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

vi.mock('@/lib/queries', () => ({
  useAdminPlans: () => ({
    isLoading: false,
    isFetching: false,
    data: [
      {
        id: 1,
        sort: 1,
        show: 1,
        renew: 0,
        name: '基础套餐',
        count: 3,
        transfer_enable: 100,
        device_limit: null,
        group_id: 2,
        month_price: 12.34,
        quarter_price: null,
        half_year_price: 56.78,
        year_price: 100,
        two_year_price: null,
        three_year_price: null,
        onetime_price: 300,
        reset_price: null,
        content: '<p>features</p>',
        speed_limit: null,
        capacity_limit: null,
        reset_traffic_method: null,
        created_at: 1,
        updated_at: 1,
      },
    ],
  }),
  useServerGroups: () => ({
    data: [{ id: 2, name: '默认权限组' }],
  }),
  useConfig: () => ({
    data: { site: { currency_symbol: '¥' }, currency_symbol: '¥' },
  }),
  useSavePlanMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
  useDropPlanMutation: () => ({
    mutate: vi.fn(),
  }),
  useUpdatePlanMutation: () => ({
    mutate: vi.fn(),
  }),
  useSortPlansMutation: () => ({
    mutate: vi.fn(),
    isPending: false,
  }),
}));

describe('PlansPage legacy subscription management', () => {
  it('renders the original plan table shell, columns, actions, and formatted prices', () => {
    const html = renderToStaticMarkup(<PlansPage />);

    expect(html).toContain('d-flex justify-content-between align-items-center');
    expect(html).toContain('block block-rounded');
    expect(html).toContain('bg-white');
    expect(html).toContain('添加订阅');
    expect(html).toContain('排序');
    expect(html).toContain('销售状态');
    expect(html).toContain('续费');
    expect(html).toContain('名称');
    expect(html).toContain('统计');
    expect(html).toContain('流量');
    expect(html).toContain('设备数限制');
    expect(html).toContain('月付');
    expect(html).toContain('季付');
    expect(html).toContain('半年付');
    expect(html).toContain('年付');
    expect(html).toContain('两年付');
    expect(html).toContain('三年付');
    expect(html).toContain('一次性');
    expect(html).toContain('重置包');
    expect(html).toContain('权限组');
    expect(html).toContain('操作');
    expect(html).toContain('基础套餐');
    expect(html).toContain('100 GB');
    expect(html).toContain('12.34');
    expect(html).toContain('56.78');
    expect(html).toContain('100.00');
    expect(html).toContain('300.00');
    expect(html).toContain('默认权限组');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-typography');
  });

  it('preserves the original row right-click edit/delete menu', () => {
    expect(plansSource).toContain('id="v2board-table-dropdown"');
    expect(plansSource).toContain('ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical');
    expect(plansSource).toContain('onContextMenu: (event) =>');
    expect(plansSource).toContain('event.preventDefault()');
    expect(plansSource).toContain('event.clientY');
    expect(plansSource).toContain('event.clientX');
    expect(plansSource).toContain("display: contextMenu ? 'unset' : 'none'");
    expect(plansSource).not.toContain('d-none');
  });

  it('keeps the legacy plan table without an explicit rowKey', () => {
    expect(plansSource).toContain('tableLayout="auto"');
    expect(plansSource).toContain('pagination={false}');
    expect(plansSource).toContain('data-sort-index');
    expect(plansSource).not.toContain('data-row-key');
    expect(plansSource).not.toContain('rowKey="id"');
  });

  it('keeps the original plan sort loading and force-update payload shape', () => {
    expect(plansSource).toContain('const [legacySortLoading, setLegacySortLoading] = useState(false);');
    expect(plansSource).toContain('setLegacySortLoading(true);');
    expect(plansSource).toContain('loading={plans.isFetching || legacySortLoading}');
    expect(plansSource).not.toContain('loading={plans.isFetching || sort.isPending}');
    expect(plansSource).not.toContain('plans.isLoading');
    expect(plansSource).not.toContain('loading={Boolean(');
    expect(plansSource).toContain('const to = Number(props[\'data-sort-index\']);');
    expect(plansSource).not.toContain("next.force_update = next.force_update ? 1 : 0");
    expect(plansSource).toContain('force_update?: boolean');
    expect(plansSource).toContain("onChange={(event) => change('force_update', event.target.checked)}");
    expect(plansSource).not.toContain('checked={Boolean(submit.force_update)}');
  });

  it('submits the original drawer state instead of rewriting prices in the page component', () => {
    expect(plansSource).toContain('await onSave({ ...submit });');
    expect(plansSource).toContain('await save.mutateAsync(payload);\n    await plans.refetch();');
    expect(plansSource).not.toContain('await save.mutateAsync(payload);\n    void plans.refetch();');
    expect(plansSource).not.toContain('serializePlan(');
    expect(plansSource).not.toContain('Math.round(100 * Number(next[key]))');
  });

  it('keeps plan mutations fetching from the page after successful requests', () => {
    const sortStart = plansSource.indexOf('sort.mutate(next.map((plan) => plan.id),');
    const sortRefetch = plansSource.indexOf('void plans.refetch().finally', sortStart);
    const dropStart = plansSource.indexOf('drop.mutate(id, {');
    const dropRefetch = plansSource.indexOf('void plans.refetch();', dropStart);
    const updateStart = plansSource.indexOf('update.mutate(');
    const updateRefetch = plansSource.indexOf('void plans.refetch();', updateStart);

    expect(sortStart).toBeGreaterThan(-1);
    expect(sortRefetch).toBeGreaterThan(sortStart);
    expect(dropStart).toBeGreaterThan(-1);
    expect(dropRefetch).toBeGreaterThan(dropStart);
    expect(updateStart).toBeGreaterThan(-1);
    expect(updateRefetch).toBeGreaterThan(updateStart);

    for (const [start, end] of [
      ['export function useSavePlanMutation()', 'export function useDropPlanMutation()'],
      ['export function useDropPlanMutation()', 'export function useUpdatePlanMutation()'],
      ['export function useUpdatePlanMutation()', 'export function useSortPlansMutation()'],
      ['export function useSortPlansMutation()', 'export function useUpdateUserMutation()'],
    ] as const) {
      const hook = adminQueriesSource.slice(
        adminQueriesSource.indexOf(start),
        adminQueriesSource.indexOf(end),
      );
      expect(hook).not.toContain('onSuccess');
      expect(hook).not.toContain('adminKeys.plans');
    }
  });

  it('keeps the original drawer record lifetime instead of resetting on each open', () => {
    expect(plansSource).toContain(
      'const [submit, setSubmit] = useState<EditablePlan>(() => ({ ...(record ?? emptyPlan()) }));',
    );
    expect(plansSource).not.toContain('setSubmit({ ...(record ?? emptyPlan()) });');
    expect(plansSource).not.toContain('[record, visible]');
    expect(plansSource).toContain('<PlanEditor\n                    key={record.id}');
  });

  it('keeps the original editor-mounted config and server-group fetches', () => {
    expect(plansSource).toContain('useCallback,');
    expect(plansSource).toContain('const refetchConfig = config.refetch;');
    expect(plansSource).toContain('const refetchGroups = groups.refetch;');
    expect(plansSource).toContain('const refetchPlanEditorDependencies = useCallback(() => {');
    expect(plansSource).toContain('void refetchConfig();');
    expect(plansSource).toContain('void refetchGroups();');
    expect(plansSource).toContain('useEffect(() => {\n    onLegacyMount();\n  }, [onLegacyMount]);');
    expect(plansSource).toContain('onLegacyMount={refetchPlanEditorDependencies}');
  });

  it('keeps the original direct drawer input bindings', () => {
    expect(plansSource).toContain('value={submit.name as string | undefined}');
    expect(plansSource).toContain('value={submit.content as string | undefined}');
    expect(plansSource).toContain('value={submit.month_price !== null ? submit.month_price : undefined}');
    expect(plansSource).toContain('value={submit.transfer_enable}');
    expect(plansSource).toContain('value={submit.device_limit}');
    expect(plansSource).toContain('value={value as string | number | undefined}');
    expect(plansSource).not.toContain('function toInputValue');
  });

  it('keeps the original site currency-symbol handoff for plan editors', () => {
    expect(plansSource).toContain('currencySymbol?: string;');
    expect(plansSource).toContain('currencySymbol={config.data?.site?.currency_symbol}');
    expect(plansSource).not.toContain("currency_symbol ?? config.data?.currency_symbol ?? ''");
    expect(plansSource).not.toContain("config.data?.site?.currency_symbol ?? ''");
  });

  it('keeps the original switch and reset-method option value wiring', () => {
    expect(plansSource).toContain('checked={parseInt(String(value), 10) as unknown as boolean}');
    expect(plansSource).not.toContain('checked={Boolean(parseInt(String(value), 10))}');
    expect(plansSource).toContain('<Select.Option key={null} value={null}>跟随系统设置</Select.Option>');
    expect(plansSource).toContain('<Select.Option key={4} value={4}>按年重置</Select.Option>');
  });

  it('keeps the original plan update key/value dispatch shape', () => {
    const hook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useUpdatePlanMutation()'),
      adminQueriesSource.indexOf('export function useSortPlansMutation()'),
    );

    expect(plansSource).toContain("const updatePlan = (id: number, key: 'show' | 'renew', value: 0 | 1) => {");
    expect(plansSource).toContain('update.mutate(\n      { id, key, value },');
    expect(plansSource).not.toContain('{ id, [key]: value }');
    expect(hook).toContain("mutationFn: (vars: { id: number; key: 'show' | 'renew'; value: 0 | 1 }) =>");
    expect(hook).toContain('admin.updatePlan(apiClient, vars.id, vars.key, vars.value)');
    expect(hook).not.toContain('show?:');
    expect(hook).not.toContain('renew?:');
    expect(hook).not.toContain('vars.show');
    expect(hook).not.toContain('vars.renew');
  });

  it('renders server groups with the original tag component styling', () => {
    expect(plansSource).toContain('Tag,');
    expect(plansSource).toContain('const tags: ReactNode[] = [];');
    expect(plansSource).toContain('group.id === parseInt(String(value), 10)');
    expect(plansSource).toContain('tags.push(<Tag>{group.name}</Tag>)');
    expect(plansSource).toContain('return tags;');
    expect(plansSource).not.toContain('return group ? <Tag>{group}</Tag> : null;');
    expect(plansSource).not.toContain('<span className="ant-tag">{group}</span>');
  });

  it('keeps the original null-only price formatter behavior', () => {
    expect(plansSource).toContain('function formatPrice(value: number | null) {');
    expect(plansSource).toContain("return value !== null ? value.toFixed(2) : '-';");
    expect(plansSource).not.toContain('value !== null && value !== undefined');
    expect(plansSource).not.toContain('Number(value).toFixed(2)');
  });
});
