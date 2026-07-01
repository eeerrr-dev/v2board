import { useEffect, useRef, useState, type SyntheticEvent } from 'react';
import type { ParseKeys } from 'i18next';
import { useParams } from 'react-router';
import { useTranslation } from 'react-i18next';
import { Send } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/cn';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { toast } from '@/lib/toast';
import { useReplyTicketMutation, useTicket } from '@/lib/queries';

export default function TicketDetailPage() {
  const { ticket_id } = useParams();
  const { t } = useTranslation();
  const ticketId = ticket_id;
  const ticket = useTicket(ticketId, { refetchInterval: 5000 });
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState('');
  const chatRef = useRef<HTMLDivElement | null>(null);
  const messages = ticket.data?.message ?? [];
  // A closed ticket (status 1) rejects replies server-side; gate the composer so
  // the user sees why instead of hitting a silent failure on submit.
  const isClosed = ticket.data?.status === 1;

  useEffect(() => {
    const chat = chatRef.current;
    if (!chat) return;
    chat.scrollTo(0, chat.scrollHeight);
  }, [messages.length]);

  const submitReply = async (event?: SyntheticEvent<HTMLFormElement>) => {
    event?.preventDefault();
    if (reply.isPending) return;
    toast.loading(t('ticket.reply_sending'));
    try {
      await reply.mutateAsync({ id: ticketId as string, message: message || undefined });
      toast.destroy();
      toast.success(t('ticket.reply_success'));
      setMessage('');
    } catch {
      toast.destroy();
    }
  };

  const data = { ...ticket.data, message: messages };
  const emptyNotice = ticket.data
    ? undefined
    : ticket.isError
      ? // Legacy flat-dictionary key resolved at runtime via the merged legacy
        // i18n resources; it is not part of the structured key tree.
        t('Ticket does not exist' as ParseKeys)
      : t('common.loading');

  return (
    <div
      className="v2board-island v2board-page-shell flex min-h-svh flex-col bg-background text-foreground"
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

      <div
        className="js-chat-messages flex-1 overflow-y-auto px-4 py-4"
        data-testid="ticket-chat"
        ref={chatRef}
      >
        <div className="space-y-4">
          {data?.message.map((item, index) => (
            <div
              className={cn('flex flex-col gap-1', item.is_me ? 'items-end' : 'items-start')}
              key={index}
            >
              <div className="text-xs text-muted-foreground">
                {formatLegacyDateMinuteSlash(item.created_at)}
              </div>
              <div
                className={cn(
                  'max-w-[82%] rounded-lg px-3 py-2 text-sm leading-6 shadow-xs',
                  item.is_me
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted text-foreground',
                )}
              >
                {item.message}
              </div>
            </div>
          ))}
          {emptyNotice ? (
            <div className="py-10 text-center text-sm text-muted-foreground">{emptyNotice}</div>
          ) : null}
        </div>
      </div>

      {isClosed ? (
        <div
          className="border-t border-border bg-background p-4 text-center text-sm text-muted-foreground"
          data-testid="ticket-closed-notice"
        >
          {t('ticket.closed_notice')}
        </div>
      ) : (
        <form
          className="js-chat-form border-t border-border bg-background p-3"
          data-testid="ticket-reply-form"
          onSubmit={(event) => void submitReply(event)}
        >
          <div className="flex items-center gap-2">
            <Input
              value={message}
              onChange={(event) => setMessage(event.target.value)}
              type="text"
              className="js-chat-input"
              data-testid="ticket-reply-input"
              placeholder={t('ticket.reply_placeholder')}
            />
            <Button
              type="submit"
              size="icon"
              data-testid="ticket-reply-send"
              loading={reply.isPending}
              aria-label={t('ticket.reply_placeholder')}
            >
              <Send className="size-4" />
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
