import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import SystemPage, { startQueuePolling } from './system';

// The queue monitor is a redesigned shadcn island (stat cards + DataTable)
// replacing the OneUI block / ant-table replica. The DOM byte-pins
// (block-rounded, ant-table-* classes, si icons) are retired; what stays
// covered is behavior: the stat values, the 'default' workload filter, the
// zh queue-name mapping, and the immediate-then-every-3s self-scheduling poll.

const mocks = vi.hoisted(() => ({
  stats: {
    data: { jobsPerMinute: 12, recentJobs: 34, failedJobs: 5, status: true },
    refetch: vi.fn(),
  },
  workload: {
    data: [
      { name: 'default', processes: 1, length: 2, wait: 3 },
      { name: 'order_handle', processes: 4, length: 5, wait: 6 },
      { name: 'traffic_fetch', processes: 7, length: 8, wait: 9 },
    ],
    refetch: vi.fn(),
  },
}));

vi.mock('@/lib/queries', () => ({
  useQueueStats: () => mocks.stats,
  useQueueWorkload: () => mocks.workload,
}));

describe('SystemPage queue monitor', () => {
  it('renders the overview stats and the running status', () => {
    render(<SystemPage />);

    expect(screen.getByText('当前作业量')).toBeInTheDocument();
    expect(screen.getByText('12')).toBeInTheDocument();
    expect(screen.getByText('34')).toBeInTheDocument();
    expect(screen.getByText('7日内报错数量')).toBeInTheDocument();
    expect(screen.getByText('运行中')).toBeInTheDocument();
    expect(document.querySelector('.block-rounded')).toBeNull();
    expect(document.querySelector('.ant-table-wrapper')).toBeNull();
  });

  it('drops the default queue and maps queue names to their zh labels', () => {
    render(<SystemPage />);

    expect(screen.getByText('订单队列')).toBeInTheDocument();
    expect(screen.getByText('流量消费队列')).toBeInTheDocument();
    // 'default' is filtered out before the table renders.
    expect(screen.getByTestId('queue-workload-table').querySelectorAll('tbody tr')).toHaveLength(2);
  });

  it('loads stats and workload immediately, then re-arms a 3s poll', () => {
    vi.useFakeTimers();
    const refetchStats = vi.fn();
    const refetchWorkload = vi.fn();
    try {
      const stop = startQueuePolling(refetchStats, refetchWorkload);
      expect(refetchStats).toHaveBeenCalledTimes(1);
      expect(refetchWorkload).toHaveBeenCalledTimes(1);

      vi.advanceTimersByTime(3000);
      expect(refetchStats).toHaveBeenCalledTimes(2);
      expect(refetchWorkload).toHaveBeenCalledTimes(2);

      stop();
      vi.advanceTimersByTime(6000);
      expect(refetchStats).toHaveBeenCalledTimes(2);
      expect(refetchWorkload).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });
});
