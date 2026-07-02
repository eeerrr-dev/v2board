import { screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import NodePage from './node';

interface ServerRow {
  id: number;
  parent_id: null;
  group_id: number[];
  route_id: null;
  name: string;
  rate: string;
  type: 'shadowsocks';
  host: string;
  port: number;
  cache_key: string;
  last_check_at: null;
  is_online: 0 | 1;
  tags?: string[] | null;
}

const queryState = vi.hoisted(() => ({
  navigate: vi.fn(),
  servers: undefined as ServerRow[] | undefined,
  serversPending: false,
  serversFetching: false,
  serversError: false,
  serversRefetch: vi.fn(),
  subscribe: undefined as { plan_id?: number | null } | undefined,
}));

const labels: Record<string, string> = {
  'node.simple_name': '名称',
  'node.status': '状态',
  'node.rate': '倍率',
  'node.tags': '标签',
  'node.status_tip': '节点五分钟内节点在线情况',
  'node.rate_tip': '使用的流量将乘以倍率进行扣除',
  'node.no_available': '没有可用节点，如果您未订阅或已过期请',
  'node.subscribe': '订阅',
  'node.renew': '续费',
  'common.loading': 'Loading...',
  'common.error_title': '加载失败',
  'common.retry': '重试',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key, i18n: { language: 'zh-CN' } }),
}));

vi.mock('react-router', () => ({
  useNavigate: () => queryState.navigate,
}));

vi.mock('@/lib/queries', () => ({
  useSubscribe: () => ({ data: queryState.subscribe }),
  useServers: () => ({
    data: queryState.servers,
    isPending: queryState.serversPending,
    isFetching: queryState.serversFetching,
    isError: queryState.serversError,
    refetch: queryState.serversRefetch,
  }),
}));

function makeServer(overrides: Partial<ServerRow>): ServerRow {
  return {
    id: 1,
    parent_id: null,
    group_id: [1],
    route_id: null,
    name: 'HK 01',
    rate: '1.5',
    type: 'shadowsocks',
    host: 'hk.example.test',
    port: 443,
    cache_key: 'hk01',
    last_check_at: null,
    is_online: 1,
    tags: null,
    ...overrides,
  };
}

beforeEach(() => {
  queryState.navigate.mockClear();
  queryState.serversRefetch.mockClear();
  queryState.servers = undefined;
  queryState.serversPending = false;
  queryState.serversFetching = false;
  queryState.serversError = false;
  queryState.subscribe = undefined;
});

describe('NodePage loading state', () => {
  it('shows only the loading status while the initial servers fetch is pending', () => {
    queryState.serversPending = true;
    queryState.serversFetching = true;

    renderWithProviders(<NodePage />);

    expect(screen.getByRole('status')).toBe(screen.getByTestId('node-loading'));
    expect(screen.getByTestId('node-loading')).toHaveTextContent('Loading...');
    expect(screen.queryByTestId('node-table')).not.toBeInTheDocument();
    expect(screen.queryByTestId('node-empty')).not.toBeInTheDocument();
  });

  it('keeps cached servers rendered during a background refetch instead of blanking to the spinner', () => {
    queryState.servers = [makeServer({ id: 1, name: 'HK 01' })];
    queryState.serversPending = false;
    queryState.serversFetching = true; // refetchOnMount('always') revisit with cached data

    renderWithProviders(<NodePage />);

    expect(screen.queryByTestId('node-loading')).not.toBeInTheDocument();
    expect(screen.getByTestId('node-table')).toBeInTheDocument();
    expect(screen.getByText('HK 01')).toBeInTheDocument();
  });
});

describe('NodePage service table', () => {
  beforeEach(() => {
    queryState.servers = [
      makeServer({ id: 1, name: 'HK 01', rate: '1.5', is_online: 1, tags: ['IEPL', 'Netflix'] }),
      makeServer({
        id: 2,
        name: 'US 01',
        rate: '2',
        host: 'us.example.test',
        cache_key: 'us01',
        is_online: 0,
        tags: null,
      }),
    ];
  });

  it('renders the parity hooks, headers, status interpretation, rates, and tag fallback', () => {
    renderWithProviders(<NodePage />);

    expect(screen.getByTestId('node-card')).toBeInTheDocument();
    const table = screen.getByTestId('node-table');
    expect(table).toHaveAttribute('data-table-kind', 'service');
    // The parity harness reads data-scroll-position off this container.
    expect(screen.getByTestId('service-table-scroll')).toHaveAttribute('data-scroll-position');

    for (const header of ['名称', '状态', '倍率', '标签']) {
      expect(within(table).getByRole('columnheader', { name: header })).toBeInTheDocument();
    }
    // The parity harness hovers [data-testid="node-table"] .v2board-service-tooltip-trigger.
    expect(table.querySelectorAll('.v2board-service-tooltip-trigger')).toHaveLength(2);

    const [hkRow, usRow] = within(table).getAllByRole('row').slice(1);
    // Rows keep server order with index-based keys, not server-id keys (1, 2).
    expect(hkRow).toHaveAttribute('data-row-key', '0');
    expect(usRow).toHaveAttribute('data-row-key', '1');

    expect(within(hkRow!).getByText('HK 01')).toBeInTheDocument();
    expect(within(hkRow!).getByLabelText('online')).toBeInTheDocument(); // is_online: 1
    expect(within(hkRow!).getByText('1.5 x')).toBeInTheDocument();
    expect(within(hkRow!).getByText('IEPL')).toBeInTheDocument();
    expect(within(hkRow!).getByText('Netflix')).toBeInTheDocument();

    expect(within(usRow!).getByText('US 01')).toBeInTheDocument();
    expect(within(usRow!).getByLabelText('offline')).toBeInTheDocument(); // is_online: 0
    expect(within(usRow!).getByText('2 x')).toBeInTheDocument();
    const usCells = within(usRow!).getAllByRole('cell');
    expect(usCells[3]).toHaveTextContent(/^-$/); // null tags fall back to a dash
  });

  it('opens the status tooltip from its parity header trigger', async () => {
    const { user } = renderWithProviders(<NodePage />);

    const trigger = screen.getByText('状态');
    expect(trigger).toHaveClass('v2board-service-tooltip-trigger');
    // The shared HeaderTooltip keeps node's centered alignment via className.
    expect(trigger).toHaveClass('justify-center');

    await user.hover(trigger);

    expect(await screen.findByRole('tooltip')).toHaveTextContent('节点五分钟内节点在线情况');
  });
});

describe('NodePage empty state routing', () => {
  it('routes the renew action to the subscribed plan', async () => {
    queryState.servers = [];
    queryState.subscribe = { plan_id: 7 };

    const { user } = renderWithProviders(<NodePage />);

    expect(screen.getByTestId('node-empty')).toBeInTheDocument();
    const action = screen.getByRole('button', { name: '续费' });
    expect(action).toHaveAttribute('data-testid', 'node-empty-action');
    // The action is a real button, not a legacy javascript-href anchor.
    expect(screen.queryByRole('link')).not.toBeInTheDocument();

    await user.click(action);

    expect(queryState.navigate).toHaveBeenCalledWith('/plan/7');
  });

  it('routes the subscribe action to the plan list when there is no plan', async () => {
    queryState.servers = [];
    queryState.subscribe = {};

    const { user } = renderWithProviders(<NodePage />);

    await user.click(screen.getByRole('button', { name: '订阅' }));

    expect(queryState.navigate).toHaveBeenCalledWith('/plan');
  });
});

describe('NodePage error state', () => {
  it('shows a retryable error state instead of the subscribe prompt when the fetch fails', async () => {
    queryState.serversError = true;
    queryState.subscribe = { plan_id: 7 };

    const { user } = renderWithProviders(<NodePage />);

    // A failed fetch must not wrongly tell a paying user to "subscribe".
    expect(screen.getByTestId('node-error')).toBeInTheDocument();
    expect(screen.queryByTestId('node-empty')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '重试' }));

    expect(queryState.serversRefetch).toHaveBeenCalled();
  });
});
