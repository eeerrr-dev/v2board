import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import {
  useAdminCoupons,
  useAdminPlans,
  useDropCouponMutation,
  useGenerateCouponMutation,
  useShowCouponMutation,
} from '@/lib/queries';
import { confirmDialog } from '@v2board/ui/confirm-dialog';
import { Badge } from '@v2board/ui/badge';
import { Button } from '@v2board/ui/button';
import { Card, CardContent } from '@v2board/ui/card';
import { PageHeader, PageShell } from '@v2board/ui/page';
import { ErrorState } from '@v2board/ui/error-state';
import { PaginationControl } from '@v2board/ui/pagination';
import { LoadingState, SkeletonRows } from '@v2board/ui/loading-state';
import { Switch } from '@v2board/ui/switch';
import { DataTable, type DataTableColumn } from '@v2board/ui/table';
import { CouponEditor } from './coupon-editor';
import {
  CopyableCode,
  copyWithToast,
  dateRange,
  PAGE_SIZE_OPTIONS,
  paginationLabels,
  type CouponRow,
  type QueryState,
} from './shared';

export function CouponsView() {
  const { t } = useTranslation();
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
      title: t(($) => $.admin.coupons.delete_confirm_title),
      description: t(($) => $.admin.coupons.delete_confirm_description),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
    });
    if (!confirmed) return;
    drop.mutate(row.id);
  };

  const copyCode = (value: string) => copyWithToast(value, (selector) => t(selector));

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
      header: () => <span>{t(($) => $.common.enable)}</span>,
      cell: ({ row }) => (
        <Switch
          checked={row.original.show}
          // §6.3 (W10): PATCH `{show}` carries the explicit target value.
          onCheckedChange={() => show.mutate({ id: row.original.id, show: !row.original.show })}
          aria-label={t(($) => $.admin.coupons.toggle_show_aria, { name: row.original.name })}
        />
      ),
    },
    {
      id: 'name',
      meta: { className: 'font-medium text-foreground' },
      header: () => <span>{t(($) => $.admin.coupons.coupon_name)}</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'type',
      header: () => <span>{t(($) => $.admin.coupons.type)}</span>,
      cell: ({ row }) =>
        row.original.type === 1
          ? t(($) => $.admin.coupons.type_amount)
          : t(($) => $.admin.coupons.type_percent),
    },
    {
      id: 'code',
      header: () => <span>{t(($) => $.admin.coupons.code)}</span>,
      cell: ({ row }) => <CopyableCode value={row.original.code} onCopy={copyCode} />,
    },
    {
      id: 'limit_use',
      meta: { align: 'center' },
      header: () => <span>{t(($) => $.admin.coupons.remaining)}</span>,
      cell: ({ row }) => (
        <Badge variant="secondary">
          {row.original.limit_use !== null
            ? row.original.limit_use
            : t(($) => $.admin.coupons.unlimited)}
        </Badge>
      ),
    },
    {
      id: 'validity',
      meta: { className: 'text-muted-foreground tabular-nums' },
      header: () => <span>{t(($) => $.admin.coupons.valid_period)}</span>,
      cell: ({ row }) => dateRange(row.original.started_at, row.original.ended_at),
    },
    {
      id: 'actions',
      meta: { align: 'right' },
      header: () => <span>{t(($) => $.common.operation)}</span>,
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
                {t(($) => $.common.edit)}
              </Button>
            </CouponEditor>
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
            onClick={() => void removeCoupon(row.original)}
            data-testid={`coupon-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            {t(($) => $.common.delete)}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="coupons-page">
      {coupons.isError ? (
        <ErrorState
          message={t(($) => $.admin.coupons.load_failed)}
          onRetry={() => void coupons.refetch()}
        />
      ) : null}
      {plans.isError ? (
        <ErrorState
          message={t(($) => $.admin.coupons.plans_load_failed)}
          onRetry={() => void plans.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.coupons.title)}
        actions={
          plansReady ? (
            <CouponEditor
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button data-testid="coupon-create">
                <Plus className="size-4" />
                {t(($) => $.admin.coupons.create)}
              </Button>
            </CouponEditor>
          ) : (
            <Button disabled data-testid="coupon-create">
              <Plus className="size-4" />
              {t(($) => $.admin.coupons.create)}
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
                ? t(($) => $.admin.coupons.empty)
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
              labels={paginationLabels((selector) => t(selector))}
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
