import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
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
import { LoadingState, SkeletonRows } from '@/components/ui/loading-state';
import { DataTable, type DataTableColumn } from '@/components/ui/table';
import { GiftcardEditor } from './giftcard-editor';
import {
  CopyableCode,
  copyWithToast,
  dateRange,
  PAGE_SIZE_OPTIONS,
  paginationLabels,
  type GiftcardRow,
  type QueryState,
} from './shared';

// Wire giftcard type codes (1-5) are the backend contract; only the labels
// are translated, resolved at render time.
function renderValue(t: TFunction, value: Giftcard['value'], type: Giftcard['type']) {
  if (value === null) return '-';
  switch (type) {
    case 1:
      return t(($) => $.admin.coupons.giftcards.value_amount, { value: value.toFixed(2) });
    case 2:
      return t(($) => $.admin.coupons.giftcards.value_days, { value });
    case 3:
      return t(($) => $.admin.coupons.giftcards.value_traffic, { value });
    case 4:
      return '-';
    case 5:
      return t(($) => $.admin.coupons.giftcards.value_days, { value });
    default:
      return value;
  }
}

function typeLabel(t: TFunction, type: Giftcard['type']) {
  switch (type) {
    case 1:
      return t(($) => $.admin.coupons.giftcards.type_amount);
    case 2:
      return t(($) => $.admin.coupons.giftcards.type_duration);
    case 3:
      return t(($) => $.admin.coupons.giftcards.type_traffic);
    case 4:
      return t(($) => $.admin.coupons.giftcards.type_reset);
    case 5:
      return t(($) => $.admin.coupons.giftcards.type_plan);
    default:
      return '';
  }
}

export function GiftcardsView() {
  const { t } = useTranslation();
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

  const copyCode = (value: string) => copyWithToast(value, (selector) => t(selector));

  const removeGiftcard = async (row: GiftcardRow) => {
    const confirmed = await confirmDialog({
      title: t(($) => $.admin.coupons.delete_confirm_title),
      description: t(($) => $.admin.coupons.delete_confirm_description),
      confirmText: t(($) => $.common.confirm),
      cancelText: t(($) => $.common.cancel),
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
      header: () => <span>{t(($) => $.admin.coupons.name)}</span>,
      cell: ({ row }) => row.original.name,
    },
    {
      id: 'type',
      header: () => <span>{t(($) => $.admin.coupons.type)}</span>,
      cell: ({ row }) => typeLabel(t, row.original.type),
    },
    {
      id: 'value',
      meta: { className: 'tabular-nums' },
      header: () => <span>{t(($) => $.admin.coupons.giftcards.value)}</span>,
      cell: ({ row }) => renderValue(t, row.original.value, row.original.type),
    },
    {
      id: 'plan',
      header: () => <span>{t(($) => $.admin.coupons.giftcards.plan)}</span>,
      cell: ({ row }) => planName(row.original.plan_id),
    },
    {
      id: 'code',
      header: () => <span>{t(($) => $.admin.coupons.giftcards.code)}</span>,
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
            <GiftcardEditor
              record={row.original}
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button variant="ghost" size="sm" data-testid={`giftcard-edit-${row.original.id}`}>
                <Pencil className="size-4" />
                {t(($) => $.common.edit)}
              </Button>
            </GiftcardEditor>
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
            onClick={() => void removeGiftcard(row.original)}
            data-testid={`giftcard-delete-${row.original.id}`}
          >
            <Trash2 className="size-4" />
            {t(($) => $.common.delete)}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <PageShell data-testid="giftcards-page">
      {giftcards.isError ? (
        <ErrorState
          message={t(($) => $.admin.coupons.giftcards.load_failed)}
          onRetry={() => void giftcards.refetch()}
        />
      ) : null}
      {plans.isError ? (
        <ErrorState
          message={t(($) => $.admin.coupons.plans_load_failed)}
          onRetry={() => void plans.refetch()}
        />
      ) : null}
      <PageHeader
        title={t(($) => $.admin.coupons.giftcards.title)}
        actions={
          plansReady ? (
            <GiftcardEditor
              plans={planOptions}
              pending={generate.isPending}
              onSave={(payload, onSuccess) => generate.mutate(payload, { onSuccess })}
            >
              <Button data-testid="giftcard-create">
                <Plus className="size-4" />
                {t(($) => $.admin.coupons.giftcards.create)}
              </Button>
            </GiftcardEditor>
          ) : (
            <Button disabled data-testid="giftcard-create">
              <Plus className="size-4" />
              {t(($) => $.admin.coupons.giftcards.create)}
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
                ? t(($) => $.admin.coupons.giftcards.empty)
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
              labels={paginationLabels((selector) => t(selector))}
              onChange={(page, pageSize) => setQuery({ current: page, pageSize })}
              testIds={{ page: 'giftcard-page', pageSize: 'giftcard-page-size' }}
            />
          ) : null}
        </CardContent>
      </Card>

      {giftcards.isPending ? (
        <LoadingState className="rounded-xl border border-border bg-card p-4">
          <SkeletonRows rows={3} />
        </LoadingState>
      ) : null}
    </PageShell>
  );
}
