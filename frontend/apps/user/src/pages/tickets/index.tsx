import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { AntBtn } from '@/components/ant-btn';
import { LegacyEmpty } from '@/components/legacy-empty';
import { LegacySelect } from '@/components/legacy-select';
import {
  userKeys,
  useCloseTicketMutation,
  useSaveTicketMutation,
  useTickets,
} from '@/lib/queries';
import { formatDateMinuteSlash } from '@v2board/config/format';
import type { TicketLevel } from '@v2board/types';

const LEVELS: { value: TicketLevel; labelKey: string }[] = [
  { value: 0, labelKey: 'ticket.level_low' },
  { value: 1, labelKey: 'ticket.level_medium' },
  { value: 2, labelKey: 'ticket.level_high' },
];

export default function TicketsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { data, isFetching } = useTickets();
  const save = useSaveTicketMutation();
  const close = useCloseTicketMutation();
  const [open, setOpen] = useState(false);
  const [subject, setSubject] = useState<string | undefined>();
  const [level, setLevel] = useState<TicketLevel | undefined>();
  const [message, setMessage] = useState<string | undefined>();

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: userKeys.tickets });
    },
    [queryClient],
  );

  const resetForm = () => {
    setSubject(undefined);
    setLevel(undefined);
    setMessage(undefined);
  };

  const saveTicket = async () => {
    try {
      await save.mutateAsync({ subject, level, message });
      setOpen(false);
      resetForm();
    } catch {}
  };

  const openTicket = (id: number) => {
    const url = `${window.location.origin}${window.location.pathname}#/ticket/${id}`;
    const userAgent = window.navigator.userAgent.toLowerCase();
    if (!userAgent.includes('mobile') && !userAgent.includes('ipad')) {
      window.open(
        url,
        'newwindow',
        'height=600,width=800,top=0,left=0,toolbar=no,menubar=no,scrollbars=no,resizable=no,location=no,status=no',
      );
      return;
    }
    window.location.href = url;
  };

  const tickets = data ?? [];

  return (
    <>
      <div className={`block block-rounded js-appear-enabled ${isFetching ? 'block-mode-loading' : ''}`}>
        <div className="block-header block-header-default">
          <h3 className="block-title">{t('ticket.history')}</h3>
          <div className="block-options">
            <button
              type="button"
              className="btn btn-primary btn-sm btn-primary btn-rounded px-3"
              onClick={() => setOpen(true)}
            >
              {t('ticket.new')}
            </button>
          </div>
        </div>
        <div className="block-content p-0">
          <div className="ant-table-wrapper">
            <div className="ant-table ant-table-default ant-table-scroll-position-left">
              <div className="ant-table-content">
                <div className="ant-table-scroll">
                  <div className="ant-table-body" style={{ overflowX: 'auto' }}>
                    <table style={{ width: 900, minWidth: '100%', tableLayout: 'auto' }}>
                      <thead className="ant-table-thead">
                        <tr>
                          <th className="ant-table-cell">{t('ticket.col_id')}</th>
                          <th className="ant-table-cell">{t('ticket.subject')}</th>
                          <th className="ant-table-cell">{t('ticket.level')}</th>
                          <th className="ant-table-cell">{t('ticket.status')}</th>
                          <th className="ant-table-cell">{t('ticket.created_at_col')}</th>
                          <th className="ant-table-cell">{t('ticket.last_reply_col')}</th>
                          <th className="ant-table-cell ant-table-cell-align-right">
                            {t('ticket.action')}
                          </th>
                        </tr>
                      </thead>
                      <tbody className="ant-table-tbody">
                        {tickets.length ? (
                          tickets.map((ticket) => {
                            const levelLabel = LEVELS[Number(ticket.level)]?.labelKey;
                            return (
                              <tr className="ant-table-row ant-table-row-level-0" key={ticket.id}>
                                <td className="ant-table-cell">{ticket.id}</td>
                                <td className="ant-table-cell">{ticket.subject}</td>
                                <td className="ant-table-cell">
                                  {levelLabel ? t(levelLabel) : ''}
                                </td>
                                <td className="ant-table-cell">
                                  {ticket.status === 1 ? (
                                    <span>
                                      <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                        <span className="ant-badge-status-dot ant-badge-status-success" />
                                      </span>
                                      <span className="ant-badge-status-text">{t('ticket.closed')}</span>
                                    </span>
                                  ) : (
                                    <span>
                                      <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                        <span
                                          className={`ant-badge-status-dot ant-badge-status-${
                                            Number.parseInt(String(ticket.reply_status), 10)
                                              ? 'processing'
                                              : 'error'
                                          }`}
                                        />
                                      </span>
                                      <span className="ant-badge-status-text">
                                        {Number.parseInt(String(ticket.reply_status), 10)
                                          ? t('ticket.replied')
                                          : t('ticket.pending')}
                                      </span>
                                    </span>
                                  )}
                                </td>
                                <td className="ant-table-cell">
                                  {formatDateMinuteSlash(ticket.created_at)}
                                </td>
                                <td className="ant-table-cell">
                                  {formatDateMinuteSlash(ticket.updated_at)}
                                </td>
                                <td className="ant-table-cell ant-table-cell-align-right">
                                  <div>
                                    <a
                                      href="javascript:void(0);"
                                      onClick={() => openTicket(ticket.id)}
                                    >
                                      {t('ticket.view')}
                                    </a>
                                    <span className="ant-divider ant-divider-vertical" role="separator" />
                                    <a
                                      href="javascript:void(0);"
                                      {...(ticket.status ? { disabled: true } : {})}
                                      onClick={async () => {
                                        try {
                                          await close.mutateAsync(ticket.id);
                                        } catch {}
                                      }}
                                    >
                                      {t('ticket.close_ticket')}
                                    </a>
                                  </div>
                                </td>
                              </tr>
                            );
                          })
                        ) : (
                          <tr className="ant-table-placeholder">
                            <td className="ant-table-cell" colSpan={7}>
                              <LegacyEmpty />
                            </td>
                          </tr>
                        )}
                      </tbody>
                    </table>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="v2board-ant-modal">
          <div>
            <div className="ant-modal-header">
              <div className="ant-modal-title">{t('ticket.new')}</div>
            </div>
            <div className="ant-modal-body">
              <div className="form-group">
                <label htmlFor="ticket-subject">{t('ticket.subject')}</label>
                <input
                  id="ticket-subject"
                  className="ant-input"
                  placeholder={t('ticket.subject_placeholder')}
                  value={subject ?? ''}
                  onChange={(event) => setSubject(event.target.value)}
                />
              </div>
              <div className="form-group">
                <label htmlFor="ticket-level">{t('ticket.level_form')}</label>
                <LegacySelect
                  id="ticket-level"
                  style={{ width: '100%' }}
                  value={level === undefined ? '' : String(level)}
                  placeholder={t('ticket.level_placeholder')}
                  options={LEVELS.map((item) => ({
                    value: String(item.value),
                    label: t(item.labelKey),
                  }))}
                  onChange={(nextLevel) =>
                    setLevel(nextLevel === '' ? undefined : (Number(nextLevel) as TicketLevel))
                  }
                />
              </div>
              <div className="form-group">
                <label htmlFor="ticket-message">{t('ticket.message')}</label>
                <textarea
                  id="ticket-message"
                  rows={5}
                  className="ant-input"
                  placeholder={t('ticket.message_placeholder')}
                  value={message ?? ''}
                  onChange={(event) => setMessage(event.target.value)}
                />
              </div>
            </div>
            <div className="ant-modal-footer">
              <AntBtn type="button" className="ant-btn" onClick={() => setOpen(false)}>
                {t('common.cancel')}
              </AntBtn>
              <AntBtn
                type="button"
                className="ant-btn ant-btn-primary"
                onClick={() => void saveTicket()}
              >
                {t('common.confirm')}
              </AntBtn>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
