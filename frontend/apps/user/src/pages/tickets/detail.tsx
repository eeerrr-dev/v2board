import { useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useReplyTicketMutation, useTicket } from '@/lib/queries';
import { formatDateMinuteSlash } from '@v2board/config/format';
import { toast } from '@/lib/legacy-toast';

export default function TicketDetailPage() {
  const { id } = useParams();
  const { t } = useTranslation();
  const ticketId = Number(id);
  const ticket = useTicket(ticketId);
  const reply = useReplyTicketMutation();
  const [message, setMessage] = useState<string | undefined>();
  const chatRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const timer = window.setInterval(() => {
      ticket.refetch();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [ticket.refetch]);

  useEffect(() => {
    const chat = chatRef.current;
    if (!chat) return;
    chat.scrollTo(0, chat.scrollHeight);
  }, [ticket.data?.message?.length]);

  const submitReply = async () => {
    if (reply.isPending) return;
    const toastId = toast.loading('发送中');
    try {
      await reply.mutateAsync({ id: ticketId, message });
      toast.dismiss(toastId);
      toast.success('发送成功');
      setMessage(undefined);
    } catch {
      toast.dismiss(toastId);
    }
  };

  const data = ticket.data;
  const messages = data?.message ?? [];

  return (
    <div>
      <div className="block-content-full bg-gray-lighter p-3">
        <span className="tag___12_9H">{data?.subject}</span>
      </div>
      <div
        className="bg-white js-chat-messages block-content block-content-full text-wrap-break-word overflow-y-auto content___DW5w1"
        ref={chatRef}
      >
        {messages.map((item) =>
          item.is_me ? (
            <div key={item.id}>
              <div className="font-size-sm text-muted my-2 text-right">
                {formatDateMinuteSlash(item.created_at)}
              </div>
              <div className="text-right ml-4">
                <div className="d-inline-block bg-gray-lighter px-3 py-2 mb-2 mw-100 rounded text-left">
                  {item.message}
                </div>
              </div>
            </div>
          ) : (
            <div key={item.id}>
              <div className="font-size-sm text-muted my-2">
                {formatDateMinuteSlash(item.created_at)}
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
          value={message ?? ''}
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
