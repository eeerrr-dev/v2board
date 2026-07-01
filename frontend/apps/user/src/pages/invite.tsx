import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import {
  CircleHelp,
  Copy,
  Plus,
  Send,
  TrendingUp,
  Users,
  WalletCards,
} from 'lucide-react';
import { getLocaleAntdMessages } from '@v2board/i18n';
import { TransferDialog } from '@/components/dialogs/transfer-dialog';
import { WithdrawDialog } from '@/components/dialogs/withdraw-dialog';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { PaginationControl, getPaginationMaxCurrent } from '@/components/ui/pagination';
import { PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/cn';
import {
  useCommConfig,
  useGenerateInviteMutation,
  useInvite,
  useInviteDetails,
  useUserInfo,
  userKeys,
} from '@/lib/queries';
import { formatCentsPlain, formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { copyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/toast';

export default function InvitePage() {
  const { t, i18n } = useTranslation();
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

  const stat = invite.data?.stat;
  const registered = stat?.[0];
  const validCommission = stat?.[1];
  const pendingCommission = stat?.[2];
  const rate = stat?.[3];
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
          .map((level) => `${Number(level ?? 0) * (rate / 100)}%`)
          .join(',')
    : rate === undefined
      ? undefined
      : `${rate}%`;
  const loading = invite.isFetching;
  const detailRows = details.data?.data ?? [];
  const detailPaginationTotal = details.data?.total ?? detailRows.length;
  const detailPaginationItemTotal = detailPaginationTotal || detailRows.length;
  const detailPaginationCurrent = getPaginationMaxCurrent(
    detailPaginationItemTotal,
    page ?? 1,
    pageSize ?? 10,
  );
  const detailsLoading = details.isFetching;
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;
  const codeColumns = [
    {
      header: t('invite.code_col'),
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
            {t('invite.invite_link')}
          </Button>
        </div>
      ),
    },
    {
      header: t('invite.created_at_col'),
      cell: ({ row }) => formatLegacyDateMinuteSlash(row.original.created_at),
      meta: { align: 'right', className: 'text-muted-foreground' },
    },
  ] satisfies DataTableColumn<(typeof codes)[number]>[];
  const detailColumns = [
    {
      header: t('invite.issued_at'),
      cell: ({ row }) => formatLegacyDateMinuteSlash(row.original.created_at),
      meta: { className: 'text-muted-foreground' },
    },
    {
      header: t('invite.commission_col'),
      cell: ({ row }) => (row.original.get_amount / 100).toFixed(2),
      meta: { align: 'right', className: 'font-medium text-foreground' },
    },
  ] satisfies DataTableColumn<(typeof detailRows)[number]>[];

  const copyInviteLink = async (code: string) => {
    const url = `${window.location.origin}${window.location.pathname}#/register?code=${code}`;
    if (await copyText(url)) toast.success(t('dashboard.copy_success'));
  };

  const generateInvite = async () => {
    if (generate.isPending) return;
    try {
      await generate.mutateAsync();
      toast.success(t('invite.generated'));
      void queryClient.invalidateQueries({ queryKey: userKeys.invite, exact: true });
    } catch {}
  };

  return (
    <TooltipProvider delayDuration={100}>
      <PageShell className="max-w-6xl gap-4" data-testid="invite-surface">
        <Card className="overflow-hidden" data-testid="invite-summary-card">
          <CardContent className="grid gap-5 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
            <div className="min-w-0 space-y-3">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <WalletCards className="size-4" />
                <span>{t('invite.title')}</span>
              </div>
              <div className="flex flex-wrap items-end gap-x-3 gap-y-1">
                <span className="text-4xl font-semibold tracking-tight text-foreground">
                  {availableText}
                </span>
                <span className="pb-1 text-sm font-medium text-muted-foreground">
                  {comm?.currency}
                </span>
              </div>
              <p className="text-sm text-muted-foreground">{t('invite.available')}</p>
            </div>
            <div className="flex flex-col gap-2 sm:flex-row lg:justify-end">
              <TransferDialog max={available}>
                <Button type="button" data-testid="invite-transfer-trigger">
                  <Send className="size-4" />
                  {t('invite.transfer')}
                </Button>
              </TransferDialog>
              {!comm?.withdraw_close && (
                <WithdrawDialog methods={comm?.withdraw_methods ?? []}>
                  <Button
                    type="button"
                    variant="outline"
                    data-testid="invite-withdraw-trigger"
                  >
                    <WalletCards className="size-4" />
                    {t('invite.withdraw_button')}
                  </Button>
                </WithdrawDialog>
              )}
            </div>
          </CardContent>
        </Card>

        <Card className={cn(loading && 'opacity-80')} data-testid="invite-stats-card">
          <CardContent className="grid gap-0 p-0 sm:grid-cols-2 xl:grid-cols-4">
            <StatTile
              icon={<Users className="size-4" />}
              label={t('invite.registered')}
              value={
                registered !== undefined
                  ? t('invite.people_count', { count: registered })
                  : undefined
              }
            />
            <StatTile
              icon={<TrendingUp className="size-4" />}
              label={
                isDistribution ? (
                  <HeaderTooltip title={t('invite.triple_hint')}>
                    {t('invite.triple_rate')}
                  </HeaderTooltip>
                ) : (
                  t('invite.commission_rate')
                )
              }
              value={commissionRate}
            />
            <StatTile
              icon={<WalletCards className="size-4" />}
              label={
                <HeaderTooltip title={t('invite.pending_hint')}>
                  {t('invite.pending_commission')}
                </HeaderTooltip>
              }
              value={
                pendingCommission !== undefined
                  ? `${symbol} ${pendingCommission / 100}`
                  : undefined
              }
            />
            <StatTile
              icon={<WalletCards className="size-4" />}
              label={t('invite.valid_commission')}
              value={
                validCommission !== undefined ? `${symbol} ${validCommission / 100}` : undefined
              }
            />
          </CardContent>
        </Card>

        <Card className="overflow-hidden" data-testid="invite-code-card">
          <CardHeader className="gap-3 sm:flex sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle>{t('invite.manage')}</CardTitle>
              <CardDescription>{t('invite.invite_link')}</CardDescription>
            </div>
            <Button
              type="button"
              data-testid="invite-generate"
              loading={generate.isPending}
              onClick={() => void generateInvite()}
            >
              <Plus className="size-4" />
              {t('invite.generate')}
            </Button>
          </CardHeader>
          <CardContent className="p-0">
            <ServiceTable
              testId="invite-code-table"
              columns={codeColumns}
              data={codes}
              empty={codes.length === 0 ? emptyDescription : undefined}
            />
          </CardContent>
        </Card>

        <Card className="overflow-hidden" data-testid="invite-history-card">
          <CardHeader className="gap-3 sm:flex sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle>{t('invite.history')}</CardTitle>
              <CardDescription>{t('invite.commission_col')}</CardDescription>
            </div>
            {detailsLoading ? (
              <div
                className="flex items-center gap-2 text-sm text-muted-foreground"
                role="status"
              >
                <Spinner className="size-4" />
                <span>{t('common.loading')}</span>
              </div>
            ) : null}
          </CardHeader>
          <CardContent className="p-0">
            <ServiceTable
              testId="invite-history-table"
              columns={detailColumns}
              data={detailRows}
              empty={!detailRows.length ? emptyDescription : undefined}
            />
            {detailPaginationItemTotal > 0 && (
              <PaginationControl
                data-testid="invite-pagination"
                current={detailPaginationCurrent}
                labels={{
                  itemsPerPage: t('common.items_per_page'),
                  nextPage: t('common.next_page'),
                  nextWindow: t('common.next_5'),
                  previousPage: t('common.prev_page'),
                  previousWindow: t('common.prev_5'),
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
    <div className="flex min-h-28 flex-col justify-between gap-4 border-b border-border p-5 last:border-b-0 sm:[&:nth-child(odd)]:border-r xl:border-b-0 xl:border-r xl:last:border-r-0">
      <div className="flex items-center justify-between gap-3 text-sm text-muted-foreground">
        <span className="inline-flex min-w-0 items-center gap-2">
          <span className="text-muted-foreground">{icon}</span>
          <span className="min-w-0 truncate">{label}</span>
        </span>
      </div>
      <div className="min-h-7 text-right text-2xl font-semibold tracking-tight text-foreground">
        {value ?? <Spinner className="ml-auto size-5 text-muted-foreground" />}
      </div>
    </div>
  );
}

function HeaderTooltip({ children, title }: { children: ReactNode; title: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="v2board-service-tooltip-trigger inline-flex cursor-help items-center gap-1">
          {children}
          <CircleHelp className="size-3.5" />
        </span>
      </TooltipTrigger>
      <TooltipContent>{title}</TooltipContent>
    </Tooltip>
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
