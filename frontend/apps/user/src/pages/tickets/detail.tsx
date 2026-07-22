import { useEffect, useRef, useState, type SyntheticEvent } from 'react';
import { useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Send } from 'lucide-react';
import { getErrorPresentation } from '@v2board/api-client';
import { Badge } from '@v2board/ui/badge';
import { Button } from '@v2board/ui/button';
import { ErrorState } from '@v2board/ui/error-state';
import { Input } from '@v2board/ui/input';
import { cn } from '@v2board/ui/cn';
import { formatBackendDateMinuteSlash } from '@v2board/config/format';
import { toast } from '@v2board/app-shell/toast';
import { useReplyTicketMutation, useTicket } from '@/lib/queries';
import { translateRuntimeMessage } from '@v2board/ui/translate-runtime-message';

export default function TicketDetailPage() {
  const { ticket_id } = useParams();
  const { t, i18n } = useTranslation();
  const ticketId = ticket_id;
  const ticket = useTicket(ticketId, { refetchInterval: 5000 });
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState('');
  const chatRef = useRef<HTMLDivElement | null>(null);
  const messages = ticket.data?.message ?? [];
  const missingTicketText = translateRuntimeMessage(i18n, 'Ticket does not exist');
  const ticketError = ticket.isError ? getErrorPresentation(ticket.error) : null;
  // GET /user/tickets/{id} is path-identified, so a missing ticket is a real
  // 404 `ticket_not_found` (docs/api-dialect.md §3.4, W8) — never a message
  // match, so unrelated transport/5xx failures stay labelled as errors.
  const isNotFound = ticketError?.status === 404;
  // A closed ticket (status 1) rejects replies server-side; gate the composer so
  // the user sees why instead of hitting a silent failure on submit.
  const isClosed = ticket.data?.status === 1;

  useEffect(() => {
    const chat = chatRef.current;
    if (!chat) return;
    chat.scrollTo(0, chat.scrollHeight);
  }, [messages.length]);

  const submitReply = (event?: SyntheticEvent<HTMLFormElement>) => {
    event?.preventDefault();
    if (reply.isPending) return;
    const loadingToast = toast.loading(t(($) => $.ticket.reply_sending));
    reply.mutate(
      { id: ticketId as string, message: message || undefined },
      {
        onSuccess: () => {
          toast.success(t(($) => $.ticket.reply_success));
          setMessage('');
        },
        onSettled: () => toast.dismiss(loadingToast),
      },
    );
  };

  const data = { ...ticket.data, message: messages };
  const emptyNotice = ticket.data
    ? undefined
    : isNotFound
      ? missingTicketText
      : ticket.isError
        ? undefined
        : t(($) => $.common.loading);

  return (
    <div
      data-slot="ticket-detail"
      className="flex min-h-svh animate-in flex-col bg-background text-foreground duration-200 fade-in-0 slide-in-from-bottom-1 motion-reduce:animate-none"
      data-testid="ticket-detail"
    >
      <header className="border-b border-border px-4 py-3" data-testid="ticket-detail-header">
        <div className="flex min-w-0 items-center gap-3">
          <Badge variant="secondary" className="shrink-0">
            #{ticketId}
          </Badge>
          <h1 className="min-w-0 truncate text-base font-semibold text-foreground">
            {data?.subject ?? emptyNotice}
          </h1>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto px-4 py-4" data-testid="ticket-chat" ref={chatRef}>
        <div className="space-y-4">
          {data?.message.map((item, index) => (
            <div
              className={cn('flex flex-col gap-1', item.is_me ? 'items-end' : 'items-start')}
              key={index}
            >
              <div className="text-xs text-muted-foreground">
                {formatBackendDateMinuteSlash(item.created_at)}
              </div>
              <div
                className={cn(
                  'max-w-[82%] rounded-lg px-3 py-2 text-sm leading-6 shadow-xs',
                  item.is_me ? 'bg-primary text-primary-foreground' : 'bg-muted text-foreground',
                )}
              >
                {item.message}
              </div>
            </div>
          ))}
          {emptyNotice ? (
            <div className="py-10 text-center text-sm text-muted-foreground">{emptyNotice}</div>
          ) : null}
          {ticket.isError && !isNotFound ? (
            <ErrorState
              message={ticketError?.message}
              onRetry={() => void ticket.refetch()}
              data-testid="ticket-detail-error"
            />
          ) : null}
        </div>
      </div>

      {ticket.data && isClosed ? (
        <div
          className="border-t border-border bg-background p-4 text-center text-sm text-muted-foreground"
          data-testid="ticket-closed-notice"
        >
          {t(($) => $.ticket.closed_notice)}
        </div>
      ) : ticket.data ? (
        <form
          className="border-t border-border bg-background p-3"
          data-testid="ticket-reply-form"
          onSubmit={submitReply}
        >
          <div className="flex items-center gap-2">
            <Input
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              type="text"
              data-testid="ticket-reply-input"
              placeholder={t(($) => $.ticket.reply_placeholder)}
            />
            <Button
              type="submit"
              size="icon"
              data-testid="ticket-reply-send"
              loading={reply.isPending}
              aria-label={t(($) => $.ticket.reply_placeholder)}
            >
              <Send className="size-4" />
            </Button>
          </div>
        </form>
      ) : null}
    </div>
  );
}
