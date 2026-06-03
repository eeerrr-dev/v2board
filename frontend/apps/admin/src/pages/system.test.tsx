import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import SystemPage, { startLegacyQueuePolling } from './system';

const systemSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'system.tsx'), 'utf8');
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

vi.mock('@/lib/queries', () => ({
  useQueueStats: () => ({
    data: {
      jobsPerMinute: 12,
      recentJobs: 34,
      failedJobs: 5,
      status: true,
    },
  }),
  useQueueWorkload: () => ({
    data: [
      { name: 'default', processes: 1, length: 2, wait: 3 },
      { name: 'order_handle', processes: 4, length: 5, wait: 6 },
      { name: 'traffic_fetch', processes: 7, length: 8, wait: 9 },
    ],
  }),
}));

describe('SystemPage legacy queue monitor', () => {
  it('renders the original queue overview and workload blocks', () => {
    const html = renderToStaticMarkup(<SystemPage />);

    expect(html).toContain('class="block block-rounded "');
    expect(html).toContain('总览');
    expect(html).toContain('当前作业量');
    expect(html).toContain('近一小时处理量');
    expect(html).toContain('7日内报错数量');
    expect(html).toContain('运行中');
    expect(html).toContain('si si-check text-success');
    expect(html).toContain('当前作业详情');
    expect(html).toContain('队列名称');
    expect(html).toContain('作业量');
    expect(html).toContain('任务量');
    expect(html).toContain('占用时间');
    expect(html).toContain('订单队列');
    expect(html).toContain('流量消费队列');
    expect(html).not.toContain('data-row-key="default"');
  });

  it('keeps the legacy workload table without an explicit rowKey', () => {
    expect(systemSource).toContain('pagination={false}');
    expect(systemSource).not.toContain('rowKey="name"');
  });

  it('uses the legacy queue status truthiness directly', () => {
    expect(systemSource).toContain("stats ? (stats.status ? '运行中' : '未启动') : null");
    expect(systemSource).toContain('stats.status ? (');
    expect(systemSource).not.toContain('const running = Boolean(stats?.status);');
  });

  it('loads queue stats and workload immediately, then polls every three seconds', () => {
    const timeoutHandlers: Array<() => void> = [];
    const timeoutIds = [
      {} as ReturnType<typeof window.setTimeout>,
      {} as ReturnType<typeof window.setTimeout>,
    ];
    const refetchStats = vi.fn();
    const refetchWorkload = vi.fn();
    const setTimeoutSpy = vi.spyOn(window, 'setTimeout').mockImplementation((handler) => {
      if (typeof handler === 'function') timeoutHandlers.push(handler);
      return timeoutIds[timeoutHandlers.length - 1] ?? timeoutIds[0]!;
    });
    const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout').mockImplementation(() => undefined);

    try {
      const stopPolling = startLegacyQueuePolling(refetchStats, refetchWorkload);

      expect(systemSource).toContain(
        [
          'useEffect(',
          '    () => startLegacyQueuePolling(queueStats.refetch, queueWorkload.refetch),',
          '    [],',
          '  );',
        ].join('\n'),
      );
      expect(systemSource).toContain('3000');
      expect(systemSource).toContain('window.setTimeout');
      expect(systemSource).toContain('window.clearTimeout');
      expect(systemSource).not.toContain('window.setInterval');
      expect(systemSource).not.toContain('[queueStats.refetch, queueWorkload.refetch]');
      expect(queriesSource).not.toContain('refetchInterval: 3000');
      expect(queriesSource).not.toContain('useSystemStatus');
      expect(queriesSource).not.toContain('useSystemLog');
      expect(queriesSource.match(/enabled: false/g)).toHaveLength(2);
      expect(refetchStats).toHaveBeenCalledTimes(1);
      expect(refetchWorkload).toHaveBeenCalledTimes(1);
      expect(setTimeoutSpy).toHaveBeenCalledWith(expect.any(Function), 3000);

      timeoutHandlers[0]?.();

      expect(refetchStats).toHaveBeenCalledTimes(2);
      expect(refetchWorkload).toHaveBeenCalledTimes(2);
      expect(setTimeoutSpy).toHaveBeenCalledTimes(2);

      stopPolling();

      expect(clearTimeoutSpy).toHaveBeenCalledWith(timeoutIds[1]);
    } finally {
      setTimeoutSpy.mockRestore();
      clearTimeoutSpy.mockRestore();
    }
  });
});
