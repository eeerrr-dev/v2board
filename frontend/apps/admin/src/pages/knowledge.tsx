import { cloneElement, useEffect, useRef, useState, type ReactElement } from 'react';
import dayjs from 'dayjs';
import { admin } from '@v2board/api-client';
import type { Knowledge, KnowledgeSummary } from '@v2board/types';
import { ArrowDown, ArrowUp, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import { apiClient } from '@/lib/api';
import {
  useAdminKnowledge,
  useAdminKnowledgeCategories,
  useDropKnowledgeMutation,
  useSaveKnowledgeMutation,
  useShowKnowledgeMutation,
  useSortKnowledgeMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { toast } from '@/lib/toast';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageHeader, PageShell } from '@/components/ui/page';
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
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { DataTable, type DataTableColumn } from '@/components/ui/table';

type SaveKnowledgePayload = Parameters<typeof admin.saveKnowledge>[1];

// Article language options. The values are the backend locale strings that ride
// along in the save payload; the list is sorted by key for a stable, deterministic
// order (preserving the legacy console's sorted-locale behavior).
const KNOWLEDGE_LOCALE_TEXT = {
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
  'en-US': 'English',
  'ja-JP': '日本語',
  'vi-VN': 'Tiếng Việt',
  'ko-KR': '한국어',
} as const;

const KNOWLEDGE_LOCALE_OPTIONS = (
  Object.keys(KNOWLEDGE_LOCALE_TEXT) as (keyof typeof KNOWLEDGE_LOCALE_TEXT)[]
)
  .sort()
  .map((locale) => ({ value: locale, label: KNOWLEDGE_LOCALE_TEXT[locale] }));

function KnowledgeEditor({
  id,
  categories,
  saveLoading,
  children,
  onSave,
  onSaved,
}: {
  id?: number;
  categories: string[];
  saveLoading?: boolean;
  children: ReactElement<{ onClick?: () => void }>;
  onSave: (payload: SaveKnowledgePayload) => Promise<unknown>;
  onSaved: () => void | Promise<unknown>;
}) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [knowledge, setKnowledge] = useState<Partial<Knowledge>>({});

  // Opening for an existing article fetches its full detail (GET /knowledge/fetch
  // with { id }) so edits round-trip every field back through the save payload.
  const show = async () => {
    setOpen(true);
    if (!id) {
      setKnowledge({});
      return;
    }
    setLoading(true);
    try {
      setKnowledge(await admin.knowledgeDetail(apiClient, id));
    } finally {
      setLoading(false);
    }
  };

  const formChange = (key: keyof Knowledge, value: unknown) => {
    setKnowledge((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    await onSave({ ...knowledge });
    await onSaved();
    setOpen(false);
    toast.success('保存成功');
  };

  return (
    <>
      {cloneElement(children, { onClick: show })}
      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent
          side="right"
          className="w-full gap-0 overflow-y-auto sm:max-w-2xl"
          data-testid="knowledge-editor"
        >
          <SheetHeader>
            <SheetTitle>{id ? '编辑知识' : '新增知识'}</SheetTitle>
          </SheetHeader>

          {loading ? (
            <div className="flex justify-center py-16" role="status">
              <Spinner className="size-5 text-muted-foreground" />
              <span className="sr-only">加载中</span>
            </div>
          ) : (
            <div className="space-y-4 px-4 pb-4">
              <div className="space-y-2">
                <Label htmlFor="knowledge-title">标题</Label>
                <Input
                  id="knowledge-title"
                  placeholder="请输入知识标题"
                  value={knowledge.title ?? ''}
                  onChange={(event) => formChange('title', event.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="knowledge-category">分类</Label>
                <Input
                  id="knowledge-category"
                  list="knowledge-category-options"
                  placeholder="请输入分类，分类将会自动归集"
                  value={knowledge.category ?? ''}
                  onChange={(event) => formChange('category', event.target.value)}
                />
                <datalist id="knowledge-category-options">
                  {categories.map((category) => (
                    <option key={category} value={category} />
                  ))}
                </datalist>
              </div>
              <div className="space-y-2">
                <Label htmlFor="knowledge-language">语言</Label>
                <Select
                  value={knowledge.language}
                  onValueChange={(value) => formChange('language', value)}
                >
                  <SelectTrigger id="knowledge-language" className="w-full">
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
              </div>
              <div className="space-y-2">
                <Label htmlFor="knowledge-body">内容</Label>
                <Textarea
                  id="knowledge-body"
                  rows={18}
                  className="font-mono text-sm"
                  placeholder="请输入知识内容，支持 Markdown"
                  value={knowledge.body ?? ''}
                  onChange={(event) => formChange('body', event.target.value)}
                  data-testid="knowledge-body"
                />
              </div>
            </div>
          )}

          <SheetFooter>
            <Button
              onClick={() => void save()}
              disabled={saveLoading || loading}
              data-testid="knowledge-submit"
            >
              {saveLoading ? <Loader2 className="size-4 animate-spin" /> : null}
              提交
            </Button>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    </>
  );
}

export default function KnowledgePage() {
  const list = useAdminKnowledge();
  const categories = useAdminKnowledgeCategories();
  const save = useSaveKnowledgeMutation();
  const drop = useDropKnowledgeMutation();
  const show = useShowKnowledgeMutation();
  const sort = useSortKnowledgeMutation();
  const [orderedKnowledge, setOrderedKnowledge] = useState<KnowledgeSummary[]>(
    () => list.data ?? [],
  );
  const [sortLoading, setSortLoading] = useState(false);
  const orderRef = useRef(orderedKnowledge);

  useEffect(() => {
    if (list.data) {
      setOrderedKnowledge(list.data);
      setSortLoading(false);
    }
  }, [list.data]);

  orderRef.current = orderedKnowledge;

  // Adjacent-swap reorder. The drag handle is retired for accessible move
  // buttons, but the persisted contract is unchanged: sort.mutate receives the
  // full id list in the new order, then the page refetches.
  const moveKnowledge = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    const current = orderRef.current;
    if (target < 0 || target >= current.length) return;
    const next = [...current];
    const a = next[index];
    const b = next[target];
    if (!a || !b) return;
    next[index] = b;
    next[target] = a;
    setOrderedKnowledge(next);
    setSortLoading(true);
    sort.mutate(
      next.map((knowledge) => knowledge.id),
      {
        onSuccess: () => {
          void list.refetch();
        },
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
    await drop.mutateAsync(row.id);
    void list.refetch();
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
          onCheckedChange={() =>
            show.mutate(row.original.id, {
              onSuccess: () => {
                void list.refetch();
              },
            })
          }
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
            <KnowledgeEditor
              id={row.original.id}
              categories={categories.data ?? []}
              saveLoading={save.isPending}
              onSave={(payload) => save.mutateAsync(payload)}
              onSaved={() => list.refetch()}
            >
              <Button variant="ghost" size="sm" data-testid={`knowledge-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </KnowledgeEditor>
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
      <PageHeader
        title="知识库管理"
        actions={
          <KnowledgeEditor
            categories={categories.data ?? []}
            saveLoading={save.isPending}
            onSave={(payload) => save.mutateAsync(payload)}
            onSaved={() => list.refetch()}
          >
            <Button data-testid="knowledge-create">
              <Plus className="size-4" />
              新增
            </Button>
          </KnowledgeEditor>
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
            empty={orderedKnowledge.length === 0 ? '暂无知识' : undefined}
            emptyTestId="knowledge-empty"
          />
        </CardContent>
      </Card>

      {sortLoading || list.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}
