import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router';
import type { ParseKeys } from 'i18next';
import { useCommConfig, usePlans } from '@/lib/queries';
import { PlanContent } from '@/components/plan-content';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { EmptyState, PageHeader, PageShell } from '@/components/ui/page';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { Spinner } from '@/components/ui/spinner';
import { StatusBadge } from '@/components/ui/status-badge';
import { cn } from '@/lib/cn';

type PlanLike = NonNullable<ReturnType<typeof usePlans>['data']>[number];

const PERIOD_PRICES: { key: keyof PlanLike; labelKey: ParseKeys }[] = [
  { key: 'month_price', labelKey: 'plan.monthly' },
  { key: 'quarter_price', labelKey: 'plan.quarterly' },
  { key: 'half_year_price', labelKey: 'plan.half_year' },
  { key: 'year_price', labelKey: 'plan.yearly' },
  { key: 'two_year_price', labelKey: 'plan.two_year' },
  { key: 'three_year_price', labelKey: 'plan.three_year' },
  { key: 'onetime_price', labelKey: 'plan.onetime' },
];
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
  const navigate = useNavigate();
  const { data, isLoading } = usePlans();
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

      {isLoading || !data ? (
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

            return (
              <button
                key={plan.id}
                type="button"
                disabled={isSoldOut}
                data-testid="plan-card"
                className={cn(
                  'group flex h-full w-full rounded-xl border border-border bg-card text-left text-card-foreground shadow-sm transition-all hover:-translate-y-0.5 hover:border-foreground/20 hover:shadow-md focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-60',
                )}
                onClick={() => navigate(`/plan/${plan.id}`)}
              >
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
              </button>
            );
          })}
        </div>
      )}
    </PageShell>
  );
}
