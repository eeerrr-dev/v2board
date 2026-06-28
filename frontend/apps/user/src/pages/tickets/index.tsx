import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { getLocaleAntdMessages } from '@v2board/i18n';
import type { TicketLevel } from '@v2board/types';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { ExternalLink, Plus, XCircle } from 'lucide-react';
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
import { PageShell } from '@/components/ui/page';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
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

const ticketLevelSchema = z.union([z.literal(0), z.literal(1), z.literal(2)]);
const ticketFormSchema = z.object({
  subject: z.string().optional(),
  level: ticketLevelSchema.optional(),
  message: z.string().optional(),
});

type TicketFormValues = z.infer<typeof ticketFormSchema>;

function normalizeTicketPayload(values: TicketFormValues): TicketFormValues {
  return {
    level: values.level,
    message: values.message || undefined,
    subject: values.subject || undefined,
  };
}

export default function TicketsPage() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const ticketsQuery = useTickets();
  const { data, isFetching } = ticketsQuery;
  const loading = useLegacyFetchLoading(isFetching, ticketsQuery.error);
  const save = useSaveTicketMutation();
  const close = useCloseTicketMutation();
  const [open, setOpen] = useState(false);
  const form = useForm<TicketFormValues>({
    resolver: zodResolver(ticketFormSchema),
    defaultValues: { subject: '', level: undefined, message: '' },
  });
  const selectedLevel = form.watch('level');
  const tickets = data ?? [];
  const emptyDescription = getLocaleAntdMessages(i18n.language).emptyDescription;
  const ticketColumns = [
    {
      className: 'font-medium text-foreground',
      headerClassName: 'w-16',
      header: t('ticket.col_id'),
      cell: ({ row }) => row.original.id,
    },
    {
      className: 'max-w-[260px] truncate font-medium text-foreground',
      header: t('ticket.subject'),
      cell: ({ row }) => row.original.subject,
    },
    {
      className: 'text-muted-foreground',
      header: t('ticket.level'),
      cell: ({ row }) => {
        const levelLabel = LEVELS[row.original.level]?.labelKey;
        return levelLabel ? t(levelLabel) : '';
      },
    },
    {
      className: 'text-muted-foreground',
      header: t('ticket.status'),
      cell: ({ row }) => (
        <TicketStatus
          closed={row.original.status === 1}
          replied={Boolean(parseInt(String(row.original.reply_status)))}
        />
      ),
    },
    {
      className: 'text-muted-foreground',
      header: t('ticket.created_at_col'),
      cell: ({ row }) => formatUserLegacyDateMinuteSlash(row.original.created_at),
    },
    {
      className: 'text-muted-foreground',
      header: t('ticket.last_reply_col'),
      cell: ({ row }) => formatUserLegacyDateMinuteSlash(row.original.updated_at),
    },
    {
      align: 'right',
      className: 'text-muted-foreground',
      header: t('ticket.action'),
      cell: ({ row }) => (
        <div className="flex justify-end gap-1">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-8 px-2"
            data-testid="ticket-view"
            onClick={() => openTicket(row.original.id)}
          >
            <ExternalLink className="size-3.5" />
            {t('ticket.view')}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className={cn('h-8 px-2', row.original.status === 1 && 'text-muted-foreground')}
            data-testid="ticket-close"
            onClick={() => void closeTicket(row.original.id)}
          >
            <XCircle className="size-3.5" />
            {t('ticket.close_ticket')}
          </Button>
        </div>
      ),
    },
  ] satisfies DataTableColumn<(typeof tickets)[number]>[];

  const resetForm = () => {
    form.reset({ subject: '', level: undefined, message: '' });
  };

  const saveTicket = form.handleSubmit(async (values) => {
    try {
      await save.mutateAsync(normalizeTicketPayload(values));
      setOpen(false);
      resetForm();
      void queryClient.invalidateQueries({ queryKey: userKeys.tickets });
    } catch {}
  });

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
      <PageShell data-testid="ticket-page">
        <Card
          className={cn('overflow-hidden py-0', loading && 'opacity-80')}
          data-testid="ticket-surface"
        >
          <CardHeader className="gap-3 py-6 sm:flex sm:flex-row sm:items-center sm:justify-between">
            <div className="space-y-1.5">
              <CardTitle>{t('ticket.history')}</CardTitle>
            </div>
            <Button
              type="button"
              data-testid="ticket-new-trigger"
              onClick={() => setOpen(true)}
            >
              <Plus className="size-4" />
              {t('ticket.new')}
            </Button>
          </CardHeader>
          <CardContent className="p-0">
            <DataTable
              className="min-w-[900px]"
              columns={ticketColumns}
              data={tickets}
              data-testid="ticket-table"
              empty={!tickets.length ? emptyDescription : undefined}
              emptyTestId="ticket-empty"
              headerClassName="border-y"
              scrollProps={{ 'data-testid': 'ticket-table-scroll' }}
              virtualizer={{ enabled: tickets.length > 30 }}
            />
          </CardContent>
        </Card>
      </PageShell>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-lg" data-testid="ticket-dialog">
          <DialogHeader>
            <DialogTitle data-testid="ticket-dialog-title">{t('ticket.new')}</DialogTitle>
            <DialogDescription>{t('ticket.message_placeholder')}</DialogDescription>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="space-y-2">
              <Label htmlFor="ticket-subject">{t('ticket.subject')}</Label>
              <Input
                id="ticket-subject"
                placeholder={t('ticket.subject_placeholder')}
                {...form.register('subject')}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="ticket-level">{t('ticket.level_form')}</Label>
              <Select
                value={selectedLevel === undefined ? undefined : String(selectedLevel)}
                onValueChange={(nextLevel) =>
                  form.setValue('level', Number(nextLevel) as TicketLevel)
                }
              >
                <SelectTrigger id="ticket-level" data-testid="ticket-select-trigger">
                  <SelectValue placeholder={t('ticket.level_placeholder')} />
                </SelectTrigger>
                <SelectContent data-testid="ticket-select-content">
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
                {...form.register('message')}
              />
            </div>
          </div>
          <DialogFooter data-testid="ticket-dialog-footer">
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
    const tone: StatusTone = closed ? 'success' : replied ? 'info' : 'destructive';
    return (
      <StatusBadge data-testid="ticket-status" tone={tone} showDot>
        {label}
      </StatusBadge>
    );
  }
}
