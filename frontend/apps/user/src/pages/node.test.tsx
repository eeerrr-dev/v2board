import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import NodePage from './node';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const queryState = vi.hoisted(() => ({
  navigate: vi.fn(),
  servers: undefined as
    | Array<{
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
      }>
    | undefined,
  serversFetching: true,
  subscribe: undefined as { plan_id?: number | null } | undefined,
}));

const labels: Record<string, string> = {
  'node.simple_name': '名称',
  'node.status': '状态',
  'node.rate': '倍率',
  'node.tags': '标签',
  'node.status_tip': '五分钟内节点在线情况',
  'node.rate_tip': '使用的流量将乘以倍率进行扣除',
  'node.no_available': '没有可用节点，如果您未订阅或已过期请',
  'node.subscribe': '订阅',
  'node.renew': '续费',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key, i18n: { language: 'zh-CN' } }),
}));

vi.mock('react-router-dom', () => ({
  useNavigate: () => queryState.navigate,
}));

vi.mock('@/lib/queries', () => ({
  useSubscribe: () => ({ data: queryState.subscribe }),
  useServers: () => ({ data: queryState.servers, isFetching: queryState.serversFetching }),
}));

describe('NodePage legacy loading timing', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    queryState.navigate.mockClear();
    queryState.servers = undefined;
    queryState.serversFetching = true;
    queryState.subscribe = undefined;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root?.unmount());
    root = null;
    container.remove();
    document.body.innerHTML = '';
  });

  it('renders the original empty-node notice before the mount fetch flips loading on', () => {
    const html = renderToStaticMarkup(<NodePage />);

    expect(html).toContain('没有可用节点，如果您未订阅或已过期请');
    expect(html).toContain('订阅');
    expect(html).not.toContain('spinner-grow');
  });

  it('shows only the original centered spinner after the mount fetch dispatch equivalent', async () => {
    await act(async () => {
      root!.render(<NodePage />);
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('spinner-grow text-primary');
    expect(container.innerHTML).toContain('Loading...');
    expect(container.innerHTML).not.toContain('ant-table-wrapper');
    expect(container.innerHTML).not.toContain('alert alert-dark');
  });
});

describe('NodePage bundled-theme table and empty state', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    queryState.navigate.mockClear();
    queryState.serversFetching = false;
    queryState.subscribe = undefined;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  it('renders the legacy antd v3 table shell, columns, badges, rate tags, and tag fallback', () => {
    queryState.servers = [
      {
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
        tags: ['IEPL', 'Netflix'],
      },
      {
        id: 2,
        parent_id: null,
        group_id: [1],
        route_id: null,
        name: 'US 01',
        rate: '2',
        type: 'shadowsocks',
        host: 'us.example.test',
        port: 443,
        cache_key: 'us01',
        last_check_at: null,
        is_online: 0,
        tags: null,
      },
    ];

    const html = renderToStaticMarkup(<NodePage />);

    expect(html).toContain('block block-rounded js-appear-enabled');
    expect(html).toContain('ant-table-wrapper');
    expect(html).toContain('style="width:900px;table-layout:auto"');
    expect(html).toContain('名称');
    expect(html).toContain('状态');
    expect(html).toContain('倍率');
    expect(html).toContain('标签');
    expect(html).toContain('HK 01');
    expect(html).toContain('ant-badge-status-processing');
    expect(html).toContain('ant-badge-status-error');
    expect(html).toContain('1.5 x');
    expect(html).toContain('IEPL');
    expect(html).toContain('Netflix');
    expect(html).toContain('<td>-</td>');
  });

  it('keeps javascript href anchors and routes empty-state actions like the original', async () => {
    queryState.servers = [];
    queryState.subscribe = { plan_id: 7 };

    await act(async () => {
      root.render(<NodePage />);
      await Promise.resolve();
    });

    const link = container.querySelector<HTMLAnchorElement>('a.alert-link');
    expect(link).toBeTruthy();
    expect(link!.getAttribute('href')).toBe('javascript:void(0);');
    expect(link!.textContent).toBe('续费');

    await act(async () => {
      link!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(queryState.navigate).toHaveBeenCalledWith('/plan/7');

    queryState.navigate.mockClear();
    queryState.subscribe = {};
    await act(async () => {
      root.render(<NodePage />);
      await Promise.resolve();
    });

    const subscribeLink = container.querySelector<HTMLAnchorElement>('a.alert-link');
    expect(subscribeLink!.getAttribute('href')).toBe('javascript:void(0);');
    expect(subscribeLink!.textContent).toBe('订阅');

    await act(async () => {
      subscribeLink!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(queryState.navigate).toHaveBeenCalledWith('/plan');
  });
});
