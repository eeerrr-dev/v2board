import { useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useReplyTicketMutation, useTicket } from '@/lib/queries';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { toast } from '@/lib/legacy-toast';

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
    toast.loading('发送中');
    try {
      await reply.mutateAsync({ id: ticketId as string, message });
      toast.destroy();
      toast.success('发送成功');
      setMessage(undefined);
      if (inputRef.current) inputRef.current.value = '';
    } catch {
      toast.destroy();
    }
  };

  const data = ticket.data ?? ({ message: [] } as NonNullable<typeof ticket.data>);
  assumeLegacyTicketMessages(data);

  return (
    <div>
      <div className="block-content-full bg-gray-lighter p-3">
        <span className="tag___12_9H">{data?.subject}</span>
      </div>
      <div
        className="bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1"
        ref={chatRef}
      >
        {data?.message.map((item) =>
          item.is_me ? (
            <div>
              <div className="font-size-sm text-muted my-2 text-right">
                {formatLegacyDateMinuteSlash(item.created_at)}
              </div>
              <div className="text-right ml-4">
                <div className="d-inline-block bg-gray-lighter px-3 py-2 mb-2 mw-100 rounded text-left">
                  {item.message}
                </div>
              </div>
            </div>
          ) : (
            <div>
              <div className="font-size-sm text-muted my-2">
                {formatLegacyDateMinuteSlash(item.created_at)}
              </div>
              <div className="mr-4">
                <div className="d-inline-block bg-success-lighter px-3 py-2 mb-2 mw-100 rounded text-left">
                  {item.message}
                </div>
              </div>
            </div>
          ),
        )}
      </div>
      <div className="js-chat-form block-content p-2 bg-body-dark input___1j_ND">
        <input
          ref={inputRef}
          onChange={(event) => setMessage(event.target.value)}
          onKeyDown={(event) => {
            if (event.keyCode === 13) void submitReply();
          }}
          type="text"
          className="js-chat-input bg-body-dark border-0 form-control form-control-alt"
          placeholder={t('ticket.reply_placeholder')}
        />
      </div>
    </div>
  );
}
