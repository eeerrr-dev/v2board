import { useState } from 'react';
import dayjs from 'dayjs';
import { Loader2, Pencil, Plus, Trash2, X } from 'lucide-react';
import type { Notice } from '@v2board/types';
import {
  useAdminNotices,
  useDropNoticeMutation,
  useSaveNoticeMutation,
  useShowNoticeMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/shadcn-dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PageHeader, PageShell } from '@/components/ui/page';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { DataTable, type DataTableColumn } from '@/components/ui/table';

export default function NoticesPage() {
  const notices = useAdminNotices({});
  const save = useSaveNoticeMutation();
  const drop = useDropNoticeMutation();
  const show = useShowNoticeMutation();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState<Partial<Notice>>({});
  const dataSource = notices.data?.data ?? [];

  const openCreate = () => {
    setDraft({});
    setOpen(true);
  };

  const openEdit = (row: Notice) => {
    setDraft({ ...row });
    setOpen(true);
  };

  const saveNotice = async () => {
    await save.mutateAsync({ ...draft });
    await notices.refetch();
    setOpen(false);
  };

  const toggleShow = (row: Notice) => {
    show.mutate(row.id, {
      onSuccess: () => {
        void notices.refetch();
      },
    });
  };

  const removeNotice = async (row: Notice) => {
    const confirmed = await confirmDialog({
      title: '删除公告',
      description: `确定要删除公告「${row.title}」吗？`,
      confirmText: '删除',
    });
    if (!confirmed) return;
    drop.mutate(row.id, {
      onSuccess: () => {
        void notices.refetch();
      },
    });
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
            empty={dataSource.length === 0 ? '暂无公告' : undefined}
            emptyTestId="notices-empty"
          />
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-lg" data-testid="notice-dialog">
          <DialogHeader>
            <DialogTitle>{draft.id ? '编辑公告' : '新建公告'}</DialogTitle>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="notice-title">标题</Label>
              <Input
                id="notice-title"
                placeholder="请输入公告标题"
                value={draft.title ?? ''}
                onChange={(event) => setDraft({ ...draft, title: event.target.value })}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="notice-content">公告内容</Label>
              <Textarea
                id="notice-content"
                rows={12}
                placeholder="请输入公告内容"
                value={draft.content ?? ''}
                onChange={(event) => setDraft({ ...draft, content: event.target.value })}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="notice-tags">公告标签</Label>
              <TagInput
                id="notice-tags"
                placeholder="输入后回车添加标签"
                value={draft.tags ?? []}
                onChange={(tags) =>
                  setDraft({ ...draft, tags: (tags.length > 0 ? tags : null) as Notice['tags'] })
                }
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="notice-img">图片URL</Label>
              <Input
                id="notice-img"
                placeholder="请输入图片URL"
                value={(draft.img_url as string | undefined) ?? ''}
                onChange={(event) => setDraft({ ...draft, img_url: event.target.value })}
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              onClick={() => void saveNotice()}
              disabled={save.isPending}
              data-testid="notice-submit"
            >
              {save.isPending ? <Loader2 className="size-4 animate-spin" /> : null}
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

interface TagInputProps {
  id?: string;
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
}

// Small shadcn-native replacement for the legacy antd `mode="tags"` select:
// Enter commits the draft, Backspace on an empty draft pops the last tag, and
// each tag renders as a removable badge. Duplicates are ignored, matching the
// old tag set semantics.
function TagInput({ id, value, onChange, placeholder }: TagInputProps) {
  const [draft, setDraft] = useState('');

  const commit = () => {
    const next = draft.trim();
    if (next && !value.includes(next)) onChange([...value, next]);
    setDraft('');
  };

  return (
    <div className="flex min-h-9 flex-wrap items-center gap-1.5 rounded-md border border-input bg-transparent px-2 py-1.5 text-sm shadow-xs focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50">
      {value.map((tag) => (
        <Badge key={tag} variant="secondary" className="gap-1 pr-1">
          {tag}
          <button
            type="button"
            className="rounded-full text-muted-foreground hover:text-foreground"
            onClick={() => onChange(value.filter((item) => item !== tag))}
            aria-label={`移除标签 ${tag}`}
          >
            <X className="size-3" />
          </button>
        </Badge>
      ))}
      <input
        id={id}
        className="min-w-24 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
        value={draft}
        placeholder={value.length ? '' : placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault();
            commit();
          } else if (event.key === 'Backspace' && !draft && value.length) {
            onChange(value.slice(0, -1));
          }
        }}
      />
    </div>
  );
}
