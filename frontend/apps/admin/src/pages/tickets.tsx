import { useEffect, useRef, useState } from 'react';
import dayjs from 'dayjs';
import { Activity, CircleX, Filter, MessageSquare, Send, User } from 'lucide-react';
import type { Ticket } from '@v2board/types';
import { useParams } from 'react-router';
import type { admin } from '@v2board/api-client';
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
import { PaginationControl } from '@/components/ui/pagination';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { Sheet, SheetContent, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { Textarea } from '@/components/ui/textarea';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { DataTable, type DataTableColumn } from '@/components/ui/table';

// The extra keys (status / email / reply_status) are the same admin ticket-fetch
// filters the legacy console sent; `fetchTickets` spreads the whole query into
// the request params, so their names/shapes are the preserved data contract.
type TicketQuery = admin.AdminPageQuery & {
  status?: number;
  email?: string;
  reply_status?: number[] | null;
};

const PAGINATION_LABELS = {
  itemsPerPage: '条/页',
  nextPage: '下一页',
  nextWindow: '向后 5 页',
  previousPage: '上一页',
  previousWindow: '向前 5 页',
};

const REPLY_STATUS_OPTIONS = [
  { label: '已回复', value: 1 },
  { label: '待回复', value: 0 },
];

const TICKET_LEVELS: Record<number, { label: string; tone: StatusTone }> = {
  0: { label: '低', tone: 'default' },
  1: { label: '中', tone: 'warning' },
  2: { label: '高', tone: 'destructive' },
};

function formatMinute(value: number) {
  return dayjs(1000 * value).format('YYYY/MM/DD HH:mm');
}

function renderTicketStatus(row: Ticket) {
  // Backend-field interpretation preserved from the replica: a closed ticket
  // wins over reply_status; otherwise reply_status 1/0 reads replied/awaiting.
  if (row.status === 1) {
    return (
      <StatusBadge tone="success" showDot>
        已关闭
      </StatusBadge>
    );
  }
  return row.reply_status ? (
    <StatusBadge tone="info" showDot>
      已回复
    </StatusBadge>
  ) : (
    <StatusBadge tone="destructive" showDot>
      待回复
    </StatusBadge>
  );
}

export default function TicketsPage() {
  const { ticket_id: ticketId } = useParams();
  if (ticketId) return <TicketChatStandalone ticketId={ticketId} />;
  return <TicketListPage />;
}

function TicketListPage() {
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

  const toggleReplyStatus = (value: number, checked: boolean) => {
    const next = checked
      ? [...replyStatus, value]
      : replyStatus.filter((item) => item !== value);
    patchQuery({ reply_status: next.length ? next : null });
  };

  const closeTicketRow = async (row: Ticket) => {
    const confirmed = await confirmDialog({
      title: '关闭工单',
      description: `确定要关闭工单「${row.subject}」吗？`,
      confirmText: '关闭',
    });
    if (!confirmed) return;
    closeTicket.mutate(row.id, {
      onSuccess: () => {
        toast.success('工单已关闭');
        void tickets.refetch();
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
      header: () => <span>主题</span>,
      cell: ({ row }) => row.original.subject,
    },
    {
      id: 'level',
      header: () => <span>工单级别</span>,
      cell: ({ row }) => {
        const level = TICKET_LEVELS[row.original.level] ?? TICKET_LEVELS[0]!;
        return <StatusBadge tone={level.tone}>{level.label}</StatusBadge>;
      },
    },
    {
      id: 'status',
      header: () => <span>工单状态</span>,
      cell: ({ row }) => renderTicketStatus(row.original),
    },
    {
      id: 'created_at',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>创建时间</span>,
      cell: ({ row }) => formatMinute(row.original.created_at),
    },
    {
      id: 'updated_at',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>最后回复</span>,
      cell: ({ row }) => formatMinute(row.original.updated_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setChatTicketId(row.original.id)}
            data-testid={`ticket-view-${row.original.id}`}
          >
            <MessageSquare className="size-4" />
            查看
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
            关闭
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="tickets-page">
      <PageHeader title="工单管理" />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <div className="flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between">
            <SegmentedControl
              aria-label="工单状态筛选"
              value={String(query.status ?? 0)}
              onValueChange={(value) => patchQuery({ status: Number(value), reply_status: null })}
              items={[
                { label: '已开启', value: '0' },
                { label: '已关闭', value: '1' },
              ]}
            />
            <div className="flex items-center gap-2">
              {query.status !== 1 ? (
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button variant="outline" size="sm" data-testid="ticket-reply-filter">
                      <Filter className="size-4" />
                      筛选
                      {replyStatus.length ? (
                        <span className="ml-1 inline-flex size-5 items-center justify-center rounded-full bg-primary text-xs text-primary-foreground">
                          {replyStatus.length}
                        </span>
                      ) : null}
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuLabel>回复状态</DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    {REPLY_STATUS_OPTIONS.map((option) => (
                      <DropdownMenuCheckboxItem
                        key={option.value}
                        checked={replyStatus.includes(option.value)}
                        onSelect={(event) => event.preventDefault()}
                        onCheckedChange={(checked) => toggleReplyStatus(option.value, checked)}
                      >
                        {option.label}
                      </DropdownMenuCheckboxItem>
                    ))}
                  </DropdownMenuContent>
                </DropdownMenu>
              ) : null}
              <Input
                placeholder="输入邮箱搜索"
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
            empty={data.length === 0 ? '暂无工单' : undefined}
            emptyTestId="tickets-empty"
          />

          {total > 0 ? (
            <PaginationControl
              current={query.current ?? 1}
              pageSize={query.pageSize ?? 10}
              total={total}
              labels={PAGINATION_LABELS}
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
            <SheetTitle>工单详情</SheetTitle>
          </SheetHeader>
          {chatTicketId !== null ? <TicketChat ticketId={chatTicketId} /> : null}
        </SheetContent>
      </Sheet>

      {tickets.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}

// The /ticket/:ticket_id route renders OUTSIDE AdminLayout's SidebarProvider, so
// there is no ancestor `.v2board-island` to resolve the shadcn design tokens.
// Unlike the in-shell list, this standalone route must carry the island
// membership class itself; dark mode still follows the <html>.dark flip.
function TicketChatStandalone({ ticketId }: { ticketId: string }) {
  return (
    <div className="v2board-island flex h-screen justify-center bg-muted/40 text-foreground sm:p-6">
      <div className="flex h-full w-full max-w-3xl flex-col overflow-hidden border-border bg-card sm:rounded-xl sm:border sm:shadow-sm">
        <TicketChat ticketId={ticketId} />
      </div>
    </div>
  );
}

function TicketChat({ ticketId }: { ticketId: number | string }) {
  const ticket = useAdminTicket(ticketId);
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState('');
  const [userOpen, setUserOpen] = useState(false);
  const [trafficOpen, setTrafficOpen] = useState(false);
  const chatRef = useRef<HTMLDivElement | null>(null);
  const current = ticket.data;
  const messageCount = current?.message?.length;

  useAdminUserInfo(current?.user_id);

  useEffect(() => {
    const chat = chatRef.current;
    if (chat) chat.scrollTo(0, chat.scrollHeight);
  }, [messageCount]);

  useEffect(() => {
    // Live refresh while the conversation is open (cadence is Tier-2).
    const timer = window.setInterval(() => void ticket.refetch(), 5000);
    return () => window.clearInterval(timer);
  }, [ticket.refetch]);

  const sendReply = async () => {
    if (reply.isPending || !message.trim()) return;
    const toastId = toast.loading('发送中');
    try {
      await reply.mutateAsync({ id: ticketId, message });
    } finally {
      toast.dismiss(toastId);
    }
    await ticket.refetch();
    setMessage('');
  };

  const emptyNotice = current ? undefined : ticket.isError ? '工单不存在' : '加载中...';

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-2 border-b border-border px-4 py-3">
        <div className="min-w-0">
          <div className="truncate text-base font-semibold text-foreground">
            {current?.subject ?? '工单详情'}
          </div>
          {current ? (
            <div className="text-xs text-muted-foreground">工单 #{current.id}</div>
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
                  aria-label="用户管理"
                  disabled={!current?.user_id}
                  onClick={() => current?.user_id && setUserOpen(true)}
                >
                  <User className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>用户管理</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-8"
                  aria-label="TA的流量记录"
                  disabled={!current?.user_id}
                  onClick={() => current?.user_id && setTrafficOpen(true)}
                >
                  <Activity className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>TA的流量记录</TooltipContent>
            </Tooltip>
          </div>
        </TooltipProvider>
      </div>

      <div
        ref={chatRef}
        data-testid="ticket-chat-messages"
        className="min-h-0 flex-1 space-y-4 overflow-y-auto break-words px-4 py-4"
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
      </div>

      <div className="border-t border-border p-3">
        <div className="flex items-end gap-2">
          <Textarea
            rows={1}
            value={message}
            placeholder="输入内容回复工单..."
            className="max-h-40 min-h-9 resize-none"
            onChange={(event) => setMessage(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                void sendReply();
              }
            }}
            data-testid="ticket-reply-input"
          />
          <Button
            size="icon"
            className="size-9 shrink-0"
            aria-label="发送"
            disabled={reply.isPending || !message.trim()}
            onClick={() => void sendReply()}
            data-testid="ticket-reply-submit"
          >
            <Send className="size-4" />
          </Button>
        </div>
      </div>

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
