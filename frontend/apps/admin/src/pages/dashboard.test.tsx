import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import DashboardPage from './dashboard';

const dashboardSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'dashboard.tsx'),
  'utf8',
);
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);
const adminAppOverridesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../styles/admin-app-overrides.css'),
  'utf8',
);
const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
}));

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('echarts', () => ({
  init: vi.fn(() => ({
    setOption: vi.fn(),
    resize: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock('echarts/theme/vintage', () => ({}));

vi.mock('@/lib/legacy-settings', () => ({
  getAdminApiBaseUrl: () => 'http://localhost/api/v1',
}));

vi.mock('@/lib/queries', () => ({
  useConfig: () => ({ data: { site: { currency: 'CNY' }, currency: 'USD' } }),
  useStat: () => ({
    data: {
      online_user: 9,
      day_income: 12345,
      day_register_total: 7,
      month_income: 67890,
      last_month_income: 45678,
      commission_last_month_payout: 1234,
      month_register_total: 42,
      ticket_pending_total: 2,
      commission_pending_total: 3,
    },
  }),
  useStatOrder: () => ({ data: [] }),
  useStatServerToday: () => ({ data: [] }),
  useStatServerLast: () => ({ data: [] }),
  useStatUserToday: () => ({ data: [] }),
  useStatUserLast: () => ({ data: [] }),
}));

describe('DashboardPage legacy OneUI layout', () => {
  it('renders the original dashboard shortcut, alert, statistic, and chart containers', () => {
    const html = renderToStaticMarkup(<DashboardPage />);

    expect(html).toContain('有 2 条工单等待处理');
    expect(html).toContain('有 3 笔佣金等待确认');
    expect(html).toContain('class="mb-0 block border-bottom js-classic-nav d-none d-sm-block"');
    expect(html).toContain('系统设置');
    expect(html).toContain('订单管理');
    expect(html).toContain('订阅管理');
    expect(html).toContain('用户管理');
    expect(html).toContain('class="block border-bottom mb-0 v2board-stats-bar"');
    expect(html).toContain('在线人数');
    expect(html).toContain('今日收入');
    expect(html).toContain('CNY');
    expect(html).not.toContain('USD');
    expect(html).toContain('实时注册');
    expect(html).toContain('本月收入');
    expect(html).toContain('上月佣金支出');
    expect(html).toContain('id="orderChart"');
    expect(html).toContain('id="serverTodayRankChart"');
    expect(html).toContain('id="serverLastRankChart"');
    expect(html).toContain('id="userTodayRankChart"');
    expect(html).toContain('id="userLastRankChart"');
  });

  it('keeps the original pending-commission shortcut filters before opening orders', () => {
    expect(dashboardSource).toContain("'v2board-admin-order-filter'");
    expect(dashboardSource).toContain("{ key: 'status', condition: '=', value: '3' }");
    expect(dashboardSource).toContain("{ key: 'commission_status', condition: '=', value: '0' }");
    expect(dashboardSource).toContain("{ key: 'commission_balance', condition: '>', value: '0' }");
    expect(dashboardSource).toContain('onClick={goPendingCommissionOrders}');
  });

  it('keeps the original shortcut anchors as click-only blocks without hrefs', () => {
    const html = renderToStaticMarkup(<DashboardPage />);
    const shortcutAnchors =
      html.match(/<a class="block block-bordered block-link-pop text-center mb-0">/g) ?? [];

    expect(shortcutAnchors).toHaveLength(4);
    expect(dashboardSource).toContain("ref={legacyHref('javascript:void(0)')}");
    expect(dashboardSource).not.toContain(
      `className="block block-bordered block-link-pop text-center mb-0"
                href="javascript:void(0)"`,
    );
  });

  it('keeps the original grouped currency lookup and stats-bar scroll logging', () => {
    expect(dashboardSource).toContain("const config = useConfig('site');");
    expect(dashboardSource).toContain('const currency = config.data?.site?.currency;');
    expect(dashboardSource).not.toContain("config.data?.site?.currency ?? ''");
    expect(dashboardSource).not.toContain('config.data?.currency');
    expect(dashboardSource).toContain(
      'onScroll={(event) => console.log(event.currentTarget.scrollLeft)}',
    );
  });

  it('keeps the original queue monitor request without swallowing failures', () => {
    const queueBlock = dashboardSource.slice(
      dashboardSource.indexOf('function useQueueStatus()'),
      dashboardSource.indexOf('const PENDING_COMMISSION_ORDER_FILTER'),
    );

    expect(queueBlock).toContain('const origin = new URL(getAdminApiBaseUrl()).origin;');
    expect(queueBlock).toContain('fetch(`${origin}/monitor/api/stats`)');
    expect(queueBlock).toContain('.then((response) => response.json())');
    expect(queueBlock).toContain('.then((data) => setQueueStatus(data?.status));');
    expect(queueBlock).not.toContain('.catch(');
  });

  it('keeps the old dashboard mount request and chart initialization order', () => {
    const pageBlock = dashboardSource.slice(
      dashboardSource.indexOf('export default function DashboardPage()'),
      dashboardSource.indexOf('const goPendingCommissionOrders'),
    );
    const hookOrder = [
      'const queueStatus = useQueueStatus();',
      'const stat = useStat();',
      'const order = useStatOrder();',
      'const serverLast = useStatServerLast();',
      'const serverToday = useStatServerToday();',
      'const userToday = useStatUserToday();',
      'const userLast = useStatUserLast();',
      "const config = useConfig('site');",
    ];
    let previous = -1;
    for (const hook of hookOrder) {
      const index = pageBlock.indexOf(hook);
      expect(index).toBeGreaterThan(previous);
      previous = index;
    }

    const chartBlock = dashboardSource.slice(
      dashboardSource.indexOf('const orderChart = useChart('),
      dashboardSource.indexOf('return (', dashboardSource.indexOf('const orderChart = useChart(')),
    );
    expect(chartBlock.indexOf('const serverLastRankChart = useChart(')).toBeLessThan(
      chartBlock.indexOf('const serverTodayRankChart = useChart('),
    );
  });

  it('keeps the original rank chart in-place reverse behavior', () => {
    expect(dashboardSource).toContain('data.reverse().forEach((item) => {');
    expect(dashboardSource).not.toContain('[...data].reverse()');
  });

  it('keeps the recovered empty order chart baseline isolated to the fallback path', () => {
    const orderChartBlock = dashboardSource.slice(
      dashboardSource.indexOf('function renderOrderChart'),
      dashboardSource.indexOf('function renderRankChart'),
    );

    expect(orderChartBlock).toContain('if (data.length === 0) {');
    expect(orderChartBlock).toContain("element.classList.add('v2board-empty-order-chart');");
    expect(dashboardSource).toContain('function renderEmptyOrderChartBaseline');
    expect(dashboardSource).toContain("canvas.className = 'v2board-empty-order-chart-canvas';");
    expect(dashboardSource).toContain("'rgb(160, 152, 137)'");
    expect(dashboardSource).toContain("'#6e7079'");
    expect(dashboardSource).toContain('document.documentElement.dataset.darkreaderMode');
    expect(dashboardSource).toContain("attributeFilter: ['data-darkreader-mode', 'data-darkreader-scheme']");
    expect(orderChartBlock).toContain('renderEmptyOrderChartBaseline(element);');
    expect(orderChartBlock.indexOf("element.style.position = '';")).toBeLessThan(
      orderChartBlock.indexOf('return chart;'),
    );
    expect(orderChartBlock).toContain("element.classList.remove('v2board-empty-order-chart');");
    expect(adminAppOverridesSource).toContain('#orderChart .v2board-empty-order-chart-canvas');
    expect(adminAppOverridesSource).toContain('bottom: 26px;');
    expect(adminAppOverridesSource).toContain('height: 1px;');
    expect(adminAppOverridesSource).toContain('position: absolute;');
    expect(adminAppOverridesSource).not.toContain('background-image: linear-gradient');
    expect(adminAppOverridesSource).not.toContain('border-top: 1px solid currentColor;');
    expect(orderChartBlock).toContain('return chart;');
  });

  it('keeps the legacy order chart axis options without an injected axis line', () => {
    const orderChartBlock = dashboardSource.slice(
      dashboardSource.indexOf('function renderOrderChart'),
      dashboardSource.indexOf('function renderRankChart'),
    );

    expect(orderChartBlock).toContain(
      "const chart = echarts.init(element, 'vintage', { renderer: 'svg' });",
    );
    expect(orderChartBlock).toContain(
      "xAxis: {\n      type: 'category',\n      boundaryGap: false,\n      data: [],\n    },",
    );
    expect(orderChartBlock).not.toContain('axisLine');
    expect(orderChartBlock).not.toContain('#6e7079');
  });

  it('keeps chart containers at the legacy static positioning after ECharts init', () => {
    expect(dashboardSource.match(/element\.style\.position = '';/g)).toHaveLength(3);
  });

  it('keeps the original rank tooltip axis payload assumption', () => {
    expect(dashboardSource).toContain(
      'formatter: (params) => `${(params as Array<{ value: unknown }>)[0]!.value} GB`,',
    );
    expect(dashboardSource).not.toContain('Array.isArray(params)');
  });

  it('keeps rank chart render callbacks stable across dashboard state updates', () => {
    expect(dashboardSource).toContain(
      'function renderServerRankChart(element: HTMLDivElement, data: ServerRankItem[])',
    );
    expect(dashboardSource).toContain(
      'function renderUserRankChart(element: HTMLDivElement, data: UserRankItem[])',
    );
    expect(dashboardSource).toMatch(
      /const serverTodayRankChart = useChart\(\s*serverToday\.data,\s*renderServerRankChart,\s*serverTodayRankChartObj,?\s*\);/,
    );
    expect(dashboardSource).toMatch(
      /const serverLastRankChart = useChart\(\s*serverLast\.data,\s*renderServerRankChart,\s*serverLastRankChartObj,?\s*\);/,
    );
    expect(dashboardSource).toMatch(
      /const userTodayRankChart = useChart\(\s*userToday\.data,\s*renderUserRankChart,\s*userTodayRankChartObj,?\s*\);/,
    );
    expect(dashboardSource).toMatch(
      /const userLastRankChart = useChart\(\s*userLast\.data,\s*renderUserRankChart,\s*userLastRankChartObj,?\s*\);/,
    );
    expect(dashboardSource).not.toContain('useChart(serverToday.data, (element, items)');
    expect(dashboardSource).not.toContain('useChart(userToday.data, (element, items)');
  });

  it('does not cache one-shot dashboard chart payloads after the page unmounts', () => {
    expect(queriesSource).toContain(
      'const legacyDashboardChartQueryOptions = { gcTime: 0 } as const;',
    );
    expect(queriesSource.match(/\.\.\.legacyDashboardChartQueryOptions/g)).toHaveLength(5);
    expect(queriesSource).toContain('queryFn: () => admin.statOrder(apiClient),');
    expect(queriesSource).toContain('queryFn: () => admin.statServerTodayRank(apiClient),');
    expect(queriesSource).toContain('queryFn: () => admin.statServerLastRank(apiClient),');
    expect(queriesSource).toContain('queryFn: () => admin.statUserTodayRank(apiClient),');
    expect(queriesSource).toContain('queryFn: () => admin.statUserLastRank(apiClient),');
    expect(queriesSource).not.toContain('legacyDashboardSummaryQueryOptions');
  });

  it('keeps the original single order-chart resize listener lifecycle', () => {
    expect(dashboardSource).toContain('const resizeAllCharts = useCallback(() => {');
    expect(dashboardSource).toContain('orderChartObj.current!.resize();');
    expect(dashboardSource).toContain('serverLastRankChartObj.current!.resize();');
    expect(dashboardSource).toContain('serverTodayRankChartObj.current!.resize();');
    expect(dashboardSource).toContain('userTodayRankChartObj.current!.resize();');
    expect(dashboardSource).toContain('userLastRankChartObj.current!.resize();');
    expect(dashboardSource).toContain("window.addEventListener('resize', resizeAllCharts);");
    expect(dashboardSource).toContain(
      "window.removeEventListener('resize', () => resizeAllCharts());",
    );
    expect(dashboardSource).not.toContain('current?.resize();');
    expect(dashboardSource).not.toContain('chartObjectRef.current = undefined;');
    expect(dashboardSource).toMatch(
      /const orderChart = useChart\(\s*order\.data,\s*renderOrderChart,\s*orderChartObj,\s*registerLegacyOrderChartResize,\s*\);/,
    );
    expect(dashboardSource).not.toContain("window.addEventListener('resize', resize);");
    expect(dashboardSource).not.toContain('chart.dispose();');
  });
});
