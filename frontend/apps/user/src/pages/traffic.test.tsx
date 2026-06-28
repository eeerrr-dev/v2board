import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import TrafficPage from './traffic';

const trafficSource = readFileSync(`${process.cwd()}/src/pages/traffic.tsx`, 'utf8');

const queryState = vi.hoisted(() => ({
  rows: [] as Array<{
    u: number;
    d: number;
    record_at: number;
    user_id: number;
    server_rate: string;
  }>,
  fetching: true,
}));

const labels: Record<string, string> = {
  'traffic.notice': '流量明细仅保留近一个月数据以供查询。',
  'traffic.record_at': '记录时间',
  'traffic.actual_upload': '实际上行',
  'traffic.actual_download': '实际下行',
  'traffic.deduct_rate': '扣费倍率',
  'traffic.total_charged': '合计',
  'traffic.total_formula': '公式：(实际上行 + 实际下行) x 扣费倍率 = 扣除流量',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => labels[key] ?? key,
    i18n: { language: 'zh-CN' },
  }),
}));

vi.mock('@/lib/queries', () => ({
  useTrafficLog: () => ({ data: queryState.rows, isFetching: queryState.fetching }),
}));

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('TrafficPage shadcn loading timing', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    queryState.rows = [];
    queryState.fetching = true;
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

  it('does not show loading until after the mount fetch dispatch equivalent', () => {
    const html = renderToStaticMarkup(<TrafficPage />);

    expect(html).toContain('流量明细仅保留近一个月数据以供查询。');
    expect(html).toContain('data-testid="traffic-empty"');
    expect(html).not.toContain('Loading...');
  });

  it('uses only the shadcn inline loading state after the mount fetch dispatch equivalent', async () => {
    await act(async () => {
      root!.render(<TrafficPage />);
      await Promise.resolve();
    });

    expect(container.querySelector('[role="status"]')?.textContent).toContain('Loading...');
    expect(container.innerHTML).not.toContain('ant-spin-spinning');
    expect(container.innerHTML).not.toContain('block-mode-loading');
  });
});

describe('TrafficPage shadcn service table', () => {
  beforeEach(() => {
    queryState.fetching = false;
    queryState.rows = [
      {
        u: 2048,
        d: 1024,
        record_at: 1_705_320_000,
        user_id: 1,
        server_rate: '1.5',
      },
      {
        u: 100,
        d: 200,
        record_at: 0,
        user_id: 1,
        server_rate: '0',
      },
    ];
  });

  it('renders the service table shell, headers, tooltip trigger, and row formatting', () => {
    const html = renderToStaticMarkup(<TrafficPage />);

    expect(html).toContain('data-testid="traffic-card"');
    expect(html).toContain('data-testid="service-table-scroll"');
    expect(html).toContain('data-scroll-position="left"');
    expect(html).toContain('data-table-kind="service"');
    expect(html).toContain('data-testid="traffic-table"');
    expect(html).toContain('记录时间');
    expect(html).toContain('实际上行');
    expect(html).toContain('实际下行');
    expect(html).toContain('扣费倍率');
    expect(html).toContain('合计');
    expect(html).toContain('cursor-help');
    expect(html).toContain('2024/01/15');
    expect(html).toContain('2.00 KB');
    expect(html).toContain('1024.00 B');
    expect(html).toContain('1.50 x');
    expect(html).toContain('4.50 KB');
    expect(html).toContain('>-</td>');
    expect(html).toContain('100.00 B');
    expect(html).toContain('200.00 B');
    expect(html).toContain('0.00 B');
    expect(html.match(/data-row-key="0"/g)).toHaveLength(1);
    expect(html.match(/data-row-key="1"/g)).toHaveLength(1);
    expect(html).not.toContain('ant-table-scroll-position');
  });

  it('uses the shared scroll-position hook without fixed-column row height shims', () => {
    expect(trafficSource).toContain('useTableScrollPosition(rows.length)');
    expect(trafficSource).not.toContain('useFixedColumnRowHeights');
    expect(trafficSource).not.toContain('bodyRowHeightOffset');
    expect(trafficSource).not.toContain('fixedBodyRowExtraPixel');
  });

  it('keeps traffic dates behind the user legacy date formatter', () => {
    expect(trafficSource).toContain(
      "import { formatUserLegacyDateSlash } from '@/lib/legacy-date';",
    );
    expect(trafficSource).toContain('formatUserLegacyDateSlash(row.original.record_at)');
    expect(trafficSource).not.toContain('formatDate(row.record_at).replaceAll');
  });

  it('keeps the legacy charged total coercion expression', () => {
    expect(trafficSource).toContain(
      '(upload + download) * (row.original.server_rate as unknown as number)',
    );
    expect(trafficSource).not.toContain('Number(row.server_rate)');
  });

  it('renders traffic rows through shared TanStack DataTable columns', () => {
    expect(trafficSource).toContain('satisfies DataTableColumn<(typeof rows)[number]>[]');
    expect(trafficSource).not.toContain('data-row-key={row.record_at}');
    expect(trafficSource).not.toContain('<TableRow');
  });
});
