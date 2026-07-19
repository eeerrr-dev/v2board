import { useState } from 'react';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import {
  useAdminCoupons,
  useAdminPlans,
  useDropCouponMutation,
  useGenerateCouponMutation,
  useShowCouponMutation,
} from '@/lib/queries';
import { confirmDialog } from '@/components/ui/confirm-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { PageHeader, PageShell } from '@/components/ui/page';
import { ErrorState } from '@/components/ui/error-state';
import { PaginationControl } from '@/components/ui/pagination';
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { Switch } from '@/components/ui/switch';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { CouponEditor } from './coupon-editor';
import {
  CopyableCode,
  copyWithToast,
  dateRange,
  PAGE_SIZE_OPTIONS,
  PAGINATION_LABELS,
  type CouponRow,
  type QueryState,
} from './shared';

export function CouponsView() {
  const [query, setQuery] = useState<QueryState>({ current: 1, pageSize: 10 });
  const coupons = useAdminCoupons(query);
  const plans = useAdminPlans();
  const generate = useGenerateCouponMutation();
  const drop = useDropCouponMutation();
  const show = useShowCouponMutation();
  const planOptions = plans.data;
  const plansReady = !plans.isError && planOptions !== undefined;

  const data = coupons.data?.items ?? [];
  const total = coupons.data?.total ?? 0;

  const removeCoupon = async (row: CouponRow) => {
    const confirmed = await confirmDialog({
      title: '警告',
      description: '确定要删除该条项目吗？',
      confirmText: '确定',
      cancelText: '取消',
    });
    if (!confirmed) return;
    drop.mutate(row.id);
  };

  const columns: DataTableColumn<CouponRow>[] = [
    {
      id: 'id',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>#</span>,
      cell: ({ row }) => row.original.id,
    },
    {
      id: 'show',
      meta: { align: 'center' },
      header: () => <span>启用</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.show}
          // §6.3 (W10): PATCH `{show}` carries the explicit target value.
          onCheckedChange={() => show.mutate({ id: row.original.id, show: !row.original.show })}
          aria-label={`切换优惠券「${row.original.name}」启用`}
        />
      ),
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>券名称</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'type',
      header: () => <span>类型</span>,
      cell: ({ row }) => (row.original.type === 1 ? '金额' : '比例'),
    },
    {
      id: 'code',
      header: () => <span>券码</span>,
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
            <CouponEditor
              record={row.original}
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button variant="ghost" size="sm" data-testid={`coupon-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                编辑
              </Button>
            </CouponEditor>
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
            onClick={() => void removeCoupon(row.original)}
            data-testid={`coupon-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            删除
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="coupons-page">
      {coupons.isError ? (
        <ErrorState message="优惠券列表加载失败" onRetry={() => void coupons.refetch()} />
      ) : null}
      {plans.isError ? (
        <ErrorState message="订阅列表加载失败" onRetry={() => void plans.refetch()} />
      ) : null}
      <PageHeader
        title="优惠券管理"
        actions={
          plansReady ? (
            <CouponEditor
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button data-testid="coupon-create">
                <Plus className="size-4" />
                添加优惠券
              </Button>
            </CouponEditor>
          ) : (
            <Button disabled data-testid="coupon-create">
              <Plus className="size-4" />
              添加优惠券
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
            className="min-w-[900px]"
            data-testid="coupons-table"
            empty={
              !coupons.isError && coupons.data !== undefined && data.length === 0
                ? '暂无优惠券'
                : undefined
            }
            emptyTestId="coupons-empty"
          />

          {total > 0 ? (
            <PaginationControl
              current={query.current}
              pageSize={query.pageSize}
              total={total}
              pageSizeOptions={PAGE_SIZE_OPTIONS}
              labels={PAGINATION_LABELS}
              onChange={(page, pageSize) => setQuery({ current: page, pageSize })}
              testIds={{ page: 'coupon-page', pageSize: 'coupon-page-size' }}
            />
          ) : null}
        </CardContent>
      </Card>

      {coupons.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}
