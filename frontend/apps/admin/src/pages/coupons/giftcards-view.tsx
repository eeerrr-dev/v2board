import { useState } from 'react';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import type { Giftcard } from '@v2board/types';
import {
  useAdminGiftcards,
  useAdminPlans,
  useDropGiftcardMutation,
  useGenerateGiftcardMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { PaginationControl } from '@/components/ui/pagination';
import { Spinner } from '@/components/ui/spinner';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { GiftcardEditor } from './giftcard-editor';
import {
  CopyableCode,
  copyWithToast,
  dateRange,
  PAGE_SIZE_OPTIONS,
  PAGINATION_LABELS,
  type GiftcardRow,
  type QueryState,
} from './shared';

export function GiftcardsView() {
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 10 });
  const giftcards = useAdminGiftcards(query);
  const plans = useAdminPlans();
  const generate = useGenerateGiftcardMutation();
  const drop = useDropGiftcardMutation();
  const planOptions = plans.data;
  const plansReady = !plans.isError && planOptions !== undefined;

  const data = giftcards.data?.items ?? [];
  const total = giftcards.data?.total ?? 0;

  const planName = (id: number | string | null | undefined) =>
    planOptions?.find((plan) => plan.id === id)?.name ?? '-';

  const renderValue = (value: Giftcard['value'], type: Giftcard['type']) => {
    if (value === null) return '-';
    switch (type) {
      case 1:
        return `${value.toFixed(2)} ¥`;
      case 2:
        return `${value} 天`;
      case 3:
        return `${value} GB`;
      case 4:
        return '-';
      case 5:
        return `${value} 天`;
      default:
        return value;
    }
  };

  const typeLabel = (type: Giftcard['type']) => {
    switch (type) {
      case 1:
        return '金额';
      case 2:
        return '时长';
      case 3:
        return '流量';
      case 4:
        return '重置';
      case 5:
        return '套餐';
      default:
        return '';
    }
  };

  const removeGiftcard = async (row: GiftcardRow) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该条项目吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(row.id);
  };

  const columns: DataTableColumn<GiftcardRow>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>#</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'type',
      header: () => <span>类型</span>,
      cell: ({ row }) => typeLabel(row.original.type),
    },
    {
      id: 'value',
      meta: { className: 'tabular-nums' },
      header: () => <span>数值</span>,
      cell: ({ row }) => renderValue(row.original.value, row.original.type),
    },
    {
      id: 'plan',
      header: () => <span>套餐</span>,
      cell: ({ row }) => planName(row.original.plan_id),
    },
    {
      id: 'code',
      header: () => <span>卡密</span>,
      cell: ({ row }) => <CopyableCode value={row.original.code} onCopy={copyWithToast} />,
    },
    {
      id: 'limit_use',
      meta: { align: 'center' },
      header: () => <span>剩余次数</span>,
      cell: ({ row }) => (
        <Badge variant="secondary">
          {row.original.limit_use !== null ? row.original.limit_use : '无限'}
        </Badge>
      ),
    },
    {
      id: 'validity',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>有效期</span>,
      cell: ({ row }) => dateRange(row.original.started_at, row.original.ended_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>操作</span>,
      cell: ({ row }) => (
        <div className="flex items-center justify-end gap-1">
          {plansReady ? (
            <GiftcardEditor
              record={row.original}
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button variant="ghost" size="sm" data-testid={`giftcard-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </GiftcardEditor>
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
            onClick={() => void removeGiftcard(row.original)}
            data-testid={`giftcard-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="giftcards-page">
      {giftcards.isError ? (
        <ErrorState message="礼品卡列表加载失败" onRetry={() => void giftcards.refetch()} />
      ) : null}
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      <PageHeader
        title="礼品卡管理"
        actions={
          plansReady ? (
            <GiftcardEditor
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button data-testid="giftcard-create">
                <Plus className="size-4" />
                添加礼品卡
              </Button>
            </GiftcardEditor>
          ) : (
            <Button disabled data-testid="giftcard-create">
              <Plus className="size-4" />
              添加礼品卡
            </Button>
          )
        }
      />

      <Card className="overflow-hidden py-0">
        <CardContent className="p-0">
          <DataTable
            columns={columns}
            data={data}
            getRowKey={(row) => row.id}
            className="min-w-[960px]"
            data-testid="giftcards-table"
            empty={
              !giftcards.isError && giftcards.data !== undefined && data.length === 0
                ? '暂无礼品卡'
                : undefined
            }
            emptyTestId="giftcards-empty"
          />

          {total > 0 ? (
            <PaginationControl
              current={query.current}
              pageSize={query.pageSize}
              total={total}
              pageSizeOptions={PAGE_SIZE_OPTIONS}
              labels={PAGINATION_LABELS}
              onChange={(page, pageSize) => setQuery({ current: page, pageSize })}
              testIds={{ page: 'giftcard-page', pageSize: 'giftcard-page-size' }}
            />
          ) : null}
        </CardContent>
      </Card>

      {giftcards.isPending ? (
        <div className="flex justify-center py-6" role="status">
          <Spinner className="size-5 text-muted-foreground" />
          <span className="sr-only">加载中</span>
        </div>
      ) : null}
    </PageShell>
  );
}
