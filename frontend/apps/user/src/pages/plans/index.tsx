import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router';
import type { SelectorParam } from 'i18next';
import { useCommConfig, usePlans } from '@/lib/queries';
import { PLAN_PERIOD_LABELS, PURCHASABLE_PLAN_PERIODS } from '@/lib/plan-periods';
import { PlanContent } from '@/components/plan-content';
import { Card, CardContent, CardHeader, CardTitle } from '@v2board/ui/card';
import { ErrorState } from '@v2board/ui/error-state';
import { EmptyState, PageHeader, PageShell } from '@v2board/ui/page';
import { SegmentedControl } from '@v2board/ui/segmented-control';
import { LoadingState, SkeletonRows } from '@v2board/ui/loading-state';
import { StatusBadge } from '@v2board/ui/status-badge';
import { cn } from '@v2board/ui/cn';

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
  let unitPrice: { labelKey: SelectorParam; price: number } | undefined;
  [...PERIOD_PRICES].reverse().forEach((period) => {
    const price = plan[period.key];
    if (typeof price === 'number' && Number.isFinite(price)) {
      unitPrice = { labelKey: period.labelKey, price };
    }
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
        title={t(($) => $.plan.pick_title)}
        description={t(($) => $.plan.pick_best_for_you)}
        actions={
          <SegmentedControl
            data-testid="plan-tabs"
            aria-label={t(($) => $.plan.pick_title)}
            value={filter}
            onValueChange={setFilter}
            items={[
              { value: 'all', label: t(($) => $.plan.filter_all) },
              { value: 'period', label: t(($) => $.plan.filter_period) },
              { value: 'traffic', label: t(($) => $.plan.filter_traffic) },
            ]}
          />
        }
      />

      {isError ? (
        <ErrorState onRetry={() => void refetch()} data-testid="plan-error" />
      ) : isLoading || !data ? (
        <Card data-testid="plan-empty">
          <CardContent className="min-h-44 py-6">
            <LoadingState>
              <SkeletonRows rows={3} />
            </LoadingState>
          </CardContent>
        </Card>
      ) : filtered.length === 0 ? (
        <EmptyState data-testid="plan-empty" title={t(($) => $.plan.no_plan)} />
      ) : (
        <div className="grid gap-4 @xl/main:grid-cols-2 @4xl/main:grid-cols-3">
          {filtered.map((plan) => {
            const unitPrice = getUnitPriceTag(plan);
            const isSoldOut = plan.capacity_limit !== null && plan.capacity_limit <= 0;
            const isPriceUnavailable = unitPrice === undefined;
            const isUnavailable = isSoldOut || isPriceUnavailable;
            const almostSoldOut =
              plan.capacity_limit !== null && plan.capacity_limit >= 1 && plan.capacity_limit <= 5;

            const cardClassName = cn(
              'group flex h-full w-full rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-all',
              isUnavailable
                ? 'pointer-events-none opacity-60'
                : 'hover:-translate-y-0.5 hover:border-foreground/20 hover:shadow-md focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none',
            );

            const cardBody = (
              <Card className="h-full border-0 bg-transparent shadow-none">
                <CardHeader className="gap-3">
                  <div className="flex items-start justify-between gap-3">
                    <CardTitle data-testid="plan-card-title" className="text-base leading-6">
                      {plan.name}
                    </CardTitle>
                    {almostSoldOut ? (
                      <StatusBadge
                        data-testid="plan-stock-badge"
                        className="whitespace-nowrap"
                        tone="warning"
                      >
                        {t(($) => $.plan.almost_sold_out)}
                      </StatusBadge>
                    ) : null}
                  </div>
                  <div>
                    <div
                      className={cn(
                        'font-semibold',
                        unitPrice
                          ? 'text-3xl tracking-normal'
                          : 'text-sm leading-6 text-destructive',
                      )}
                    >
                      {unitPrice ? (
                        <>
                          {symbol} {(unitPrice.price / 100).toFixed(2)}
                        </>
                      ) : (
                        <span data-testid="plan-price-unavailable">
                          {t(
                            ($) =>
                              $.errors[
                                'This payment period cannot be purchased, please choose another period'
                              ],
                          )}
                        </span>
                      )}
                    </div>
                    <div className="mt-1 text-sm text-muted-foreground">
                      {unitPrice ? t(unitPrice.labelKey) : ''}
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="flex flex-1 flex-col gap-5">
                  {plan.content ? (
                    <PlanContent content={plan.content} htmlClassName="custom-html-style" />
                  ) : (
                    <div />
                  )}
                  <span
                    className={cn(
                      'inline-flex h-9 w-fit items-center justify-center rounded-md px-4 text-sm font-medium transition-colors',
                      isUnavailable
                        ? 'border border-border bg-secondary text-secondary-foreground'
                        : 'bg-primary text-primary-foreground group-hover:bg-primary/90',
                    )}
                  >
                    {isSoldOut
                      ? t(($) => $.plan.sold_out)
                      : isPriceUnavailable
                        ? t(($) => $.plan.select_other)
                        : t(($) => $.plan.buy_now)}
                  </span>
                </CardContent>
              </Card>
            );

            // Sold-out and price-invalid cards render as non-interactive elements
            // rather than disabled anchors (anchors have no `disabled`). The sold-out
            // label keeps priority when both conditions apply; valid cards remain real
            // links so the hash router's native href affordances work.
            return isUnavailable ? (
              <div
                key={plan.id}
                data-testid="plan-card"
                aria-disabled="true"
                className={cardClassName}
              >
                {cardBody}
              </div>
            ) : (
              <Link
                key={plan.id}
                to={`/plan/${plan.id}`}
                data-testid="plan-card"
                className={cardClassName}
              >
                {cardBody}
              </Link>
            );
          })}
        </div>
      )}
    </PageShell>
  );
}
