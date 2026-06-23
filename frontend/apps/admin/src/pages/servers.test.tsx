import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ServersPage, {
  createServerSortPayload,
  getLegacyBinarySelectValue,
  getLegacyNetworkSettingsPlaceholder,
  getLegacyNumericSelectValue,
  getLegacyServerInitialValues,
  getLegacyV2nodeSecurityValue,
  installLegacyServerSortPrompt,
  moveServerNodeByLegacyDragIndexes,
  shouldPromptLegacyServerSortClick,
} from './servers';

const serversSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'servers.tsx'),
  'utf8',
);
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);
const defaultUserAgent = window.navigator.userAgent;

const mocks = vi.hoisted(() => ({
  pathname: '/server/group',
}));

vi.mock('react-router-dom', () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/lib/queries', () => ({
  useServerGroups: () => ({
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
    data: [
      {
        id: 1,
        name: 'VIP',
        user_count: 12,
        server_count: 3,
        created_at: 1,
        updated_at: 1,
      },
    ],
  }),
  useSaveServerGroupMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useDropServerGroupMutation: () => ({
    mutate: vi.fn(),
  }),
  useServerNodes: () => ({
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
    data: [
      {
        id: 1,
        name: 'Tokyo',
        group_id: [1],
        route_id: [],
        type: 'shadowsocks',
        host: 'example.com',
        port: 443,
        server_port: null,
        show: 1,
        rate: '1',
        parent_id: null,
        online: 8,
        last_check_at: null,
        is_online: 1,
        available_status: 2,
      },
      {
        id: 2,
        name: 'Child',
        group_id: [1],
        route_id: [],
        type: 'vmess',
        host: 'child.example.com',
        port: 8443,
        server_port: null,
        show: 0,
        rate: '2',
        parent_id: 1,
        online: 0,
        last_check_at: null,
        is_online: 0,
        available_status: 0,
      },
    ],
  }),
  useSortServerNodesMutation: () => ({
    mutate: vi.fn(),
  }),
  useServerRoutes: () => ({
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
    data: [
      {
        id: 1,
        remarks: 'Netflix',
        match: ['geosite:netflix', 'domain:example.com'],
        action: 'route',
        action_value: '{}',
        created_at: 1,
        updated_at: 1,
      },
      {
        id: 2,
        remarks: 'Default',
        match: [],
        action: 'default_out',
        action_value: '{}',
        created_at: 1,
        updated_at: 1,
      },
    ],
  }),
  useUpdateServerMutation: vi.fn(),
  useDropServerMutation: vi.fn(),
  useCopyServerMutation: vi.fn(),
  useSaveServerRouteMutation: () => ({
    isPending: false,
    mutateAsync: vi.fn(),
  }),
  useDropServerRouteMutation: () => ({
    mutate: vi.fn(),
  }),
}));

function setUserAgent(value: string) {
  Object.defineProperty(window.navigator, 'userAgent', {
    configurable: true,
    value,
  });
}

beforeEach(() => {
  mocks.pathname = '/server/group';
  setUserAgent(defaultUserAgent);
  document.body.innerHTML = '';
  vi.restoreAllMocks();
});

describe('ServersPage legacy server group route', () => {
  it('renders /server/group as the original standalone permission group table', () => {
    mocks.pathname = '/server/group';
    const html = renderToStaticMarkup(<ServersPage />);

    expect(html).toContain('class="d-flex justify-content-between align-items-center"');
    expect(html).toContain('class="block block-rounded"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('<button type="button" class="ant-btn">');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('添加权限组');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-spin-nested-loading"');
    expect(html).toContain('class="ant-table ant-table-default ant-table-scroll-position-left"');
    expect(html).toContain('class="ant-table-header-column"');
    expect(html).toContain('class="ant-table-column-sorter"');
    expect(html).toContain('data-row-key="0"');
    expect(html).toContain('组ID');
    expect(html).toContain('组名称');
    expect(html).toContain('用户数量');
    expect(html).toContain('节点数量');
    expect(html).toContain('操作');
    expect(html).toContain(
      'class="ant-table-align-right ant-table-row-cell-last" style="text-align:right"',
    );
    expect(html).toContain('anticon-user');
    expect(html).toContain('anticon-database');
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-tabs');
    expect(html).not.toContain('created_at');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('css-dev-only');
  });

  it('renders /server/route as the original standalone route table', () => {
    mocks.pathname = '/server/route';
    const html = renderToStaticMarkup(<ServersPage />);

    expect(html).toContain('class="d-flex justify-content-between align-items-center"');
    expect(html).toContain('class="block block-rounded"');
    expect(html).toContain('class="bg-white"');
    expect(html).toContain('<button type="button" class="ant-btn">');
    expect(html).toContain('aria-label="图标: plus"');
    expect(html).toContain('添加路由');
    expect(html).toContain('class="ant-table-wrapper"');
    expect(html).toContain('class="ant-spin-nested-loading"');
    expect(html).toContain('class="ant-table ant-table-default ant-table-scroll-position-left"');
    expect(html).toContain('class="ant-table-header-column"');
    expect(html).toContain('class="ant-table-column-sorter"');
    expect(html).toContain('data-row-key="0"');
    expect(html).toContain('ID');
    expect(html).toContain('备注');
    expect(html).toContain('匹配数量');
    expect(html).toContain('动作');
    expect(html).toContain('操作');
    expect(html).toContain('匹配 2 条规则');
    expect(html).toContain('无规则时默认');
    expect(html).toContain('指定出站服务器(域名目标)');
    expect(html).toContain('自定义默认出站');
    expect(html).toContain('编辑');
    expect(html).toContain('删除');
    expect(html).not.toContain('ant-tabs');
    expect(html).not.toContain('action_value');
    expect(html).not.toContain('ant-table-cell');
    expect(html).not.toContain('css-dev-only');
  });

  it('keeps the original server group modal submit loading tied to group fetching', () => {
    const groupModalSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupModal'),
      serversSource.indexOf('function getRouteMatchLabel'),
    );

    expect(serversSource).toContain("import { LegacyModal } from '@/components/legacy-modal';");
    expect(groupModalSource).toContain('<LegacyModal');
    expect(groupModalSource).toContain("title={`${submit.id ? '编辑组' : '创建组'}`}");
    expect(groupModalSource).toContain('visible={visible}');
    expect(groupModalSource).toContain('cancelText="取消"');
    expect(groupModalSource).not.toContain('<Modal');
    expect(groupModalSource).not.toContain('open={visible}');
    expect(groupModalSource).toContain('const groups = useServerGroups()');
    expect(groupModalSource).toContain('if (groups.isFetching) return;');
    expect(groupModalSource).toContain('await save.mutateAsync({ ...submit });');
    expect(groupModalSource).toContain('await groups.refetch();');
    expect(groupModalSource.indexOf('await save.mutateAsync({ ...submit });')).toBeLessThan(
      groupModalSource.indexOf('await groups.refetch();'),
    );
    expect(groupModalSource.indexOf('await groups.refetch();')).toBeLessThan(
      groupModalSource.indexOf('setVisible(false);'),
    );
    expect(groupModalSource).not.toContain('void groups.refetch();\n    setVisible(false);');
    expect(groupModalSource).toContain(
      "okText={groups.isFetching ? <LegacyLoadingIcon /> : '提交'}",
    );
    expect(groupModalSource).not.toContain('LoadingOutlined');
    expect(groupModalSource).not.toContain(
      'save.mutateAsync({ id: submit.id, name: submit.name })',
    );
    expect(groupModalSource).not.toContain('okText={save.isPending');
  });

  it('keeps the original server route modal submit loading tied to route fetching', () => {
    const routeModalSource = serversSource.slice(
      serversSource.indexOf('function ServerRouteModal'),
      serversSource.indexOf('function getServerTypeTag'),
    );

    expect(routeModalSource).toContain('<LegacyModal');
    expect(routeModalSource).toContain("title={`${route.id ? '编辑路由' : '创建路由'}`}");
    expect(routeModalSource).toContain('visible={visible}');
    expect(routeModalSource).toContain('cancelText="取消"');
    expect(routeModalSource).not.toContain('<Modal');
    expect(routeModalSource).not.toContain('open={visible}');
    expect(routeModalSource).toContain('const routes = useServerRoutes()');
    expect(routeModalSource).toContain('if (routes.isFetching) return;');
    expect(routeModalSource).toContain('await save.mutateAsync(payload);');
    expect(routeModalSource).toContain('await routes.refetch();');
    expect(routeModalSource.indexOf('await save.mutateAsync(payload);')).toBeLessThan(
      routeModalSource.indexOf('await routes.refetch();'),
    );
    expect(routeModalSource.indexOf('await routes.refetch();')).toBeLessThan(
      routeModalSource.indexOf('setVisible(false);'),
    );
    expect(routeModalSource).not.toContain('void routes.refetch();\n    setVisible(false);');
    expect(routeModalSource).toContain(
      "okText={routes.isFetching ? <LegacyLoadingIcon /> : '提交'}",
    );
    expect(routeModalSource).not.toContain('LoadingOutlined');
    expect(routeModalSource).not.toContain('okText={save.isPending');
  });

  it('keeps the legacy standalone server group and route tables without explicit rowKey props', () => {
    const groupPageSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupPage'),
      serversSource.indexOf('function ServerGroupModal'),
    );
    const routePageSource = serversSource.slice(
      serversSource.indexOf('function ServerRoutePage'),
      serversSource.indexOf('function ServerRouteModal'),
    );

    expect(groupPageSource).toContain('<LegacyStandaloneTable');
    expect(groupPageSource).toContain('headers={headers}');
    expect(groupPageSource).toContain('isEmpty={groupItems.length === 0}');
    expect(groupPageSource).toContain('{...legacyRowKey(index)}');
    expect(groupPageSource).toContain(
      'className="ant-table-align-right ant-table-row-cell-last"',
    );
    expect(groupPageSource).not.toContain('<Table<admin.ServerGroup>');
    expect(groupPageSource).not.toContain('tableLayout="auto"');
    expect(groupPageSource).not.toContain('pagination={false}');
    expect(groupPageSource).not.toContain('rowKey="id"');
    expect(routePageSource).toContain('<LegacyStandaloneTable');
    expect(routePageSource).toContain('headers={headers}');
    expect(routePageSource).toContain('isEmpty={routeItems.length === 0}');
    expect(routePageSource).toContain('{...legacyRowKey(index)}');
    expect(routePageSource).toContain(
      'className="ant-table-align-right ant-table-row-cell-last"',
    );
    expect(routePageSource).not.toContain('<Table<admin.ServerRoute>');
    expect(routePageSource).not.toContain('tableLayout="auto"');
    expect(routePageSource).not.toContain('pagination={false}');
    expect(routePageSource).not.toContain('rowKey="id"');
  });

  it('keeps the original keyed edit modal instances in group and route action columns', () => {
    const groupPageSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupPage'),
      serversSource.indexOf('function ServerGroupModal'),
    );
    const routePageSource = serversSource.slice(
      serversSource.indexOf('function ServerRoutePage'),
      serversSource.indexOf('function ServerRouteModal'),
    );

    expect(groupPageSource).toContain('<ServerGroupModal key={record.id} record={record}>');
    expect(routePageSource).toContain('<ServerRouteModal key={record.id} route={record}>');
  });

  it('keeps standalone server group and route mutations fetching from the page after success', () => {
    const groupPageSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupPage'),
      serversSource.indexOf('function ServerGroupModal'),
    );
    const routePageSource = serversSource.slice(
      serversSource.indexOf('function ServerRoutePage'),
      serversSource.indexOf('function ServerRouteModal'),
    );
    const groupHooksSource = queriesSource.slice(
      queriesSource.indexOf('export function useSaveServerGroupMutation()'),
      queriesSource.indexOf('export function useSaveServerRouteMutation()'),
    );
    const routeHooksSource = queriesSource.slice(
      queriesSource.indexOf('export function useSaveServerRouteMutation()'),
      queriesSource.indexOf('export function useDropServerMutation()'),
    );

    expect(groupPageSource).toContain('drop.mutate(record.id, {');
    expect(groupPageSource).toContain('void groups.refetch();');
    expect(routePageSource).toContain('drop.mutate(record.id, {');
    expect(routePageSource).toContain('void routes.refetch();');
    expect(groupHooksSource).not.toContain('onSuccess');
    expect(groupHooksSource).not.toContain('adminKeys.serverGroups');
    expect(routeHooksSource).not.toContain('onSuccess');
    expect(routeHooksSource).not.toContain('adminKeys.serverRoutes');
  });

  it('keeps server node refreshes fire-and-forget across the page source', () => {
    expect(serversSource).not.toContain('await nodes.refetch();');
  });

  it('keeps the original modal label targets without generated control ids', () => {
    const groupModalSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupModal'),
      serversSource.indexOf('function getRouteMatchLabel'),
    );
    const routeModalSource = serversSource.slice(
      serversSource.indexOf('function ServerRouteModal'),
      serversSource.indexOf('function getServerTypeTag'),
    );

    expect(groupModalSource).toContain('htmlFor="example-text-input-alt"');
    expect(routeModalSource.match(/htmlFor="example-text-input-alt"/g)).toHaveLength(5);
    expect(groupModalSource).not.toContain('server-group-name');
    expect(routeModalSource).not.toContain('server-route-');
  });

  it('keeps the original modal input values without empty-string fallbacks', () => {
    const groupModalSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupModal'),
      serversSource.indexOf('function getRouteMatchLabel'),
    );
    const routeModalSource = serversSource.slice(
      serversSource.indexOf('function ServerRouteModal'),
      serversSource.indexOf('function getServerTypeTag'),
    );

    expect(serversSource).toContain("} from '@/components/legacy-input';");
    expect(serversSource).toContain('LegacyInput,');
    expect(serversSource).toContain('LegacyTextArea,');
    expect(groupModalSource).toContain('<LegacyInput');
    expect(groupModalSource).toContain('className="ant-input"');
    expect(groupModalSource).not.toContain('<Input');
    expect(routeModalSource.match(/<LegacyInput/g)).toHaveLength(2);
    expect(routeModalSource.match(/<LegacyTextArea/g)).toHaveLength(2);
    expect(routeModalSource).toContain('className="ant-input"');
    expect(routeModalSource).not.toContain('<Input');
    expect(routeModalSource).not.toContain('Input.TextArea');
    expect(groupModalSource).toContain('value={submit.name}');
    expect(routeModalSource).toContain('value={route.remarks}');
    expect(
      routeModalSource.match(/value=\{legacyInputValue\(route\.action_value\)\}/g),
    ).toHaveLength(2);
    expect(serversSource).toContain('function legacyInputValue(value: unknown)');
    expect(serversSource).toContain("return value?.split(',').join('\\n');");
    expect(groupModalSource).not.toContain("submit.name ?? ''");
    expect(routeModalSource).not.toContain("route.remarks ?? ''");
    expect(routeModalSource).not.toContain('route.action_value ?? undefined');
    expect(routeModalSource).not.toContain("route.action_value ?? ''");
    expect(serversSource).not.toContain("return value?.split(',').join('\\n') ?? ''");
  });

  it('keeps the original network settings placeholder lookup without defaulting to tcp', () => {
    expect(serversSource).toContain(
      "return LEGACY_NETWORK_SETTINGS_PLACEHOLDERS[type]?.[String(network)] || '';",
    );
    expect(serversSource).toContain(
      'placeholder={getLegacyNetworkSettingsPlaceholder(type, network)}',
    );
    expect(serversSource).not.toContain(
      "LEGACY_NETWORK_SETTINGS_PLACEHOLDERS[String(network ?? 'tcp')]",
    );
    expect(serversSource).not.toContain(
      "LEGACY_NETWORK_SETTINGS_PLACEHOLDERS[String(network)] ?? ''",
    );
  });

  it('uses the original type-specific transport placeholders', () => {
    expect(getLegacyNetworkSettingsPlaceholder('v2node', 'tcp')).toContain(
      '"acceptProxyProtocol": false',
    );
    expect(getLegacyNetworkSettingsPlaceholder('v2node', 'http')).toContain(
      '"Host": "xtls.github.io"',
    );
    expect(getLegacyNetworkSettingsPlaceholder('vmess', 'tcp')).not.toContain(
      'acceptProxyProtocol',
    );
    expect(getLegacyNetworkSettingsPlaceholder('vmess', 'ws')).toContain('"Host": "v2ray.com"');
    expect(getLegacyNetworkSettingsPlaceholder('vmess', 'xhttp')).not.toContain('"mode": "auto"');
    expect(getLegacyNetworkSettingsPlaceholder('vless', 'ws')).toContain('"security": "auto"');
    expect(getLegacyNetworkSettingsPlaceholder('vless', 'xhttp')).toContain('"mode": "auto"');
    expect(getLegacyNetworkSettingsPlaceholder('trojan', 'tcp')).toBe('');
    expect(getLegacyNetworkSettingsPlaceholder('trojan', 'httpupgrade')).toBe('');
    expect(serversSource).toContain('tcp: LEGACY_VMESS_NETWORK_SETTINGS_PLACEHOLDERS.tcp!,');
    expect(serversSource).toContain('xhttp: LEGACY_VLESS_NETWORK_SETTINGS_PLACEHOLDERS.xhttp!,');
  });

  it('does not keep the unused tabbed server fallback absent from the bundled routes', () => {
    const serversPageSource = serversSource.slice(
      serversSource.indexOf('export default function ServersPage'),
      serversSource.indexOf('function ServerGroupPage'),
    );

    expect(serversPageSource).toContain(
      "if (location.pathname === '/server/group') return <ServerGroupPage />;",
    );
    expect(serversPageSource).toContain(
      "if (location.pathname === '/server/route') return <ServerRoutePage />;",
    );
    expect(serversPageSource).toContain(
      "if (location.pathname === '/server/manage') return <ServerManagePage />;",
    );
    expect(serversPageSource).toContain('return null;');
    expect(serversSource).not.toContain('function NodesTab');
    expect(serversSource).not.toContain('function GroupsTab');
    expect(serversSource).not.toContain('function RoutesTab');
    expect(serversSource).not.toContain('<Tabs');
    expect(serversSource).not.toContain('<Card');
    expect(serversSource).not.toContain('Typography.Title');
  });

  it('keeps the original modal open behavior without resetting form state', () => {
    const groupModalSource = serversSource.slice(
      serversSource.indexOf('function ServerGroupModal'),
      serversSource.indexOf('function getRouteMatchLabel'),
    );
    const routeModalSource = serversSource.slice(
      serversSource.indexOf('function ServerRouteModal'),
      serversSource.indexOf('function getServerTypeTag'),
    );

    expect(groupModalSource).toContain('const open = () => {\n    setVisible(true);\n  };');
    expect(routeModalSource).toContain('const open = () => {\n    setVisible(true);\n  };');
    expect(groupModalSource).not.toContain('setSubmit(record ?? {})');
    expect(routeModalSource).not.toContain('setRoute(initialRoute ?? {})');
  });

  it('keeps the original handwritten route action options without mapped keys', () => {
    const routeModalSource = serversSource.slice(
      serversSource.indexOf('function ServerRouteModal'),
      serversSource.indexOf('function getServerTypeTag'),
    );

    const optionOrder = [
      "value: 'block'",
      "value: 'block_ip'",
      "value: 'block_port'",
      "value: 'protocol'",
      "value: 'dns'",
      "value: 'route'",
      "value: 'route_ip'",
      "value: 'default_out'",
    ];

    const optionIndexes = optionOrder.map((text) => routeModalSource.indexOf(text));

    expect(optionIndexes.every((index) => index >= 0)).toBe(true);
    expect(optionIndexes).toEqual([...optionIndexes].sort((a, b) => a - b));
    expect(routeModalSource).toContain('const routeActionOptions: LegacySelectOption[] = [');
    expect(routeModalSource).toContain('options={routeActionOptions}');
    expect(routeModalSource).toContain('<LegacySelect');
    expect(routeModalSource).not.toContain('<Select');
    expect(routeModalSource).not.toContain('Object.entries(ROUTE_ACTION_TEXT).map');
    expect(routeModalSource).not.toContain('<Select.Option key={value}');
  });

  it('keeps the original vertical divider markup in server action columns', () => {
    expect(serversSource).toContain("import { LegacyDivider } from '@/components/legacy-divider';");
    expect(serversSource.match(/<LegacyDivider type="vertical" \/>/g)).toHaveLength(3);
    expect(serversSource).not.toContain(
      '<div className="ant-divider ant-divider-vertical" role="separator" />',
    );
    expect(serversSource).not.toContain('<span className="ant-divider ant-divider-vertical"');
  });

  it('renders /server/manage with the original initial non-sort table before getNodes completes', () => {
    mocks.pathname = '/server/manage';
    const html = renderToStaticMarkup(<ServersPage />);

    expect(html).toContain('class="block block-bottom undefined"');
    expect(html).toContain('class="v2board-table-action"');
    expect(html).toContain('输入任意关键字搜索');
    expect(html).toContain('class="ant-input ml-2"');
    expect(html).toContain('编辑排序');
    expect(html).toContain('class="ant-btn ant-btn-primary"');
    expect(html).toContain('class="ant-table ant-table-default');
    expect(html).toContain('ant-table-scroll-position-left');
    expect(html).toContain('class="ant-table-fixed"');
    expect(html).toContain('style="width:1300px"');
    expect(html).toContain(
      'class="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last" style="text-align:right"',
    );
    expect(html).toContain('class="anticon anticon-filter ant-dropdown-trigger"');
    expect(html).toContain(
      'd="M880.1 154H143.9c-24.5 0-39.8 26.7-27.5 48L349 597.4V838c0 17.7 14.2 32 31.8 32h262.4c17.6 0 31.8-14.3 31.8-32V597.4L907.7 202c12.2-21.3-3.1-48-27.6-48z"',
    );
    expect(html).not.toContain('M613 561.4');
    expect(html).toContain('class="ant-table-align-left" style="text-align:left"');
    expect(html).toContain('class="ant-table-align-center" style="text-align:center"');
    expect(html).toContain('节点ID');
    expect(html).toContain('节点');
    expect(html).toContain('Tokyo');
    expect(html).toContain('显隐');
    expect(html).toContain(
      '<button type="button" role="switch" aria-checked="true" class="ant-switch-small ant-switch ant-switch-checked">',
    );
    expect(html).toContain(
      '<button type="button" role="switch" aria-checked="false" class="ant-switch-small ant-switch">',
    );
    expect(html).not.toContain('ant-switch-handle');
    expect(html).toContain('地址');
    expect(html).toContain('人数');
    expect(html).toContain('倍率');
    expect(html).toContain('权限组');
    expect(html).toContain('操作');
    expect(html).not.toContain('保存排序');
    expect(html).not.toContain('<th class="ant-table-cell" scope="col">排序</th>');
    expect(html).not.toContain('ant-tabs');
  });

  it('keeps /server/manage fixed-right action body cells using the original last-column classes', () => {
    const manageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(manageSource).toContain(
      'className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"',
    );
    expect(manageSource).toContain('className="ant-table-align-left"');
    expect(manageSource).toContain('className="ant-table-align-center"');
    expect(manageSource).toContain('className="ant-table-align-right ant-table-row-cell-last"');
    expect(manageSource).not.toContain(
      'className="ant-table-fixed-columns-in-body"\n                                            style={{ textAlign: \'right\' }}',
    );
    expect(manageSource).not.toContain('<td style={{ textAlign: \'right\' }}>{actionCell(node)}</td>');
  });

  it('keeps the legacy server manage hidden filter dropdown outside table content', () => {
    mocks.pathname = '/server/manage';
    document.body.innerHTML = renderToStaticMarkup(<ServersPage />);

    const table = document.querySelector('.ant-table');
    const directChildren = Array.from(table?.children ?? []).map((child) => ({
      className: child.getAttribute('class') ?? '',
      position: (child as HTMLElement).style.position,
    }));

    expect(directChildren).toEqual([
      { className: 'ant-table-content', position: '' },
      { className: '', position: 'absolute' },
    ]);
    expect(document.querySelector('.ant-table-content > [style*="position:absolute"]')).toBeNull();
    expect(
      Array.from(document.querySelector('.bg-white')?.children ?? []).map((child) => child.id),
    ).toEqual(['', '']);
    expect(document.querySelector('#v2board-table-dropdown')?.parentElement).toBe(
      document.querySelector('.ant-table-wrapper')?.parentElement,
    );
  });

  it('keeps the legacy server manage table without an explicit rowKey', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(serversSource).toContain("import { LegacySwitch } from '@/components/legacy-switch';");
    expect(serversSource).toContain('<LegacySwitch');
    expect(serversSource).not.toContain('<Switch');
    expect(serversSource).not.toContain('Switch,');
    expect(managePageSource).toContain('<div className="ant-table-wrapper">');
    expect(managePageSource).toContain('<div className="ant-spin-nested-loading">');
    expect(managePageSource).toContain('className="ant-table-fixed"');
    expect(managePageSource).toContain('style={{ width: 1300 }}');
    expect(managePageSource).toContain('<LegacyEmpty />');
    expect(managePageSource).toContain('<LegacyDragSort');
    expect(managePageSource).toContain('nodeSelector="tr"');
    expect(managePageSource).toContain('handleSelector="i"');
    expect(managePageSource).toContain('<LegacyMenuIcon />');
    expect(managePageSource).not.toContain('<Table<admin.ServerNode>');
    expect(managePageSource).not.toContain('scroll={{ x: 1300 }}');
    expect(managePageSource).not.toContain('data-sort-index');
    expect(managePageSource).not.toContain('data-row-key');
    expect(managePageSource).not.toContain('rowKey=');
  });

  it('renders /server/manage as the original mobile node list on mobile user agents', () => {
    mocks.pathname = '/server/manage';
    setUserAgent('Mozilla/5.0 Mobile');
    const html = renderToStaticMarkup(<ServersPage />);

    expect(html).toContain('ant-list');
    expect(html).toContain('v2board_node_mobile');
    expect(html).toContain('child_node');
    expect(html).toContain('example.com:443');
    expect(html).toContain('操作');
    expect(html).not.toContain('编辑排序');
    expect(serversSource).toContain('function isLegacyMobile()');
    expect(serversSource).toContain("window.navigator.userAgent.toLowerCase().includes('mobile')");
    expect(serversSource).toContain('function LegacyServerMobileNodeList({');
    expect(serversSource).toContain(
      'className="ant-list ant-list-vertical ant-list-split v2board-table"',
    );
    expect(serversSource).toContain('className="ant-list-item-action"');
    expect(serversSource).toContain('className="ant-list-item-extra"');
    expect(serversSource).toContain('<LegacyServerMobileNodeList');
    expect(serversSource).not.toContain('<List');
    expect(serversSource).not.toContain('actions={[');
    expect(serversSource).not.toContain('<Fragment');
    expect(serversSource).not.toContain('<span key="summary">');
  });

  it('uses the original available_status-only badge mapping', () => {
    expect(serversSource).toContain('function getLegacyAvailableStatus(status?: number | null)');
    expect(serversSource).toContain('getLegacyAvailableStatus(node.available_status)');
    expect(serversSource).toContain("import { LegacyBadge } from '@/components/legacy-badge';");
    expect(serversSource).toContain('<LegacyBadge');
    expect(serversSource).not.toContain('available_status ??');
    expect(serversSource).not.toContain('<Badge');
  });

  it('keeps the original parseInt switch checked values in server manage', () => {
    expect(serversSource).toContain('checked={checked as unknown as boolean}');
    expect(serversSource).toContain(
      'checked={parseInt(String(node.show), 10) as unknown as boolean}',
    );
    expect(serversSource).not.toContain('checked={Boolean(checked)}');
    expect(serversSource).not.toContain('checked={Boolean(parseInt(String(node.show), 10))}');
  });

  it('keeps the original server show update key/value dispatch shape', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );
    const hook = queriesSource.slice(
      queriesSource.indexOf('export function useUpdateServerMutation()'),
      queriesSource.indexOf('export function useSortServerNodesMutation()'),
    );

    expect(managePageSource).toContain("key: 'show',");
    expect(managePageSource).toContain('value: checked ? 0 : 1,');
    expect(managePageSource).not.toContain('show: checked ? 0 : 1');
    expect(hook).toContain(
      "mutationFn: (vars: { type: admin.ServerTypeName; id: number; key: 'show'; value: 0 | 1 }) =>",
    );
    expect(hook).toContain(
      'admin.updateServer(apiClient, vars.type, vars.id, vars.key, vars.value)',
    );
    expect(hook).not.toContain('vars.show');
  });

  it('keeps the original permission-group filter and tag rendering details', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(serversSource).toContain('<LegacyTag key={name}>{name}</LegacyTag>');
    expect(serversSource).toContain("import { LegacyTag } from '@/components/legacy-tag';");
    expect(serversSource).not.toContain('function LegacyTag');
    expect(serversSource).toContain('className="ant-table-filter-dropdown"');
    expect(serversSource).toContain('<span>{group.name}</span>');
    expect(managePageSource).toContain(
      '.map((id) => groups.data?.find((group) => group.id === Number(id))?.name)',
    );
    expect(managePageSource).toContain('.filter(Boolean);');
    expect(managePageSource).not.toContain('?? String(id)');
    expect(managePageSource).not.toContain(".join(', ')");
    expect(serversSource).not.toContain('row.group_id.map(String).includes(String(value))');
    expect(serversSource).not.toContain('<Tag key={name}>{name}</Tag>');
  });

  it('wires /server/manage edit actions to the node edit drawer instead of a placeholder notice', () => {
    expect(serversSource).not.toContain('编辑节点表单会继续按旧版逐类补齐');
    expect(serversSource).toContain('function LegacyNodeEditMenuTrigger');
    expect(serversSource).toContain('key={row.id}');
    expect(serversSource).toContain('record={row}');
    expect(serversSource).toContain('record={contextRecord}');
    expect(serversSource).not.toContain("runNodeAction('edit', contextRecord)");
    expect(serversSource).not.toContain('record={editing ?? undefined}');
  });

  it('wires the original /server/manage add-node type menu to the node edit drawer', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(serversSource).toContain(
      'function LegacyDropdown({ children, closeOnOverlayClick = true, overlay, trigger }',
    );
    expect(serversSource).toContain('function LegacyDropdownMenu({ children }');
    expect(serversSource).toContain('function LegacyDropdownMenuItem({');
    expect(serversSource).toContain("import { LegacyTag } from '@/components/legacy-tag';");
    expect(serversSource).toContain('const LEGACY_DROPDOWN_HOVER_CLOSE_DELAY = 100;');
    expect(serversSource).not.toContain('const LEGACY_DROPDOWN_HOVER_CLOSE_DELAY = 120;');
    expect(serversSource).toContain('if (!open) return undefined;');
    expect(serversSource).not.toContain('if (!open || !opensOnClick) return undefined;');
    expect(serversSource).toContain('if (closeOnOverlayClick) setOpen(false);');
    expect(serversSource).not.toContain('if (opensOnClick) setOpen(false);');
    expect(managePageSource).toContain('<LegacyDropdown');
    expect(managePageSource).toContain('closeOnOverlayClick={false}');
    expect(managePageSource).toContain('overlay={');
    expect(managePageSource).toContain('<LegacyDropdownMenu>');
    expect(managePageSource).toContain('<LegacyButton className="ant-btn">');
    expect(managePageSource).toContain('<LegacyPlusIcon />');
    expect(managePageSource).toContain('SERVER_TYPES.map((type) => (');
    expect(managePageSource).toContain('<LegacyDropdownMenuItem key={type}>');
    expect(serversSource).toContain('<LegacyNodeEditMenuTrigger');
    expect(serversSource).toContain('key={Math.random()}');
    expect(serversSource).toContain('type={type}');
    expect(serversSource).not.toContain('Dropdown, Form');
    expect(serversSource).not.toContain('Menu, Select');
    expect(serversSource).not.toContain('Tag, Badge');
    expect(managePageSource).not.toContain('menu={{');
    expect(managePageSource).not.toContain('items: SERVER_TYPES.map');
    expect(managePageSource).not.toContain(
      '<Button>\n                <PlusOutlined />\n              </Button>',
    );
    expect(serversSource).not.toContain(
      'onClick: ({ key }) => setEditing({ type: key as admin.ServerTypeName })',
    );
    expect(serversSource).not.toContain(
      'setEditing({ ...row, type: row.type as admin.ServerTypeName })',
    );
    expect(serversSource).toContain('record?: Partial<admin.ServerNode>');
  });

  it('keeps /server/manage row operation dropdowns on the original overlay menu', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(managePageSource).toContain('const actionMenu = (row: admin.ServerNode) => (');
    expect(managePageSource).toContain(
      '<LegacyDropdownMenuItem onContextMenu={(event) => event.stopPropagation()}>',
    );
    expect(managePageSource).toContain(
      "<LegacyDropdownMenuItem onClick={() => runNodeAction('copy', row)}>",
    );
    expect(managePageSource).toContain("style={{ color: '#ff4d4f' }}");
    expect(managePageSource).toContain('<LegacyDropdownMenuItem');
    expect(managePageSource).toContain('trigger={LEGACY_DROPDOWN_CLICK_TRIGGER}');
    expect(managePageSource).toContain('overlay={actionMenu(row)}');
    expect(managePageSource).toContain('actionMenu={actionMenu}');
    expect(serversSource).toContain('overlay={actionMenu(node)}');
    expect(managePageSource).not.toContain('menu={actionMenu');
    expect(managePageSource).not.toContain('const actionMenu = (row: admin.ServerNode): MenuProps');
    expect(managePageSource).not.toContain('<Menu>');
    expect(managePageSource).not.toContain('<Menu.Item');
  });

  it('keeps the original add-node protocol menu order', () => {
    expect(serversSource).toContain(`const SERVER_TYPES: admin.ServerTypeName[] = [
  'v2node',
  'shadowsocks',
  'vmess',
  'trojan',
  'hysteria',
  'tuic',
  'vless',
  'anytls',
]`);
  });

  it('uses the original server drawer shell for node editing', () => {
    expect(serversSource).toContain('function NodeEditDrawer');
    expect(serversSource).toContain("import { LegacyDrawer } from '@/components/legacy-drawer';");
    expect(serversSource).toContain("import { LegacyTooltip } from '@/components/legacy-tooltip';");
    expect(serversSource).toContain("import { App, Form } from 'antd';");
    expect(serversSource).not.toContain("import { App, Form, Input } from 'antd';");
    expect(serversSource).not.toContain('<Input');
    expect(serversSource).not.toContain('Input.TextArea');
    expect(serversSource).toContain('<LegacyDrawer');
    expect(serversSource).toContain('id="server"');
    expect(serversSource).toContain('maskClosable');
    expect(serversSource).toContain("title={id ? '编辑节点' : '新建节点'}");
    expect(serversSource).toContain('width="80%"');
    expect(serversSource).toContain('closable={false}');
    expect(serversSource).toContain('className="v2board-drawer-action"');
    expect(serversSource).toContain('<LegacyButton className="ant-btn"');
    expect(serversSource).toContain(
      "className={`ant-btn ant-btn-primary${saving ? ' ant-btn-loading' : ''}`}",
    );
    expect(serversSource).toContain('{saving ? <LegacyLoadingIcon /> : null}');
    expect(serversSource).not.toContain('LegacyServerButtonLoadingIcon');
    expect(serversSource).toContain('提交');
    expect(serversSource).toContain('取消');
    expect(serversSource).not.toContain('function NodeEditModal');
    expect(serversSource).not.toContain('width={720}');
    expect(serversSource).not.toContain('import {\\n  App,\\n  Button,\\n  Drawer,');
    expect(serversSource).not.toContain("Tooltip } from 'antd'");
    expect(serversSource).not.toContain('<Tooltip');
  });

  it('keeps the original node edit drawer mounted after close', () => {
    const nodeDrawerSource = serversSource.slice(
      serversSource.indexOf('function NodeEditDrawer'),
      serversSource.indexOf('function parseLegacyJsonPayloadField'),
    );

    expect(nodeDrawerSource).not.toContain('destroyOnClose');
  });

  it('uses the original server drawer form-group layout for common node fields', () => {
    const nodeDrawerSource = serversSource.slice(
      serversSource.indexOf('function NodeEditDrawer'),
      serversSource.indexOf('function parseLegacyJsonPayloadField'),
    );

    expect(serversSource).toContain('component={false}');
    expect(serversSource).toContain('className="form-group col-8"');
    expect(serversSource).toContain('className="form-group col-4"');
    expect(serversSource).toContain('className="form-group col-md-12 col-xs-12"');
    expect(serversSource).toContain('className="form-group col-md-6 col-xs-12"');
    expect(serversSource).toContain('节点名称');
    expect(serversSource).toContain('请输入节点名称');
    expect(serversSource).toContain('<Form.Item noStyle name="name">');
    expect(nodeDrawerSource).toContain(
      '<LegacyInput className="ant-input" placeholder="请输入节点名称" />',
    );
    expect(nodeDrawerSource).not.toContain('<Input placeholder="请输入节点名称" />');
    expect(serversSource).not.toContain(
      '<Form.Item noStyle name="name" rules={[{ required: true }]}>',
    );
    expect(serversSource).toContain('倍率');
    expect(serversSource).toContain('addonAfter="x"');
    expect(serversSource).toContain('<Form.Item noStyle name="rate">');
    expect(serversSource).toContain('LegacyInputGroup,');
    expect(nodeDrawerSource).toContain(
      '<LegacyInputGroup addonAfter="x" placeholder="请输入节点倍率" />',
    );
    expect(nodeDrawerSource).not.toContain('<Input addonAfter="x" placeholder="请输入节点倍率" />');
    expect(serversSource).not.toContain(
      '<Form.Item noStyle name="rate" rules={[{ required: true }]}>',
    );
    expect(serversSource).toContain('节点标签');
    expect(serversSource).toContain('输入后回车添加标签');
    expect(serversSource).toContain('function normalizeLegacyNullableArray');
    expect(serversSource).toContain('getValueFromEvent={normalizeLegacyNullableArray}');
    expect(serversSource).toContain('mode="tags"');
    expect(serversSource).toContain('options={[]}');
    expect(serversSource).toContain(
      'getValueProps={(value) => ({ value: Array.isArray(value) ? value : [] })}',
    );
    expect(serversSource).toContain('权限组');
    expect(serversSource).toContain('添加权限组');
    expect(serversSource).toContain('<LegacyTooltip>');
    expect(serversSource).toContain('<Form.Item noStyle name="group_id">');
    expect(serversSource).not.toContain(
      '<Form.Item noStyle name="group_id" rules={[{ required: true }]}>',
    );
    expect(serversSource).toContain('节点地址');
    expect(serversSource).toContain('地址或IP');
    expect(serversSource).toContain('<Form.Item noStyle name="host">');
    expect(nodeDrawerSource).toContain(
      '<LegacyInput className="ant-input" placeholder="地址或IP" />',
    );
    expect(nodeDrawerSource).toContain(
      '<LegacyInput className="ant-input" placeholder="请输入连接地址" />',
    );
    expect(serversSource).not.toContain(
      '<Form.Item noStyle name="host" rules={[{ required: true }]}>',
    );
    expect(serversSource).toContain('连接端口');
    expect(serversSource).toContain('用户连接端口');
    expect(serversSource).toContain('<Form.Item noStyle name="port">');
    expect(nodeDrawerSource).toContain(
      '<LegacyInput className="ant-input" placeholder="用户连接端口" />',
    );
    expect(serversSource).not.toContain(
      '<Form.Item noStyle name="port" rules={[{ required: true }]}>',
    );
    expect(serversSource).toContain('服务端口');
    expect(serversSource).toContain('非NAT同连接端口');
    expect(nodeDrawerSource).toContain(
      '<LegacyInput className="ant-input" placeholder="服务端开放端口" />',
    );
    expect(nodeDrawerSource).toContain(
      '<LegacyInput className="ant-input" placeholder="非NAT同连接端口" />',
    );
    expect(serversSource).toContain('父节点');
    expect(serversSource).toContain('更多解答');
    expect(serversSource).toContain('LegacyReadIcon');
    expect(serversSource).toContain('<LegacyTooltip placement="top">');
    expect(serversSource).toContain(
      "type === 'vmess' || type === 'vless' ? <LegacyReadIcon /> : '更多解答'",
    );
    expect(serversSource).toContain('<LegacyLinkIcon />');
    expect(serversSource).not.toContain('@ant-design/icons');
    expect(serversSource).not.toContain('LinkOutlined');
    expect(serversSource).not.toContain('ReadOutlined');
    expect(serversSource).toContain("} from '@/components/legacy-select';");
    expect(serversSource).toContain('LegacySelect,');
    expect(serversSource).toContain('type LegacySelectOption,');
    expect(serversSource).toContain('type LegacySelectValue,');
    expect(serversSource).toContain('const parentOptions: LegacySelectOption[] = [');
    expect(serversSource).toContain("{ value: '', label: '无' }");
    expect(serversSource).toContain(
      '...parentCandidates.map((node) => ({ value: node.id, label: node.name })),',
    );
    expect(serversSource).toContain('const groupOptions: LegacySelectOption[] = groups.map');
    expect(serversSource).toContain('value: String(group.id)');
    expect(serversSource).toContain('const routeOptions: LegacySelectOption[] = routes.map');
    expect(serversSource).toContain('label: route.id');
    expect(serversSource).not.toContain('label: route.remarks');
    expect(serversSource).toContain("getValueProps={(value) => ({ value: value || '' })}");
    expect(serversSource).toContain(
      "<LegacySelect style={{ width: '100%' }} options={parentOptions} />",
    );
    expect(serversSource).toContain('路由组');
    expect(serversSource).toContain('请选择路由组');
    expect(serversSource).toContain('name="route_id"');
    expect(serversSource).toContain('options={groupOptions}');
    expect(serversSource).toContain('options={routeOptions}');
    expect(serversSource).not.toContain('<Select mode="tags"');
    expect(serversSource).not.toContain('<Select mode="multiple" placeholder="请选择权限组"');
    expect(serversSource).not.toContain('<Select mode="multiple" placeholder="请选择路由组"');
    expect(serversSource).not.toContain('<Select.Option key={group.id}>');
    expect(serversSource).not.toContain('<Select.Option key={route.id}>');
    expect(serversSource).not.toContain('<Select.Option key={Math.random()} value={node.id}>');
    expect(serversSource).not.toContain('key={`${node.type}-${node.id}`}');
    expect(serversSource).not.toContain('<Select.Option key={group.id} value={group.id}>');
    expect(serversSource).not.toContain('<Select.Option key={route.id} value={route.id}>');
    expect(serversSource).not.toContain('label="Host"');
    expect(serversSource).not.toContain('label="Port"');
    expect(serversSource).not.toContain('label="Server Port"');
  });

  it('uses the original new-node defaults for each server type', () => {
    expect(getLegacyServerInitialValues('vmess')).toEqual({ rate: 1, tls: 0 });
    expect(getLegacyServerInitialValues('shadowsocks')).toEqual({
      rate: 1,
      cipher: 'chacha20-ietf-poly1305',
    });
    expect(getLegacyServerInitialValues('hysteria')).toEqual({
      rate: 1,
      insecure: 0,
      version: 1,
    });
    expect(getLegacyServerInitialValues('vless')).toEqual({
      rate: 1,
      tls: 0,
      flow: null,
    });
    expect(getLegacyServerInitialValues('trojan')).toEqual({ rate: 1, tls: 0 });
    expect(getLegacyServerInitialValues('tuic')).toEqual({
      rate: 1,
      insecure: 0,
      disable_sni: 0,
      udp_relay_mode: 'native',
      zero_rtt_handshake: 0,
      congestion_control: 'cubic',
    });
    expect(getLegacyServerInitialValues('anytls')).toEqual({ rate: 1, insecure: 0 });
    expect(getLegacyServerInitialValues('v2node')).toEqual({
      rate: 1,
      tls: 0,
      network: 'tcp',
      disable_sni: 0,
      zero_rtt_handshake: 0,
      flow: null,
    });
    expect(getLegacyServerInitialValues('vmess')).not.toHaveProperty('show');
    expect(getLegacyServerInitialValues('vmess')).not.toHaveProperty('port');
    expect(getLegacyServerInitialValues('vmess')).not.toHaveProperty('group_id');
    expect(getLegacyServerInitialValues('vmess')).not.toHaveProperty('route_id');
    expect(getLegacyServerInitialValues('vmess')).not.toHaveProperty('network');
    expect(getLegacyServerInitialValues('trojan')).not.toHaveProperty('network');
    expect(getLegacyServerInitialValues('vless')).not.toHaveProperty('network');
    expect(serversSource.match(/name="network" initialValue="tcp"/g)).toHaveLength(2);
  });

  it('keeps edit-node initialization on the saved record instead of applying new-node defaults', () => {
    const values = getLegacyServerInitialValues('shadowsocks', {
      name: 'Tokyo',
      host: 'jp.example.com',
    } as unknown as Parameters<typeof getLegacyServerInitialValues>[1]);

    expect(values).toEqual({
      name: 'Tokyo',
      host: 'jp.example.com',
    });
    expect(values).not.toHaveProperty('rate');
    expect(values).not.toHaveProperty('cipher');
  });

  it('keeps original V2node edit values instead of forcing TLS during initialization', () => {
    expect(
      getLegacyServerInitialValues('v2node', {
        type: 'v2node',
        protocol: 'anytls',
        tls: 0,
        network: 'tcp',
      } as unknown as Parameters<typeof getLegacyServerInitialValues>[1]),
    ).toMatchObject({
      protocol: 'anytls',
      tls: 0,
      network: 'tcp',
    });
    expect(
      getLegacyServerInitialValues('v2node', {
        type: 'v2node',
        protocol: 'hysteria2',
        tls: 0,
      } as unknown as Parameters<typeof getLegacyServerInitialValues>[1]),
    ).toMatchObject({
      protocol: 'hysteria2',
      tls: 0,
    });
  });

  it('matches the original V2node security select fallback protocols', () => {
    expect(getLegacyV2nodeSecurityValue('anytls', 0)).toBe(0);
    expect(getLegacyV2nodeSecurityValue('hysteria2', 0)).toBe(1);
    expect(getLegacyV2nodeSecurityValue('trojan', 0)).toBe(1);
    expect(getLegacyV2nodeSecurityValue('tuic', 0)).toBe(1);
    expect(getLegacyV2nodeSecurityValue('vless', 0)).toBe(0);
    expect(getLegacyV2nodeSecurityValue('anytls', 2)).toBe(2);

    expect(serversSource).toContain(
      "const LEGACY_V2NODE_SECURITY_FALLBACK_PROTOCOLS = ['hysteria2', 'trojan', 'tuic']",
    );
    expect(serversSource).toContain('function getLegacyV2nodeSecurityOptions');
    expect(serversSource).toContain('options={getLegacyV2nodeSecurityOptions(protocolValue)}');
    expect(serversSource).toContain('getValueProps={(value) => ({');
  });

  it('coerces legacy numeric select display values like the original parseInt bindings', () => {
    expect(getLegacyBinarySelectValue('0')).toBe(0);
    expect(getLegacyBinarySelectValue('1')).toBe(1);
    expect(getLegacyBinarySelectValue(2)).toBe(1);
    expect(getLegacyBinarySelectValue(undefined)).toBe(0);
    expect(getLegacyNumericSelectValue('2')).toBe(2);
    expect(getLegacyNumericSelectValue('0', 1)).toBe(1);
    expect(getLegacyNumericSelectValue(undefined, 1)).toBe(1);

    expect(serversSource.match(/getValueProps=\{legacyBinarySelectValueProps\}/g)).toHaveLength(7);
    expect(serversSource).toContain('getValueProps={legacyNumericSelectValueProps}');
    expect(serversSource).toContain(
      'getValueProps={(value) => legacyNumericSelectValueProps(value, 1)}',
    );
    expect(serversSource).toContain('const LEGACY_BINARY_SELECT_OPTIONS: LegacySelectOption[] = [');
    expect(serversSource).toContain('const LEGACY_TLS_SUPPORT_OPTIONS: LegacySelectOption[] = [');
    expect(serversSource).toContain('const LEGACY_SECURITY_NONE_OPTION: LegacySelectOption');
    expect(serversSource).toContain('const LEGACY_SECURITY_TLS_OPTION: LegacySelectOption');
    expect(serversSource).toContain('const LEGACY_SECURITY_REALITY_OPTION: LegacySelectOption');
    expect(serversSource).toContain(
      'const LEGACY_STREAM_NETWORK_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_TROJAN_NETWORK_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_V2NODE_PROTOCOL_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_V2NODE_SHADOWSOCKS_NETWORK_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_V2NODE_TRANSPORT_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_SHADOWSOCKS_CIPHER_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_SHADOWSOCKS_OBFS_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_VLESS_ENCRYPTION_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain('const LEGACY_VLESS_FLOW_OPTIONS: LegacySelectOption[] = [');
    expect(serversSource).toContain(
      'const LEGACY_HYSTERIA_VERSION_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_HYSTERIA_V1_OBFS_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_HYSTERIA2_OBFS_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_TUIC_RELAY_MODE_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain('function getLegacyV2nodeTransportOptions');
    expect(serversSource).toContain('function getLegacyVlessFlowOptions');
    expect(serversSource).not.toContain('import { App, Form, Input, List, Select');
    expect(serversSource).not.toContain('List, Space, Badge');
    expect(serversSource).not.toContain('<Select');
  });

  it('uses the original Shadowsocks-specific drawer fields', () => {
    expect(serversSource).toContain('form: FormInstance');
    expect(serversSource).toContain("Form.useWatch('obfs', form)");
    expect(getLegacyServerInitialValues('shadowsocks').cipher).toBe('chacha20-ietf-poly1305');
    expect(getLegacyServerInitialValues('v2node').cipher).toBeUndefined();
    expect(serversSource).toContain("initialValue={editing ? undefined : 'chacha20-ietf-poly1305'}");
    expect(serversSource).toContain('<Form.Item noStyle name="cipher" initialValue="aes-128-gcm">');
    expect(serversSource).toContain('加密算法');
    expect(serversSource).toContain('aes-128-gcm');
    expect(serversSource).toContain('aes-192-gcm');
    expect(serversSource).toContain('aes-256-gcm');
    expect(serversSource).toContain('chacha20-ietf-poly1305');
    expect(serversSource).toContain('2022-blake3-aes-128-gcm');
    expect(serversSource).toContain('2022-blake3-aes-256-gcm');
    expect(serversSource).toContain('混淆');
    expect(serversSource).toContain('options={LEGACY_SHADOWSOCKS_CIPHER_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_SHADOWSOCKS_OBFS_OPTIONS}');
    expect(serversSource).toContain("{ value: '', label: '无' }");
    expect(serversSource).toContain("{ value: 'http', label: 'HTTP' }");
    expect(serversSource).toContain("shadowsocksObfs === 'http'");
    expect(serversSource).toContain('className="row mt-2"');
    expect(serversSource).toContain('className="form-group col-4 mb-0"');
    expect(serversSource).toContain('className="form-group col-8 mb-0"');
    expect(serversSource).toContain('placeholder="路径"');
    expect(serversSource).toContain('placeholder="Host"');
    expect(serversSource).not.toContain('label="Cipher"');
  });

  it('uses the original V2node-specific listen address, protocol, transport, and install command fields', () => {
    expect(serversSource).toContain("type === 'v2node' ? (");
    expect(serversSource).toContain('连接地址');
    expect(serversSource).toContain('监听地址');
    expect(serversSource).toContain('name="listen_ip"');
    expect(serversSource).toContain('地址或IP默认为0.0.0.0');
    expect(serversSource).toContain('placeholder="服务端开放端口"');
    expect(serversSource).toContain('tls: 0');
    expect(serversSource).toContain("network: 'tcp'");
    expect(serversSource).toContain('flow: null');
    expect(serversSource).toContain('function V2nodeFields');
    expect(serversSource).toContain('节点协议');
    expect(serversSource).toContain('AnyTLS');
    expect(serversSource).toContain('Hysteria2');
    expect(serversSource).toContain('Shadowsocks');
    expect(serversSource).toContain('Trojan');
    expect(serversSource).toContain('Tuic');
    expect(serversSource).toContain('VLess');
    expect(serversSource).toContain('VMess');
    expect(serversSource).toContain('LEGACY_TLS_FORCED_PROTOCOLS');
    expect(serversSource).toContain("showChildDrawer('编辑安全性配置', 'tls_settings')");
    expect(serversSource).toContain('HTTP伪装');
    expect(serversSource).toContain("showChildDrawer('编辑协议配置', 'network_settings')");
    expect(serversSource).toContain('id="v2ray-protocol"');
    expect(serversSource).toContain('协议详细配置');
    expect(serversSource).toContain('https://www.v2ray.com/chapter_02/05_transport.html');
    expect(serversSource).toContain('LEGACY_NETWORK_SETTINGS_PLACEHOLDERS');
    expect(serversSource).toContain('GunService');
    expect(serversSource).toContain('HTTPUpgrade');
    expect(serversSource).toContain('XHTTP');
    expect(serversSource).toContain("showChildDrawer('编辑填充方案', 'padding_scheme')");
    expect(serversSource).toContain('混淆方式obfs');
    expect(serversSource).toContain('salamander');
    expect(serversSource).toContain('客户端启用 0-RTT');
    expect(serversSource).toContain('加密方式');
    expect(serversSource).toContain('XTLS流控算法');
    expect(serversSource).toContain('一键安装指令');
    expect(serversSource).toContain('readOnly');
    expect(serversSource).toContain('<LegacyInput className="ant-input" type="hidden" />');
    expect(serversSource).toContain('<LegacyTextArea');
    expect(serversSource).toContain('className="ant-input"');
    expect(serversSource).toContain("style={{ backgroundColor: '#f5f5f5a0', cursor: 'text' }}");
    expect(serversSource).not.toContain('<Input type="hidden" />');
    expect(serversSource).not.toContain(
      '<Input.TextArea\n                rows={4}\n                readOnly',
    );
    expect(serversSource).toContain('delete payload.install_command');
    expect(serversSource).toContain('options={LEGACY_V2NODE_PROTOCOL_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_V2NODE_SHADOWSOCKS_NETWORK_OPTIONS}');
    expect(serversSource).toContain('options={getLegacyV2nodeTransportOptions(protocolValue)}');
    expect(serversSource).toContain('options={LEGACY_HYSTERIA2_OBFS_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_TUIC_RELAY_MODE_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_SHADOWSOCKS_CIPHER_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_VLESS_ENCRYPTION_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_VLESS_FLOW_OPTIONS}');
    expect(serversSource).toContain("return protocol === 'trojan'");
  });

  it('parses the original protocol JSON fields before saving node payloads', () => {
    const nodeDrawerSource = serversSource.slice(
      serversSource.indexOf('function NodeEditDrawer'),
      serversSource.indexOf('function parseLegacyJsonPayloadField'),
    );

    expect(serversSource).toContain('function prepareLegacyServerPayload');
    expect(serversSource).toContain('function parseLegacyJsonPayloadField');
    expect(serversSource).toContain("parseLegacyJsonPayloadField(payload, 'networkSettings')");
    expect(serversSource).toContain("parseLegacyJsonPayloadField(payload, 'network_settings')");
    expect(serversSource).toContain(
      "if (type === 'trojan' || type === 'vless' || type === 'v2node')",
    );
    expect(serversSource).toContain("if (type === 'vmess')");
    expect(serversSource).toContain('payload.dnsSettings = null');
    expect(serversSource).toContain("message.error('传输协议配置格式有误')");
    expect(serversSource).toContain(
      'const payload = prepareLegacyServerPayload(type, values, id);',
    );
    expect(nodeDrawerSource).toContain('await admin.saveServer(apiClient, type, payload);');
    expect(nodeDrawerSource).toContain('await onSaved?.();');
    expect(nodeDrawerSource).toContain('onClose();');
    expect(
      nodeDrawerSource.indexOf('await admin.saveServer(apiClient, type, payload);'),
    ).toBeLessThan(nodeDrawerSource.indexOf('await onSaved?.();'));
    expect(nodeDrawerSource.indexOf('await onSaved?.();')).toBeLessThan(
      nodeDrawerSource.indexOf('onClose();'),
    );
    expect(nodeDrawerSource).not.toContain('void onSaved?.();');
    expect(serversSource).toContain('onSaved?: () => void | Promise<unknown>;');
    expect(serversSource).toContain('onSaved: () => void | Promise<unknown>;');
    expect(serversSource.match(/onSaved=\{\(\) => nodes\.refetch\(\)\}/g)).toHaveLength(3);
    expect(nodeDrawerSource).not.toContain("message.success(t('common.success'))");
    expect(nodeDrawerSource).not.toContain("message.success('操作成功')");
    expect(serversSource).not.toContain('JSON.parse(payload.network_settings)');
    expect(serversSource).not.toContain('payload.tags = payload.tags.length > 0');
    expect(serversSource).not.toContain('payload.route_id = payload.route_id.length > 0');
  });

  it('keeps cancel-only node drawer closes separate from save-triggered node refetches', () => {
    const triggerSource = serversSource.slice(
      serversSource.indexOf('function LegacyNodeEditMenuTrigger'),
      serversSource.indexOf('function ServerManagePage'),
    );

    expect(triggerSource).toContain('onSaved={onSaved}');
    expect(triggerSource).toContain('onClose={() => setOpen(false)}');
    expect(triggerSource).not.toContain('onClose();');
    expect(triggerSource).not.toContain('void nodes.refetch();');
  });

  it('formats the original VMess protocol object before opening the edit drawer', () => {
    const networkSettings = {
      path: '/',
      headers: {
        Host: 'v2ray.com',
      },
    };
    const values = getLegacyServerInitialValues('vmess', {
      networkSettings,
    } as unknown as Parameters<typeof getLegacyServerInitialValues>[1]);

    expect(values.networkSettings).toBe(JSON.stringify(networkSettings, null, 2));
    expect(serversSource).toContain('normalizedRecord.networkSettings');
    expect(serversSource).toContain('normalizedRecord.networkSettings = JSON.stringify(');
  });

  it('uses the original Trojan-specific drawer fields and child config drawer', () => {
    const trojanAllowInsecureSource = serversSource.slice(
      serversSource.indexOf('function TrojanAllowInsecureField'),
      serversSource.indexOf('function ServerInsecureField'),
    );

    expect(serversSource).toContain('showChildDrawer');
    expect(serversSource).toContain('childDrawer.field');
    expect(serversSource).toContain('closable={false}');
    expect(serversSource).toContain('title={childDrawer.title}');
    expect(serversSource).toContain('field={childDrawer.field}');
    expect(serversSource).toContain('name={field}');
    expect(serversSource).toContain(
      "type === 'trojan' ? <TrojanAllowInsecureField /> : <ServerInsecureField />",
    );
    expect(serversSource).toContain('function TrojanAllowInsecureField');
    expect(serversSource).toContain('className="form-group col-md-4 col-xs-12"');
    expect(serversSource).toContain('placeholder="服务端开放端口"');
    expect(serversSource).toContain('允许不安全');
    expect(serversSource).toContain('使用自签名证书需要允许不安全，用户才可以连接');
    expect(serversSource).toContain('placeholder="允许不安全"');
    expect(trojanAllowInsecureSource).toContain(
      '<LegacyTooltip placement="top" title="使用自签名证书需要允许不安全，用户才可以连接">',
    );
    expect(trojanAllowInsecureSource).toContain('<LegacySelect');
    expect(trojanAllowInsecureSource).toContain('options={LEGACY_BINARY_SELECT_OPTIONS}');
    expect(trojanAllowInsecureSource).not.toContain('<Select');
    expect(serversSource).toContain('服务器名称指示(sni)');
    expect(serversSource).toContain('当节点地址与证书不一致时用于证书验证');
    expect(serversSource).toContain('传输协议');
    expect(serversSource).toContain('编辑配置');
    expect(serversSource).toContain("showChildDrawer('编辑协议配置', 'network_settings')");
    expect(serversSource).toContain('placeholder="选择传输协议"');
    expect(serversSource).toContain('options={LEGACY_TROJAN_NETWORK_OPTIONS}');
    expect(serversSource).not.toContain('label="Allow insecure"');
  });

  it('uses the original VMess-specific TLS and protocol drawer fields', () => {
    const vmessTlsSource = serversSource.slice(
      serversSource.indexOf('function VmessTlsField'),
      serversSource.indexOf('function VlessSecurityField'),
    );

    expect(serversSource).toContain("type === 'vmess' ? (");
    expect(serversSource).toContain('function VmessTlsField');
    expect(serversSource).toContain('className="form-group col-md-8 col-xs-12"');
    expect(serversSource).toContain('placeholder="请输入连接地址"');
    expect(serversSource).toContain("showChildDrawer('编辑TLS配置', 'tlsSettings')");
    expect(serversSource).toContain('placeholder="是否支持TLS"');
    expect(serversSource).toContain('不支持');
    expect(serversSource).toContain('支持');
    expect(vmessTlsSource).toContain('<LegacySelect');
    expect(vmessTlsSource).toContain('options={LEGACY_TLS_SUPPORT_OPTIONS}');
    expect(vmessTlsSource).not.toContain('<Select');
    expect(serversSource).toContain("showChildDrawer('编辑协议配置', 'networkSettings')");
    expect(serversSource).toContain('options={LEGACY_STREAM_NETWORK_OPTIONS}');
    expect(serversSource).toContain("{ value: 'kcp', label: 'mKCP' }");
    expect(serversSource).toContain("{ value: 'httpupgrade', label: 'HTTPUpgrade' }");
    expect(serversSource).toContain("{ value: 'xhttp', label: 'XHTTP' }");
  });

  it('uses the original VLess-specific security, protocol, encryption, and flow fields', () => {
    const vlessSecuritySource = serversSource.slice(
      serversSource.indexOf('function VlessSecurityField'),
      serversSource.indexOf('function V2nodeFields'),
    );

    expect(serversSource).toContain("type === 'vmess' || type === 'vless' ? (");
    expect(serversSource).toContain('function VlessSecurityField');
    expect(serversSource).toContain('安全性');
    expect(serversSource).toContain("showChildDrawer('编辑安全性配置', 'tls_settings')");
    expect(vlessSecuritySource).toContain('<LegacySelect');
    expect(vlessSecuritySource).toContain('LEGACY_SECURITY_NONE_OPTION');
    expect(vlessSecuritySource).toContain('LEGACY_SECURITY_TLS_OPTION');
    expect(vlessSecuritySource).toContain('LEGACY_SECURITY_REALITY_OPTION');
    expect(vlessSecuritySource).not.toContain('<Select');
    expect(serversSource).toContain('Reality');
    expect(serversSource).toContain("showChildDrawer('编辑协议配置', 'network_settings')");
    expect(serversSource).toContain('加密方式');
    expect(serversSource).toContain("showChildDrawer('编辑加密配置', 'encryption_settings')");
    expect(serversSource).toContain('placeholder="选择加密方式"');
    expect(serversSource).toContain('options={LEGACY_VLESS_ENCRYPTION_OPTIONS}');
    expect(serversSource).toContain('MLKEM768X25519PLUS');
    expect(serversSource).toContain('XTLS流控算法');
    expect(serversSource).toContain('placeholder="选择XTLS流控算法"');
    expect(serversSource).toContain('options={getLegacyVlessFlowOptions(vlessNetwork)}');
    expect(serversSource).toContain('xtls-rprx-vision');
    expect(serversSource).toContain("const vlessNetwork = Form.useWatch('network', form);");
    expect(serversSource).toContain("return String(network) === 'tcp'");
    expect(serversSource).not.toContain('name="reality_settings" label="Reality settings (JSON)"');
  });

  it('uses the original TLS and encryption child drawer forms instead of raw textareas', () => {
    const serverChildDrawerSource = serversSource.slice(
      serversSource.indexOf('function ServerChildDrawerField'),
      serversSource.indexOf('function LegacyTlsSettingsField'),
    );
    const tlsSettingsSource = serversSource.slice(
      serversSource.indexOf('function LegacyTlsSettingsField'),
      serversSource.indexOf('function LegacyEncryptionSettingsField'),
    );
    const encryptionSettingsSource = serversSource.slice(
      serversSource.indexOf('function LegacyEncryptionSettingsField'),
      serversSource.indexOf('function TrojanAllowInsecureField'),
    );

    expect(serversSource).toContain('LEGACY_TLS_SETTINGS_DEFAULTS');
    expect(serversSource).toContain('LEGACY_ENCRYPTION_SETTINGS_DEFAULTS');
    expect(serversSource).toContain('const LEGACY_TLS_CERT_MODE_OPTIONS: LegacySelectOption[] = [');
    expect(serversSource).toContain(
      'const LEGACY_PROXY_PROTOCOL_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_TLS_FINGERPRINT_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain('const LEGACY_ECH_MODE_OPTIONS: LegacySelectOption[] = [');
    expect(serversSource).toContain(
      'const LEGACY_ENCRYPTION_MODE_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      'const LEGACY_ENCRYPTION_RTT_OPTIONS: LegacySelectOption[] = [',
    );
    expect(serversSource).toContain(
      "import { LegacyAceJsonEditor } from '@/components/legacy-ace-editor';",
    );
    expect(serverChildDrawerSource).toContain(
      "if (field === 'network_settings' || field === 'networkSettings')",
    );
    expect(serverChildDrawerSource).toContain('<LegacyAceJsonEditor');
    expect(serverChildDrawerSource).toContain(
      'placeholder={getLegacyNetworkSettingsPlaceholder(type, network)}',
    );
    expect(serverChildDrawerSource).not.toContain(
      '<LegacyTextArea\n              className="ant-input"\n              rows={8}\n              placeholder={getLegacyNetworkSettingsPlaceholder(type, network)}',
    );
    expect(serversSource).toContain('function LegacyTlsSettingsField');
    expect(tlsSettingsSource).toContain('<LegacySelect');
    expect(tlsSettingsSource).not.toContain('<Select');
    expect(serversSource).toContain("field === 'tls_settings' || field === 'tlsSettings'");
    expect(serversSource).toContain("certApply={field === 'tls_settings'}");
    expect(serversSource).toContain('Server Name(SNI)');
    expect(serversSource).toContain('REALITY必填，与后端保持一致');
    expect(serversSource).toContain('证书模式Cert Mode');
    expect(serversSource).toContain("value={legacySelectValue(value.cert_mode ?? 'self')}");
    expect(tlsSettingsSource).toContain('options={LEGACY_TLS_CERT_MODE_OPTIONS}');
    expect(serversSource).not.toContain("legacyText(value.cert_mode) || 'self'");
    expect(serversSource).toContain('HTTP申请');
    expect(serversSource).toContain('DNS申请');
    expect(serversSource).toContain('无证书(关闭TLS)');
    expect(serversSource).toContain('DNS解析提供商Provider');
    expect(serversSource).toContain('https://go-acme.github.io/lego/dns/index.html');
    expect(serversSource).toContain('证书公钥文件地址Cert File Path');
    expect(serversSource).toContain('证书私钥文件地址Key File Path');
    expect(serversSource).toContain('Server Address');
    expect(serversSource).toContain('Proxy Protocol');
    expect(tlsSettingsSource).toContain('options={LEGACY_PROXY_PROTOCOL_OPTIONS}');
    expect(serversSource).toContain('Private Key');
    expect(serversSource).toContain('Public Key');
    expect(serversSource).toContain('ShortId');
    expect(serversSource).toContain('FingerPrint');
    expect(tlsSettingsSource).toContain('value={legacySelectValue(value.fingerprint)}');
    expect(tlsSettingsSource).toContain('options={LEGACY_TLS_FINGERPRINT_OPTIONS}');
    expect(serversSource).toContain('Reject unknown sni');
    expect(serversSource).toContain('Allow Insecure');
    expect(serversSource).toContain('ECH (Encrypted Client Hello)');
    expect(tlsSettingsSource).toContain('options={LEGACY_ECH_MODE_OPTIONS}');
    expect(serversSource).toContain('Cloudflare 托管 ECH');
    expect(serversSource).toContain('ECH Server Name (伪装域名/外层SNI)');
    expect(serversSource).toContain('function LegacyEncryptionSettingsField');
    expect(encryptionSettingsSource).toContain('<LegacySelect');
    expect(encryptionSettingsSource).not.toContain('<Select');
    expect(serversSource).toContain("field === 'encryption_settings'");
    expect(serversSource).toContain('form.setFieldsValue({ encryption_settings: value });');
    expect(serversSource).toContain('Mode');
    expect(encryptionSettingsSource).toContain('options={LEGACY_ENCRYPTION_MODE_OPTIONS}');
    expect(serversSource).toContain('xorpub');
    expect(serversSource).toContain('random');
    expect(serversSource).toContain('RTT');
    expect(encryptionSettingsSource).toContain('options={LEGACY_ENCRYPTION_RTT_OPTIONS}');
    expect(serversSource).toContain('Ticket time');
    expect(serversSource).toContain('Server Padding');
    expect(serversSource).toContain('Client Padding');
    expect(serversSource).toContain('Password');
  });

  it('uses the original Hysteria-specific version, insecure, obfs, and bandwidth fields', () => {
    const serverInsecureSource = serversSource.slice(
      serversSource.indexOf('function ServerInsecureField'),
      serversSource.indexOf('function VmessTlsField'),
    );

    expect(serversSource).toContain("type === 'hysteria' ||");
    expect(serversSource).toContain("type === 'tuic' ||");
    expect(serversSource).toContain("type === 'anytls' ? (");
    expect(serversSource).toContain('function ServerInsecureField');
    expect(serversSource).toContain('name="insecure"');
    expect(serversSource).toContain('getValueProps={legacyBinarySelectValueProps}');
    expect(serverInsecureSource).toContain(
      '<LegacyTooltip placement="top" title="使用自签名证书需要允许不安全，用户才可以连接">',
    );
    expect(serverInsecureSource).toContain('<LegacySelect');
    expect(serverInsecureSource).toContain('options={LEGACY_BINARY_SELECT_OPTIONS}');
    expect(serverInsecureSource).not.toContain('<Select');
    expect(serversSource).toContain('HYSTERIA版本');
    expect(serversSource).toContain('options={LEGACY_HYSTERIA_VERSION_OPTIONS}');
    expect(serversSource).toContain('v1');
    expect(serversSource).toContain('v2');
    expect(serversSource).toContain('混淆方式obfs');
    expect(serversSource).toContain('options={LEGACY_HYSTERIA_V1_OBFS_OPTIONS}');
    expect(serversSource).toContain('options={LEGACY_HYSTERIA2_OBFS_OPTIONS}');
    expect(serversSource).toContain('xplus');
    expect(serversSource).toContain('salamander');
    expect(serversSource).toContain('混淆密码obfsParam');
    expect(serversSource).toContain('混淆密码obfs_password');
    expect(serversSource).toContain('留空自动生成');
    expect(serversSource).toContain('上行带宽');
    expect(serversSource).toContain('服务端发送带宽,留空或填0使用BBR');
    expect(serversSource).toContain('下行带宽');
    expect(serversSource).toContain('服务端接收带宽,留空或填0使用BBR');
    expect(serversSource).not.toContain('label="Up Mbps"');
    expect(serversSource).not.toContain('label="Down Mbps"');
    expect(serversSource).not.toContain('label="Obfs password"');
  });

  it('uses the original TUIC-specific insecure, SNI, relay, congestion, and 0-RTT fields', () => {
    expect(serversSource).toContain("type === 'tuic'");
    expect(serversSource).toContain('disable_sni: 0');
    expect(serversSource).toContain("udp_relay_mode: 'native'");
    expect(serversSource).toContain('zero_rtt_handshake: 0');
    expect(serversSource).toContain("congestion_control: 'cubic'");
    expect(serversSource).toContain("Form.useWatch('disable_sni', form)");
    expect(serversSource).toContain('禁用SNI');
    expect(serversSource).toContain('name="disable_sni"');
    expect(serversSource).toContain('数据包中继模式');
    expect(serversSource).toContain('name="udp_relay_mode" initialValue="native"');
    expect(serversSource).toContain('options={LEGACY_TUIC_RELAY_MODE_OPTIONS}');
    expect(serversSource).toContain('native');
    expect(serversSource).toContain('quic');
    expect(serversSource).toContain('tuicDisableSni');
    expect(serversSource).toContain('服务器名称指示(sni)');
    expect(serversSource).toContain('拥塞控制算法');
    expect(serversSource).toContain('name="congestion_control" initialValue="cubic"');
    expect(serversSource).toContain('options={LEGACY_TUIC_CONGESTION_CONTROL_OPTIONS}');
    expect(serversSource).toContain('new_reno');
    expect(serversSource).toContain('客户端启用 0-RTT');
    expect(serversSource).toContain('name="zero_rtt_handshake"');
    expect(serversSource).not.toContain('label="SNI"');
    expect(serversSource).not.toContain('label="ALPN"');
    expect(serversSource).not.toContain('label="Congestion control"');
  });

  it('uses the original AnyTLS-specific SNI and padding scheme child drawer', () => {
    expect(serversSource).toContain("type === 'anytls'");
    expect(serversSource).toContain(
      "const anyTlsDefaults = type === 'anytls' ? { insecure: 0 } : {};",
    );
    expect(serversSource).toContain("showChildDrawer('编辑填充方案', 'padding_scheme')");
    expect(serversSource).toContain('编辑填充方案');
    expect(serversSource).toContain('function ServerChildDrawerField');
    expect(serversSource).toContain("field === 'padding_scheme'");
    expect(serversSource).toContain('id="anytls-padding-scheme"');
    expect(serversSource).toContain('ANYTLS_PADDING_SCHEME_PLACEHOLDER');
    expect(serversSource).toContain('stop=8');
    expect(serversSource).toContain('0=30-30');
    expect(serversSource).toContain('7=500-1000');
    expect(serversSource).not.toContain('label="Padding scheme"');
    expect(serversSource).not.toContain('label="Insecure"');
  });

  it('preserves the original /server/manage row right-click menu outside sort mode', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(serversSource).toContain('id="v2board-table-dropdown"');
    expect(serversSource).toContain(
      'ant-dropdown-menu ant-dropdown-menu-light ant-dropdown-menu-root ant-dropdown-menu-vertical',
    );
    expect(serversSource).toContain('sortMode');
    expect(serversSource).toContain('sortMode\n      ? {}');
    expect(serversSource).toContain(
      'onContextMenu: (event: ReactMouseEvent<HTMLTableRowElement>) =>',
    );
    expect(serversSource).toContain('event.preventDefault()');
    expect(serversSource).toContain('event.clientY');
    expect(serversSource).toContain('event.clientX');
    expect(serversSource).toContain("display: contextMenu && !sortMode ? 'unset' : 'none'");
    expect(serversSource).toContain('<LegacyFormIcon /> 编辑');
    expect(serversSource).toContain('<LegacyCopyIcon /> 复制');
    expect(serversSource).toContain('<LegacyDeleteIcon /> 删除');
    expect(managePageSource).toContain('{contextDropdown}');
    expect(managePageSource.indexOf('{contextDropdown}')).toBeGreaterThan(
      managePageSource.indexOf('<div className="ant-table-wrapper">'),
    );
    expect(managePageSource.indexOf('{contextDropdown}')).toBeLessThan(
      managePageSource.indexOf('</LegacyDragSort>'),
    );
    expect(serversSource).not.toContain('<FormOutlined /> 编辑');
    expect(serversSource).toContain("runNodeAction('copy', contextRecord)");
    expect(serversSource).toContain("runNodeAction('delete', contextRecord)");
  });

  it('keeps standalone server pages under the original refetch loading mask', () => {
    expect(serversSource).toContain('<LegacySpin loading={groups.isFetching}>');
    expect(serversSource).toContain('<LegacySpin loading={routes.isFetching}>');
    expect(serversSource).toContain(
      '<LegacySpin loading={legacyFetchLoading(nodes.isFetching, nodes.error) || sortingLoading}>',
    );
    expect(serversSource).not.toContain('<LegacySpin loading={groups.isLoading}>');
    expect(serversSource).not.toContain('<LegacySpin loading={routes.isLoading}>');
    expect(serversSource).not.toContain('<LegacySpin loading={nodes.isLoading}>');
  });

  it('preserves the legacy remembered server table page size habit', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );
    const wrapperSource = managePageSource.slice(
      managePageSource.indexOf('<div className="ant-table-wrapper">'),
      managePageSource.indexOf('{contextDropdown}'),
    );

    expect(serversSource).toContain("const LEGACY_HABIT_KEY = 'habit'");
    expect(serversSource).toContain(
      "const LEGACY_SERVER_PAGE_SIZE_KEY = 'server_manage_page_size'",
    );
    expect(serversSource).toContain('function readLegacyServerPageSize()');
    expect(serversSource).toContain('useState(readLegacyServerPageSize)');
    expect(serversSource).toContain('LegacyTablePagination');
    expect(serversSource).toContain('mini={false}');
    expect(serversSource).toContain(
      'filteredNodes.slice((activePage - 1) * pageSize, activePage * pageSize);',
    );
    expect(serversSource).toContain(
      'const changeServerPagination = (pagination: LegacyTablePaginationChange) =>',
    );
    expect(serversSource).toContain(
      'writeLegacyHabit(LEGACY_SERVER_PAGE_SIZE_KEY, pagination.pageSize)',
    );
    expect(wrapperSource).toContain('<LegacyTablePagination');
    expect(serversSource).toContain(
      'const legacyHabit = stored as unknown as Record<string, unknown>;',
    );
    expect(serversSource).toContain('legacyHabit[key] = value;');
    expect(serversSource).toContain(
      'window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify(legacyHabit));',
    );
    expect(serversSource).toContain(
      'window.localStorage.setItem(LEGACY_HABIT_KEY, JSON.stringify({ [key]: value }));',
    );
    expect(serversSource).not.toContain('const parsed = stored ? JSON.parse(stored) : {};');
    expect(serversSource).not.toContain('next[key] = value;');
  });

  it('uses the old copy helper for server address copying', () => {
    expect(serversSource).toContain("import { legacyCopyText } from '@/lib/legacy-copy';");
    expect(serversSource).toContain('legacyCopyText(node.host)');
    expect(serversSource).not.toContain('legacyCopyText(`${node.host}:${node.port}`)');
    expect(serversSource).not.toContain("message.success('复制成功')");
    expect(serversSource).not.toContain('navigator.clipboard?.writeText');
  });

  it('builds the original grouped server sort payload from the full node order', () => {
    const nodes = [
      { id: 1, type: 'shadowsocks' },
      { id: 9, type: 'vmess' },
      { id: 3, type: 'shadowsocks' },
    ] as Parameters<typeof createServerSortPayload>[0];

    expect(createServerSortPayload(nodes)).toEqual({
      shadowsocks: { 1: 0, 3: 2 },
      vmess: { 9: 1 },
    });
    expect(serversSource).toContain('sort.mutate(createServerSortPayload(orderedNodes)');
    expect(serversSource).toContain('void nodes.refetch();');
    expect(serversSource).not.toContain('sort.mutate(createServerSortPayload(filteredNodes)');
  });

  it('keeps the old server-manage sort-mode lifecycle', () => {
    expect(serversSource).toContain('const [sortMode, setSortMode] = useState(false)');
    expect(serversSource).toContain('if (nodes.data) {');
    expect(serversSource).toContain('setOrderedNodes(nodes.data)');
    expect(serversSource).toContain('setSortMode(false)');
    expect(serversSource).toContain("{sortMode ? '保存排序' : '编辑排序'}");
    expect(serversSource).toContain(
      'onSuccess: () => {\n                      void nodes.refetch();\n                    },',
    );
    expect(serversSource).not.toContain('onSuccess: () => setSortMode(false)');
  });

  it('keeps server-manage node mutations fetching from the page after success', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );
    const nodeHooksSource = queriesSource.slice(
      queriesSource.indexOf('export function useDropServerMutation()'),
      queriesSource.length,
    );

    expect(managePageSource).toContain('update.mutate(');
    expect(managePageSource).toContain('copy.mutate(');
    expect(managePageSource).toContain('drop.mutate(');
    expect(managePageSource).toContain('sort.mutate(createServerSortPayload(orderedNodes), {');
    expect(managePageSource.match(/void nodes\.refetch\(\);/g)?.length).toBeGreaterThanOrEqual(4);
    expect(managePageSource.match(/onSaved=\{\(\) => nodes\.refetch\(\)\}/g)).toHaveLength(3);
    expect(nodeHooksSource).not.toContain('onSuccess');
    expect(nodeHooksSource).not.toContain('adminKeys.serverNodes');
  });

  it('keeps the original uncontrolled server-manage search input', () => {
    const managePageSource = serversSource.slice(
      serversSource.indexOf('function ServerManagePage'),
      serversSource.indexOf('function NodeEditDrawer'),
    );

    expect(managePageSource).toContain(
      'const [searchKey, setSearchKey] = useState<string | undefined>()',
    );
    expect(managePageSource).toContain('<LegacyInput');
    expect(managePageSource).toContain('placeholder="输入任意关键字搜索"');
    expect(managePageSource).toContain('className="ant-input ml-2"');
    expect(managePageSource).toContain('setSearchKey(event.target.value);');
    expect(managePageSource).toContain('setCurrentPage(1);');
    expect(managePageSource).not.toContain(
      '<Input\n              placeholder="输入任意关键字搜索"',
    );
    expect(managePageSource).not.toContain('value={searchKey}');
  });

  it('restores the original unsaved server sort navigation prompt', () => {
    mocks.pathname = '/server/manage';
    const html = renderToStaticMarkup(<ServersPage />);

    expect(html).toContain('block block-bottom undefined');
    expect(serversSource).toContain(
      "const LEGACY_SERVER_SORT_PROMPT = '节点排序还没有保存，是否离开'",
    );
    expect(serversSource).toContain('<LegacyServerSortPrompt when={sortMode} />');
    expect(serversSource).toContain('installLegacyServerSortPrompt()');
    expect(serversSource).toContain(
      "document.addEventListener('click', warnBeforeRouteClick, true)",
    );
    expect(serversSource).toContain("window.addEventListener('hashchange', warnBeforeHashChange)");
    expect(serversSource).toContain("window.addEventListener('beforeunload', warnBeforeUnload)");
    expect(serversSource).toContain('event.returnValue = message');
    expect(serversSource).not.toContain('unstable_usePrompt as usePrompt');
  });

  it('prompts for legacy server sort route clicks without blocking page-local controls', () => {
    const navLink = document.createElement('a');
    navLink.className = 'nav-main-link';
    navLink.appendChild(document.createElement('span'));
    document.body.appendChild(navLink);

    const pageButton = document.createElement('button');
    document.body.appendChild(pageButton);

    expect(shouldPromptLegacyServerSortClick(navLink.firstElementChild)).toBe(true);
    expect(shouldPromptLegacyServerSortClick(pageButton)).toBe(false);
  });

  it('cancels legacy server sort route clicks before React navigation runs', () => {
    const confirm = vi.fn(() => false);
    Object.defineProperty(window, 'confirm', { configurable: true, value: confirm });
    const dispose = installLegacyServerSortPrompt('节点排序还没有保存，是否离开');
    const navLink = document.createElement('a');
    navLink.className = 'nav-main-link';
    document.body.appendChild(navLink);
    const click = new MouseEvent('click', { bubbles: true, cancelable: true });

    expect(navLink.dispatchEvent(click)).toBe(false);
    expect(click.defaultPrevented).toBe(true);
    expect(confirm).toHaveBeenCalledWith('节点排序还没有保存，是否离开');

    dispose();
  });

  it('restores the previous hash when the legacy server sort prompt rejects a hash transition', () => {
    const confirm = vi.fn(() => false);
    Object.defineProperty(window, 'confirm', { configurable: true, value: confirm });
    window.location.hash = '#/server/manage';
    const dispose = installLegacyServerSortPrompt('节点排序还没有保存，是否离开');

    window.location.hash = '#/dashboard';
    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(window.location.hash).toBe('#/server/manage');
    expect(confirm).toHaveBeenCalledWith('节点排序还没有保存，是否离开');

    dispose();
  });

  it('reorders server rows with the old sortable-table index behavior', () => {
    const nodes = [
      { id: 1, type: 'shadowsocks' },
      { id: 2, type: 'vmess' },
      { id: 3, type: 'trojan' },
    ] as Parameters<typeof moveServerNodeByLegacyDragIndexes>[0];

    expect(moveServerNodeByLegacyDragIndexes(nodes, 0, 2).map((node) => node.id)).toEqual([
      2, 3, 1,
    ]);
    expect(moveServerNodeByLegacyDragIndexes(nodes, 2, 0).map((node) => node.id)).toEqual([
      3, 1, 2,
    ]);
    expect(serversSource).toContain(
      'onDragEnd={(fromIndex, toIndex) => sortServerNodes(fromIndex, toIndex)}',
    );
    expect(serversSource).toContain(
      'setOrderedNodes(moveServerNodeByLegacyDragIndexes(orderRef.current, fromIndex, toIndex));',
    );
    expect(serversSource).not.toContain('components={sortMode ? sortComponents : undefined}');
  });
});
