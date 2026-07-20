import { useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { admin } from '@v2board/api-client';
import { formatBackendDate, formatBytes } from '@v2board/config/format';
import { useAdminUserTraffic } from '@/lib/queries';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { ErrorState } from '@v2board/ui/error-state';
import { PaginationControl } from '@v2board/ui/pagination';
import { LoadingState, SkeletonRows } from '@v2board/ui/loading-state';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';

function paginationLabels(t: TFunction) {
  return {
    itemsPerPage: t(($) => $.common.items_per_page),
    nextPage: t(($) => $.common.next_page),
    nextWindow: t(($) => $.common.next_5),
    previousPage: t(($) => $.common.prev_page),
    previousWindow: t(($) => $.common.prev_5),
  };
}

interface TrafficPagination {
  current: number;
  pageSize: number;
}

// The modal keeps its local {current, pageSize} state; the API layer mints
// the §8 page/per_page query for GET stats/user-traffic (§6.8, W14). The
// shadcn presentation around it is Tier-2.
const INITIAL_PAGINATION: TrafficPagination = { current: 1, pageSize: 10 };

function trafficColumns(t: TFunction): DataTableColumn<admin.AdminUserTrafficRecord>[] {
  return [
    {
      id: 'record_at',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.users.date)}</span>,
      // §6.8 (W14): record_at crosses the wire as an RFC 3339 instant.
      cell: ({ row }) => formatBackendDate(row.original.record_at),
    },
    {
      id: 'u',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.users.upload)}</span>,
      cell: ({ row }) => formatBytes(row.original.u),
    },
    {
      id: 'd',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.users.download)}</span>,
      cell: ({ row }) => formatBytes(row.original.d),
    },
    {
      id: 'server_rate',
      meta: { align: 'right', className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.users.rate)}</span>,
      cell: ({ row }) => row.original.server_rate,
    },
  ];
}

export function UserTrafficModal({
  userId,
  open,
  onClose,
}: {
  userId?: number | null;
  open: boolean;
  onClose: () => void;
}) {
  return (
    <UserTrafficModalContent
      key={userId ?? 'no-user'}
      userId={userId}
      open={open}
      onClose={onClose}
    />
  );
}

function UserTrafficModalContent({
  userId,
  open,
  onClose,
}: {
  userId?: number | null;
  open: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [pagination, setPagination] = useState<TrafficPagination>(INITIAL_PAGINATION);
  // The wrapper keys this stateful body by user id, so switching users resets
  // pagination before the first render/query without an effect-driven repair.
  const records = useAdminUserTraffic(userId ?? undefined, pagination, open);

  const data = records.data?.data ?? [];
  const total = records.data?.total ?? 0;
  const columns = trafficColumns(t);

  return (
    <Dialog open={open} onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent
        className="max-h-[calc(100vh-6rem)] gap-0 overflow-hidden p-0 sm:max-w-3xl"
        data-testid="user-traffic-modal"
      >
        <DialogHeader className="border-b border-border px-6 py-4 text-left">
          <DialogTitle>{t(($) => $.admin.users.traffic_title)}</DialogTitle>
          <DialogDescription>{t(($) => $.admin.users.traffic_description)}</DialogDescription>
        </DialogHeader>
        {records.isError ? (
          <div className="px-6 py-8">
            <ErrorState
              data-testid="user-traffic-error"
              message={t(($) => $.admin.users.traffic_load_failed)}
              onRetry={() => void records.refetch()}
            />
          </div>
        ) : records.isPending ? (
          <LoadingState className="min-h-44 py-4" data-testid="user-traffic-loading">
            <SkeletonRows rows={4} />
          </LoadingState>
        ) : (
          <>
            <DataTable
              columns={columns}
              data={data}
              getRowKey={(_row, index) => index}
              scrollClassName="max-h-[60vh] overflow-y-auto"
              data-testid="user-traffic-table"
              empty={data.length === 0 ? t(($) => $.common.empty) : undefined}
              emptyTestId="user-traffic-empty"
            />
            {total > 0 ? (
              <PaginationControl
                current={pagination.current}
                pageSize={pagination.pageSize}
                total={total}
                labels={paginationLabels(t)}
                onChange={(page, pageSize) => setPagination({ current: page, pageSize })}
                testIds={{ page: 'user-traffic-page', pageSize: 'user-traffic-page-size' }}
              />
            ) : null}
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
