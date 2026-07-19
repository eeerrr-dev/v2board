import { useState } from 'react';
import type { SelectorParam } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { admin, FilterClause } from '@v2board/api-client';
import { formatBackendDateTime } from '@v2board/config/format';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { Input } from '@/components/ui/input';
import { PageHeader, PageShell } from '@/components/ui/page';
import { PaginationControl } from '@/components/ui/pagination';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { useAuditLogs } from '@/lib/queries';

// Keyed by the wire `surface` values; labels resolve through t() at render.
const SURFACE_LABEL_KEYS: Record<string, SelectorParam> = {
  admin: ($) => $.admin.audit.surface_admin,
  staff: ($) => $.admin.audit.surface_staff,
};

// Radix Select items cannot carry an empty value, so the "all" choices use a
// sentinel that simply omits the §7 filter clause.
const ALL_SURFACES = 'all';
const ALL_METHODS = 'all';

// The trail only records non-GET/HEAD requests, so these are the only method
// values that can appear in a row.
const METHODS = ['POST', 'PUT', 'PATCH', 'DELETE'];

interface QueryState {
  current: number;
  pageSize: number;
  surface: string;
  actorEmail: string;
  method: string;
}

function auditFilter(query: QueryState): FilterClause<admin.AuditLogFilterField>[] | undefined {
  const clauses: FilterClause<admin.AuditLogFilterField>[] = [];
  if (query.surface !== ALL_SURFACES) {
    clauses.push({ field: 'surface', op: 'eq', value: query.surface });
  }
  if (query.actorEmail !== '') {
    clauses.push({ field: 'actor_email', op: 'like', value: query.actorEmail });
  }
  if (query.method !== ALL_METHODS) {
    clauses.push({ field: 'method', op: 'eq', value: query.method });
  }
  return clauses.length ? clauses : undefined;
}

/**
 * The §6.11 operator audit trail (`GET system/audit-logs`): a read-only,
 * newest-first view of every recorded admin/staff mutation. The table is
 * append-only on the backend — this page never mutates anything.
 */
export default function AuditPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState<QueryState>({
    current: 1,
    pageSize: 20,
    surface: ALL_SURFACES,
    actorEmail: '',
    method: ALL_METHODS,
  });
  // The email input is drafted locally and only becomes a filter clause on
  // Enter/blur, so typing does not fire a request per keystroke.
  const [emailDraft, setEmailDraft] = useState('');
  const logs = useAuditLogs({
    page: query.current,
    per_page: query.pageSize,
    filter: auditFilter(query),
  });

  const applyEmailDraft = () => {
    const actorEmail = emailDraft.trim();
    setQuery((state) =>
      state.actorEmail === actorEmail ? state : { ...state, current: 1, actorEmail },
    );
  };

  const data = logs.data?.items ?? [];
  const total = logs.data?.total ?? 0;

  const columns: DataTableColumn<admin.AdminAuditLogRecord>[] = [
    {
      id: 'created_at',
      meta: { className: 'whitespace-nowrap tabular-nums' },
      header: () => <span>{t(($) => $.admin.audit.time)}</span>,
      cell: ({ row }) => formatBackendDateTime(row.original.created_at),
    },
    {
      id: 'actor_email',
      meta: { className: 'text-foreground' },
      header: () => <span>{t(($) => $.admin.audit.actor)}</span>,
      cell: ({ row }) => row.original.actor_email,
    },
    {
      id: 'surface',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.admin.audit.surface)}</span>,
      cell: ({ row }) => {
        const labelKey = SURFACE_LABEL_KEYS[row.original.surface];
        return (
          <Badge variant={row.original.surface === 'admin' ? 'default' : 'secondary'}>
            {labelKey ? t(labelKey) : row.original.surface}
          </Badge>
        );
      },
    },
    {
      id: 'method',
      meta: { align: 'center', className: 'font-mono text-xs' },
      header: () => <span>{t(($) => $.admin.audit.method)}</span>,
      cell: ({ row }) => row.original.method,
    },
    {
      id: 'path',
      meta: { className: 'font-mono text-xs break-all' },
      header: () => <span>{t(($) => $.admin.audit.path)}</span>,
      cell: ({ row }) => row.original.path,
    },
    {
      id: 'status_code',
      meta: { align: 'center', className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.audit.status)}</span>,
      cell: ({ row }) => (
        <Badge variant={row.original.status_code < 400 ? 'secondary' : 'destructive'}>
          {row.original.status_code}
        </Badge>
      ),
    },
    {
      id: 'client_ip',
      meta: { className: 'font-mono text-xs' },
      header: () => <span>{t(($) => $.admin.audit.client_ip)}</span>,
      cell: ({ row }) => row.original.client_ip ?? '-',
    },
    {
      id: 'request_id',
      meta: { className: 'max-w-40 truncate font-mono text-xs' },
      header: () => <span>{t(($) => $.admin.audit.request_id)}</span>,
      cell: ({ row }) => row.original.request_id ?? '-',
    },
  ];

  return (
    <PageShell data-testid="audit-page">
      <PageHeader
        title={t(($) => $.admin.audit.title)}
        description={t(($) => $.admin.audit.description)}
      />

      <Card>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={query.surface}
              onValueChange={(surface) => setQuery((state) => ({ ...state, current: 1, surface }))}
            >
              <SelectTrigger
                className="w-40"
                aria-label={t(($) => $.admin.audit.surface_filter_label)}
                data-testid="audit-surface-filter"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL_SURFACES}>{t(($) => $.admin.audit.all_surfaces)}</SelectItem>
                <SelectItem value="admin">{t(($) => $.admin.audit.surface_admin)}</SelectItem>
                <SelectItem value="staff">{t(($) => $.admin.audit.surface_staff)}</SelectItem>
              </SelectContent>
            </Select>
            <Select
              value={query.method}
              onValueChange={(method) => setQuery((state) => ({ ...state, current: 1, method }))}
            >
              <SelectTrigger
                className="w-40"
                aria-label={t(($) => $.admin.audit.method_filter_label)}
                data-testid="audit-method-filter"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL_METHODS}>{t(($) => $.admin.audit.all_methods)}</SelectItem>
                {METHODS.map((method) => (
                  <SelectItem key={method} value={method}>
                    {method}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Input
              className="w-56"
              placeholder={t(($) => $.admin.audit.email_filter_placeholder)}
              aria-label={t(($) => $.admin.audit.email_filter_label)}
              data-testid="audit-email-filter"
              value={emailDraft}
              onChange={(event) => setEmailDraft(event.target.value)}
              onBlur={applyEmailDraft}
              onKeyDown={(event) => {
                if (event.key === 'Enter') applyEmailDraft();
              }}
            />
          </div>

          {logs.isError ? (
            <ErrorState
              message={t(($) => $.admin.audit.load_error)}
              onRetry={() => void logs.refetch()}
              data-testid="audit-error"
            />
          ) : (
            <>
              <DataTable
                columns={columns}
                data={data}
                getRowKey={(row) => row.id}
                className="min-w-[960px]"
                data-testid="audit-table"
                empty={
                  !logs.isError && logs.data !== undefined && data.length === 0
                    ? t(($) => $.admin.audit.empty)
                    : undefined
                }
                emptyTestId="audit-empty"
              />

              {total > 0 ? (
                <PaginationControl
                  current={query.current}
                  pageSize={query.pageSize}
                  total={total}
                  labels={{
                    itemsPerPage: t(($) => $.common.items_per_page),
                    nextPage: t(($) => $.common.next_page),
                    nextWindow: t(($) => $.common.next_5),
                    previousPage: t(($) => $.common.prev_page),
                    previousWindow: t(($) => $.common.prev_5),
                  }}
                  onChange={(page, pageSize) =>
                    setQuery((state) => ({ ...state, current: page, pageSize }))
                  }
                  testIds={{ page: 'audit-page-control', pageSize: 'audit-page-size' }}
                />
              ) : null}
            </>
          )}

          {logs.isPending ? (
            <LoadingState className="py-4">
              <SkeletonRows rows={3} />
            </LoadingState>
          ) : null}
        </CardContent>
      </Card>
    </PageShell>
  );
}
