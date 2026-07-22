import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { Copy, Plus, Send, TrendingUp, Users, WalletCards } from 'lucide-react';
import { useEmptyDescription } from '@/lib/use-empty-description';
import { TransferDialog } from '@/components/dialogs/transfer-dialog';
import { WithdrawDialog } from '@/components/dialogs/withdraw-dialog';
import { Button } from '@v2board/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@v2board/ui/card';
import { ErrorState } from '@v2board/ui/error-state';
import { HeaderTooltip } from '@v2board/ui/header-tooltip';
import { PaginationControl } from '@v2board/ui/pagination';
import { PageShell } from '@v2board/ui/page';
import { LoadingState } from '@v2board/ui/loading-state';
import { Skeleton } from '@v2board/ui/skeleton';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';
import { TooltipProvider } from '@v2board/ui/tooltip';
import { cn } from '@v2board/ui/cn';
import {
  useCommConfig,
  useGenerateInviteMutation,
  useInvite,
  useInviteDetails,
  useUserInfo,
  userKeys,
} from '@/lib/queries';
import { formatBackendDateMinuteSlash, formatCentsPlain } from '@v2board/config/format';
import { copyText } from '@v2board/config/clipboard';
import { toast } from '@v2board/app-shell/toast';

// The invite API receives the raw requested page; only the visible pagination
// control clamps a now-empty final page after the total changes.
function getVisibleCommissionPage(total: number, current: number, pageSize: number) {
  if (pageSize <= 0) return 0;
  const pageCount = Math.floor((total - 1) / pageSize) + 1;
  return (current - 1) * pageSize >= total ? pageCount : current;
}

export default function InvitePage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  // Old componentDidMount dispatch order: user/getUserInfo, invite/details,
  // invite/fetch, then comm/config.
  const userInfo = useUserInfo({ refetchOnMount: 'always' });
  const [page, setPage] = useState<number | undefined>();
  const [pageSize, setPageSize] = useState<number | undefined>();
  const details = useInviteDetails(page, pageSize);
  const invite = useInvite();
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const generate = useGenerateInviteMutation();
  const symbol = comm?.currency_symbol;

  // §9.2 named stat object (was the legacy 5-tuple): commissions in cents,
  // rate an integer percent.
  const stat = invite.data?.stat;
  const registered = stat?.registered_count;
  const validCommission = stat?.valid_commission;
  const pendingCommission = stat?.pending_commission;
  const rate = stat?.commission_rate;
  const available = userInfo.data?.commission_balance;
  const availableText =
    userInfo.data?.commission_balance !== undefined
      ? formatCentsPlain(userInfo.data.commission_balance)
      : '--.--';
  const codes = invite.data?.codes ?? [];
  const isDistribution = Boolean(comm?.commission_distribution_enable);
  const commissionRate = isDistribution
    ? rate === undefined
      ? undefined
      : [
          comm?.commission_distribution_l1,
          comm?.commission_distribution_l2,
          comm?.commission_distribution_l3,
        ]
          .map((level) => `${Number((Number(level ?? 0) * (rate / 100)).toFixed(2))}%`)
          .join(',')
    : rate === undefined
      ? undefined
      : `${rate}%`;
  const loading = invite.isFetching;
  const detailRows = details.data?.data ?? [];
  const detailPaginationTotal = details.data?.total ?? detailRows.length;
  const detailPaginationItemTotal = detailPaginationTotal || detailRows.length;
  const detailPaginationCurrent = getVisibleCommissionPage(
    detailPaginationItemTotal,
    page ?? 1,
    pageSize ?? 10,
  );
  const detailsLoading = details.isFetching;
  const emptyDescription = useEmptyDescription();
  const codeColumns = [
    {
      header: t(($) => $.invite.code_col),
      cell: ({ row }) => (
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium text-foreground">{row.original.code}</span>
          <Button
            type="button"
            variant="link"
            className="h-auto p-0 text-sm"
            data-testid="invite-copy-link"
            onClick={() => void copyInviteLink(row.original.code)}
          >
            <Copy className="size-3.5" />
            {t(($) => $.invite.invite_link)}
          </Button>
        </div>
      ),
    },
    {
      header: t(($) => $.invite.created_at_col),
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
      meta: { align: 'right', className: 'text-muted-foreground' },
    },
  ] satisfies DataTableColumn<(typeof codes)[number]>[];
  const detailColumns = [
    {
      header: t(($) => $.invite.issued_at),
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
      meta: { className: 'text-muted-foreground' },
    },
    {
      header: t(($) => $.invite.commission_col),
      cell: ({ row }) => formatCentsPlain(row.original.get_amount),
      meta: { align: 'right', className: 'font-medium text-foreground' },
    },
  ] satisfies DataTableColumn<(typeof detailRows)[number]>[];

  const copyInviteLink = async (code: string) => {
    // Tier-1 copy-link URL, path-style since history routing
    // (docs/api-dialect.md §10.1/§10.4): external invitees land on /register.
    const url = `${window.location.origin}/register?code=${code}`;
    if (await copyText(url)) toast.success(t(($) => $.dashboard.copy_success));
  };

  const generateInvite = () => {
    if (generate.isPending) return;
    generate.mutate(undefined, {
      onSuccess: () => {
        toast.success(t(($) => $.invite.generated));
        void queryClient.invalidateQueries({ queryKey: userKeys.invite, exact: true });
      },
    });
  };

  return (
    <TooltipProvider delayDuration={100}>
      <PageShell className="max-w-6xl gap-4" data-testid="invite-surface">
        <Card className="overflow-hidden" data-testid="invite-summary-card">
          <CardContent className="grid gap-5 p-6 @3xl/main:grid-cols-[minmax(0,1fr)_auto] @3xl/main:items-end">
            <div className="min-w-0 space-y-3">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <WalletCards className="size-4" />
                <span>{t(($) => $.invite.title)}</span>
              </div>
              <div className="flex flex-wrap items-end gap-x-3 gap-y-1">
                <span className="text-4xl font-semibold tracking-tight text-foreground">
                  {availableText}
                </span>
                <span className="pb-1 text-sm font-medium text-muted-foreground">
                  {comm?.currency}
                </span>
              </div>
              <p className="text-sm text-muted-foreground">{t(($) => $.invite.available)}</p>
            </div>
            <div className="flex flex-col gap-2 sm:flex-row @3xl/main:justify-end">
              <TransferDialog max={available}>
                <Button type="button" data-testid="invite-transfer-trigger">
                  <Send className="size-4" />
                  {t(($) => $.invite.transfer)}
                </Button>
              </TransferDialog>
              {!comm?.withdraw_close && (
                <WithdrawDialog methods={comm?.withdraw_methods ?? []}>
                  <Button type="button" variant="outline" data-testid="invite-withdraw-trigger">
                    <WalletCards className="size-4" />
                    {t(($) => $.invite.withdraw_button)}
                  </Button>
                </WithdrawDialog>
              )}
            </div>
          </CardContent>
        </Card>

        {/* A failed invite fetch must not leave the stat tiles spinning forever
            or fall through to an empty-looking code table — surface the error
            with a retry instead. */}
        <Card className={cn(loading && 'opacity-80')} data-testid="invite-stats-card">
          {invite.isError ? (
            <CardContent>
              <ErrorState onRetry={() => void invite.refetch()} data-testid="invite-stats-error" />
            </CardContent>
          ) : (
            <CardContent className="grid gap-0 p-0 @xl/main:grid-cols-2 @5xl/main:grid-cols-4">
              <StatTile
                icon={<Users className="size-4" />}
                label={t(($) => $.invite.registered)}
                value={
                  registered !== undefined
                    ? t(($) => $.invite.people_count, { count: registered })
                    : undefined
                }
              />
              <StatTile
                icon={<TrendingUp className="size-4" />}
                label={
                  isDistribution ? (
                    <HeaderTooltip title={t(($) => $.invite.triple_hint)}>
                      {t(($) => $.invite.triple_rate)}
                    </HeaderTooltip>
                  ) : (
                    t(($) => $.invite.commission_rate)
                  )
                }
                value={commissionRate}
              />
              <StatTile
                icon={<WalletCards className="size-4" />}
                label={
                  <HeaderTooltip title={t(($) => $.invite.pending_hint)}>
                    {t(($) => $.invite.pending_commission)}
                  </HeaderTooltip>
                }
                value={
                  pendingCommission !== undefined
                    ? `${symbol} ${formatCentsPlain(pendingCommission)}`
                    : undefined
                }
              />
              <StatTile
                icon={<WalletCards className="size-4" />}
                label={t(($) => $.invite.valid_commission)}
                value={
                  validCommission !== undefined
                    ? `${symbol} ${formatCentsPlain(validCommission)}`
                    : undefined
                }
              />
            </CardContent>
          )}
        </Card>

        <Card className="overflow-hidden" data-testid="invite-code-card">
          <CardHeader className="gap-3 sm:flex sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle>{t(($) => $.invite.manage)}</CardTitle>
              <CardDescription>{t(($) => $.invite.invite_link)}</CardDescription>
            </div>
            <Button
              type="button"
              data-testid="invite-generate"
              loading={generate.isPending}
              onClick={() => void generateInvite()}
            >
              <Plus className="size-4" />
              {t(($) => $.invite.generate)}
            </Button>
          </CardHeader>
          <CardContent className={invite.isError ? undefined : 'p-0'}>
            {invite.isError ? (
              <ErrorState onRetry={() => void invite.refetch()} data-testid="invite-code-error" />
            ) : (
              <ServiceTable
                testId="invite-code-table"
                columns={codeColumns}
                data={codes}
                empty={
                  invite.data !== undefined && codes.length === 0 ? emptyDescription : undefined
                }
              />
            )}
          </CardContent>
        </Card>

        <Card className="overflow-hidden" data-testid="invite-history-card">
          <CardHeader className="gap-3 sm:flex sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle>{t(($) => $.invite.history)}</CardTitle>
              <CardDescription>{t(($) => $.invite.commission_col)}</CardDescription>
            </div>
            {detailsLoading ? (
              <LoadingState className="w-auto">
                <Skeleton className="h-4 w-28" aria-hidden />
              </LoadingState>
            ) : null}
          </CardHeader>
          <CardContent className={details.isError ? undefined : 'p-0'}>
            {details.isError ? (
              <ErrorState
                onRetry={() => void details.refetch()}
                data-testid="invite-history-error"
              />
            ) : (
              <>
                <ServiceTable
                  testId="invite-history-table"
                  columns={detailColumns}
                  data={detailRows}
                  empty={
                    details.data !== undefined && detailRows.length === 0
                      ? emptyDescription
                      : undefined
                  }
                />
                {detailPaginationItemTotal > 0 && (
                  <PaginationControl
                    data-testid="invite-pagination"
                    current={detailPaginationCurrent}
                    labels={{
                      itemsPerPage: t(($) => $.common.items_per_page),
                      nextPage: t(($) => $.common.next_page),
                      nextWindow: t(($) => $.common.next_5),
                      previousPage: t(($) => $.common.prev_page),
                      previousWindow: t(($) => $.common.prev_5),
                    }}
                    pageSize={pageSize ?? 10}
                    total={detailPaginationItemTotal}
                    testIds={{
                      page: 'invite-page',
                      pageSize: 'invite-page-size',
                    }}
                    onChange={(nextPage, nextPageSize) => {
                      setPage(nextPage);
                      setPageSize(nextPageSize);
                    }}
                  />
                )}
              </>
            )}
          </CardContent>
        </Card>
      </PageShell>
    </TooltipProvider>
  );
}

function StatTile({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: ReactNode;
  value?: ReactNode;
}) {
  return (
    <div className="flex min-h-28 flex-col justify-between gap-4 border-b border-border p-5 last:border-b-0 xl:border-r xl:border-b-0 xl:last:border-r-0 sm:[&:nth-child(odd)]:border-r">
      <div className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
        <span className="inline-flex min-w-0 items-center gap-2">
          <span className="text-muted-foreground">{icon}</span>
          <span className="min-w-0 truncate">{label}</span>
        </span>
      </div>
      <div className="min-h-7 text-right text-2xl font-semibold tracking-tight text-foreground">
        {value ?? <Skeleton className="ml-auto h-7 w-20" aria-hidden />}
      </div>
    </div>
  );
}

function ServiceTable<TData>({
  columns,
  data,
  empty,
  testId,
}: {
  columns: DataTableColumn<TData>[];
  data: TData[];
  empty?: string;
  testId: string;
}) {
  return (
    <DataTable
      className="min-w-[620px]"
      columns={columns}
      data={data}
      data-testid={testId}
      empty={empty}
      emptyTestId="invite-empty"
      headerClassName="border-y"
      scrollProps={{ 'data-testid': 'invite-table-scroll' }}
    />
  );
}
