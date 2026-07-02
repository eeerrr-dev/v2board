import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router';
import type { ParseKeys } from 'i18next';
import { useCommConfig, usePlans } from '@/lib/queries';
import { PLAN_PERIOD_LABELS, PURCHASABLE_PLAN_PERIODS } from '@/lib/plan-periods';
import { PlanContent } from '@/components/plan-content';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ErrorState } from '@/components/ui/error-state';
import { EmptyState, PageHeader, PageShell } from '@/components/ui/page';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { cn } from '@/lib/cn';

type PlanLike = NonNullable<ReturnType<typeof usePlans>['data']>[number];

// Derived from the canonical lib/plan-periods tables; plan-periods.test.ts pins
// this rebuild byte-for-byte.
const PERIOD_PRICES = PURCHASABLE_PLAN_PERIODS.map((key) => ({
  key,
  labelKey: PLAN_PERIOD_LABELS[key],
}));
const RENEWAL_PRICE_KEYS = PERIOD_PRICES.filter((p) => p.key !== 'onetime_price');

type FilterKind = 'all' | 'period' | 'traffic';

function getUnitPriceTag(plan: PlanLike) {
  let unitPrice: { key: keyof PlanLike; labelKey: ParseKeys } | undefined;
  [...PERIOD_PRICES].reverse().forEach((period) => {
    if (plan[period.key] !== null) unitPrice = period;
  });
  return unitPrice;
}

export default function PlansPage() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = usePlans();
  const { data: comm } = useCommConfig({ refetchOnMount: 'always' });
  const symbol = comm?.currency_symbol;
  const [filter, setFilter] = useState<FilterKind>('all');

  // React Compiler memoizes this derivation; no manual useMemo needed.
  const filtered = !data
    ? []
    : filter === 'all'
      ? data
      : filter === 'period'
        ? data.filter((p) => RENEWAL_PRICE_KEYS.some((k) => Boolean(p[k.key])))
        : data.filter((p) => Boolean(p.onetime_price));

  return (
    <PageShell data-testid="plans-page">
      <PageHeader
        title={t('plan.pick_title')}
        description={t('plan.pick_best_for_you')}
        actions={
          <SegmentedControl
            data-testid="plan-tabs"
            value={filter}
            onValueChange={setFilter}
            items={[
              { value: 'all', label: t('plan.filter_all') },
              { value: 'period', label: t('plan.filter_period') },
              { value: 'traffic', label: t('plan.filter_traffic') },
            ]}
          />
        }
      />

      {isError ? (
        <ErrorState onRetry={() => void refetch()} data-testid="plan-error" />
      ) : isLoading || !data ? (
        <Card data-testid="plan-empty">
          <CardContent className="flex min-h-44 items-center justify-center">
            <Spinner className="size-5" />
          </CardContent>
        </Card>
      ) : filtered.length === 0 ? (
        <EmptyState data-testid="plan-empty" title={t('plan.no_plan')} />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {filtered.map((plan) => {
            const unitPrice = getUnitPriceTag(plan);
            const isSoldOut = plan.capacity_limit !== null && plan.capacity_limit <= 0;
            const almostSoldOut =
              plan.capacity_limit !== null &&
              plan.capacity_limit >= 1 &&
              plan.capacity_limit <= 5;

            const cardClassName = cn(
              'group flex h-full w-full rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-all',
              isSoldOut
                ? 'pointer-events-none opacity-60'
                : 'hover:-translate-y-0.5 hover:border-foreground/20 hover:shadow-md focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
            );

            const cardBody = (
              <Card className="h-full border-0 bg-transparent shadow-none">
                <CardHeader className="gap-3">
                  <div className="flex items-start justify-between gap-3">
                    <CardTitle data-testid="plan-card-title" className="text-base leading-6">
                      {plan.name}
                    </CardTitle>
                    {almostSoldOut ? (
                      <StatusBadge data-testid="plan-stock-badge" className="whitespace-nowrap" tone="warning">
                        {t('plan.almost_sold_out')}
                      </StatusBadge>
                    ) : null}
                  </div>
                  <div>
                    <div className="text-3xl font-semibold tracking-normal">
                      {symbol} {((unitPrice ? (plan[unitPrice.key] as number) : NaN) / 100).toFixed(2)}
                    </div>
                    <div className="mt-1 text-sm text-muted-foreground">
                      {unitPrice ? t(unitPrice.labelKey) : ''}
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="flex flex-1 flex-col gap-5">
                  {plan.content ? <PlanContent content={plan.content} /> : <div />}
                  <span
                    className={cn(
                      'inline-flex h-9 w-fit items-center justify-center rounded-md px-4 text-sm font-medium transition-colors',
                      isSoldOut
                        ? 'border border-border bg-secondary text-secondary-foreground'
                        : 'bg-primary text-primary-foreground group-hover:bg-primary/90',
                    )}
                  >
                    {isSoldOut ? t('plan.sold_out') : t('plan.buy_now')}
                  </span>
                </CardContent>
              </Card>
            );

            // Sold-out cards (capacity_limit <= 0) render as a non-interactive
            // element rather than a disabled anchor (anchors have no `disabled`),
            // keeping the Tier-1 sold-out block intact. Purchasable cards are real
            // links so the hash router's native href affordances work.
            return isSoldOut ? (
              <div key={plan.id} data-testid="plan-card" aria-disabled="true" className={cardClassName}>
                {cardBody}
              </div>
            ) : (
              <Link key={plan.id} to={`/plan/${plan.id}`} data-testid="plan-card" className={cardClassName}>
                {cardBody}
              </Link>
            );
          })}
        </div>
      )}
    </PageShell>
  );
}
