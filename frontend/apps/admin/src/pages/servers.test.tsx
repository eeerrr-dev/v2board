import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ServersPage, {
  applyServerNodeColumnControls,
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

// The admin server manager is a redesigned pure shadcn island (route dispatch →
// PageHeader + DataTable + Dialog/Sheet editors). Legacy ant-table / OneUI /
// LegacyModal DOM byte-pins and the `serversSource` string assertions are
// retired. What stays covered is the permanent Tier-1 contract that real proxy
// nodes and the shared backend consume: the per-protocol node-config field
// keys/defaults/coercions (the exported pure helpers), the node/group/route save
// payloads, route match parsing, and the grouped sort payload.

const GROUPS = [
  { id: 1, name: 'VIP', user_count: 12, server_count: 3, created_at: 1, updated_at: 1 },
];

const NODES = [
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
];

const ROUTES = [
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
];

const mocks = vi.hoisted(() => ({
  pathname: '/server/group',
  saveGroup: vi.fn(),
  dropGroup: vi.fn(),
  saveRoute: vi.fn(),
  dropRoute: vi.fn(),
  dropServer: vi.fn(),
  copyServer: vi.fn(),
  updateServer: vi.fn(),
  sortServer: vi.fn(),
  saveServer: vi.fn(),
  refetch: vi.fn(),
  confirm: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

const defaultUserAgent = window.navigator.userAgent;

vi.mock('react-router', () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/lib/api', () => ({ apiClient: {} }));

vi.mock('@v2board/api-client', () => ({ admin: { saveServer: mocks.saveServer } }));

vi.mock('@/lib/toast', () => ({
  toast: { success: mocks.toastSuccess, error: mocks.toastError, loading: vi.fn(), dismiss: vi.fn() },
}));

vi.mock('@/components/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/queries', () => ({
  useServerGroups: () => ({ isFetching: false, refetch: mocks.refetch, data: GROUPS }),
  useServerNodes: () => ({ isFetching: false, refetch: mocks.refetch, data: NODES }),
  useServerRoutes: () => ({ isFetching: false, refetch: mocks.refetch, data: ROUTES }),
  useSaveServerGroupMutation: () => ({ isPending: false, mutateAsync: mocks.saveGroup }),
  useDropServerGroupMutation: () => ({ mutate: mocks.dropGroup }),
  useSaveServerRouteMutation: () => ({ isPending: false, mutateAsync: mocks.saveRoute }),
  useDropServerRouteMutation: () => ({ mutate: mocks.dropRoute }),
  useDropServerMutation: () => ({ mutate: mocks.dropServer }),
  useCopyServerMutation: () => ({ mutate: mocks.copyServer }),
  useUpdateServerMutation: () => ({ mutate: mocks.updateServer }),
  useSortServerNodesMutation: () => ({ mutate: mocks.sortServer }),
}));

function setUserAgent(value: string) {
  Object.defineProperty(window.navigator, 'userAgent', { configurable: true, value });
}

beforeEach(() => {
  mocks.pathname = '/server/group';
  setUserAgent(defaultUserAgent);
  document.body.innerHTML = '';
  mocks.saveGroup.mockReset().mockResolvedValue(undefined);
  mocks.dropGroup.mockReset();
  mocks.saveRoute.mockReset().mockResolvedValue(undefined);
  mocks.dropRoute.mockReset();
  mocks.dropServer.mockReset();
  mocks.copyServer.mockReset();
  mocks.updateServer.mockReset();
  mocks.sortServer.mockReset();
  mocks.saveServer.mockReset().mockResolvedValue(undefined);
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.confirm.mockReset().mockResolvedValue(true);
  mocks.toastSuccess.mockReset();
  mocks.toastError.mockReset();
  // Radix Select / DropdownMenu pointer + scroll shims for happy-dom.
  window.HTMLElement.prototype.scrollIntoView = vi.fn();
  window.HTMLElement.prototype.hasPointerCapture = vi.fn(() => false);
  window.HTMLElement.prototype.setPointerCapture = vi.fn();
  window.HTMLElement.prototype.releasePointerCapture = vi.fn();
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// Server groups — shadcn island DOM + save payload.
// ---------------------------------------------------------------------------

describe('ServerGroupPage (shadcn island)', () => {
  beforeEach(() => {
    mocks.pathname = '/server/group';
  });

  it('renders the permission group table with counts', () => {
    render(<ServersPage />);
    const table = screen.getByTestId('server-groups-table');
    expect(within(table).getByText('VIP')).toBeInTheDocument();
    expect(within(table).getByText('12')).toBeInTheDocument();
    expect(within(table).getByText('3')).toBeInTheDocument();
    expect(within(table).getByText('组名称')).toBeInTheDocument();
  });

  it('submits the group save payload from the create dialog', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-group-create'));
    fireEvent.change(await screen.findByTestId('server-group-name'), {
      target: { value: 'New Group' },
    });
    await user.click(screen.getByTestId('server-group-submit'));
    await waitFor(() =>
      expect(mocks.saveGroup).toHaveBeenCalledWith(expect.objectContaining({ name: 'New Group' })),
    );
  });

  it('drops a group only after confirmation', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-group-delete-1'));
    await waitFor(() => expect(mocks.dropGroup).toHaveBeenCalledWith(1, expect.anything()));
  });
});

// ---------------------------------------------------------------------------
// Server routes — shadcn island DOM + match parsing + save payload.
// ---------------------------------------------------------------------------

describe('ServerRoutePage (shadcn island)', () => {
  beforeEach(() => {
    mocks.pathname = '/server/route';
  });

  it('renders route rows with match counts and action text', () => {
    render(<ServersPage />);
    const table = screen.getByTestId('server-routes-table');
    expect(within(table).getByText('Netflix')).toBeInTheDocument();
    expect(within(table).getByText('匹配 2 条规则')).toBeInTheDocument();
    expect(within(table).getByText('无规则时默认')).toBeInTheDocument();
    expect(within(table).getByText('指定出站服务器(域名目标)')).toBeInTheDocument();
    expect(within(table).getByText('自定义默认出站')).toBeInTheDocument();
  });

  it('parses the multiline match value into a filtered array on save', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-route-create'));
    fireEvent.change(await screen.findByTestId('server-route-remarks'), {
      target: { value: 'Rule' },
    });
    fireEvent.change(screen.getByTestId('server-route-match'), {
      target: { value: 'a.com\n\nb.com' },
    });
    await user.click(screen.getByTestId('server-route-submit'));
    await waitFor(() =>
      expect(mocks.saveRoute).toHaveBeenCalledWith(
        expect.objectContaining({ remarks: 'Rule', match: ['a.com', 'b.com'] }),
      ),
    );
  });

  it('keeps the existing match array when editing without changes', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-route-edit-1'));
    await user.click(await screen.findByTestId('server-route-submit'));
    await waitFor(() =>
      expect(mocks.saveRoute).toHaveBeenCalledWith(
        expect.objectContaining({ match: ['geosite:netflix', 'domain:example.com'], action: 'route' }),
      ),
    );
  });
});

// ---------------------------------------------------------------------------
// Server nodes — shadcn island DOM + mutation dispatch shapes.
// ---------------------------------------------------------------------------

describe('ServerManagePage (shadcn island)', () => {
  beforeEach(() => {
    mocks.pathname = '/server/manage';
  });

  it('renders node rows with parent formatting, address and online count', () => {
    render(<ServersPage />);
    const table = screen.getByTestId('server-nodes-table');
    expect(within(table).getByText('Tokyo')).toBeInTheDocument();
    expect(within(table).getByText('example.com:443')).toBeInTheDocument();
    expect(within(table).getByText('8')).toBeInTheDocument();
    // Child node carries a parent id → "id => parent_id".
    expect(within(table).getByText('2 => 1')).toBeInTheDocument();
  });

  it('dispatches the show toggle with the original key/value shape', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByLabelText('切换「Tokyo」显隐'));
    expect(mocks.updateServer).toHaveBeenCalledWith(
      { type: 'shadowsocks', id: 1, key: 'show', value: 0 },
      expect.anything(),
    );
  });

  it('copies the node host to the clipboard with the success toast', async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(<ServersPage />);
    await user.click(screen.getByText('example.com:443'));
    expect(writeText).toHaveBeenCalledWith('example.com');
    expect(mocks.toastSuccess).toHaveBeenCalledWith('复制成功');
  });

  it('copies a node through the row action menu', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('node-actions-1'));
    await user.click(await screen.findByTestId('node-copy-1'));
    expect(mocks.copyServer).toHaveBeenCalledWith({ type: 'shadowsocks', id: 1 }, expect.anything());
  });

  it('deletes a node only after confirmation', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('node-actions-1'));
    await user.click(await screen.findByTestId('node-delete-1'));
    await waitFor(() =>
      expect(mocks.dropServer).toHaveBeenCalledWith(
        { type: 'shadowsocks', id: 1 },
        expect.anything(),
      ),
    );
  });

  it('saves a new Shadowsocks node with its per-type default field set', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('node-add'));
    await user.click(await screen.findByTestId('node-add-shadowsocks'));
    fireEvent.change(await screen.findByTestId('node-name'), { target: { value: 'JP' } });
    fireEvent.change(screen.getByTestId('node-host'), { target: { value: 'jp.example.com' } });
    fireEvent.change(screen.getByTestId('node-port'), { target: { value: '443' } });
    fireEvent.change(screen.getByTestId('node-server-port'), { target: { value: '8443' } });
    await user.click(screen.getByTestId('node-submit'));
    await waitFor(() =>
      expect(mocks.saveServer).toHaveBeenCalledWith(
        expect.anything(),
        'shadowsocks',
        expect.objectContaining({
          name: 'JP',
          host: 'jp.example.com',
          port: '443',
          server_port: '8443',
          rate: 1,
          cipher: 'chacha20-ietf-poly1305',
          obfs: '',
        }),
      ),
    );
  });

  it('builds the grouped sort payload from the reordered node list', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('node-sort-toggle'));
    // Move the first row (shadowsocks id 1) down past the vmess row.
    const [firstMoveDown] = screen.getAllByLabelText('下移');
    await user.click(firstMoveDown!);
    await user.click(screen.getByTestId('node-sort-toggle'));
    expect(mocks.sortServer).toHaveBeenCalledWith(
      { vmess: { 2: 0 }, shadowsocks: { 1: 1 } },
      expect.anything(),
    );
  });
});

// ---------------------------------------------------------------------------
// Pure contract helpers (Tier-1) — preserved unchanged from the replica suite.
// ---------------------------------------------------------------------------

describe('server node config contract helpers', () => {
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
  });

  it('coerces legacy numeric select display values like the original parseInt bindings', () => {
    expect(getLegacyBinarySelectValue('0')).toBe(0);
    expect(getLegacyBinarySelectValue('1')).toBe(1);
    expect(getLegacyBinarySelectValue(2)).toBe(1);
    expect(getLegacyBinarySelectValue(undefined)).toBe(0);
    expect(getLegacyNumericSelectValue('2')).toBe(2);
    expect(getLegacyNumericSelectValue('0', 1)).toBe(1);
    expect(getLegacyNumericSelectValue(undefined, 1)).toBe(1);
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
  });
});

// ---------------------------------------------------------------------------
// Unsaved-sort navigation prompt — behavioral, unchanged from the replica suite.
// ---------------------------------------------------------------------------

describe('legacy server sort navigation prompt', () => {
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
});

// ---------------------------------------------------------------------------
// Node table column controls — pure filter/sort behavior, unchanged.
// ---------------------------------------------------------------------------

describe('applyServerNodeColumnControls (node table sort/filter)', () => {
  const nodes = [
    { id: 1, type: 'shadowsocks', group_id: ['1'], online: 8 },
    { id: 2, type: 'vmess', group_id: ['2'], online: 0 },
    { id: 3, type: 'trojan', group_id: ['1', '2'], online: 4 },
  ] as Parameters<typeof applyServerNodeColumnControls>[0];
  const ids = (list: (typeof nodes)[number][]) => list.map((node) => node.id);

  it('filters by type matching node.type to the lowercased column label', () => {
    expect(
      ids(applyServerNodeColumnControls(nodes, { typeFilter: ['Shadowsocks'], groupFilter: [], onlineSort: '' })),
    ).toEqual([1]);
    expect(
      ids(
        applyServerNodeColumnControls(nodes, {
          typeFilter: ['Vmess', 'Trojan'],
          groupFilter: [],
          onlineSort: '',
        }),
      ),
    ).toEqual([2, 3]);
  });

  it('filters by group membership as strings, OR-combined across selected groups', () => {
    expect(
      ids(applyServerNodeColumnControls(nodes, { typeFilter: [], groupFilter: ['2'], onlineSort: '' })),
    ).toEqual([2, 3]);
    expect(
      ids(applyServerNodeColumnControls(nodes, { typeFilter: [], groupFilter: ['1'], onlineSort: '' })),
    ).toEqual([1, 3]);
  });

  it('sorts by online ascending/descending and preserves source order when unsorted', () => {
    expect(
      applyServerNodeColumnControls(nodes, { typeFilter: [], groupFilter: [], onlineSort: 'ascend' }).map(
        (node) => node.online,
      ),
    ).toEqual([0, 4, 8]);
    expect(
      applyServerNodeColumnControls(nodes, { typeFilter: [], groupFilter: [], onlineSort: 'descend' }).map(
        (node) => node.online,
      ),
    ).toEqual([8, 4, 0]);
    expect(
      ids(applyServerNodeColumnControls(nodes, { typeFilter: [], groupFilter: [], onlineSort: '' })),
    ).toEqual([1, 2, 3]);
  });

  it('applies filters before the sort like the legacy antd table', () => {
    expect(
      ids(
        applyServerNodeColumnControls(nodes, {
          typeFilter: ['Shadowsocks', 'Trojan'],
          groupFilter: ['1'],
          onlineSort: 'ascend',
        }),
      ),
    ).toEqual([3, 1]);
  });
});
