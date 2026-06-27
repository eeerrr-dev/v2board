import { useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Send } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/cn';
import { formatUserLegacyDateMinuteSlash } from '@/lib/legacy-date';
import { toast } from '@/lib/toast';
import { useReplyTicketMutation, useTicket } from '@/lib/queries';

function legacyTicketMessageLength(data?: { message?: unknown[] }) {
  return data?.message!.length;
}

function assumeLegacyTicketMessages<T extends { message?: unknown }>(
  _data: T | undefined,
): asserts _data is T & { message: NonNullable<T['message']> } {}

export default function TicketDetailPage() {
  const { ticket_id } = useParams();
  const { t } = useTranslation();
  const ticketId = ticket_id;
  const ticket = useTicket(ticketId);
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState<string | undefined>();
  const chatRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const pollRef = useRef<(() => void) | undefined>(undefined);

  useEffect(() => {
    // The bundled component stores a module-level `r = () => setTimeout(...)` and
    // componentWillUnmount only sets `r = undefined`; it does not clear the pending
    // timeout. That leaves exactly one already-scheduled fetch after the chat popup
    // closes, then the loop stops because `typeof r` is no longer "function".
    pollRef.current = () => {
      window.setTimeout(() => {
        void ticket.refetch();
        if (typeof pollRef.current === 'function') pollRef.current();
      }, 5000);
    };
    pollRef.current();
    return () => {
      pollRef.current = undefined;
    };
  }, [ticket.refetch]);

  useEffect(() => {
    const chat = chatRef.current;
    if (!chat) return;
    chat.scrollTo(0, chat.scrollHeight);
  }, [legacyTicketMessageLength(ticket.data)]);

  const submitReply = async () => {
    if (reply.isPending) return;
    toast.loading(t('ticket.reply_sending'));
    try {
      await reply.mutateAsync({ id: ticketId as string, message });
      toast.destroy();
      toast.success(t('ticket.reply_success'));
      setMessage(undefined);
      if (inputRef.current) inputRef.current.value = '';
    } catch {
      toast.destroy();
    }
  };

  const data = ticket.data ?? ({ message: [] } as NonNullable<typeof ticket.data>);
  assumeLegacyTicketMessages(data);
  const emptyNotice = ticket.data
    ? undefined
    : ticket.isError
      ? t('Ticket does not exist')
      : t('common.loading');

  return (
    <div
      className="flex min-h-screen flex-col bg-background text-foreground"
      data-testid="ticket-detail"
    >
      <header className="border-b border-border px-4 py-3" data-testid="ticket-detail-header">
        <div className="flex min-w-0 items-center gap-3">
          <Badge variant="secondary" className="shrink-0">
            #{ticketId}
          </Badge>
          <h1 className="min-w-0 truncate text-base font-semibold">
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
                {formatUserLegacyDateMinuteSlash(item.created_at)}
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

      <div
        className="js-chat-form border-t border-border bg-background p-3"
        data-testid="ticket-reply-form"
      >
        <div className="flex items-center gap-2">
          <Input
            ref={inputRef}
            value={message ?? ''}
            onChange={(event) => setMessage(event.target.value)}
            onKeyDown={(event) => {
              // eslint-disable-next-line @typescript-eslint/no-deprecated -- behavior-parity: deprecated API mirrors the legacy frontend (AGENTS.md)
              if (event.keyCode === 13) void submitReply();
            }}
            type="text"
            className="js-chat-input"
            data-testid="ticket-reply-input"
            placeholder={t('ticket.reply_placeholder')}
          />
          <Button
            type="button"
            size="icon"
            data-testid="ticket-reply-send"
            loading={reply.isPending}
            aria-label={t('ticket.reply_placeholder')}
            onClick={() => void submitReply()}
          >
            <Send className="size-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
