import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
  TableScroll,
} from '@/components/ui/table';
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
import { formatUserLegacyDateMinuteSlash } from '@/lib/legacy-date';
import { legacyCopyText } from '@/lib/legacy-settings';
import { toast } from '@/lib/legacy-toast';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

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
  // Faithful to the original: the distribution branch computes the rate
  // unconditionally (l1 * (rate/100), …), so during the load window where comm config
  // has arrived but invite stats have not, rate is undefined → "NaN%,NaN%,NaN%".
  // Only the non-distribution branch guards (rate !== undefined ? `${rate}%` : loading).
  const commissionRate = isDistribution
    ? [
        comm?.commission_distribution_l1,
        comm?.commission_distribution_l2,
        comm?.commission_distribution_l3,
      ]
        .map((level) => `${Number(level) * (Number(rate) / 100)}%`)
        .join(',')
    : rate === undefined
      ? undefined
      : `${rate}%`;
  const loading = invite.isFetching;
  const detailRows = details.data?.data ?? [];
  const detailPaginationTotal = details.data?.total ?? detailRows.length;
  const detailPaginationItemTotal = detailPaginationTotal || detailRows.length;
  const detailPaginationCurrent = getLegacyMaxCurrent(
    detailPaginationItemTotal,
    page ?? 1,
    pageSize ?? 10,
  );
  const detailsLoading = useLegacyFetchLoading(details.isFetching);
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;

  const copyInviteLink = (code: string) => {
    legacyCopyText(`${window.location.origin}${window.location.pathname}#/register?code=${code}`);
    toast.success(t('dashboard.copy_success'));
  };

  const generateInvite = async () => {
    if (generate.isPending) return;
    try {
      await generate.mutateAsync();
      toast.success('已生成');
      void queryClient.invalidateQueries({ queryKey: userKeys.invite, exact: true });
    } catch {}
  };

  return (
    <TooltipProvider delayDuration={100}>
      <div className="v2board-invite-surface space-y-4">
        <Card className="v2board-invite-summary-card overflow-hidden">
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
                <Button type="button" className="v2board-invite-transfer-trigger">
                  <Send className="size-4" />
                  {t('invite.transfer')}
                </Button>
              </TransferDialog>
              {!comm?.withdraw_close && (
                <WithdrawDialog methods={comm?.withdraw_methods ?? []}>
                  <Button
                    type="button"
                    variant="outline"
                    className="v2board-invite-withdraw-trigger"
                  >
                    <WalletCards className="size-4" />
                    {t('invite.withdraw_button')}
                  </Button>
                </WithdrawDialog>
              )}
            </div>
          </CardContent>
        </Card>

        <Card className={cn('v2board-invite-stats-card', loading && 'opacity-80')}>
          <CardContent className="grid gap-0 p-0 sm:grid-cols-2 xl:grid-cols-4">
            <StatTile
              icon={<Users className="size-4" />}
              label={t('invite.registered')}
              value={
                registered !== undefined ? (
                  <>
                    {registered}人
                  </>
                ) : undefined
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

        <Card className="v2board-invite-code-card overflow-hidden">
          <CardHeader className="gap-3 sm:flex sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle>{t('invite.manage')}</CardTitle>
              <CardDescription>{t('invite.invite_link')}</CardDescription>
            </div>
            <Button
              type="button"
              className="v2board-invite-generate"
              loading={generate.isPending}
              onClick={() => void generateInvite()}
            >
              <Plus className="size-4" />
              {t('invite.generate')}
            </Button>
          </CardHeader>
          <CardContent className="p-0">
            <ServiceTable
              className="v2board-invite-code-table"
              empty={codes.length === 0 ? emptyDescription : undefined}
              headers={[
                <span key="code">{t('invite.code_col')}</span>,
                <span key="created" className="inline-block text-right">
                  {t('invite.created_at_col')}
                </span>,
              ]}
            >
              {codes.map((code, index) => (
                <TableRow data-row-key={index} key={index}>
                  <TableCell>
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium text-foreground">{code.code}</span>
                      <Button
                        type="button"
                        variant="link"
                        className="v2board-invite-copy-link h-auto p-0 text-sm"
                        onClick={() => void copyInviteLink(code.code)}
                      >
                        <Copy className="size-3.5" />
                        {t('invite.invite_link')}
                      </Button>
                    </div>
                  </TableCell>
                  <TableCell className="text-right text-muted-foreground">
                    {formatUserLegacyDateMinuteSlash(code.created_at)}
                  </TableCell>
                </TableRow>
              ))}
            </ServiceTable>
          </CardContent>
        </Card>

        <Card className="v2board-invite-history-card overflow-hidden">
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
                <span>Loading...</span>
              </div>
            ) : null}
          </CardHeader>
          <CardContent className="p-0">
            <ServiceTable
              className="v2board-invite-history-table"
              empty={!detailRows.length ? emptyDescription : undefined}
              headers={[
                <span key="issued">{t('invite.issued_at')}</span>,
                <span key="commission" className="inline-block text-right">
                  {t('invite.commission_col')}
                </span>,
              ]}
            >
              {detailRows.map((row, index) => (
                <TableRow data-row-key={index} key={index}>
                  <TableCell className="text-muted-foreground">
                    {formatUserLegacyDateMinuteSlash(row.created_at)}
                  </TableCell>
                  <TableCell className="text-right font-medium text-foreground">
                    {(row.get_amount / 100).toFixed(2)}
                  </TableCell>
                </TableRow>
              ))}
            </ServiceTable>
            {detailPaginationItemTotal > 0 && (
              <InvitePagination
                current={detailPaginationCurrent}
                pageSize={pageSize ?? 10}
                total={detailPaginationItemTotal}
                onChange={(nextPage, nextPageSize) => {
                  setPage(nextPage);
                  setPageSize(nextPageSize);
                }}
              />
            )}
          </CardContent>
        </Card>
      </div>
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

function ServiceTable({
  children,
  className,
  empty,
  headers,
}: {
  children: ReactNode;
  className: string;
  empty?: string;
  headers: ReactNode[];
}) {
  return (
    <TableScroll className="v2board-invite-table-scroll">
      <Table className={cn('v2board-invite-table min-w-[620px]', className)}>
        <TableHeader className="border-y">
          <tr>
            {headers.map((header, index) => (
              <TableHead
                className={index === 0 ? 'text-left' : 'text-right'}
                key={index}
              >
                {header}
              </TableHead>
            ))}
          </tr>
        </TableHeader>
        <TableBody>
          {empty ? (
            <TableEmpty colSpan={2} rowClassName="v2board-invite-empty">
              {empty}
            </TableEmpty>
          ) : (
            children
          )}
        </TableBody>
      </Table>
    </TableScroll>
  );
}

function formatCentsPlain(cents: number) {
  return (parseInt(String(cents)) / 100).toFixed(2);
}

function getLegacyMaxCurrent(total: number, current: number, pageSize: number) {
  return (current - 1) * pageSize >= total ? Math.floor((total - 1) / pageSize) + 1 : current;
}

function InvitePagination({
  current,
  pageSize,
  total,
  onChange,
}: {
  current: number;
  pageSize: number;
  total: number;
  onChange: (page: number, pageSize: number) => void;
}) {
  const { t } = useTranslation();
  // rc-pagination's page count is Math.floor((total - 1) / pageSize) + 1,
  // so total=0 renders its disabled "0" pager instead of an active page 1.
  const totalPages = Math.floor((total - 1) / pageSize) + 1;
  const items = getPaginationItems(current, totalPages);
  const jumpPage = (item: 'jump-prev' | 'jump-next') =>
    item === 'jump-prev' ? Math.max(1, current - 5) : Math.min(totalPages, current + 5);
  const changePage = (targetPage: number) => {
    let nextPage = targetPage;
    if (nextPage > totalPages) nextPage = totalPages;
    if (nextPage < 1) nextPage = 1;
    onChange(nextPage, pageSize);
  };
  const goPrev = () => {
    if (current > 1) onChange(current - 1, pageSize);
  };
  const goNext = () => {
    if (current < totalPages) onChange(current + 1, pageSize);
  };

  return (
    <div className="v2board-invite-pagination flex flex-col gap-3 border-t border-border p-4 sm:flex-row sm:items-center sm:justify-end">
      <div className="flex flex-wrap items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t('common.prev_page')}
          disabled={current <= 1}
          onClick={goPrev}
        >
          <ChevronLeft className="size-4" />
        </Button>
        {items.map((item) =>
          typeof item === 'number' ? (
            <Button
              type="button"
              variant={item === current ? 'default' : 'ghost'}
              size="sm"
              className={cn('v2board-invite-page', `v2board-invite-page-${item}`)}
              aria-current={item === current ? 'page' : undefined}
              disabled={item === 0}
              key={item}
              onClick={() => changePage(item)}
            >
              {item}
            </Button>
          ) : (
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={item === 'jump-prev' ? t('common.prev_5') : t('common.next_5')}
              key={item}
              onClick={() => onChange(jumpPage(item), pageSize)}
            >
              {item === 'jump-prev' ? (
                <ChevronsLeft className="size-4" />
              ) : (
                <ChevronsRight className="size-4" />
              )}
            </Button>
          ),
        )}
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t('common.next_page')}
          disabled={current >= totalPages}
          onClick={goNext}
        >
          <ChevronRight className="size-4" />
        </Button>
      </div>
      <Select
        value={String(pageSize)}
        onValueChange={(value) => {
          const ps = Number.parseInt(value, 10);
          const nextTotalPages = Math.floor((total - 1) / ps) + 1;
          onChange(nextTotalPages === 0 ? current : Math.min(current, nextTotalPages), ps);
        }}
      >
        <SelectTrigger className="v2board-invite-page-size h-9 w-full sm:w-36">
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          {[10, 50, 100, 150].map((size) => (
            <SelectItem key={size} value={String(size)}>
              {size} {t('common.items_per_page')}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function getPaginationItems(
  current: number,
  totalPages: number,
): Array<number | 'jump-prev' | 'jump-next'> {
  if (totalPages === 0) return [0];
  if (totalPages <= 9) return Array.from({ length: totalPages }, (_, index) => index + 1);

  let left = Math.max(2, current - 2);
  let right = Math.min(totalPages - 1, current + 2);
  if (current - 1 <= 2) right = 5;
  if (totalPages - current <= 2) left = totalPages - 4;
  const items: Array<number | 'jump-prev' | 'jump-next'> = [1];

  if (left > 2) items.push('jump-prev');
  for (let page = left; page <= right; page += 1) items.push(page);
  if (right < totalPages - 1) items.push('jump-next');
  items.push(totalPages);

  return items;
}
