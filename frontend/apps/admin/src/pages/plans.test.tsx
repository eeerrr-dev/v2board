import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import PlansPage from './plans';

const plansSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'plans.tsx'),
  'utf8',
);
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
    expect(html).toContain('class="ant-btn"');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-table-fixed" style="width:1300px"');
    expect(html).toContain('class="ant-table-fixed-right"');
    expect(html).toContain('class="ant-switch-small ant-switch ant-switch-checked"');
    expect(html).toContain('aria-label="图标: menu"');
    expect(html).toContain('aria-label="图标: user"');
    expect(html).toContain('aria-label="图标: caret-down"');
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
    expect(html).toContain('class="ant-tag"');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('css-dev-only');
    expect(html).not.toContain('ant-typography');
  });

  it('preserves the original row right-click edit/delete menu', () => {
    expect(plansSource).toContain('id="v2board-table-dropdown"');
    expect(plansSource).toContain(
      'ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical',
    );
    expect(plansSource).toContain('onContextMenu={(event) => {');
    expect(plansSource).toContain('event.preventDefault()');
    expect(plansSource).toContain('event.clientY');
    expect(plansSource).toContain('event.clientX');
    expect(plansSource).toContain("display: contextMenu ? 'unset' : 'none'");
    expect(plansSource).not.toContain('d-none');
  });

  it('keeps the legacy action dropdown delete color on the menu item', () => {
    expect(plansSource).toContain('LegacyDropdownMenu,');
    expect(plansSource).toContain('LegacyDropdownMenuItem,');
    expect(plansSource).not.toContain("import type { DropdownProps } from 'antd';");
    expect(plansSource).not.toContain('popupRender={() => overlay}');
    expect(plansSource).not.toContain('<Menu>');
    expect(plansSource).toContain('trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}');
    expect(plansSource).toContain('overlay={');
    expect(plansSource).toContain(
      '<LegacyDropdownMenuItem key="edit" onContextMenu={(event) => event.stopPropagation()}>',
    );
    expect(plansSource).toContain('key="delete"');
    expect(plansSource).toContain("style={{ color: '#ff4d4f' }}");
    expect(plansSource).toContain('onClick={() => dropPlan(record.id)}');
    expect(plansSource).toContain('<LegacyEditIcon /> 编辑');
    expect(plansSource).toContain('<LegacyDeleteIcon /> 删除');
    expect(plansSource).not.toContain("key: 'delete',");
    expect(plansSource).not.toContain('menu={{');
    expect(plansSource).not.toContain("<span style={{ color: '#ff4d4f' }}>");
    expect(plansSource).not.toContain('<DeleteOutlined');
    expect(plansSource).not.toContain('<EditOutlined');
  });

  it('keeps the legacy plan table without an explicit rowKey', () => {
    expect(plansSource).toContain('<LegacyStandaloneTable');
    expect(plansSource).toContain('scrollX={1300}');
    expect(plansSource).toContain('scrollPositionRight={false}');
    expect(plansSource).toContain('fixedRightRowHeight={75}');
    expect(plansSource).toContain('fixedRightChildren={order.map((record, index) => (');
    expect(plansSource).toContain('<LegacyDragSort');
    expect(plansSource).toContain('nodeSelector="tr"');
    expect(plansSource).toContain('handleSelector="i"');
    expect(plansSource).toContain("<LegacyMenuIcon style={{ cursor: 'move' }} />");
    expect(plansSource).toContain('{...legacyTableRowKey(index)}');
    expect(plansSource).not.toContain('<Table<Plan>');
    expect(plansSource).not.toContain('tableLayout="auto"');
    expect(plansSource).not.toContain('pagination={false}');
    expect(plansSource).not.toContain('data-sort-index');
    expect(plansSource).not.toContain('data-row-key');
    expect(plansSource).not.toContain('rowKey="id"');
  });

  it('keeps the original plan sort loading and force-update payload shape', () => {
    expect(plansSource).toContain(
      'const [legacySortLoading, setLegacySortLoading] = useState(false);',
    );
    expect(plansSource).toContain('setLegacySortLoading(true);');
    expect(plansSource).toContain('loading={plans.isFetching || legacySortLoading}');
    expect(plansSource).not.toContain('loading={plans.isFetching || sort.isPending}');
    expect(plansSource).not.toContain('plans.isLoading');
    expect(plansSource).not.toContain('loading={Boolean(');
    expect(plansSource).toContain('const sortPlan = (fromIndex: number, toIndex: number) => {');
    expect(plansSource).toContain('next.splice(toIndex + 1, 0, moved);');
    expect(plansSource).toContain('next.splice(fromIndex + 1, 1);');
    expect(plansSource).not.toContain('next.force_update = next.force_update ? 1 : 0');
    expect(plansSource).toContain('force_update?: boolean');
    expect(plansSource).toContain(
      "onChange={(event) => change('force_update', event.target.checked)}",
    );
    expect(plansSource).not.toContain('checked={Boolean(submit.force_update)}');
  });

  it('submits the original drawer state instead of rewriting prices in the page component', () => {
    expect(plansSource).toContain('await onSave({ ...submit });');
    expect(plansSource).toContain('await save.mutateAsync(payload);\n    void plans.refetch();');
    expect(plansSource).not.toContain(
      'await save.mutateAsync(payload);\n    await plans.refetch();',
    );
    expect(plansSource).not.toContain('serializePlan(');
    expect(plansSource).not.toContain('Math.round(100 * Number(next[key]))');
  });

  it('keeps plan mutations fetching from the page after successful requests', () => {
    const sortStart = plansSource.indexOf('sort.mutate(\n      next.map((plan) => plan.id),');
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
    expect(plansSource).toContain('key={record.id}');
  });

  it('keeps the original editor-mounted config and server-group fetches', () => {
    expect(plansSource).toContain('useCallback,');
    expect(plansSource).toContain('const refetchConfig = config.refetch;');
    expect(plansSource).toContain('const refetchGroups = groups.refetch;');
    expect(plansSource).toContain('const refetchPlanEditorDependencies = useCallback(() => {');
    expect(plansSource).toContain('void refetchConfig();');
    expect(plansSource).toContain('void refetchGroups();');
    expect(plansSource).toContain(
      'useEffect(() => {\n    onLegacyMount();\n  }, [onLegacyMount]);',
    );
    expect(plansSource).toContain('onLegacyMount={refetchPlanEditorDependencies}');
  });

  it('keeps the original direct drawer input bindings', () => {
    expect(plansSource).toContain("import { LegacyDrawer } from '@/components/legacy-drawer';");
    expect(plansSource).toContain('LegacyInputGroup,');
    expect(plansSource).toContain('LegacyTextArea,');
    expect(plansSource).toContain(
      "import { LegacySelect, type LegacySelectOption } from '@/components/legacy-select';",
    );
    expect(plansSource).toContain('<LegacyDrawer');
    expect(plansSource).toContain('<LegacyInput');
    expect(plansSource).toContain('<LegacyTextArea');
    expect(plansSource).toContain('<LegacyInputGroup');
    expect(plansSource).toContain('<LegacySelect');
    expect(plansSource).toContain('<LegacyInfoCircleIcon />');
    expect(plansSource).toContain('LegacyInfoCircleIcon,');
    expect(plansSource).toContain('value={legacyInputValue(submit.name)}');
    expect(plansSource).toContain('value={legacyInputValue(submit.content)}');
    expect(plansSource).toContain(
      'value={submit.month_price !== null ? submit.month_price : undefined}',
    );
    expect(plansSource).toContain('value={submit.transfer_enable}');
    expect(plansSource).toContain('value={submit.device_limit}');
    expect(plansSource).toContain('value={legacyInputValue(value)}');
    expect(plansSource).toContain('function legacyInputValue(value: unknown)');
    expect(plansSource).toContain('className="ant-btn"');
    expect(plansSource).toContain(
      "className={`ant-btn ant-btn-primary${saveLoading ? ' ant-btn-loading' : ''}`}",
    );
    expect(plansSource).not.toContain('<Drawer');
    expect(plansSource).not.toContain('<Input');
    expect(plansSource).not.toContain('<Button');
    expect(plansSource).not.toContain('<Select');
    expect(plansSource).not.toContain('<Checkbox');
    expect(plansSource).not.toContain('@ant-design/icons');
    expect(plansSource).not.toContain('InfoCircleOutlined');
  });

  it('keeps the original site currency-symbol handoff for plan editors', () => {
    expect(plansSource).toContain('currencySymbol?: string;');
    expect(plansSource).toContain('currencySymbol={config.data?.site?.currency_symbol}');
    expect(plansSource).not.toContain("currency_symbol ?? config.data?.currency_symbol ?? ''");
    expect(plansSource).not.toContain("config.data?.site?.currency_symbol ?? ''");
  });

  it('keeps the original switch and reset-method option value wiring', () => {
    expect(plansSource).toContain(
      'const renderPlanSwitch = (checked: 0 | 1, onClick: () => void) => {',
    );
    expect(plansSource).toContain('const enabled = Boolean(parseInt(String(checked), 10));');
    expect(plansSource).toContain('aria-checked={enabled}');
    expect(plansSource).toContain('className={`ant-switch-small ant-switch${enabled ?');
    expect(plansSource).not.toContain('<Switch');
    expect(plansSource).not.toContain('checked={Boolean(parseInt(String(value), 10))}');
    expect(plansSource).toContain('const LEGACY_RESET_TRAFFIC_OPTIONS: LegacySelectOption[] = [');
    expect(plansSource).toContain("{ value: null, label: '跟随系统设置' }");
    expect(plansSource).toContain('跟随系统设置');
    expect(plansSource).toContain("{ value: 4, label: '按年重置' }");
    expect(plansSource).toContain('按年重置');
  });

  it('keeps the original plan update key/value dispatch shape', () => {
    const hook = adminQueriesSource.slice(
      adminQueriesSource.indexOf('export function useUpdatePlanMutation()'),
      adminQueriesSource.indexOf('export function useSortPlansMutation()'),
    );

    expect(plansSource).toContain(
      "const updatePlan = (id: number, key: 'show' | 'renew', value: 0 | 1) => {",
    );
    expect(plansSource).toContain('update.mutate(\n      { id, key, value },');
    expect(plansSource).not.toContain('{ id, [key]: value }');
    expect(hook).toContain(
      "mutationFn: (vars: { id: number; key: 'show' | 'renew'; value: 0 | 1 }) =>",
    );
    expect(hook).toContain('admin.updatePlan(apiClient, vars.id, vars.key, vars.value)');
    expect(hook).not.toContain('show?:');
    expect(hook).not.toContain('renew?:');
    expect(hook).not.toContain('vars.show');
    expect(hook).not.toContain('vars.renew');
  });

  it('renders server groups with the original tag component styling', () => {
    expect(plansSource).toContain('className="ant-tag"');
    expect(plansSource).toContain('const tags: ReactNode[] = [];');
    expect(plansSource).toContain('group.id === parseInt(String(value), 10)');
    expect(plansSource).toContain('{group.name}');
    expect(plansSource).toContain('return tags;');
    expect(plansSource).not.toContain('Tag,');
    expect(plansSource).not.toContain('tags.push(<Tag>{group.name}</Tag>)');
    expect(plansSource).not.toContain('return group ? <Tag>{group}</Tag> : null;');
  });

  it('keeps the original null-only price formatter behavior', () => {
    expect(plansSource).toContain('function formatPrice(value: number | null) {');
    expect(plansSource).toContain("return value !== null ? value.toFixed(2) : '-';");
    expect(plansSource).not.toContain('value !== null && value !== undefined');
    expect(plansSource).not.toContain('Number(value).toFixed(2)');
  });
});
