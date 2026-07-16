import { useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import dayjs from 'dayjs';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { ArrowDown, ArrowUp, Pencil, Plus, Trash2 } from 'lucide-react';
import {
  useAdminKnowledge,
  useAdminKnowledgeCategories,
  useAdminKnowledgeDetail,
  useDropKnowledgeMutation,
  useSaveKnowledgeMutation,
  useShowKnowledgeMutation,
  useSortKnowledgeMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { toast } from '@/lib/toast';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Field, FieldError, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import {
  KNOWLEDGE_LOCALES,
  knowledgeEditorSchema,
  type KnowledgeEditorValues,
  type KnowledgeLocale,
  type KnowledgeSavePayload,
} from './knowledge-form-schema';

// Article language options. The values are the backend locale strings that ride
// along in the save payload; the list is sorted by key for a stable, deterministic
// order (preserving the established sorted-locale behavior).
const KNOWLEDGE_LOCALE_TEXT: Record<KnowledgeLocale, string> = {
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
  'en-US': 'English',
  'ja-JP': '日本語',
  'vi-VN': 'Tiếng Việt',
  'ko-KR': '한국어',
};

const KNOWLEDGE_LOCALE_OPTIONS = [...KNOWLEDGE_LOCALES]
  .sort()
  .map((locale) => ({ value: locale, label: KNOWLEDGE_LOCALE_TEXT[locale] }));

function knowledgeEditorValues(knowledge?: Knowledge): KnowledgeEditorValues {
  return {
    ...(knowledge ? { id: knowledge.id } : {}),
    category: knowledge?.category ?? '',
    language: (knowledge?.language ?? KNOWLEDGE_LOCALES[0]) as KnowledgeEditorValues['language'],
    title: knowledge?.title ?? '',
    body: knowledge?.body ?? '',
  };
}

function KnowledgeEditorForm({
  categories,
  formId,
  initialValues,
  onSubmit,
}: {
  categories: string[];
  formId: string;
  initialValues: KnowledgeEditorValues;
  onSubmit: (values: KnowledgeSavePayload) => void;
}) {
  const { control, handleSubmit, register } = useForm<
    KnowledgeEditorValues,
    unknown,
    KnowledgeSavePayload
  >({
    resolver: zodResolver(knowledgeEditorSchema),
    defaultValues: initialValues,
  });
  // useFormState, not the mutable formState proxy: the React Compiler caches
  // proxy reads, which freezes error/submit UI after the first render.
  const { errors } = useFormState({ control });

  return (
    <form
      id={formId}
      className="space-y-4 px-4 pb-4"
      onSubmit={(event) => void handleSubmit(onSubmit)(event)}
      noValidate
    >
      <Field data-invalid={Boolean(errors.title)}>
        <FieldLabel htmlFor="knowledge-title">标题</FieldLabel>
        <Input
          id="knowledge-title"
          placeholder="请输入知识标题"
          aria-invalid={Boolean(errors.title)}
          {...register('title')}
        />
        <FieldError errors={[errors.title]} />
      </Field>
      <Field data-invalid={Boolean(errors.category)}>
        <FieldLabel htmlFor="knowledge-category">分类</FieldLabel>
        <Input
          id="knowledge-category"
          list="knowledge-category-options"
          placeholder="请输入分类，分类将会自动归集"
          aria-invalid={Boolean(errors.category)}
          {...register('category')}
        />
        <datalist id="knowledge-category-options">
          {categories.map((category) => (
            <option key={category} value={category} />
          ))}
        </datalist>
        <FieldError errors={[errors.category]} />
      </Field>
      <Field data-invalid={Boolean(errors.language)}>
        <FieldLabel htmlFor="knowledge-language">语言</FieldLabel>
        <Controller
          control={control}
          name="language"
          render={({ field }) => (
            <Select
              name={field.name}
              value={field.value}
              onValueChange={field.onChange}
              onOpenChange={(selectOpen) => {
                if (!selectOpen) field.onBlur();
              }}
            >
              <SelectTrigger
                id="knowledge-language"
                className="w-full"
                aria-invalid={Boolean(errors.language)}
              >
                <SelectValue placeholder="请选择知识语言" />
              </SelectTrigger>
              <SelectContent>
                {KNOWLEDGE_LOCALE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        />
        <FieldError errors={[errors.language]} />
      </Field>
      <Field data-invalid={Boolean(errors.body)}>
        <FieldLabel htmlFor="knowledge-body">内容</FieldLabel>
        <Textarea
          id="knowledge-body"
          rows={18}
          className="font-mono text-sm"
          placeholder="请输入知识内容，支持 Markdown"
          aria-invalid={Boolean(errors.body)}
          {...register('body')}
          data-testid="knowledge-body"
        />
        <FieldError errors={[errors.body]} />
      </Field>
    </form>
  );
}

function KnowledgeEditor({
  id,
  categories,
  open,
  saveLoading,
  onOpenChange,
  onSave,
}: {
  id: number | undefined;
  categories: string[];
  open: boolean;
  saveLoading?: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: (payload: KnowledgeSavePayload, onSuccess: () => void) => void;
}) {
  const detail = useAdminKnowledgeDetail(id, open);
  const detailLoading = id != null && detail.isPending;
  const detailError = id != null && detail.isError;
  const detailReady = id == null || detail.data != null;
  const formId = `knowledge-editor-form-${id ?? 'new'}`;

  const saveValues = (values: KnowledgeSavePayload) => {
    onSave(values, () => {
      onOpenChange(false);
      toast.success('保存成功');
    });
  };

  const retryDetail = () => {
    void detail.refetch();
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="w-full gap-0 overflow-y-auto sm:max-w-2xl"
        data-testid="knowledge-editor"
      >
        <SheetHeader>
          <SheetTitle>{id ? '编辑知识' : '新增知识'}</SheetTitle>
          <SheetDescription>编辑文章分类、语言、标题和 Markdown 内容。</SheetDescription>
        </SheetHeader>

        {detailError ? (
          <div className="px-4 pb-4">
            <ErrorState
              message="知识详情加载失败"
              onRetry={retryDetail}
              data-testid="knowledge-detail-error"
            />
          </div>
        ) : detailLoading || !detailReady ? (
          <div className="flex justify-center py-16" role="status">
            <Spinner className="size-5 text-muted-foreground" />
            <span className="sr-only">加载中</span>
          </div>
        ) : (
          <KnowledgeEditorForm
            categories={categories}
            formId={formId}
            initialValues={knowledgeEditorValues(detail.data)}
            onSubmit={saveValues}
          />
        )}

        <SheetFooter>
          <Button
            type="submit"
            form={formId}
            disabled={saveLoading || detailLoading || detailError || !detailReady}
            loading={saveLoading}
            data-testid="knowledge-submit"
          >
            提交
          </Button>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

export default function KnowledgePage() {
  const list = useAdminKnowledge();
  const categories = useAdminKnowledgeCategories();
  const categoryData = categories.data;
  const categoriesReady = !categories.isError && categoryData !== undefined;
  const save = useSaveKnowledgeMutation();
  const drop = useDropKnowledgeMutation();
  const show = useShowKnowledgeMutation();
  const sort = useSortKnowledgeMutation();
  const [orderOverride, setOrderOverride] = useState<KnowledgeSummary[] | null>(null);
  const [editor, setEditor] = useState<{
    id: number | undefined;
    key: number;
    open: boolean;
  }>({ id: undefined, key: 0, open: false });
  const orderedKnowledge = orderOverride ?? list.data ?? [];

  // Keep exactly one page-level editor. Row-local Sheet instances are both
  // wasteful (one disabled detail query per row) and fragile: rebuilding table
  // column definitions can remount a cell and discard an in-progress editor.
  const openEditor = (id?: number) => {
    setEditor((current) => ({ id, key: current.key + 1, open: true }));
  };

  const setEditorOpen = (open: boolean) => {
    setEditor((current) => ({ ...current, open }));
  };

  // Adjacent-swap reorder. The drag handle is retired for accessible move
  // buttons, but the persisted contract is unchanged: sort.mutate receives the
  // full id list in the new order, then the page refetches.
  const moveKnowledge = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    const current = orderedKnowledge;
    if (target < 0 || target >= current.length) return;
    const next = [...current];
    const a = next[index];
    const b = next[target];
    if (!a || !b) return;
    next[index] = b;
    next[target] = a;
    setOrderOverride(next);
    sort.mutate(
      next.map((knowledge) => knowledge.id),
      {
        onSettled: () => setOrderOverride(null),
      },
    );
  };

  const removeKnowledge = async (row: KnowledgeSummary) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该条项目吗？',
      confirmText: '确定',
    });
    if (!confirmed) return;
    drop.mutate(row.id);
  };

  const columns: DataTableColumn<KnowledgeSummary>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>文章ID</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>显示</span>,
      cell: ({ row }) => (
        <Switch
          checked={Boolean(row.original.show)}
          onCheckedChange={() => show.mutate(row.original.id)}
          aria-label={`切换「${row.original.title}」显示`}
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
      id: 'category',
      meta: { className: 'text-muted-foreground' },
      header: () => <span>分类</span>,
      cell: ({ row }) => row.original.category,
    },
    {
      id: 'updated_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>更新时间</span>,
      cell: ({ row }) => dayjs(1000 * row.original.updated_at).format('YYYY/MM/DD HH:mm'),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => {
        const index = orderedKnowledge.findIndex((item) => item.id === row.original.id);
        return (
          <div className="flex items-center justify-end gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index <= 0}
              onClick={() => moveKnowledge(index, -1)}
              aria-label="上移"
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedKnowledge.length - 1}
              onClick={() => moveKnowledge(index, 1)}
              aria-label="下移"
            >
              <ArrowDown className="size-4" />
            </Button>
            {categoriesReady ? (
              <Button
                variant="ghost"
                size="sm"
                data-testid={`knowledge-edit-${row.original.id}`}
                onClick={() => openEditor(row.original.id)}
              >
                <Pencil className="size-4" />
                编辑
              </Button>
            ) : (
              <Button variant="ghost" size="sm" disabled>
                <Pencil className="size-4" />
                编辑
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => void removeKnowledge(row.original)}
              data-testid={`knowledge-delete-${row.original.id}`}
            >
              <Trash2 className="size-4" />
              删除
            </Button>
          </div>
        );
      },
    },
  ];

  return (
    <PageShell data-testid="knowledge-page">
      {list.isError ? (
        <ErrorState message="知识库加载失败" onRetry={() => void list.refetch()} />
      ) : null}
      {categories.isError ? (
        <ErrorState message="知识分类加载失败" onRetry={() => void categories.refetch()} />
      ) : null}
      <PageHeader
        title="知识库管理"
        actions={
          categoriesReady ? (
            <Button data-testid="knowledge-create" onClick={() => openEditor()}>
              <Plus className="size-4" />
              新增
            </Button>
          ) : (
            <Button disabled data-testid="knowledge-create">
              <Plus className="size-4" />
              新增
            </Button>
          )
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={orderedKnowledge}
            getRowKey={(row) => row.id}
            className="min-w-[820px]"
            data-testid="knowledge-table"
            empty={
              !list.isError && list.data !== undefined && orderedKnowledge.length === 0
                ? '暂无知识'
                : undefined
            }
            emptyTestId="knowledge-empty"
          />
        </CardContent>
      </Card>

      {sort.isPending || list.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}

      <KnowledgeEditor
        key={editor.key}
        id={editor.id}
        categories={categoryData ?? []}
        open={editor.open}
        saveLoading={save.isPending}
        onOpenChange={setEditorOpen}
        onSave={(payload, onSuccess) => save.mutate(payload, { onSuccess })}
      />
    </PageShell>
  );
}
