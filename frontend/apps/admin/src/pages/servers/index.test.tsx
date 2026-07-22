import { useSyncExternalStore } from 'react';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ServersPage, {
  applyServerNodeColumnControls,
  createServerSortPayload,
  getBinarySelectValue,
  getNetworkSettingsPlaceholder,
  getNumericSelectValue,
  getV2nodeSecurityValue,
  moveServerNodeByDragIndexes,
} from './index';
import { serverNodeFormSchema, switchV2nodeProtocol, type V2nodeEditorValues } from './form-schema';
import { zhCN } from '@v2board/i18n/testing';
import { createTestTranslation } from '@/test/i18next-selector';

// The admin server manager is a redesigned pure shadcn island (route dispatch →
// PageHeader + DataTable + Dialog/Sheet editors). Retired ant-table / OneUI /
// modal DOM byte-pins and the `serversSource` string assertions are
// retired. What stays covered is the permanent Tier-1 contract that real proxy
// nodes and the shared backend consume: the per-protocol node-config field
// keys/defaults/coercions (the exported pure helpers), the node/group/route save
// payloads, route match parsing, and the grouped sort payload.

// Query data arrives through the dialect-v2 projections (§6.7, W13): boolean
// show, numeric rate/port, integer id arrays, RFC 3339 timestamps.
const GROUPS = [
  {
    id: 1,
    name: 'VIP',
    user_count: 12,
    server_count: 3,
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
  },
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
    show: true,
    rate: 1,
    parent_id: null,
    online: 8,
    last_check_at: null,
    last_push_at: null,
    available_status: 2,
    api_key: null,
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
    show: false,
    rate: 2,
    parent_id: 1,
    online: 0,
    last_check_at: null,
    last_push_at: null,
    available_status: 0,
    api_key: null,
  },
];

const ROUTES = [
  {
    id: 1,
    remarks: 'Netflix',
    match: ['geosite:netflix', 'domain:example.com'],
    action: 'route',
    action_value: '{}',
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
  },
  {
    id: 2,
    remarks: 'Default',
    match: [],
    action: 'default_out',
    action_value: '{}',
    created_at: '2023-11-14T22:13:20Z',
    updated_at: '2023-11-14T22:13:20Z',
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
  groupsData: undefined as typeof GROUPS | undefined,
  nodesData: undefined as typeof NODES | undefined,
  routesData: undefined as typeof ROUTES | undefined,
  groupsError: false,
  nodesError: false,
  routesError: false,
  blockerState: 'unblocked' as 'unblocked' | 'blocked' | 'proceeding',
  blockerListeners: new Set<() => void>(),
  blockerProceed: vi.fn(),
  blockerReset: vi.fn(),
  useBlocker: vi.fn(),
  beforeUnload: undefined as ((event: BeforeUnloadEvent) => void) | undefined,
  useBeforeUnload: vi.fn(),
}));

const defaultUserAgent = window.navigator.userAgent;

vi.mock('react-router', () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
  useBlocker: mocks.useBlocker,
  useBeforeUnload: mocks.useBeforeUnload,
}));

// The mock resolves selectors and runtime keys against the flattened real
// zh-CN tree so rendered copy (and zod messages routed through FieldError →
// translateRuntimeMessage) stays byte-identical to the pre-i18n strings.
function flattenTranslations(tree: object, prefix = ''): Record<string, string> {
  const labels: Record<string, string> = {};
  for (const [key, value] of Object.entries(tree) as [string, unknown][]) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') labels[path] = value;
    else if (value && typeof value === 'object') {
      Object.assign(labels, flattenTranslations(value, path));
    }
  }
  return labels;
}

let zhCnLabels: Record<string, string> | undefined;

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation((zhCnLabels ??= flattenTranslations(zhCN))),
}));

vi.mock('@v2board/app-shell/toast', () => ({
  toast: {
    success: mocks.toastSuccess,
    error: mocks.toastError,
    loading: vi.fn(),
    dismiss: vi.fn(),
  },
}));

vi.mock('@v2board/ui/confirm-dialog', () => ({ confirmDialog: mocks.confirm }));

vi.mock('@/lib/queries', () => ({
  useServerGroups: () => ({
    isFetching: false,
    isError: mocks.groupsError,
    isSuccess: !mocks.groupsError && mocks.groupsData !== undefined,
    refetch: mocks.refetch,
    data: mocks.groupsData,
  }),
  useServerNodes: () => ({
    isFetching: false,
    isError: mocks.nodesError,
    isSuccess: !mocks.nodesError && mocks.nodesData !== undefined,
    refetch: mocks.refetch,
    data: mocks.nodesData,
  }),
  useServerRoutes: () => ({
    isFetching: false,
    isError: mocks.routesError,
    isSuccess: !mocks.routesError && mocks.routesData !== undefined,
    refetch: mocks.refetch,
    data: mocks.routesData,
  }),
  useSaveServerGroupMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveGroup(payload)).then(options?.onSuccess, () => undefined);
    },
  }),
  useDropServerGroupMutation: () => ({ mutate: mocks.dropGroup }),
  useSaveServerRouteMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveRoute(payload)).then(options?.onSuccess, () => undefined);
    },
  }),
  useDropServerRouteMutation: () => ({ mutate: mocks.dropRoute }),
  useDropServerMutation: () => ({ mutate: mocks.dropServer }),
  useCopyServerMutation: () => ({ mutate: mocks.copyServer }),
  useUpdateServerMutation: () => ({ mutate: mocks.updateServer }),
  useSortServerNodesMutation: () => ({ mutate: mocks.sortServer }),
  useSaveServerMutation: () => ({
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.saveServer(payload)).then(options?.onSuccess, () => undefined);
    },
  }),
}));

function setUserAgent(value: string) {
  Object.defineProperty(window.navigator, 'userAgent', { configurable: true, value });
}

beforeEach(() => {
  mocks.pathname = '/server/group';
  mocks.groupsData = GROUPS;
  mocks.nodesData = NODES;
  mocks.routesData = ROUTES;
  mocks.groupsError = false;
  mocks.nodesError = false;
  mocks.routesError = false;
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
  mocks.blockerState = 'unblocked';
  mocks.blockerListeners.clear();
  mocks.blockerProceed.mockReset();
  mocks.blockerReset.mockReset();
  // Like the real useBlocker, the mock self-subscribes to blocker state: the
  // compiled guard bails out of prop-unchanged parent re-renders, so a mutable
  // module read without a subscription would never repaint the leave dialog.
  mocks.useBlocker.mockReset().mockImplementation(() => ({
    state: useSyncExternalStore(
      (onStoreChange) => {
        mocks.blockerListeners.add(onStoreChange);
        return () => mocks.blockerListeners.delete(onStoreChange);
      },
      () => mocks.blockerState,
    ),
    location: undefined,
    proceed: mocks.blockerProceed,
    reset: mocks.blockerReset,
  }));
  mocks.beforeUnload = undefined;
  mocks.useBeforeUnload
    .mockReset()
    .mockImplementation((callback: (event: BeforeUnloadEvent) => void) => {
      mocks.beforeUnload = callback;
    });
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

function setBlockerState(next: typeof mocks.blockerState) {
  mocks.blockerState = next;
  act(() => {
    for (const notify of mocks.blockerListeners) notify();
  });
}

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

  it('does not render the empty state when the group query failed', () => {
    mocks.groupsData = undefined;
    mocks.groupsError = true;
    render(<ServersPage />);

    expect(screen.getByText('权限组加载失败')).toBeInTheDocument();
    expect(screen.queryByTestId('server-groups-empty')).not.toBeInTheDocument();
  });

  it('submits the group save payload from the create dialog', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-group-create'));
    fireEvent.change(await screen.findByTestId('server-group-name'), {
      target: { value: 'New Group' },
    });
    await user.click(screen.getByTestId('server-group-submit'));
    await waitFor(() => expect(mocks.saveGroup).toHaveBeenCalledWith({ name: 'New Group' }));
  });

  it('submits the exact group edit payload with its record id', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('server-group-edit-1'));
    fireEvent.change(await screen.findByTestId('server-group-name'), {
      target: { value: 'VIP Updated' },
    });
    await user.click(screen.getByTestId('server-group-submit'));

    await waitFor(() =>
      expect(mocks.saveGroup).toHaveBeenCalledWith({ id: 1, name: 'VIP Updated' }),
    );
  });

  it('shows an inline group error and sends no request for an empty name', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('server-group-create'));
    await user.click(await screen.findByTestId('server-group-submit'));

    expect(await screen.findByText('组名不能为空')).toBeInTheDocument();
    expect(mocks.saveGroup).not.toHaveBeenCalled();
    expect(screen.getByTestId('server-group-editor')).toBeInTheDocument();
  });

  it('preserves the group editor and value after the save request fails', async () => {
    const user = userEvent.setup();
    mocks.saveGroup.mockRejectedValueOnce(new Error('save failed'));
    render(<ServersPage />);

    await user.click(screen.getByTestId('server-group-create'));
    fireEvent.change(await screen.findByTestId('server-group-name'), {
      target: { value: 'Retry Group' },
    });
    await user.click(screen.getByTestId('server-group-submit'));

    await waitFor(() => expect(mocks.saveGroup).toHaveBeenCalledOnce());
    expect(screen.getByTestId('server-group-editor')).toBeInTheDocument();
    expect(screen.getByTestId('server-group-name')).toHaveValue('Retry Group');
  });

  it('drops a group only after confirmation', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-group-delete-1'));
    await waitFor(() => expect(mocks.dropGroup).toHaveBeenCalledWith(1));
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

  it('does not render the empty state when the route query failed', () => {
    mocks.routesData = undefined;
    mocks.routesError = true;
    render(<ServersPage />);

    expect(screen.getByText('路由列表加载失败')).toBeInTheDocument();
    expect(screen.queryByTestId('server-routes-empty')).not.toBeInTheDocument();
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
      expect(mocks.saveRoute).toHaveBeenCalledWith({
        remarks: 'Rule',
        match: ['a.com', 'b.com'],
        action: 'block',
        action_value: null,
      }),
    );
  });

  it('keeps the route editor open and sends no request for an invalid form', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('server-route-create'));
    await user.click(await screen.findByTestId('server-route-submit'));

    expect(await screen.findByText('备注不能为空')).toBeInTheDocument();
    expect(screen.getByText('匹配值不能为空')).toBeInTheDocument();
    expect(mocks.saveRoute).not.toHaveBeenCalled();
    expect(screen.getByTestId('server-route-editor')).toBeInTheDocument();
  });

  it('preserves the route editor and values after the save request fails', async () => {
    const user = userEvent.setup();
    mocks.saveRoute.mockRejectedValueOnce(new Error('save failed'));
    render(<ServersPage />);

    await user.click(screen.getByTestId('server-route-create'));
    fireEvent.change(await screen.findByTestId('server-route-remarks'), {
      target: { value: 'Retry Route' },
    });
    fireEvent.change(screen.getByTestId('server-route-match'), {
      target: { value: 'retry.example.test' },
    });
    await user.click(screen.getByTestId('server-route-submit'));

    await waitFor(() => expect(mocks.saveRoute).toHaveBeenCalledOnce());
    expect(screen.getByTestId('server-route-editor')).toBeInTheDocument();
    expect(screen.getByTestId('server-route-remarks')).toHaveValue('Retry Route');
    expect(screen.getByTestId('server-route-match')).toHaveValue('retry.example.test');
  });

  it('keeps the existing match array when editing without changes', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('server-route-edit-1'));
    await user.click(await screen.findByTestId('server-route-submit'));
    await waitFor(() =>
      expect(mocks.saveRoute).toHaveBeenCalledWith(
        expect.objectContaining({
          match: ['geosite:netflix', 'domain:example.com'],
          action: 'route',
        }),
      ),
    );
  });

  it('submits the exact default-out edit payload without requiring match rows', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('server-route-edit-2'));
    await user.click(await screen.findByTestId('server-route-submit'));

    await waitFor(() =>
      expect(mocks.saveRoute).toHaveBeenCalledWith({
        id: 2,
        remarks: 'Default',
        match: [],
        action: 'default_out',
        action_value: '{}',
      }),
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

  it('does not render the empty state when the node query failed', () => {
    mocks.nodesData = undefined;
    mocks.nodesError = true;
    render(<ServersPage />);

    expect(screen.getByText('节点列表加载失败')).toBeInTheDocument();
    expect(screen.queryByTestId('server-nodes-empty')).not.toBeInTheDocument();
  });

  it('blocks node editing while a dependent group query is unavailable', async () => {
    const user = userEvent.setup();
    mocks.groupsData = undefined;
    mocks.groupsError = true;
    render(<ServersPage />);

    const addButton = screen.getByTestId('node-add');
    expect(addButton).toBeDisabled();
    expect(screen.getByText('权限组加载失败，无法编辑节点')).toBeInTheDocument();
    await user.click(addButton);

    expect(screen.queryByTestId('node-add-shadowsocks')).not.toBeInTheDocument();
    expect(mocks.saveServer).not.toHaveBeenCalled();
  });

  it('blocks node editing while the dependent route query is unavailable', () => {
    mocks.routesData = undefined;
    mocks.routesError = true;
    render(<ServersPage />);

    expect(screen.getByTestId('node-add')).toBeDisabled();
    expect(screen.getByText('路由列表加载失败，无法编辑节点')).toBeInTheDocument();
    expect(mocks.saveServer).not.toHaveBeenCalled();
  });

  it('dispatches the show toggle as the merged §6.7 boolean PATCH', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByLabelText('切换「Tokyo」显隐'));
    expect(mocks.updateServer).toHaveBeenCalledWith({
      type: 'shadowsocks',
      id: 1,
      show: false,
    });
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
    expect(mocks.copyServer).toHaveBeenCalledWith({ type: 'shadowsocks', id: 1 });
  });

  it('deletes a node only after confirmation', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);
    await user.click(screen.getByTestId('node-actions-1'));
    await user.click(await screen.findByTestId('node-delete-1'));
    await waitFor(() =>
      expect(mocks.dropServer).toHaveBeenCalledWith({ type: 'shadowsocks', id: 1 }),
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
    await user.click(within(screen.getByTestId('node-group-ids')).getByRole('checkbox'));
    await user.click(screen.getByTestId('node-submit'));
    await waitFor(() =>
      expect(mocks.saveServer).toHaveBeenCalledWith({
        type: 'shadowsocks',
        data: {
          name: 'JP',
          group_id: ['1'],
          host: 'jp.example.com',
          port: '443',
          server_port: '8443',
          rate: 1,
          cipher: 'chacha20-ietf-poly1305',
          obfs: '',
        },
      }),
    );
  });

  it('blocks an invalid node locally and keeps the editor open', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('node-add'));
    await user.click(await screen.findByTestId('node-add-shadowsocks'));
    await user.click(await screen.findByTestId('node-submit'));

    expect(await screen.findByText('节点名称不能为空')).toBeInTheDocument();
    expect(screen.getByText('权限组不能为空')).toBeInTheDocument();
    expect(mocks.saveServer).not.toHaveBeenCalled();
    expect(screen.getByTestId('node-editor')).toBeInTheDocument();
  });

  it('keeps a valid node and its editor intact after the save request fails', async () => {
    const user = userEvent.setup();
    mocks.saveServer.mockRejectedValueOnce(new Error('save failed'));
    render(<ServersPage />);

    await user.click(screen.getByTestId('node-add'));
    await user.click(await screen.findByTestId('node-add-shadowsocks'));
    fireEvent.change(await screen.findByTestId('node-name'), { target: { value: 'Retry me' } });
    fireEvent.change(screen.getByTestId('node-host'), {
      target: { value: 'retry.example.test' },
    });
    fireEvent.change(screen.getByTestId('node-port'), { target: { value: '443' } });
    fireEvent.change(screen.getByTestId('node-server-port'), { target: { value: '10443' } });
    await user.click(within(screen.getByTestId('node-group-ids')).getByRole('checkbox'));
    await user.click(screen.getByTestId('node-submit'));

    await waitFor(() => expect(mocks.saveServer).toHaveBeenCalledOnce());
    expect(screen.getByTestId('node-editor')).toBeInTheDocument();
    expect(screen.getByTestId('node-name')).toHaveValue('Retry me');
    expect(screen.getByTestId('node-host')).toHaveValue('retry.example.test');
  });

  it('remounts the editor for every open so edits cannot race into another record', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('node-actions-1'));
    await user.click(await screen.findByTestId('node-edit-1'));
    fireEvent.change(await screen.findByTestId('node-name'), { target: { value: 'Unsaved' } });
    await user.click(
      within(screen.getByTestId('node-editor')).getByRole('button', { name: '取消' }),
    );

    await user.click(screen.getByTestId('node-actions-2'));
    await user.click(await screen.findByTestId('node-edit-2'));

    expect(await screen.findByTestId('node-name')).toHaveValue('Child');
    expect(screen.getByTestId('node-host')).toHaveValue('child.example.com');
    expect(screen.getByTestId('node-name')).not.toHaveValue('Unsaved');
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

  it('blocks SPA navigation with the shadcn leave dialog while sorting', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    expect(mocks.useBlocker).toHaveBeenLastCalledWith(false);
    await user.click(screen.getByTestId('node-sort-toggle'));
    expect(mocks.useBlocker).toHaveBeenLastCalledWith(true);

    setBlockerState('blocked');

    expect(await screen.findByTestId('server-sort-leave-dialog')).toBeInTheDocument();
    expect(screen.getByText('节点排序还没有保存，是否离开')).toBeInTheDocument();

    await user.click(screen.getByTestId('server-sort-stay'));
    expect(mocks.blockerReset).toHaveBeenCalledOnce();
    expect(mocks.blockerProceed).not.toHaveBeenCalled();
  });

  it('proceeds through the blocked router navigation after confirmation', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    await user.click(screen.getByTestId('node-sort-toggle'));
    setBlockerState('blocked');
    await user.click(await screen.findByTestId('server-sort-leave'));

    expect(mocks.blockerProceed).toHaveBeenCalledOnce();
    expect(mocks.blockerReset).not.toHaveBeenCalled();
  });

  it('warns on hard navigation only while sort mode is active', async () => {
    const user = userEvent.setup();
    render(<ServersPage />);

    const cleanEvent = new Event('beforeunload', { cancelable: true }) as BeforeUnloadEvent;
    mocks.beforeUnload?.(cleanEvent);
    expect(cleanEvent.defaultPrevented).toBe(false);

    await user.click(screen.getByTestId('node-sort-toggle'));
    const dirtyEvent = new Event('beforeunload', { cancelable: true }) as BeforeUnloadEvent;
    mocks.beforeUnload?.(dirtyEvent);
    expect(dirtyEvent.defaultPrevented).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Pure contract helpers (Tier-1) — preserved unchanged from the replica suite.
// ---------------------------------------------------------------------------

describe('server node config contract helpers', () => {
  const commonNodeValues = {
    name: 'Protocol Node',
    group_id: ['1'],
    route_id: [2],
    parent_id: null,
    host: 'node.example.test',
    port: '443',
    server_port: '10443',
    tags: ['edge'],
    rate: '1.5',
    show: 1 as const,
  };

  it.each([
    {
      type: 'shadowsocks',
      values: {
        ...commonNodeValues,
        type: 'shadowsocks',
        cipher: 'chacha20-ietf-poly1305',
        obfs: 'http',
        obfs_settings: { path: '/obfs', host: 'cdn.example.test' },
      },
      data: {
        ...commonNodeValues,
        cipher: 'chacha20-ietf-poly1305',
        obfs: 'http',
        obfs_settings: { path: '/obfs', host: 'cdn.example.test' },
      },
    },
    {
      type: 'vmess',
      values: {
        ...commonNodeValues,
        type: 'vmess',
        tls: 1,
        network: 'ws',
        networkSettings: '{"path":"/vmess"}',
        tlsSettings: { serverName: 'sni.example.test' },
        ruleSettings: { domain: ['geosite:category-ads-all'] },
        dnsSettings: { servers: [] },
      },
      data: {
        ...commonNodeValues,
        tls: 1,
        network: 'ws',
        networkSettings: { path: '/vmess' },
        tlsSettings: { serverName: 'sni.example.test' },
        ruleSettings: { domain: ['geosite:category-ads-all'] },
        dnsSettings: null,
      },
    },
    {
      type: 'trojan',
      values: {
        ...commonNodeValues,
        type: 'trojan',
        network: 'ws',
        network_settings: '{"path":"/trojan"}',
        allow_insecure: 1,
        server_name: 'trojan-sni.example.test',
        tls: 1,
        cipher: 'must-not-leak',
      },
      data: {
        ...commonNodeValues,
        network: 'ws',
        network_settings: { path: '/trojan' },
        allow_insecure: 1,
        server_name: 'trojan-sni.example.test',
      },
    },
    {
      type: 'hysteria',
      values: {
        ...commonNodeValues,
        type: 'hysteria',
        version: 2,
        up_mbps: '50',
        down_mbps: '100',
        obfs: 'salamander',
        obfs_password: 'secret',
        server_name: 'hysteria-sni.example.test',
        insecure: 0,
      },
      data: {
        ...commonNodeValues,
        version: 2,
        up_mbps: '50',
        down_mbps: '100',
        obfs: 'salamander',
        obfs_password: 'secret',
        server_name: 'hysteria-sni.example.test',
        insecure: 0,
      },
    },
    {
      type: 'tuic',
      values: {
        ...commonNodeValues,
        type: 'tuic',
        server_name: 'tuic-sni.example.test',
        insecure: 0,
        disable_sni: 0,
        udp_relay_mode: 'quic',
        zero_rtt_handshake: 1,
        congestion_control: 'bbr',
      },
      data: {
        ...commonNodeValues,
        server_name: 'tuic-sni.example.test',
        insecure: 0,
        disable_sni: 0,
        udp_relay_mode: 'quic',
        zero_rtt_handshake: 1,
        congestion_control: 'bbr',
      },
    },
    {
      type: 'vless',
      values: {
        ...commonNodeValues,
        type: 'vless',
        sort: 7,
        tls: 2,
        tls_settings: { server_name: 'reality.example.test' },
        flow: 'xtls-rprx-vision',
        network: 'tcp',
        network_settings: '{"acceptProxyProtocol":false}',
        encryption: 'mlkem768x25519plus',
        encryption_settings: { rtt: '1rtt' },
      },
      data: {
        ...commonNodeValues,
        sort: 7,
        tls: 2,
        tls_settings: { server_name: 'reality.example.test' },
        flow: 'xtls-rprx-vision',
        network: 'tcp',
        network_settings: { acceptProxyProtocol: false },
        encryption: 'mlkem768x25519plus',
        encryption_settings: { rtt: '1rtt' },
      },
    },
    {
      type: 'anytls',
      values: {
        ...commonNodeValues,
        type: 'anytls',
        server_name: 'anytls-sni.example.test',
        insecure: 0,
        padding_scheme: '["stop=8","1=2-4"]',
      },
      data: {
        ...commonNodeValues,
        server_name: 'anytls-sni.example.test',
        insecure: 0,
        padding_scheme: ['stop=8', '1=2-4'],
      },
    },
    {
      type: 'v2node',
      values: {
        ...commonNodeValues,
        type: 'v2node',
        sort: 7,
        listen_ip: '0.0.0.0',
        install_command: 'must-not-leak',
        config: {
          protocol: 'vless',
          tls: 2,
          tls_settings: { server_name: 'v2node-reality.example.test' },
          flow: 'xtls-rprx-vision',
          network: 'tcp',
          network_settings: '{"acceptProxyProtocol":true}',
          encryption: 'mlkem768x25519plus',
          encryption_settings: { rtt: '0rtt' },
          disable_sni: 0,
          zero_rtt_handshake: 0,
          udp_relay_mode: 'must-not-leak',
          congestion_control: 'must-not-leak',
          cipher: 'must-not-leak',
          up_mbps: '999',
          down_mbps: '999',
          obfs: 'must-not-leak',
          obfs_password: 'must-not-leak',
          padding_scheme: '["must-not-leak"]',
        },
      },
      data: {
        ...commonNodeValues,
        sort: 7,
        listen_ip: '0.0.0.0',
        protocol: 'vless',
        tls: 2,
        tls_settings: { server_name: 'v2node-reality.example.test' },
        flow: 'xtls-rprx-vision',
        network: 'tcp',
        network_settings: { acceptProxyProtocol: true },
        encryption: 'mlkem768x25519plus',
        encryption_settings: { rtt: '0rtt' },
        disable_sni: 0,
        zero_rtt_handshake: 0,
      },
    },
  ])('builds the exact $type save payload', ({ type, values, data }) => {
    expect(serverNodeFormSchema.parse(values)).toEqual({ type, data });
  });

  it('rejects unknown nested settings and non-string AnyTLS padding before save', () => {
    const vmessValues = {
      ...commonNodeValues,
      type: 'vmess' as const,
      tls: 1 as const,
      network: 'ws' as const,
      networkSettings: '{"path":"/vmess","pth":"typo"}',
      tlsSettings: null,
      ruleSettings: null,
      dnsSettings: null,
    };
    expect(serverNodeFormSchema.safeParse(vmessValues).success).toBe(false);
    expect(
      serverNodeFormSchema.safeParse({
        ...vmessValues,
        networkSettings: '{"path":"/vmess"}',
        tlsSettings: '{"serverName":"sni.example.test","reject_unknown_sni":"yes"}',
      }).success,
    ).toBe(false);

    const anytlsValues = {
      ...commonNodeValues,
      type: 'anytls' as const,
      server_name: 'anytls-sni.example.test',
      insecure: 0 as const,
    };
    expect(
      serverNodeFormSchema.safeParse({
        ...anytlsValues,
        padding_scheme: '{"stop":"8"}',
      }).success,
    ).toBe(false);
    expect(
      serverNodeFormSchema.safeParse({
        ...anytlsValues,
        padding_scheme: '["stop=8",42]',
      }).success,
    ).toBe(false);
  });

  it('adds the record id only at the edit save boundary', () => {
    const values = {
      ...commonNodeValues,
      type: 'shadowsocks',
      cipher: 'aes-256-gcm',
      obfs: '',
    };

    expect(serverNodeFormSchema.parse(values).data).not.toHaveProperty('id');
    expect(serverNodeFormSchema.parse({ ...values, id: 42 })).toEqual({
      type: 'shadowsocks',
      data: {
        ...commonNodeValues,
        id: 42,
        cipher: 'aes-256-gcm',
        obfs: '',
      },
    });
  });

  it('clears stale protocol-only fields when a V2node changes protocol', () => {
    const values: V2nodeEditorValues = {
      ...commonNodeValues,
      type: 'v2node',
      listen_ip: '0.0.0.0',
      install_command: 'keep-read-only-command',
      config: {
        protocol: 'vless',
        tls: 2,
        tls_settings: { server_name: 'old-reality.example.test' },
        network: 'xhttp',
        network_settings: { path: '/old' },
        flow: 'xtls-rprx-vision',
        encryption: 'mlkem768x25519plus',
        encryption_settings: { rtt: '1rtt' },
        disable_sni: 1,
        zero_rtt_handshake: 1,
      },
    };

    const next = switchV2nodeProtocol(values, 'tuic');

    expect(next).toMatchObject({
      type: 'v2node',
      name: 'Protocol Node',
      install_command: 'keep-read-only-command',
      config: {
        protocol: 'tuic',
        tls: 1,
        network: 'tcp',
        network_settings: null,
        disable_sni: 0,
        zero_rtt_handshake: 0,
        udp_relay_mode: 'native',
        congestion_control: 'cubic',
      },
    });
    for (const staleField of [
      'tls_settings',
      'flow',
      'encryption',
      'encryption_settings',
      'cipher',
      'up_mbps',
      'down_mbps',
      'obfs',
      'obfs_password',
      'padding_scheme',
    ]) {
      expect(next.config).not.toHaveProperty(staleField);
    }
  });

  it('uses the original type-specific transport placeholders', () => {
    expect(getNetworkSettingsPlaceholder('v2node', 'tcp')).toContain(
      '"acceptProxyProtocol": false',
    );
    expect(getNetworkSettingsPlaceholder('v2node', 'http')).toContain('"Host": "xtls.github.io"');
    expect(getNetworkSettingsPlaceholder('vmess', 'tcp')).not.toContain('acceptProxyProtocol');
    expect(getNetworkSettingsPlaceholder('vmess', 'ws')).toContain('"Host": "v2ray.com"');
    expect(getNetworkSettingsPlaceholder('vmess', 'xhttp')).not.toContain('"mode": "auto"');
    expect(getNetworkSettingsPlaceholder('vless', 'ws')).toContain('"security": "auto"');
    expect(getNetworkSettingsPlaceholder('vless', 'xhttp')).toContain('"mode": "auto"');
    expect(getNetworkSettingsPlaceholder('trojan', 'tcp')).toBe('');
    expect(getNetworkSettingsPlaceholder('trojan', 'httpupgrade')).toBe('');
  });

  it('matches the original V2node security select fallback protocols', () => {
    expect(getV2nodeSecurityValue('anytls', 0)).toBe(0);
    expect(getV2nodeSecurityValue('hysteria2', 0)).toBe(1);
    expect(getV2nodeSecurityValue('trojan', 0)).toBe(1);
    expect(getV2nodeSecurityValue('tuic', 0)).toBe(1);
    expect(getV2nodeSecurityValue('vless', 0)).toBe(0);
    expect(getV2nodeSecurityValue('anytls', 2)).toBe(2);
  });

  it('coerces numeric select display values using the established parseInt contract', () => {
    expect(getBinarySelectValue('0')).toBe(0);
    expect(getBinarySelectValue('1')).toBe(1);
    expect(getBinarySelectValue(2)).toBe(1);
    expect(getBinarySelectValue(undefined)).toBe(0);
    expect(getNumericSelectValue('2')).toBe(2);
    expect(getNumericSelectValue('0', 1)).toBe(1);
    expect(getNumericSelectValue(undefined, 1)).toBe(1);
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
    ] as Parameters<typeof moveServerNodeByDragIndexes>[0];

    expect(moveServerNodeByDragIndexes(nodes, 0, 2).map((node) => node.id)).toEqual([2, 3, 1]);
    expect(moveServerNodeByDragIndexes(nodes, 2, 0).map((node) => node.id)).toEqual([3, 1, 2]);
  });
});

// ---------------------------------------------------------------------------
// Node table column controls — pure filter/sort behavior, unchanged.
// ---------------------------------------------------------------------------

describe('applyServerNodeColumnControls (node table sort/filter)', () => {
  const nodes = [
    { id: 1, type: 'shadowsocks', group_id: [1], online: 8 },
    { id: 2, type: 'vmess', group_id: [2], online: 0 },
    { id: 3, type: 'trojan', group_id: [1, 2], online: 4 },
  ] as unknown as Parameters<typeof applyServerNodeColumnControls>[0];
  const ids = (list: (typeof nodes)[number][]) => list.map((node) => node.id);

  it('filters by type matching node.type to the lowercased column label', () => {
    expect(
      ids(
        applyServerNodeColumnControls(nodes, {
          typeFilter: ['Shadowsocks'],
          groupFilter: [],
          onlineSort: '',
        }),
      ),
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
      ids(
        applyServerNodeColumnControls(nodes, {
          typeFilter: [],
          groupFilter: ['2'],
          onlineSort: '',
        }),
      ),
    ).toEqual([2, 3]);
    expect(
      ids(
        applyServerNodeColumnControls(nodes, {
          typeFilter: [],
          groupFilter: ['1'],
          onlineSort: '',
        }),
      ),
    ).toEqual([1, 3]);
  });

  it('sorts by online ascending/descending and preserves source order when unsorted', () => {
    expect(
      applyServerNodeColumnControls(nodes, {
        typeFilter: [],
        groupFilter: [],
        onlineSort: 'ascend',
      }).map((node) => node.online),
    ).toEqual([0, 4, 8]);
    expect(
      applyServerNodeColumnControls(nodes, {
        typeFilter: [],
        groupFilter: [],
        onlineSort: 'descend',
      }).map((node) => node.online),
    ).toEqual([8, 4, 0]);
    expect(
      ids(
        applyServerNodeColumnControls(nodes, { typeFilter: [], groupFilter: [], onlineSort: '' }),
      ),
    ).toEqual([1, 2, 3]);
  });

  it('applies filters before sorting according to the table contract', () => {
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
