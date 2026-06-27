import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { getLocaleAntdMessages } from '@v2board/i18n';
import type { TicketLevel } from '@v2board/types';
import { ExternalLink, Plus, XCircle } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
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
import { Textarea } from '@/components/ui/textarea';
import { cn } from '@/lib/cn';
import { formatUserLegacyDateMinuteSlash } from '@/lib/legacy-date';
import {
  userKeys,
  useCloseTicketMutation,
  useSaveTicketMutation,
  useTickets,
} from '@/lib/queries';
import { useLegacyFetchLoading } from '@/lib/use-legacy-fetch-loading';

const LEVELS: { value: TicketLevel; labelKey: string }[] = [
  { value: 0, labelKey: 'ticket.level_low' },
  { value: 1, labelKey: 'ticket.level_medium' },
  { value: 2, labelKey: 'ticket.level_high' },
];

export default function TicketsPage() {
  const { t, i18n } = useTranslation();
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
  const tickets = data ?? [];
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;

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

  return (
    <>
      <Card className={cn('v2board-ticket-surface overflow-hidden', loading && 'opacity-80')}>
        <CardHeader className="gap-3 sm:flex sm:flex-row sm:items-center sm:justify-between">
          <div className="space-y-1.5">
            <CardTitle>{t('ticket.history')}</CardTitle>
          </div>
          <Button
            type="button"
            className="v2board-ticket-new-trigger"
            onClick={() => setOpen(true)}
          >
            <Plus className="size-4" />
            {t('ticket.new')}
          </Button>
        </CardHeader>
        <CardContent className="p-0">
          <TableScroll className="v2board-ticket-table-scroll">
            <Table className="v2board-ticket-table min-w-[900px]">
              <TableHeader className="border-y">
                <tr>
                  <TicketHeader className="w-16">{t('ticket.col_id')}</TicketHeader>
                  <TicketHeader>{t('ticket.subject')}</TicketHeader>
                  <TicketHeader>{t('ticket.level')}</TicketHeader>
                  <TicketHeader>{t('ticket.status')}</TicketHeader>
                  <TicketHeader>{t('ticket.created_at_col')}</TicketHeader>
                  <TicketHeader>{t('ticket.last_reply_col')}</TicketHeader>
                  <TicketHeader className="text-right">{t('ticket.action')}</TicketHeader>
                </tr>
              </TableHeader>
              <TableBody>
                {tickets.length ? (
                  tickets.map((ticket, index) => {
                    const levelLabel = LEVELS[ticket.level]?.labelKey;
                    return (
                      <TableRow data-row-key={index} key={index}>
                        <TicketCell className="font-medium text-foreground">{ticket.id}</TicketCell>
                        <TicketCell className="max-w-[260px] truncate font-medium text-foreground">
                          {ticket.subject}
                        </TicketCell>
                        <TicketCell>{levelLabel ? t(levelLabel) : ''}</TicketCell>
                        <TicketCell>
                          <TicketStatus
                            closed={ticket.status === 1}
                            replied={Boolean(parseInt(String(ticket.reply_status)))}
                          />
                        </TicketCell>
                        <TicketCell>{formatUserLegacyDateMinuteSlash(ticket.created_at)}</TicketCell>
                        <TicketCell>{formatUserLegacyDateMinuteSlash(ticket.updated_at)}</TicketCell>
                        <TicketCell className="text-right">
                          <div className="flex justify-end gap-1">
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              className="v2board-ticket-view h-8 px-2"
                              onClick={() => openTicket(ticket.id)}
                            >
                              <ExternalLink className="size-3.5" />
                              {t('ticket.view')}
                            </Button>
                            <Button
                              type="button"
                              variant="ghost"
                              size="sm"
                              className={cn(
                                'v2board-ticket-close h-8 px-2',
                                ticket.status === 1 && 'text-muted-foreground',
                              )}
                              onClick={() => void closeTicket(ticket.id)}
                            >
                              <XCircle className="size-3.5" />
                              {t('ticket.close_ticket')}
                            </Button>
                          </div>
                        </TicketCell>
                      </TableRow>
                    );
                  })
                ) : (
                  <TableEmpty colSpan={7} rowClassName="v2board-ticket-empty">
                    {emptyDescription}
                  </TableEmpty>
                )}
              </TableBody>
            </Table>
          </TableScroll>
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="v2board-ticket-dialog sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="v2board-ticket-dialog-title">{t('ticket.new')}</DialogTitle>
            <DialogDescription>{t('ticket.message_placeholder')}</DialogDescription>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="space-y-2">
              <Label htmlFor="ticket-subject">{t('ticket.subject')}</Label>
              <Input
                id="ticket-subject"
                placeholder={t('ticket.subject_placeholder')}
                value={subject ?? ''}
                onChange={(event) => setSubject(event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="ticket-level">{t('ticket.level_form')}</Label>
              <Select
                value={level === undefined ? undefined : String(level)}
                onValueChange={(nextLevel) => setLevel(Number(nextLevel) as TicketLevel)}
              >
                <SelectTrigger id="ticket-level" className="v2board-ticket-select-trigger">
                  <SelectValue placeholder={t('ticket.level_placeholder')} />
                </SelectTrigger>
                <SelectContent className="v2board-ticket-select-content">
                  {LEVELS.map((item) => (
                    <SelectItem key={item.value} value={String(item.value)}>
                      {t(item.labelKey)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="ticket-message">{t('ticket.message')}</Label>
              <Textarea
                id="ticket-message"
                rows={5}
                placeholder={t('ticket.message_placeholder')}
                value={message ?? ''}
                onChange={(event) => setMessage(event.target.value)}
              />
            </div>
          </div>
          <DialogFooter className="v2board-ticket-dialog-footer">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="button" loading={save.isPending} onClick={() => void saveTicket()}>
              {t('ticket.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );

  function TicketStatus({ closed, replied }: { closed: boolean; replied: boolean }) {
    const label = closed ? t('ticket.closed') : replied ? t('ticket.replied') : t('ticket.pending');
    const tone = closed ? 'success' : replied ? 'processing' : 'error';
    return (
      <Badge
        variant="outline"
        className={cn(
          'v2board-ticket-status',
          tone === 'success' && 'border-emerald-200 text-emerald-700',
          tone === 'processing' && 'border-sky-200 text-sky-700',
          tone === 'error' && 'border-destructive/30 text-destructive',
        )}
      >
        <span
          className={cn(
            'size-1.5 rounded-full',
            tone === 'success' && 'bg-emerald-500',
            tone === 'processing' && 'bg-sky-500',
            tone === 'error' && 'bg-destructive',
          )}
        />
        {label}
      </Badge>
    );
  }
}

function TicketHeader({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return <TableHead className={className}>{children}</TableHead>;
}

function TicketCell({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return <TableCell className={cn('text-muted-foreground', className)}>{children}</TableCell>;
}
