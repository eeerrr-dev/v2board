import { useEffect, useState } from 'react';
import type { AnchorHTMLAttributes } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { LegacyEmpty } from '@/components/legacy-empty';
import { LegacySelect } from '@/components/legacy-select';
import { useTableScrollPosition } from '@/lib/use-table-scroll-position';
import { useFixedColumnRowHeights } from '@/lib/use-fixed-column-row-heights';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';
import {
  userKeys,
  useCloseTicketMutation,
  useSaveTicketMutation,
  useTickets,
} from '@/lib/queries';
import { formatLegacyDateMinuteSlash } from '@v2board/config/format';
import { legacyHref } from '@/lib/legacy-href';
import type { TicketLevel } from '@v2board/types';

const LEVELS: { value: TicketLevel; labelKey: string }[] = [
  { value: 0, labelKey: 'ticket.level_low' },
  { value: 1, labelKey: 'ticket.level_medium' },
  { value: 2, labelKey: 'ticket.level_high' },
];

function legacyDisabledAnchorProps(disabled: unknown): AnchorHTMLAttributes<HTMLAnchorElement> {
  return { disabled } as unknown as AnchorHTMLAttributes<HTMLAnchorElement>;
}

export default function TicketsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const ticketsQuery = useTickets();
  const { data, isFetching } = ticketsQuery;
  const loading = useLegacyFetchLoading(isFetching, ticketsQuery.error);
  const save = useSaveTicketMutation();
  const close = useCloseTicketMutation();
  const [open, setOpen] = useState(false);
  const [subject, setSubject] = useState<string | undefined>();
  const [level, setLevel] = useState<TicketLevel | undefined>();
  const [message, setMessage] = useState<string | undefined>();
  const [hoverKey, setHoverKey] = useState<number | null>(null);

  useEffect(
    () => () => {
      queryClient.removeQueries({ queryKey: userKeys.tickets });
      queryClient.removeQueries({ queryKey: ['user', 'ticket'] });
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
      void queryClient.invalidateQueries({ queryKey: userKeys.tickets });
    } catch {}
  };

  const closeTicket = async (id: number) => {
    try {
      await close.mutateAsync(id);
      void queryClient.invalidateQueries({ queryKey: userKeys.tickets });
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
  const { bodyRef, onScroll, scrollPositionClassName } = useTableScrollPosition(tickets.length);
  const { mainTableRef, fixedTableRef } = useFixedColumnRowHeights(tickets.length);
  const tableClassName = [
    'ant-table',
    'ant-table-default',
    tickets.length ? '' : 'ant-table-empty',
    scrollPositionClassName,
  ].filter(Boolean).join(' ');

  return (
    <>
      <div className={`block block-rounded js-appear-enabled ${loading ? 'block-mode-loading' : ''}`}>
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
            {/* antd v3 Table always wraps its content in Spin (loading defaults to
                false); with no loading prop the ticket table never spins, so the
                spinner div / ant-spin-blur are absent — only the two static wrappers. */}
            <div className="ant-spin-nested-loading">
              <div className="ant-spin-container">
            <div className={tableClassName}>
              <div className="ant-table-content">
                <div className="ant-table-scroll">
                  <div
                    ref={bodyRef}
                    tabIndex={-1}
                    className="ant-table-body"
                    style={{ overflowX: 'scroll', WebkitTransform: 'translate3d (0, 0, 0)' }}
                    onScroll={onScroll}
                  >
                    <table ref={mainTableRef} className="ant-table-fixed" style={{ width: 900 }}>
                      <colgroup>
                        <col />
                        <col />
                        <col />
                        <col />
                        <col />
                        <col />
                        <col />
                      </colgroup>
                      <thead className="ant-table-thead">
                        <tr>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.col_id')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.subject')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.level')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.status')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.created_at_col')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th className="">
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.last_reply_col')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                          <th
                            className="ant-table-fixed-columns-in-body ant-table-align-right ant-table-row-cell-last"
                            style={{ textAlign: 'right' }}
                          >
                            <span className="ant-table-header-column">
                              <div>
                                <span className="ant-table-column-title">{t('ticket.action')}</span>
                                <span className="ant-table-column-sorter" />
                              </div>
                            </span>
                          </th>
                        </tr>
                      </thead>
                      <tbody className="ant-table-tbody">
                        {tickets.map((ticket, index) => {
                          const levelLabel = LEVELS[ticket.level]?.labelKey;
                          return (
                            <tr
                              className={`ant-table-row ant-table-row-level-0${hoverKey === index ? ' ant-table-row-hover' : ''}`}
                              data-row-key={index}
                              key={index}
                              onMouseEnter={() => setHoverKey(index)}
                              onMouseLeave={() => setHoverKey(null)}
                            >
                              <td>{ticket.id}</td>
                              <td>{ticket.subject}</td>
                              <td>
                                {levelLabel ? t(levelLabel) : ''}
                              </td>
                              <td>
                                {ticket.status === 1 ? (
                                  <span>
                                    <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                      <span className="ant-badge-status-dot ant-badge-status-success" />
                                      <span className="ant-badge-status-text" />
                                    </span>
                                    {t('ticket.closed')}
                                  </span>
                                ) : (
                                  <span>
                                    <span className="ant-badge ant-badge-status ant-badge-not-a-wrapper">
                                      <span
                                        className={`ant-badge-status-dot ant-badge-status-${
                                          parseInt(String(ticket.reply_status))
                                            ? 'processing'
                                            : 'error'
                                        }`}
                                      />
                                      <span className="ant-badge-status-text" />
                                    </span>
                                    {parseInt(String(ticket.reply_status))
                                      ? t('ticket.replied')
                                      : t('ticket.pending')}
                                  </span>
                                )}
                              </td>
                              <td>
                                {formatLegacyDateMinuteSlash(ticket.created_at)}
                              </td>
                              <td>
                                {formatLegacyDateMinuteSlash(ticket.updated_at)}
                              </td>
                              <td
                                className="ant-table-fixed-columns-in-body"
                                style={{ textAlign: 'right' }}
                              >
                                <div>
                                  <a
                                    ref={legacyHref()}
                                    onClick={() => openTicket(ticket.id)}
                                  >
                                    {t('ticket.view')}
                                  </a>
                                  <div className="ant-divider ant-divider-vertical" />
                                  <a
                                    ref={legacyHref()}
                                    {...legacyDisabledAnchorProps(ticket.status)}
                                    onClick={() => void closeTicket(ticket.id)}
                                  >
                                    {t('ticket.close_ticket')}
                                  </a>
                                </div>
                              </td>
                            </tr>
                          );
                        })}
                      </tbody>
                    </table>
                  </div>
                  {tickets.length === 0 && (
                    <div className="ant-table-placeholder">
                      <LegacyEmpty />
                    </div>
                  )}
                </div>
                <div className="ant-table-fixed-right">
                  <div
                    className="ant-table-body-outer"
                    style={{ WebkitTransform: 'translate3d (0, 0, 0)' }}
                  >
                    <div className="ant-table-body-inner">
                      <table ref={fixedTableRef} className="ant-table-fixed">
                        <colgroup>
                          <col />
                        </colgroup>
                        <thead className="ant-table-thead">
                          <tr>
                            <th
                              className="ant-table-align-right ant-table-row-cell-last"
                              style={{ textAlign: 'right' }}
                            >
                              <span className="ant-table-header-column">
                                <div>
                                  <span className="ant-table-column-title">{t('ticket.action')}</span>
                                  <span className="ant-table-column-sorter" />
                                </div>
                              </span>
                            </th>
                          </tr>
                        </thead>
                        <tbody className="ant-table-tbody">
                          {tickets.map((ticket, index) => (
                            <tr
                              className={`ant-table-row ant-table-row-level-0${hoverKey === index ? ' ant-table-row-hover' : ''}`}
                              data-row-key={index}
                              key={index}
                              onMouseEnter={() => setHoverKey(index)}
                              onMouseLeave={() => setHoverKey(null)}
                            >
                              <td style={{ textAlign: 'right' }}>
                                <div>
                                  <a
                                    ref={legacyHref()}
                                    onClick={() => openTicket(ticket.id)}
                                  >
                                    {t('ticket.view')}
                                  </a>
                                  <div className="ant-divider ant-divider-vertical" />
                                  <a
                                    ref={legacyHref()}
                                    {...legacyDisabledAnchorProps(ticket.status)}
                                    onClick={() => void closeTicket(ticket.id)}
                                  >
                                    {t('ticket.close_ticket')}
                                  </a>
                                </div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                </div>
              </div>
            </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent
          title={t('ticket.new')}
          okText={t('ticket.confirm')}
          cancelText={t('common.cancel')}
          maskClosable
          // Original reads `d = e.saveLoading`, but ticket/save never sets that
          // flag, so the compiled `onOk: ()=>d || this.save()` always calls save().
          onOk={() => void saveTicket()}
        >
          {/* Original wraps the form-groups in a class-less <div> (umi.js @2008600). */}
          <div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">{t('ticket.subject')}</label>
              <input
                className="ant-input"
                placeholder={t('ticket.subject_placeholder')}
                value={subject}
                onChange={(event) => setSubject(event.target.value)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">{t('ticket.level_form')}</label>
              <LegacySelect
                style={{ width: '100%' }}
                value={level}
                placeholder={t('ticket.level_placeholder')}
                options={LEVELS.map((item) => ({
                  value: item.value,
                  label: t(item.labelKey),
                }))}
                onChange={(nextLevel) => setLevel(nextLevel as TicketLevel)}
              />
            </div>
            <div className="form-group">
              <label htmlFor="example-text-input-alt">{t('ticket.message')}</label>
              <textarea
                rows={5}
                className="ant-input"
                placeholder={t('ticket.message_placeholder')}
                value={message}
                onChange={(event) => setMessage(event.target.value)}
              />
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
