import { useEffect, useState } from 'react';
import dayjs from 'dayjs';
import { useNavigate } from 'react-router';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import {
  ArrowDown,
  ArrowUp,
  Ban,
  ChevronsUpDown,
  FileSpreadsheet,
  ListFilter,
  Mail,
  SlidersHorizontal,
  Trash2,
  UserPlus,
} from 'lucide-react';
import type { AdminFilter } from '@v2board/api-client';
import type { AdminUserRow } from '@v2board/types';
import { copyText } from '@v2board/config/clipboard';
import { formatBackendDateMinuteSlash, formatDateTime } from '@v2board/config/format';
import { takeStoredAdminFilters } from '@/lib/stored-admin-filters';
import {
  useAdminPlans,
  useAdminUsers,
  useBanUsersMutation,
  useDeleteAllUsersMutation,
  useDeleteUserMutation,
  useDumpUsersCsvMutation,
  useGenerateUserMutation,
  useResetUserSecretMutation,
  useSendMailToUsersMutation,
  useServerGroups,
} from '@/lib/queries';
import { UserManageDrawer } from '@/components/user-manage-drawer';
import { UserTrafficModal } from '@/components/user-traffic-modal';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { toast } from '@/lib/toast';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { PaginationControl } from '@/components/ui/pagination';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge } from '@/components/ui/status-badge';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { cn } from '@/lib/cn';
import { AssignOrderModal } from './assign-order-modal';
import { UserFilterSheet } from './filter-sheet';
import { GenerateUserModal } from './generate-user-modal';
import { UserRowActions } from './row-actions';
import { SendMailModal } from './send-mail-modal';
import { PLAN_NONE, type FilterField, type PlanOption } from './shared';

interface QueryState {
  current: number;
  pageSize: number;
  filter: AdminFilter[];
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
}

const PAGE_SIZE_OPTIONS = [10, 50, 100, 150];

function paginationLabels(t: TFunction) {
  return {
    itemsPerPage: t(($) => $.common.items_per_page),
    nextPage: t(($) => $.common.next_page),
    nextWindow: t(($) => $.common.next_5),
    previousPage: t(($) => $.common.prev_page),
    previousWindow: t(($) => $.common.prev_5),
  };
}

// Cross-page Tier-1 contract: the order manager (and the dashboard) seed an
// AdminFilter[] into sessionStorage then navigate to /user. Read it on mount,
// apply it as the initial filter, and clear it.
function readStoredUserFilter(): AdminFilter[] {
  return takeStoredAdminFilters('v2board-admin-user-filter');
}

function downloadText(name: string, buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = name;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

function downloadGeneratedUserCsv(buffer: unknown) {
  downloadText(`USER ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`, buffer);
}

function SortableColumnHeader({
  label,
  active,
  direction,
  onSort,
}: {
  label: string;
  active: boolean;
  direction?: 'ASC' | 'DESC';
  onSort: () => void;
}) {
  const Icon = active ? (direction === 'ASC' ? ArrowUp : ArrowDown) : ChevronsUpDown;
  return (
    <button
      type="button"
      className="inline-flex items-center gap-1.5 rounded-sm transition-colors outline-none select-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={onSort}
    >
      {label}
      <Icon className={cn('size-3.5', !active && 'opacity-50')} aria-hidden="true" />
    </button>
  );
}

export default function UsersPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [currentUnixTime, setCurrentUnixTime] = useState(() => Date.now() / 1000);
  const [query, setQuery] = useState<QueryState>(() => ({
    current: 1,
    pageSize: 10,
    filter: readStoredUserFilter(),
  }));
  const users = useAdminUsers(query);
  const plans = useAdminPlans();
  const groups = useServerGroups();
  const remove = useDeleteUserMutation();
  const resetSecret = useResetUserSecretMutation();
  const generate = useGenerateUserMutation();
  const dumpCsv = useDumpUsersCsvMutation();
  const sendMail = useSendMailToUsersMutation();
  const banUsers = useBanUsersMutation();
  const deleteAll = useDeleteAllUsersMutation();
  const planData = plans.data;
  const groupData = groups.data;
  const plansReady = !plans.isError && planData !== undefined;
  const groupsReady = !groups.isError && groupData !== undefined;

  const [editing, setEditing] = useState<AdminUserRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [mailOpen, setMailOpen] = useState(false);
  const [filterOpen, setFilterOpen] = useState(false);
  const [assigning, setAssigning] = useState<AdminUserRow | null>(null);
  const [trafficUser, setTrafficUser] = useState<AdminUserRow | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => setCurrentUnixTime(Date.now() / 1000), 60_000);
    return () => window.clearInterval(timer);
  }, []);

  const planOptions: PlanOption[] = plansReady
    ? planData.map((plan) => ({ label: plan.name, value: plan.id }))
    : [];

  const groupMap = new Map<number, string>();
  if (groupsReady) {
    for (const group of groupData) groupMap.set(group.id, group.name);
  }

  // The `condition` entries (including '模糊') are wire values sent verbatim in
  // the admin filter payload — data contracts, not display copy.
  const filterFields: FilterField[] = [
    { key: 'email', title: t(($) => $.admin.users.email), condition: ['模糊'] },
    { key: 'id', title: t(($) => $.admin.users.user_id), condition: ['=', '>=', '>', '<', '<='] },
    ...(plansReady
      ? [
          {
            key: 'plan_id',
            title: t(($) => $.admin.users.plan),
            condition: ['='],
            type: 'select' as const,
            options: [
              { label: t(($) => $.admin.users.no_plan), value: PLAN_NONE },
              ...planOptions.map((plan) => ({ label: plan.label, value: plan.value })),
            ],
          },
        ]
      : []),
    {
      key: 'transfer_enable',
      title: t(($) => $.admin.users.transfer),
      condition: ['>=', '>', '<', '<='],
    },
    { key: 'd', title: t(($) => $.admin.users.download), condition: ['>=', '>', '<', '<='] },
    {
      key: 'expired_at',
      title: t(($) => $.admin.users.expired_at),
      condition: ['>=', '>', '<', '<='],
      type: 'date',
    },
    { key: 'uuid', title: 'UUID', condition: ['='] },
    { key: 'token', title: 'TOKEN', condition: ['='] },
    {
      key: 'banned',
      title: t(($) => $.admin.users.account_status),
      condition: ['='],
      type: 'select',
      options: [
        { label: t(($) => $.admin.users.status_normal), value: 0 },
        { label: t(($) => $.admin.users.status_banned), value: 1 },
      ],
    },
    {
      key: 'invite_by_email',
      title: t(($) => $.admin.users.invite_user_email),
      condition: ['模糊'],
    },
    { key: 'invite_user_id', title: t(($) => $.admin.users.invite_user_id), condition: ['='] },
    { key: 'remarks', title: t(($) => $.admin.users.remarks), condition: ['模糊'] },
    {
      key: 'is_admin',
      title: t(($) => $.admin.users.is_admin),
      condition: ['='],
      type: 'select',
      options: [
        { label: t(($) => $.common.yes), value: 1 },
        { label: t(($) => $.common.no), value: 0 },
      ],
    },
  ];

  const data = users.data?.data ?? [];
  const total = users.data?.total ?? 0;

  const setFilter = (filter: AdminFilter[]) =>
    setQuery((state) => ({ ...state, current: 1, filter }));

  const sortBy = (key: string) =>
    setQuery((state) => ({
      ...state,
      current: 1,
      sort: key,
      sort_type: state.sort === key && state.sort_type === 'ASC' ? 'DESC' : 'ASC',
    }));

  const jumpOrderFilter = (key: string, condition: string, value: string | number) => {
    window.sessionStorage.setItem(
      'v2board-admin-order-filter',
      JSON.stringify([{ key, condition, value }]),
    );
    void navigate('/order');
  };

  const resetUserSecret = async (row: AdminUserRow) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.users.reset_secret_title),
      description: t(($) => $.admin.users.reset_secret_confirm, { email: row.email }),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    resetSecret.mutate(row.id, {
      onSuccess: () => {
        toast.success(t(($) => $.admin.users.reset_success));
      },
    });
  };

  const deleteUser = async (row: AdminUserRow) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.users.delete_user),
      description: t(($) => $.admin.users.delete_user_confirm, { email: row.email }),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    remove.mutate(row.id, {
      onSuccess: () => {
        toast.success(t(($) => $.admin.users.delete_success));
      },
    });
  };

  const copySubscribeUrl = async (row: AdminUserRow) => {
    if (await copyText(row.subscribe_url)) toast.success(t(($) => $.admin.users.copy_success));
    else toast.error(t(($) => $.admin.users.copy_fail));
  };

  const runUserAction = (key: string, row: AdminUserRow) => {
    if (key === 'edit') setEditing(row);
    if (key === 'assign' && plansReady) setAssigning(row);
    if (key === 'copy') void copySubscribeUrl(row);
    if (key === 'reset') void resetUserSecret(row);
    if (key === 'orders') jumpOrderFilter('user_id', '=', row.id);
    if (key === 'invite') setFilter([{ key: 'invite_user_id', condition: '=', value: row.id }]);
    if (key === 'traffic') setTrafficUser(row);
    if (key === 'delete') void deleteUser(row);
  };

  const exportCsv = () => {
    const toastId = toast.loading(t(($) => $.admin.users.exporting));
    dumpCsv.mutate(query.filter, {
      onSuccess: (response) => {
        downloadText(`${formatDateTime(Date.now() / 1000)}.csv`, response.buffer);
      },
      onSettled: () => {
        toast.dismiss(toastId);
      },
    });
  };

  const bulkBan = async () => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.users.reminder),
      description: t(($) => $.admin.users.bulk_ban_confirm),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    banUsers.mutate(query.filter);
  };

  const bulkDelete = async () => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.users.reminder),
      description: t(($) => $.admin.users.bulk_delete_confirm),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    deleteAll.mutate(query.filter);
  };

  const sortHeader = (label: string, key: string) => () => (
    <SortableColumnHeader
      label={label}
      active={query.sort === key}
      direction={query.sort_type}
      onSort={() => sortBy(key)}
    />
  );

  const renderEmail = (row: AdminUserRow) => {
    // §6.6 (W12): the `t` last-online marker is dropped from the modern list
    // projection, so an absent value degrades to offline rather than a
    // misleading always-online dot.
    const onlineAt = (row as AdminUserRow & { t?: number | null }).t;
    const online = onlineAt != null && currentUnixTime - 600 <= Number(onlineAt);
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="inline-flex items-center gap-2">
            <span
              className={cn(
                'size-2 shrink-0 rounded-full',
                online ? 'bg-success' : 'bg-muted-foreground',
              )}
            />
            {row.email}
          </span>
        </TooltipTrigger>
        <TooltipContent>
          {onlineAt
            ? t(($) => $.admin.users.last_online_at, { time: formatDateTime(Number(onlineAt)) })
            : t(($) => $.admin.users.never_online)}
        </TooltipContent>
      </Tooltip>
    );
  };

  const renderDeviceLimit = (row: AdminUserRow) => {
    const deviceCount = row.alive_ip !== null ? row.alive_ip : 0;
    const deviceLimit = row.device_limit !== null ? row.device_limit : '∞';
    const text = `${deviceCount} / ${deviceLimit}`;
    return row.ips ? (
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="tabular-nums">{text}</span>
        </TooltipTrigger>
        <TooltipContent>{row.ips}</TooltipContent>
      </Tooltip>
    ) : (
      <span className="tabular-nums">{text}</span>
    );
  };

  const renderExpiredAt = (value: string | null) => {
    // §6.6 (W12): `expired_at` crosses as an RFC 3339 string (null = long-term).
    const epoch = value == null ? null : dayjs(value).unix();
    const expired = epoch !== null && epoch < currentUnixTime;
    return (
      <StatusBadge tone={expired ? 'destructive' : 'success'}>
        {value ? formatBackendDateMinuteSlash(value) : t(($) => $.common.long_term)}
      </StatusBadge>
    );
  };

  const columns: DataTableColumn<AdminUserRow>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: sortHeader('ID', 'id'),
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'email',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.users.email)}</span>,
      cell: ({ row }) => renderEmail(row.original),
    },
    {
      id: 'banned',
      header: sortHeader(
        t(($) => $.admin.users.status),
        'banned',
      ),
      cell: ({ row }) => (
        <StatusBadge tone={row.original.banned ? 'destructive' : 'success'}>
          {row.original.banned
            ? t(($) => $.admin.users.status_banned)
            : t(($) => $.admin.users.status_normal)}
        </StatusBadge>
      ),
    },
    {
      id: 'plan_id',
      header: sortHeader(
        t(($) => $.admin.users.plan),
        'plan_id',
      ),
      cell: ({ row }) => row.original.plan_name || '-',
    },
    {
      id: 'group_id',
      header: sortHeader(
        t(($) => $.admin.users.group),
        'group_id',
      ),
      cell: ({ row }) =>
        row.original.group_id != null ? (groupMap.get(row.original.group_id) ?? '-') : '-',
    },
    {
      id: 'total_used',
      meta: { align: 'right' },
      header: sortHeader(
        t(($) => $.admin.users.used_g),
        'total_used',
      ),
      cell: ({ row }) => {
        const over =
          parseFloat(String(row.original.total_used)) >
          parseFloat(String(row.original.transfer_enable));
        return (
          <StatusBadge tone={over ? 'destructive' : 'success'}>
            {row.original.total_used}
          </StatusBadge>
        );
      },
    },
    {
      id: 'transfer_enable',
      meta: { align: 'right', className: 'tabular-nums' },
      header: sortHeader(
        t(($) => $.admin.users.transfer_g),
        'transfer_enable',
      ),
      cell: ({ row }) => row.original.transfer_enable,
    },
    {
      id: 'device',
      meta: { align: 'right' },
      header: sortHeader(
        t(($) => $.admin.users.device_count),
        'updated_at',
      ),
      cell: ({ row }) => renderDeviceLimit(row.original),
    },
    {
      id: 'expired_at',
      header: sortHeader(
        t(($) => $.admin.users.expired_at),
        'expired_at',
      ),
      cell: ({ row }) => renderExpiredAt(row.original.expired_at),
    },
    {
      id: 'balance',
      meta: { align: 'right', className: 'tabular-nums' },
      header: sortHeader(
        t(($) => $.admin.users.balance),
        'balance',
      ),
      cell: ({ row }) => row.original.balance,
    },
    {
      id: 'commission_balance',
      meta: { align: 'right', className: 'tabular-nums' },
      header: sortHeader(
        t(($) => $.admin.users.commission),
        'commission_balance',
      ),
      cell: ({ row }) => row.original.commission_balance,
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: sortHeader(
        t(($) => $.admin.users.created_at),
        'created_at',
      ),
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
      cell: ({ row }) => (
        <UserRowActions row={row.original} onAction={runUserAction} assignDisabled={!plansReady} />
      ),
    },
  ];

  return (
    <PageShell data-testid="users-page">
      {users.isError ? (
        <ErrorState
          message={t(($) => $.admin.users.load_failed)}
          onRetry={() => void users.refetch()}
        />
      ) : null}
      {plans.isError ? (
        <ErrorState
          message={t(($) => $.admin.users.plans_load_failed)}
          onRetry={() => void plans.refetch()}
        />
      ) : null}
      {groups.isError ? (
        <ErrorState
          message={t(($) => $.admin.users.groups_load_failed)}
          onRetry={() => void groups.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.users.title)}
        actions={
          <Button
            onClick={() => setCreating(true)}
            disabled={!plansReady}
            data-testid="user-create"
          >
            <UserPlus className="size-4" />
            {t(($) => $.admin.users.create_user)}
          </Button>
        }
      />

      <TooltipProvider delayDuration={100}>
        <Card className="overflow-hidden py-0">
          <CardContent className="p-0">
            <div className="flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  variant={query.filter.length ? 'default' : 'outline'}
                  size="sm"
                  onClick={() => setFilterOpen(true)}
                  data-testid="user-filter-open"
                >
                  <ListFilter className="size-4" />
                  {t(($) => $.admin.users.filter)}
                  {query.filter.length ? (
                    <span className="ml-1 inline-flex size-5 items-center justify-center rounded-full bg-primary-foreground text-xs text-primary">
                      {query.filter.length}
                    </span>
                  ) : null}
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm" data-testid="user-bulk-actions">
                      <SlidersHorizontal className="size-4" />
                      {t(($) => $.common.operation)}
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start">
                    <DropdownMenuItem onClick={exportCsv} data-testid="user-export-csv">
                      <FileSpreadsheet className="size-4" />
                      {t(($) => $.admin.users.export_csv)}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onClick={() => setMailOpen(true)}
                      data-testid="user-send-mail"
                    >
                      <Mail className="size-4" />
                      {t(($) => $.admin.users.send_mail)}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      disabled={!query.filter.length}
                      onClick={() => void bulkBan()}
                      data-testid="user-bulk-ban"
                    >
                      <Ban className="size-4" />
                      {t(($) => $.admin.users.bulk_ban)}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      variant="destructive"
                      disabled={!query.filter.length}
                      onClick={() => void bulkDelete()}
                      data-testid="user-bulk-delete"
                    >
                      <Trash2 className="size-4" />
                      {t(($) => $.admin.users.bulk_delete)}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
                {query.filter.length ? (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setFilter([])}
                    data-testid="user-filter-reset"
                  >
                    {t(($) => $.admin.users.clear_filter)}
                  </Button>
                ) : null}
              </div>
              <p className="text-xs text-muted-foreground">{t(($) => $.admin.users.filter_tips)}</p>
            </div>

            <DataTable
              columns={columns}
              data={data}
              getRowKey={(row) => row.id}
              className="min-w-[1280px]"
              data-testid="users-table"
              empty={
                !users.isError && users.data !== undefined && data.length === 0
                  ? t(($) => $.admin.users.no_users)
                  : undefined
              }
              emptyTestId="users-empty"
              virtualizer={{ enabled: data.length > VIRTUALIZE_MIN_ROWS }}
            />

            {total > 0 ? (
              <PaginationControl
                current={query.current}
                pageSize={query.pageSize}
                total={total}
                pageSizeOptions={PAGE_SIZE_OPTIONS}
                labels={paginationLabels(t)}
                onChange={(page, pageSize) =>
                  setQuery((state) => ({ ...state, current: page, pageSize }))
                }
                testIds={{ page: 'user-page', pageSize: 'user-page-size' }}
              />
            ) : null}
          </CardContent>
        </Card>
      </TooltipProvider>

      <UserFilterSheet
        open={filterOpen}
        onOpenChange={setFilterOpen}
        fields={filterFields}
        value={query.filter}
        onApply={setFilter}
      />

      <UserManageDrawer
        userId={editing?.id}
        open={editing != null}
        onClose={() => setEditing(null)}
      />

      <GenerateUserModal
        open={creating && plansReady}
        plans={planOptions}
        loading={generate.isPending}
        onClose={() => setCreating(false)}
        onSubmit={async (values) => {
          if (!plansReady) return;
          const response = await generate.mutateAsync(values);
          if (values.generate_count) downloadGeneratedUserCsv(response.buffer);
          setCreating(false);
        }}
      />

      <SendMailModal
        open={mailOpen}
        filter={query.filter}
        loading={sendMail.isPending}
        onClose={() => setMailOpen(false)}
        onSubmit={async (values) => {
          await sendMail.mutateAsync({ filter: query.filter, ...values });
          toast.success(t(($) => $.admin.users.mail_queued));
          setMailOpen(false);
        }}
      />

      <AssignOrderModal
        user={plansReady ? assigning : null}
        plans={planOptions}
        onClose={() => setAssigning(null)}
      />

      <UserTrafficModal
        userId={trafficUser?.id}
        open={trafficUser != null}
        onClose={() => setTrafficUser(null)}
      />

      {users.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}
