import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import type { ReactNode } from 'react';
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

vi.mock('@/components/legacy-tooltip', () => ({
  LegacyTooltip: ({
    title,
    placement,
    children,
  }: {
    title: string;
    placement?: string;
    children: ReactNode;
  }) => (
    <span className="traffic-tooltip-probe" data-placement={placement} data-title={title}>
      {children}
    </span>
  ),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe('TrafficPage legacy loading timing', () => {
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

  it('does not show block loading until after the mount fetch dispatch equivalent', () => {
    const html = renderToStaticMarkup(<TrafficPage />);

    expect(html).toContain('流量明细仅保留近一个月数据以供查询。');
    expect(html).toContain('ant-table-empty');
    expect(html).not.toContain('block-mode-loading');
  });

  it('uses only the outer block-mode-loading state after the mount fetch dispatch equivalent', async () => {
    await act(async () => {
      root!.render(<TrafficPage />);
      await Promise.resolve();
    });

    expect(container.querySelector('.block.block-rounded')?.className).toContain('block-mode-loading');
    expect(container.innerHTML).not.toContain('ant-spin-spinning');
    expect(container.innerHTML).not.toContain('ant-spin-blur');
  });
});

describe('TrafficPage bundled-theme table', () => {
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

  it('renders the legacy table shell, fixed total column, headers, and row formatting', () => {
    const html = renderToStaticMarkup(<TrafficPage />);

    expect(html).toContain('block block-rounded  ');
    expect(html).toContain('ant-table-wrapper');
    expect(html).toContain('style="border-top:1px solid #e8e8e8"');
    expect(html).toContain('class="ant-table-fixed" style="width:800px;table-layout:auto"');
    expect(html).toContain('class="ant-table-fixed" style="table-layout:auto"');
    expect(html).toContain('ant-table-fixed-right');
    expect(html).toContain('记录时间');
    expect(html).toContain('实际上行');
    expect(html).toContain('实际下行');
    expect(html).toContain('扣费倍率');
    expect(html).toContain('合计');
    expect(html).toContain(
      'class="traffic-tooltip-probe" data-placement="topRight" data-title="公式：(实际上行 + 实际下行) x 扣费倍率 = 扣除流量"',
    );
    expect(html).toContain('2024/01/15');
    expect(html).toContain('2.00 KB');
    expect(html).toContain('1024.00 B');
    expect(html).toContain('1.50 x');
    expect(html).toContain('4.50 KB');
    expect(html).toContain('<td>-</td>');
    expect(html).toContain('<td style="text-align:right">100.00 B</td>');
    expect(html).toContain('<td style="text-align:right">200.00 B</td>');
    expect(html).toContain('<td style="text-align:right">0.00 B</td>');
    expect(html).toContain('<span class="ant-tag" style="min-width:60px">-</span>');
    expect(html).not.toContain('data-row-key');
  });

  it('keeps the legacy charged total coercion expression', () => {
    expect(trafficSource).toContain(
      '(upload + download) * (row.server_rate as unknown as number)',
    );
    expect(trafficSource).toContain(
      '(parseInt(String(row.u)) +\n                                  parseInt(String(row.d))) *\n                                (row.server_rate as unknown as number)',
    );
    expect(trafficSource).not.toContain('Number(row.server_rate)');
  });

  it('keeps bundled antd table row keys internal-only', () => {
    expect(trafficSource).not.toContain('data-row-key');
  });
});
