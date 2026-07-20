import { useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
import dayjs from 'dayjs';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import type { Notice } from '@v2board/types';
import {
  useAdminNotices,
  useDropNoticeMutation,
  useSaveNoticeMutation,
  useShowNoticeMutation,
} from '@/lib/queries';
import { confirmDialog } from '@v2board/ui/confirm-dialog';
import { Button } from '@v2board/ui/button';
import { Card, CardContent } from '@v2board/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@v2board/ui/dialog';
import { Input } from '@v2board/ui/input';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { PageHeader, PageShell } from '@v2board/ui/page';
import { ErrorState } from '@v2board/ui/error-state';
import { LoadingState, SkeletonRows } from '@v2board/ui/loading-state';
import { Switch } from '@v2board/ui/switch';
import { TagsInput } from '@/components/ui/tags-input';
import { Textarea } from '@v2board/ui/textarea';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';
import {
  noticeEditorSchema,
  type NoticeEditorValues,
  type NoticeSavePayload,
} from './notice-form-schema';

const NOTICE_FORM_ID = 'notice-editor-form';

function noticeEditorValues(notice?: Notice): NoticeEditorValues {
  return {
    ...(notice ? { id: notice.id } : {}),
    title: notice?.title ?? '',
    content: notice?.content ?? '',
    img_url: notice?.img_url ?? '',
    tags: notice?.tags ?? [],
  };
}

export default function NoticesPage() {
  const { t } = useTranslation();
  const notices = useAdminNotices();
  const save = useSaveNoticeMutation();
  const drop = useDropNoticeMutation();
  const show = useShowNoticeMutation();
  const [open, setOpen] = useState(false);
  const form = useForm<NoticeEditorValues, unknown, NoticeSavePayload>({
    resolver: zodResolver(noticeEditorSchema),
    defaultValues: noticeEditorValues(),
  });
  // useFormState, not the mutable form.formState proxy: the React Compiler
  // caches proxy reads, which freezes error/submit UI after the first render.
  const { errors: formErrors } = useFormState({ control: form.control });
  const editingId = useWatch({ control: form.control, name: 'id' });
  const dataSource = notices.data ?? [];

  const openCreate = () => {
    form.reset(noticeEditorValues());
    setOpen(true);
  };

  const openEdit = (row: Notice) => {
    form.reset(noticeEditorValues(row));
    setOpen(true);
  };

  const saveNotice = form.handleSubmit((values) => {
    save.mutate(values, { onSuccess: () => setOpen(false) });
  });

  const toggleShow = (row: Notice) => {
    // §6.3 (W10): PATCH `{show}` carries the explicit target value.
    show.mutate({ id: row.id, show: !row.show });
  };

  const removeNotice = async (row: Notice) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.notices.delete_confirm_title),
      description: t(($) => $.admin.notices.delete_confirm_description, { title: row.title }),
      confirmText: t(($) => $.common.delete),
    });
    if (!confirmed) return;
    drop.mutate(row.id);
  };

  const columns: DataTableColumn<Notice>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>#</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.admin.notices.show)}</span>,
      cell: ({ row }) => (
        <Switch
          checked={Boolean(row.original.show)}
          onCheckedChange={() => toggleShow(row.original)}
          aria-label={t(($) => $.admin.notices.toggle_show, { title: row.original.title })}
        />
      ),
    },
    {
      id: 'title',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.common.title)}</span>,
      cell: ({ row }) => row.original.title,
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.notices.created_at)}</span>,
      // §4.5 (W10): timestamps arrive as RFC 3339 strings.
      cell: ({ row }) => dayjs(row.original.created_at).format('YYYY/MM/DD HH:mm'),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => openEdit(row.original)}
            data-testid={`notice-edit-${row.original.id}`}
          >
            <Pencil className="size-4" />
            {t(($) => $.common.edit)}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeNotice(row.original)}
            data-testid={`notice-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            {t(($) => $.common.delete)}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="notices-page">
      {notices.isError ? (
        <ErrorState
          message={t(($) => $.admin.notices.list_error)}
          onRetry={() => void notices.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.notices.page_title)}
        actions={
          <Button onClick={openCreate} data-testid="notice-create">
            <Plus className="size-4" />
            {t(($) => $.admin.notices.create)}
          </Button>
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={dataSource}
            getRowKey={(row) => row.id}
            className="min-w-[720px]"
            data-testid="notices-table"
            empty={
              notices.isSuccess && notices.data !== undefined && dataSource.length === 0
                ? t(($) => $.admin.notices.empty)
                : undefined
            }
            emptyTestId="notices-empty"
          />
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-lg" data-testid="notice-dialog">
          <DialogHeader>
            <DialogTitle>
              {editingId
                ? t(($) => $.admin.notices.edit_title)
                : t(($) => $.admin.notices.create_title)}
            </DialogTitle>
            <DialogDescription>{t(($) => $.admin.notices.dialog_description)}</DialogDescription>
          </DialogHeader>

          <form id={NOTICE_FORM_ID} className="space-y-4" onSubmit={saveNotice} noValidate>
            <Field data-invalid={Boolean(formErrors.title)}>
              <FieldLabel htmlFor="notice-title">{t(($) => $.common.title)}</FieldLabel>
              <Input
                id="notice-title"
                placeholder={t(($) => $.admin.notices.title_placeholder)}
                aria-invalid={Boolean(formErrors.title)}
                {...form.register('title')}
              />
              <FieldError errors={[formErrors.title]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.content)}>
              <FieldLabel htmlFor="notice-content">
                {t(($) => $.admin.notices.content_label)}
              </FieldLabel>
              <Textarea
                id="notice-content"
                rows={12}
                placeholder={t(($) => $.admin.notices.content_placeholder)}
                aria-invalid={Boolean(formErrors.content)}
                {...form.register('content')}
              />
              <FieldError errors={[formErrors.content]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.tags)}>
              <FieldLabel htmlFor="notice-tags">{t(($) => $.admin.notices.tags_label)}</FieldLabel>
              <Controller
                control={form.control}
                name="tags"
                render={({ field }) => (
                  <TagsInput
                    id="notice-tags"
                    placeholder={t(($) => $.admin.notices.tags_placeholder)}
                    value={field.value}
                    onChange={field.onChange}
                    onBlur={field.onBlur}
                    invalid={Boolean(formErrors.tags)}
                  />
                )}
              />
              <FieldError errors={[formErrors.tags]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.img_url)}>
              <FieldLabel htmlFor="notice-img">
                {t(($) => $.admin.notices.img_url_label)}
              </FieldLabel>
              <Input
                id="notice-img"
                placeholder={t(($) => $.admin.notices.img_url_placeholder)}
                aria-invalid={Boolean(formErrors.img_url)}
                {...form.register('img_url')}
              />
              <FieldError errors={[formErrors.img_url]} />
            </Field>
          </form>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              {t(($) => $.common.cancel)}
            </Button>
            <Button
              type="submit"
              form={NOTICE_FORM_ID}
              disabled={save.isPending}
              loading={save.isPending}
              data-testid="notice-submit"
            >
              {t(($) => $.common.submit)}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {notices.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}
