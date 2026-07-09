import { useEffect, useRef, useState } from 'react';
import type { admin } from '@v2board/api-client';
import { formatBytes, formatDate } from '@v2board/config/format';
import { useAdminUserTraffic } from '@/lib/queries';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { PaginationControl } from '@/components/ui/pagination';
import { DataTable, type DataTableColumn } from '@/components/ui/table';

const PAGINATION_LABELS = {
  itemsPerPage: '条/页',
  nextPage: '下一页',
  nextWindow: '向后 5 页',
  previousPage: '上一页',
  previousWindow: '向前 5 页',
};

interface TrafficPagination {
  current: number;
  pageSize: number;
}

// The backend `/stat/getStatUser` reads `current` + `pageSize` (see the Rust
// `page()` helper / Laravel StatController). Sending those two keys is the Tier-1
// contract; the shadcn presentation around it is Tier-2.
const INITIAL_PAGINATION: TrafficPagination = { current: 1, pageSize: 10 };

const columns: DataTableColumn<admin.AdminUserTrafficRecord>[] = [
  {
    id: 'record_at',
    meta: { className: 'text-muted-foreground tabular-nums' },
    header: () => <span>日期</span>,
    cell: ({ row }) => formatDate(row.original.record_at),
  },
  {
    id: 'u',
    meta: { align: 'right', className: 'tabular-nums' },
    header: () => <span>上行</span>,
    cell: ({ row }) => formatBytes(row.original.u),
  },
  {
    id: 'd',
    meta: { align: 'right', className: 'tabular-nums' },
    header: () => <span>下行</span>,
    cell: ({ row }) => formatBytes(row.original.d),
  },
  {
    id: 'server_rate',
    meta: { align: 'right', className: 'tabular-nums' },
    header: () => <span>倍率</span>,
    cell: ({ row }) => row.original.server_rate,
  },
];

export function UserTrafficModal({
  userId,
  open,
  onClose,
}: {
  userId?: number | null;
  open: boolean;
  onClose: () => void;
}) {
  const [pagination, setPagination] = useState<TrafficPagination>(INITIAL_PAGINATION);
  // Opening the modal for a different user must jump back to page 1 in the same
  // render that issues the fetch, so the first request is never for a stale page.
  const lastUserIdRef = useRef<number | null | undefined>(undefined);
  const shouldResetPagination =
    open &&
    userId != null &&
    lastUserIdRef.current !== undefined &&
    lastUserIdRef.current !== userId;
  const queryPagination = shouldResetPagination ? INITIAL_PAGINATION : pagination;
  const records = useAdminUserTraffic(userId ?? undefined, queryPagination, open);

  useEffect(() => {
    if (!open || userId == null) return;
    if (shouldResetPagination) setPagination(INITIAL_PAGINATION);
    lastUserIdRef.current = userId;
  }, [open, shouldResetPagination, userId]);

  const data = records.data?.data ?? [];
  const total = records.data?.total ?? 0;

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent
        className="max-h-[calc(100vh-6rem)] gap-0 overflow-hidden p-0 sm:max-w-3xl"
        data-testid="user-traffic-modal"
      >
        <DialogHeader className="border-b border-border px-6 py-4 text-left">
          <DialogTitle>流量记录</DialogTitle>
        </DialogHeader>
        <DataTable
          columns={columns}
          data={data}
          getRowKey={(_row, index) => index}
          scrollClassName="max-h-[60vh] overflow-y-auto"
          data-testid="user-traffic-table"
          empty={data.length === 0 ? '暂无数据' : undefined}
          emptyTestId="user-traffic-empty"
        />
        {total > 0 ? (
          <PaginationControl
            current={queryPagination.current}
            pageSize={pagination.pageSize}
            total={total}
            labels={PAGINATION_LABELS}
            onChange={(page, pageSize) => setPagination({ current: page, pageSize })}
            testIds={{ page: 'user-traffic-page', pageSize: 'user-traffic-page-size' }}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
