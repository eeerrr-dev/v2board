import { useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState, useWatch } from 'react-hook-form';
import dayjs from 'dayjs';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import type { Notice } from '@v2board/types';
import {
  useAdminNotices,
  useDropNoticeMutation,
  useSaveNoticeMutation,
  useShowNoticeMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { TagsInput } from '@/components/ui/tags-input';
import { Textarea } from '@/components/ui/textarea';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
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
  const notices = useAdminNotices({});
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
  const dataSource = notices.data?.data ?? [];

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
    show.mutate(row.id);
  };

  const removeNotice = async (row: Notice) => {
    const confirmed = await confirmDialog({
      title: '删除公告',
      description: `确定要删除公告「${row.title}」吗？`,
      confirmText: '删除',
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
      header: () => <span>显示</span>,
      cell: ({ row }) => (
        <Switch
          checked={Boolean(row.original.show)}
          onCheckedChange={() => toggleShow(row.original)}
          aria-label={`切换公告「${row.original.title}」显示`}
        />
      ),
    },
    {
      id: 'title',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>标题</span>,
      cell: ({ row }) => row.original.title,
    },
    {
      id: 'created_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>创建时间</span>,
      cell: ({ row }) => dayjs(1000 * row.original.created_at).format('YYYY/MM/DD HH:mm'),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => openEdit(row.original)}
            data-testid={`notice-edit-${row.original.id}`}
          >
            <Pencil className="size-4" />
            编辑
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive"
            onClick={() => void removeNotice(row.original)}
            data-testid={`notice-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="notices-page">
      {notices.isError ? (
        <ErrorState message="公告列表加载失败" onRetry={() => void notices.refetch()} />
      ) : null}
      <PageHeader
        title="公告管理"
        actions={
          <Button onClick={openCreate} data-testid="notice-create">
            <Plus className="size-4" />
            添加公告
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
                ? '暂无公告'
                : undefined
            }
            emptyTestId="notices-empty"
          />
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-lg" data-testid="notice-dialog">
          <DialogHeader>
            <DialogTitle>{editingId ? '编辑公告' : '新建公告'}</DialogTitle>
            <DialogDescription>编辑公告标题、内容、标签和图片。</DialogDescription>
          </DialogHeader>

          <form id={NOTICE_FORM_ID} className="space-y-4" onSubmit={saveNotice} noValidate>
            <Field data-invalid={Boolean(formErrors.title)}>
              <FieldLabel htmlFor="notice-title">标题</FieldLabel>
              <Input
                id="notice-title"
                placeholder="请输入公告标题"
                aria-invalid={Boolean(formErrors.title)}
                {...form.register('title')}
              />
              <FieldError errors={[formErrors.title]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.content)}>
              <FieldLabel htmlFor="notice-content">公告内容</FieldLabel>
              <Textarea
                id="notice-content"
                rows={12}
                placeholder="请输入公告内容"
                aria-invalid={Boolean(formErrors.content)}
                {...form.register('content')}
              />
              <FieldError errors={[formErrors.content]} />
            </Field>
            <Field data-invalid={Boolean(formErrors.tags)}>
              <FieldLabel htmlFor="notice-tags">公告标签</FieldLabel>
              <Controller
                control={form.control}
                name="tags"
                render={({ field }) => (
                  <TagsInput
                    id="notice-tags"
                    placeholder="输入后回车添加标签"
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
              <FieldLabel htmlFor="notice-img">图片URL</FieldLabel>
              <Input
                id="notice-img"
                placeholder="请输入图片URL"
                aria-invalid={Boolean(formErrors.img_url)}
                {...form.register('img_url')}
              />
              <FieldError errors={[formErrors.img_url]} />
            </Field>
          </form>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              type="submit"
              form={NOTICE_FORM_ID}
              disabled={save.isPending}
              loading={save.isPending}
              data-testid="notice-submit"
            >
              提交
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {notices.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}
