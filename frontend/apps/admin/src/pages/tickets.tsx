import { useEffect, useRef, useState } from 'react';
import dayjs from 'dayjs';
import type { SelectorParam } from 'i18next';
import { useTranslation } from 'react-i18next';
import { Activity, CircleX, Filter, MessageSquare, Send, User } from 'lucide-react';
import type { Ticket } from '@v2board/types';
import { useParams } from 'react-router';
import { getErrorPresentation, type admin } from '@v2board/api-client';
import {
  useAdminTicket,
  useAdminTickets,
  useAdminUserInfo,
  useCloseTicketMutation,
  useReplyTicketMutation,
} from '@/lib/queries';
import { UserManageDrawer } from '@/components/user-manage-drawer';
import { UserTrafficModal } from '@/components/user-traffic-modal';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { toast } from '@/lib/toast';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { PaginationControl } from '@/components/ui/pagination';
import { SegmentedControl } from '@/components/ui/segmented-control';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { Textarea } from '@/components/ui/textarea';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { DataTable, type DataTableColumn } from '@/components/ui/table';

// The status / email / reply_status keys are the §6.5 admin ticket list
// filters; the page keeps its local {current, pageSize} state and the API
// layer mints the §8 page/per_page wire query (docs/api-dialect.md, W14).
type TicketQuery = admin.AdminTicketListQuery;

// The 1/0 values are the §6.5 reply_status wire codes; only the labels are copy.
const REPLY_STATUS_OPTIONS: { labelKey: SelectorParam; value: number }[] = [
  { labelKey: ($) => $.admin.tickets.replied, value: 1 },
  { labelKey: ($) => $.admin.tickets.awaiting_reply, value: 0 },
];

// Keyed by the wire ticket `level` codes; labels resolve through t() at render.
const TICKET_LEVELS: Record<number, { labelKey: SelectorParam; tone: StatusTone }> = {
  0: { labelKey: ($) => $.admin.tickets.level_low, tone: 'default' },
  1: { labelKey: ($) => $.admin.tickets.level_medium, tone: 'warning' },
  2: { labelKey: ($) => $.admin.tickets.level_high, tone: 'destructive' },
};

// §4.5 (W14): ticket timestamps cross the wire as RFC 3339 UTC strings.
function formatMinute(value: string) {
  return dayjs(value).format('YYYY/MM/DD HH:mm');
}

function renderTicketStatus(row: Ticket, translate: (selector: SelectorParam) => string) {
  // Backend-field interpretation preserved from the replica: a closed ticket
  // wins over reply_status; otherwise reply_status 1/0 reads replied/awaiting.
  if (row.status === 1) {
    return (
      <StatusBadge tone="success" showDot>
        {translate(($) => $.admin.tickets.closed)}
      </StatusBadge>
    );
  }
  return row.reply_status ? (
    <StatusBadge tone="info" showDot>
      {translate(($) => $.admin.tickets.replied)}
    </StatusBadge>
  ) : (
    <StatusBadge tone="destructive" showDot>
      {translate(($) => $.admin.tickets.awaiting_reply)}
    </StatusBadge>
  );
}

export default function TicketsPage() {
  const { ticket_id: ticketId } = useParams();
  if (ticketId) return <TicketChatStandalone ticketId={ticketId} />;
  return <TicketListPage />;
}

function TicketListPage() {
  const { t } = useTranslation();
  const [query, setQuery] = useState<TicketQuery>({ current: 1, pageSize: 10, status: 0 });
  const tickets = useAdminTickets(query);
  const closeTicket = useCloseTicketMutation();
  const searchTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [chatTicketId, setChatTicketId] = useState<number | null>(null);
  const data = tickets.data?.data ?? [];
  const total = tickets.data?.total ?? 0;
  const replyStatus = query.reply_status ?? [];

  const patchQuery = (patch: Partial<TicketQuery>) => {
    setQuery((current) => ({ ...current, current: 1, ...patch }));
  };

  const onEmailSearch = (value: string) => {
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => patchQuery({ email: value || undefined }), 300);
  };

  useEffect(() => () => clearTimeout(searchTimer.current), []);

  const toggleReplyStatus = (value: number, checked: boolean) => {
    const next = checked ? [...replyStatus, value] : replyStatus.filter((item) => item !== value);
    patchQuery({ reply_status: next.length ? next : null });
  };

  const closeTicketRow = async (row: Ticket) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.tickets.close_confirm_title),
      description: t(($) => $.admin.tickets.close_confirm_description, { subject: row.subject }),
      confirmText: t(($) => $.common.close),
    });
    if (!confirmed) return;
    closeTicket.mutate(row.id, {
      onSuccess: () => {
        toast.success(t(($) => $.admin.tickets.close_success));
      },
    });
  };

  const columns: DataTableColumn<Ticket>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>#</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'subject',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.tickets.subject)}</span>,
      cell: ({ row }) => row.original.subject,
    },
    {
      id: 'level',
      header: () => <span>{t(($) => $.admin.tickets.level)}</span>,
      cell: ({ row }) => {
        const level = TICKET_LEVELS[row.original.level] ?? TICKET_LEVELS[0]!;
        return <StatusBadge tone={level.tone}>{t(level.labelKey)}</StatusBadge>;
      },
    },
    {
      id: 'status',
      header: () => <span>{t(($) => $.admin.tickets.status)}</span>,
      cell: ({ row }) => renderTicketStatus(row.original, (selector) => t(selector)),
    },
    {
      id: 'created_at',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.tickets.created_at)}</span>,
      cell: ({ row }) => formatMinute(row.original.created_at),
    },
    {
      id: 'updated_at',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.tickets.last_reply)}</span>,
      cell: ({ row }) => formatMinute(row.original.updated_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setChatTicketId(row.original.id)}
            data-testid={`ticket-view-${row.original.id}`}
          >
            <MessageSquare className="size-4" />
            {t(($) => $.admin.tickets.view)}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            disabled={row.original.status === 1}
            onClick={() => void closeTicketRow(row.original)}
            data-testid={`ticket-close-${row.original.id}`}
          >
            <CircleX className="size-4" />
            {t(($) => $.common.close)}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="tickets-page">
      {tickets.isError ? (
        <ErrorState
          message={t(($) => $.admin.tickets.list_error)}
          onRetry={() => void tickets.refetch()}
        />
      ) : null}
      <PageHeader title={t(($) => $.admin.tickets.title)} />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <div className="flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between">
            <SegmentedControl
              aria-label={t(($) => $.admin.tickets.status_filter_label)}
              value={String(query.status ?? 0)}
              onValueChange={(value) => patchQuery({ status: Number(value), reply_status: null })}
              items={[
                { label: t(($) => $.admin.tickets.open), value: '0' },
                { label: t(($) => $.admin.tickets.closed), value: '1' },
              ]}
            />
            <div className="flex items-center gap-2">
              {query.status !== 1 ? (
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm" data-testid="ticket-reply-filter">
                      <Filter className="size-4" />
                      {t(($) => $.admin.tickets.filter)}
                      {replyStatus.length ? (
                        <span className="ml-1 inline-flex size-5 items-center justify-center rounded-full bg-primary text-xs text-primary-foreground">
                          {replyStatus.length}
                        </span>
                      ) : null}
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuLabel>{t(($) => $.admin.tickets.reply_status)}</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    {REPLY_STATUS_OPTIONS.map((option) => (
                      <DropdownMenuCheckboxItem
                        key={option.value}
                        checked={replyStatus.includes(option.value)}
                        onSelect={(event) => event.preventDefault()}
                        onCheckedChange={(checked) => toggleReplyStatus(option.value, checked)}
                      >
                        {t(option.labelKey)}
                      </DropdownMenuCheckboxItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
              ) : null}
              <Input
                placeholder={t(($) => $.admin.tickets.email_search_placeholder)}
                className="w-full sm:w-56"
                onChange={(event) => onEmailSearch(event.target.value)}
                data-testid="ticket-email-search"
              />
            </div>
          </div>

          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[840px]"
            data-testid="tickets-table"
            empty={
              !tickets.isError && tickets.data !== undefined && data.length === 0
                ? t(($) => $.admin.tickets.empty)
                : undefined
            }
            emptyTestId="tickets-empty"
          />

          {total > 0 ? (
            <PaginationControl
              current={query.current ?? 1}
              pageSize={query.pageSize ?? 10}
              total={total}
              labels={{
                itemsPerPage: t(($) => $.common.items_per_page),
                nextPage: t(($) => $.common.next_page),
                nextWindow: t(($) => $.common.next_5),
                previousPage: t(($) => $.common.prev_page),
                previousWindow: t(($) => $.common.prev_5),
              }}
              onChange={(page, pageSize) =>
                setQuery((current) => ({ ...current, current: page, pageSize }))
              }
              testIds={{ page: 'ticket-page', pageSize: 'ticket-page-size' }}
            />
          ) : null}
        </CardContent>
      </Card>

      <Sheet
        open={chatTicketId !== null}
        onOpenChange={(open) => {
          if (!open) setChatTicketId(null);
        }}
      >
        <SheetContent
          side="right"
          className="flex w-full flex-col gap-0 p-0 sm:max-w-xl"
          data-testid="ticket-chat"
        >
          <SheetHeader className="sr-only">
            <SheetTitle>{t(($) => $.admin.tickets.detail)}</SheetTitle>
            <SheetDescription>{t(($) => $.admin.tickets.detail_description)}</SheetDescription>
          </SheetHeader>
          {chatTicketId !== null ? <TicketChat ticketId={chatTicketId} /> : null}
        </SheetContent>
      </Sheet>

      {tickets.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}

function TicketChatStandalone({ ticketId }: { ticketId: string }) {
  return (
    <div
      data-slot="ticket-chat-standalone"
      className="flex h-screen justify-center bg-muted/40 text-foreground sm:p-6"
    >
      <div className="flex h-full w-full max-w-3xl flex-col overflow-hidden border-border bg-card sm:rounded-xl sm:border sm:shadow-sm">
        <TicketChat ticketId={ticketId} />
      </div>
    </div>
  );
}

function TicketChat({ ticketId }: { ticketId: number | string }) {
  const { t } = useTranslation();
  const ticket = useAdminTicket(ticketId);
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState('');
  const [userOpen, setUserOpen] = useState(false);
  const [trafficOpen, setTrafficOpen] = useState(false);
  const chatRef = useRef<HTMLDivElement | null>(null);
  const current = ticket.data;
  const ticketError = ticket.isError ? getErrorPresentation(ticket.error) : null;
  // §3.2: a missing ticket is the modern 404 (`ticket_not_found`); the
  // legacy exact-message match died with the W14 teardown.
  const isNotFound = Boolean(ticketError && ticketError.status === 404);
  const messageCount = current?.message?.length;

  useAdminUserInfo(current?.user_id);

  useEffect(() => {
    const chat = chatRef.current;
    if (chat) chat.scrollTo(0, chat.scrollHeight);
  }, [messageCount]);

  const sendReply = () => {
    if (reply.isPending || !message.trim()) return;
    const toastId = toast.loading(t(($) => $.admin.tickets.reply_sending));
    reply.mutate(
      { id: ticketId, message },
      {
        onSuccess: () => setMessage(''),
        onSettled: () => toast.dismiss(toastId),
      },
    );
  };

  const emptyNotice = current
    ? undefined
    : isNotFound
      ? t(($) => $.admin.tickets.not_found)
      : ticket.isError
        ? undefined
        : t(($) => $.common.loading);

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-2 border-b border-border px-4 py-3">
        <div className="min-w-0">
          <div className="truncate text-base font-semibold text-foreground">
            {current?.subject ?? t(($) => $.admin.tickets.detail)}
          </div>
          {current ? (
            <div className="text-xs text-muted-foreground">
              {t(($) => $.admin.tickets.ticket_number, { id: current.id })}
            </div>
          ) : null}
        </div>
        <TooltipProvider delayDuration={100}>
          <div className="flex items-center gap-1">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  aria-label={t(($) => $.admin.tickets.manage_user)}
                  disabled={!current?.user_id}
                  onClick={() => current?.user_id && setUserOpen(true)}
                >
                  <User className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t(($) => $.admin.tickets.manage_user)}</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  aria-label={t(($) => $.admin.tickets.user_traffic)}
                  disabled={!current?.user_id}
                  onClick={() => current?.user_id && setTrafficOpen(true)}
                >
                  <Activity className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t(($) => $.admin.tickets.user_traffic)}</TooltipContent>
            </Tooltip>
          </div>
        </TooltipProvider>
      </div>

      <div
        ref={chatRef}
        data-testid="ticket-chat-messages"
        className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4 break-words"
      >
        {current?.message?.map((item, index) =>
          item.is_me ? (
            <div key={index} className="flex flex-col items-end">
              <div className="mb-1 text-xs text-muted-foreground">
                {formatMinute(item.created_at)}
              </div>
              <div className="max-w-[85%] rounded-lg bg-muted px-3 py-2 text-sm text-foreground">
                {item.message}
              </div>
            </div>
          ) : (
            <div key={index} className="flex flex-col items-start">
              <div className="mb-1 text-xs text-muted-foreground">
                {formatMinute(item.created_at)}
              </div>
              <div className="max-w-[85%] rounded-lg border border-success/20 bg-success/10 px-3 py-2 text-sm text-foreground">
                {item.message}
              </div>
            </div>
          ),
        )}
        {emptyNotice ? (
          <div className="py-8 text-center text-sm text-muted-foreground">{emptyNotice}</div>
        ) : null}
        {ticket.isError && !isNotFound ? (
          <ErrorState
            message={ticketError?.message}
            onRetry={() => void ticket.refetch()}
            data-testid="ticket-detail-error"
          />
        ) : null}
      </div>

      {current ? (
        <div className="border-t border-border p-3">
          <div className="flex items-end gap-2">
            <Textarea
              rows={1}
              value={message}
              placeholder={t(($) => $.admin.tickets.reply_placeholder)}
              className="max-h-40 min-h-9 resize-none"
              onChange={(event) => setMessage(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !event.shiftKey) {
                  event.preventDefault();
                  sendReply();
                }
              }}
              data-testid="ticket-reply-input"
            />
            <Button
              size="icon"
              className="size-9 shrink-0"
              aria-label={t(($) => $.admin.tickets.send)}
              disabled={reply.isPending || !message.trim()}
              onClick={sendReply}
              data-testid="ticket-reply-submit"
            >
              <Send className="size-4" />
            </Button>
          </div>
        </div>
      ) : null}

      <UserTrafficModal
        key={current?.user_id}
        userId={current?.user_id}
        open={trafficOpen}
        onClose={() => setTrafficOpen(false)}
      />
      <UserManageDrawer
        userId={current?.user_id}
        open={userOpen}
        onClose={() => setUserOpen(false)}
      />
    </div>
  );
}
