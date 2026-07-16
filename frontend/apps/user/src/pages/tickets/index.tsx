import { useState } from 'react';
import type { SelectorParam } from 'i18next';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useEmptyDescription } from '@/lib/use-empty-description';
import type { TicketLevel } from '@v2board/types';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm } from 'react-hook-form';
import { z } from 'zod';
import { Eye, Plus, XCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { PageShell } from '@/components/ui/page';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { ErrorState } from '@/components/ui/error-state';
import { StatusBadge, type StatusTone } from '@/components/ui/status-badge';
import { DataTable, VIRTUALIZE_MIN_ROWS, type DataTableColumn } from '@/components/ui/table';
import { Textarea } from '@/components/ui/textarea';
import { cn } from '@/lib/cn';
import { formatBackendDateMinuteSlash } from '@v2board/config/format';
import { useCloseTicketMutation, useSaveTicketMutation, useTickets } from '@/lib/queries';

const LEVELS: { value: TicketLevel; labelKey: SelectorParam }[] = [
  { value: 0, labelKey: $ => $.ticket.level_low },
  { value: 1, labelKey: $ => $.ticket.level_medium },
  { value: 2, labelKey: $ => $.ticket.level_high },
];

const ticketLevelSchema = z.union([z.literal(0), z.literal(1), z.literal(2)]);
const ticketFormSchema = z
  .object({
    // The backend requires all three fields, so gate them client-side with the
    // existing placeholder copy ("请输入工单主题" / "请选择工单等级" / …) as the
    // required message rather than letting an empty submit round-trip.
    subject: z.string().trim().min(1, 'ticket.subject_placeholder'),
    level: ticketLevelSchema.optional(),
    message: z.string().trim().min(1, 'ticket.message_placeholder'),
  })
  .superRefine((values, ctx) => {
    if (values.level === undefined) {
      ctx.addIssue({
        code: 'custom',
        path: ['level'],
        message: 'ticket.level_placeholder',
      });
    }
  })
  .transform((values) => ({
    level: values.level,
    message: values.message,
    subject: values.subject,
  }));

type TicketFormValues = z.input<typeof ticketFormSchema>;
type TicketPayload = z.output<typeof ticketFormSchema>;

const TICKET_SUBJECT_ID = 'ticket-subject';
const TICKET_LEVEL_ID = 'ticket-level';
const TICKET_MESSAGE_ID = 'ticket-message';

export default function TicketsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const ticketsQuery = useTickets();
  const { data, isFetching, isError, refetch } = ticketsQuery;
  const loading = isFetching;
  const save = useSaveTicketMutation();
  const close = useCloseTicketMutation();
  const [open, setOpen] = useState(false);
  const form = useForm<TicketFormValues, unknown, TicketPayload>({
    resolver: zodResolver(ticketFormSchema),
    defaultValues: { subject: '', level: undefined, message: '' },
  });
  const tickets = data ?? [];
  const emptyDescription = useEmptyDescription();
  const ticketColumns = [
    {
      meta: { className: 'font-medium text-foreground', headerClassName: 'w-16' },
      header: t($ => $.ticket.col_id),
      cell: ({ row }) => row.original.id,
    },
    {
      meta: { className: 'max-w-[260px] truncate font-medium text-foreground' },
      header: t($ => $.ticket.subject),
      cell: ({ row }) => row.original.subject,
    },
    {
      meta: { className: 'text-muted-foreground' },
      header: t($ => $.ticket.level),
      cell: ({ row }) => {
        const levelLabel = LEVELS[row.original.level]?.labelKey;
        return levelLabel ? t(levelLabel) : '';
      },
    },
    {
      meta: { className: 'text-muted-foreground' },
      header: t($ => $.ticket.status),
      cell: ({ row }) => (
        <TicketStatus
          closed={row.original.status === 1}
          replied={Boolean(parseInt(String(row.original.reply_status)))}
        />
      ),
    },
    {
      meta: { className: 'text-muted-foreground' },
      header: t($ => $.ticket.created_at_col),
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.created_at),
    },
    {
      meta: { className: 'text-muted-foreground' },
      header: t($ => $.ticket.last_reply_col),
      cell: ({ row }) => formatBackendDateMinuteSlash(row.original.updated_at),
    },
    {
      meta: { align: 'right', className: 'text-muted-foreground' },
      header: t($ => $.ticket.action),
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
            <Eye className="size-3.5" />
            {t($ => $.ticket.view)}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className={cn('h-8 px-2', row.original.status === 1 && 'text-muted-foreground')}
            data-testid="ticket-close"
            onClick={() => closeTicket(row.original.id)}
          >
            <XCircle className="size-3.5" />
            {t($ => $.ticket.close_ticket)}
          </Button>
        </div>
      ),
    },
  ] satisfies DataTableColumn<(typeof tickets)[number]>[];

  const resetForm = () => {
    form.reset({ subject: '', level: undefined, message: '' });
  };

  const saveTicket = form.handleSubmit((values) => {
    save.mutate(values, {
      onSuccess: () => {
        setOpen(false);
        resetForm();
      },
    });
  });

  const closeTicket = (id: number) => {
    // Closing a ticket cannot be undone, so confirm through the shared
    // AlertDialog before firing the mutation. The dialog owns the in-flight
    // loading state and swallows a rejected close.
    void confirmDialog({
      title: t($ => $.common.attention),
      description: t($ => $.ticket.confirm_close),
      confirmText: t($ => $.ticket.close_ticket),
      onConfirm: () => close.mutateAsync(id),
    });
  };

  const openTicket = (id: number) => {
    void navigate(`/ticket/${id}`);
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
              <CardTitle>{t($ => $.ticket.history)}</CardTitle>
            </div>
            <Button type="button" data-testid="ticket-new-trigger" onClick={() => setOpen(true)}>
              <Plus className="size-4" />
              {t($ => $.ticket.new)}
            </Button>
          </CardHeader>
          <CardContent className="p-0">
            {isError ? (
              // A failed fetch must not render as an empty ticket table (which
              // reads as "no tickets"); show the error with a retry instead.
              <div className="border-t border-border p-4">
                <ErrorState onRetry={() => void refetch()} data-testid="ticket-error" />
              </div>
            ) : (
              <DataTable
                className="min-w-[900px]"
                columns={ticketColumns}
                data={tickets}
                data-testid="ticket-table"
                empty={data !== undefined && tickets.length === 0 ? emptyDescription : undefined}
                emptyTestId="ticket-empty"
                headerClassName="border-y"
                scrollProps={{ 'data-testid': 'ticket-table-scroll' }}
                virtualizer={{ enabled: tickets.length > VIRTUALIZE_MIN_ROWS }}
              />
            )}
          </CardContent>
        </Card>
      </PageShell>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-lg" data-testid="ticket-dialog">
          <DialogHeader>
            <DialogTitle data-testid="ticket-dialog-title">{t($ => $.ticket.new)}</DialogTitle>
            <DialogDescription>{t($ => $.ticket.message_placeholder)}</DialogDescription>
          </DialogHeader>
          <form className="grid gap-4" onSubmit={saveTicket} noValidate>
            <Controller
              control={form.control}
              name="subject"
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor={TICKET_SUBJECT_ID}>{t($ => $.ticket.subject)}</FieldLabel>
                  <Input
                    {...field}
                    id={TICKET_SUBJECT_ID}
                    aria-invalid={fieldState.invalid}
                    aria-describedby={fieldState.error ? `${TICKET_SUBJECT_ID}-error` : undefined}
                    placeholder={t($ => $.ticket.subject_placeholder)}
                  />
                  <FieldError id={`${TICKET_SUBJECT_ID}-error`} errors={[fieldState.error]} />
                </Field>
              )}
            />
            <Controller
              control={form.control}
              name="level"
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor={TICKET_LEVEL_ID}>{t($ => $.ticket.level_form)}</FieldLabel>
                  <Select
                    name={field.name}
                    value={field.value === undefined ? undefined : String(field.value)}
                    onValueChange={(nextLevel) => field.onChange(Number(nextLevel) as TicketLevel)}
                  >
                    <SelectTrigger
                      ref={field.ref}
                      id={TICKET_LEVEL_ID}
                      aria-invalid={fieldState.invalid}
                      aria-describedby={fieldState.error ? `${TICKET_LEVEL_ID}-error` : undefined}
                      data-testid="ticket-select-trigger"
                      onBlur={field.onBlur}
                    >
                      <SelectValue placeholder={t($ => $.ticket.level_placeholder)} />
                    </SelectTrigger>
                    <SelectContent data-testid="ticket-select-content">
                      {LEVELS.map((item) => (
                        <SelectItem key={item.value} value={String(item.value)}>
                          {t(item.labelKey)}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <FieldError id={`${TICKET_LEVEL_ID}-error`} errors={[fieldState.error]} />
                </Field>
              )}
            />
            <Controller
              control={form.control}
              name="message"
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor={TICKET_MESSAGE_ID}>{t($ => $.ticket.message)}</FieldLabel>
                  <Textarea
                    {...field}
                    id={TICKET_MESSAGE_ID}
                    rows={5}
                    aria-invalid={fieldState.invalid}
                    aria-describedby={fieldState.error ? `${TICKET_MESSAGE_ID}-error` : undefined}
                    placeholder={t($ => $.ticket.message_placeholder)}
                  />
                  <FieldError id={`${TICKET_MESSAGE_ID}-error`} errors={[fieldState.error]} />
                </Field>
              )}
            />
            <DialogFooter data-testid="ticket-dialog-footer">
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                {t($ => $.common.cancel)}
              </Button>
              <Button type="submit" loading={save.isPending}>
                {t($ => $.ticket.confirm)}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}

function TicketStatus({ closed, replied }: { closed: boolean; replied: boolean }) {
  const { t } = useTranslation();
  const label = closed ? t($ => $.ticket.closed) : replied ? t($ => $.ticket.replied) : t($ => $.ticket.pending);
  const tone: StatusTone = closed ? 'success' : replied ? 'info' : 'destructive';
  return (
    <StatusBadge data-testid="ticket-status" tone={tone} showDot>
      {label}
    </StatusBadge>
  );
}
