import { useState } from 'react';
import { zodResolver } from '@hookform/resolvers/zod';
import { Controller, useForm, useFormState } from 'react-hook-form';
import { useTranslation } from 'react-i18next';
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
import { confirmDialog } from '@v2board/ui/confirm-dialog';
import { toast } from '@/lib/toast';
import { Button } from '@v2board/ui/button';
import { Card, CardContent } from '@v2board/ui/card';
import { Field, FieldError, FieldLabel } from '@v2board/ui/field';
import { Input } from '@v2board/ui/input';
import { PageHeader, PageShell } from '@v2board/ui/page';
import { ErrorState } from '@v2board/ui/error-state';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@v2board/ui/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@v2board/ui/sheet';
import { LoadingState, SkeletonFields, SkeletonRows } from '@v2board/ui/loading-state';
import { Switch } from '@v2board/ui/switch';
import { Textarea } from '@v2board/ui/textarea';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';
import {
  KNOWLEDGE_LOCALES,
  knowledgeEditorSchema,
  type KnowledgeEditorValues,
  type KnowledgeSavePayload,
} from './knowledge-form-schema';

// Article language options. The values are the backend locale strings that ride
// along in the save payload; the list is sorted by key for a stable, deterministic
// order (preserving the established sorted-locale behavior). Labels resolve at
// render through the locale_labels dictionary subtree (language autonyms).
const SORTED_KNOWLEDGE_LOCALES = [...KNOWLEDGE_LOCALES].sort();

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
  const { t } = useTranslation();
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
  const localeOptions = SORTED_KNOWLEDGE_LOCALES.map((locale) => ({
    value: locale,
    label: t(($) => $.admin.knowledge.locale_labels[locale]),
  }));

  return (
    <form
      id={formId}
      className="space-y-4 px-4 pb-4"
      onSubmit={(event) => void handleSubmit(onSubmit)(event)}
      noValidate
    >
      <Field data-invalid={Boolean(errors.title)}>
        <FieldLabel htmlFor="knowledge-title">{t(($) => $.common.title)}</FieldLabel>
        <Input
          id="knowledge-title"
          placeholder={t(($) => $.admin.knowledge.title_placeholder)}
          aria-invalid={Boolean(errors.title)}
          {...register('title')}
        />
        <FieldError errors={[errors.title]} />
      </Field>
      <Field data-invalid={Boolean(errors.category)}>
        <FieldLabel htmlFor="knowledge-category">{t(($) => $.admin.knowledge.category)}</FieldLabel>
        <Input
          id="knowledge-category"
          list="knowledge-category-options"
          placeholder={t(($) => $.admin.knowledge.category_placeholder)}
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
        <FieldLabel htmlFor="knowledge-language">{t(($) => $.common.language)}</FieldLabel>
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
                <SelectValue placeholder={t(($) => $.admin.knowledge.language_placeholder)} />
              </SelectTrigger>
              <SelectContent>
                {localeOptions.map((option) => (
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
        <FieldLabel htmlFor="knowledge-body">{t(($) => $.admin.knowledge.body_label)}</FieldLabel>
        <Textarea
          id="knowledge-body"
          rows={18}
          className="font-mono text-sm"
          placeholder={t(($) => $.admin.knowledge.body_placeholder)}
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
  const { t } = useTranslation();
  const detail = useAdminKnowledgeDetail(id, open);
  const detailLoading = id != null && detail.isPending;
  const detailError = id != null && detail.isError;
  const detailReady = id == null || detail.data != null;
  const formId = `knowledge-editor-form-${id ?? 'new'}`;

  const saveValues = (values: KnowledgeSavePayload) => {
    onSave(values, () => {
      onOpenChange(false);
      toast.success(t(($) => $.admin.knowledge.save_success));
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
          <SheetTitle>
            {id ? t(($) => $.admin.knowledge.edit_title) : t(($) => $.admin.knowledge.create_title)}
          </SheetTitle>
          <SheetDescription>{t(($) => $.admin.knowledge.sheet_description)}</SheetDescription>
        </SheetHeader>

        {detailError ? (
          <div className="px-4 pb-4">
            <ErrorState
              message={t(($) => $.admin.knowledge.detail_error)}
              onRetry={retryDetail}
              data-testid="knowledge-detail-error"
            />
          </div>
        ) : detailLoading || !detailReady ? (
          <LoadingState className="px-4 py-6">
            <SkeletonFields fields={4} />
          </LoadingState>
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
            {t(($) => $.common.submit)}
          </Button>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t(($) => $.common.cancel)}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}

export default function KnowledgePage() {
  const { t } = useTranslation();
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
      title: t(($) => $.admin.knowledge.delete_confirm_title),
      description: t(($) => $.admin.knowledge.delete_confirm_description),
      confirmText: t(($) => $.common.confirm),
    });
    if (!confirmed) return;
    drop.mutate(row.id);
  };

  const columns: DataTableColumn<KnowledgeSummary>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.knowledge.id_column)}</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.admin.knowledge.show)}</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.show}
          // §6.3 (W10): PATCH `{show}` carries the explicit target value.
          onCheckedChange={() => show.mutate({ id: row.original.id, show: !row.original.show })}
          aria-label={t(($) => $.admin.knowledge.toggle_show, { title: row.original.title })}
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
      id: 'category',
      meta: { className: 'text-muted-foreground' },
      header: () => <span>{t(($) => $.admin.knowledge.category)}</span>,
      cell: ({ row }) => row.original.category,
    },
    {
      id: 'updated_at',
      meta: { align: 'right', className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.knowledge.updated_at)}</span>,
      // §4.5 (W10): timestamps arrive as RFC 3339 strings.
      cell: ({ row }) => dayjs(row.original.updated_at).format('YYYY/MM/DD HH:mm'),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
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
              aria-label={t(($) => $.admin.knowledge.move_up)}
            >
              <ArrowUp className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              disabled={index < 0 || index >= orderedKnowledge.length - 1}
              onClick={() => moveKnowledge(index, 1)}
              aria-label={t(($) => $.admin.knowledge.move_down)}
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
                {t(($) => $.common.edit)}
              </Button>
            ) : (
              <Button variant="ghost" size="sm" disabled>
                <Pencil className="size-4" />
                {t(($) => $.common.edit)}
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
              {t(($) => $.common.delete)}
            </Button>
          </div>
        );
      },
    },
  ];

  return (
    <PageShell data-testid="knowledge-page">
      {list.isError ? (
        <ErrorState
          message={t(($) => $.admin.knowledge.list_error)}
          onRetry={() => void list.refetch()}
        />
      ) : null}
      {categories.isError ? (
        <ErrorState
          message={t(($) => $.admin.knowledge.categories_error)}
          onRetry={() => void categories.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.knowledge.page_title)}
        actions={
          categoriesReady ? (
            <Button data-testid="knowledge-create" onClick={() => openEditor()}>
              <Plus className="size-4" />
              {t(($) => $.common.add)}
            </Button>
          ) : (
            <Button disabled data-testid="knowledge-create">
              <Plus className="size-4" />
              {t(($) => $.common.add)}
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
                ? t(($) => $.admin.knowledge.empty)
                : undefined
            }
            emptyTestId="knowledge-empty"
          />
        </CardContent>
      </Card>

      {sort.isPending || list.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
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
