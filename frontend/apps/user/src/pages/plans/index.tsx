import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useCommConfig, usePlans } from '@/lib/queries';
import { PlanContent } from '@/components/plan-content';
import { legacyHref } from '@/lib/legacy-href';

type PlanLike = NonNullable<ReturnType<typeof usePlans>['data']>[number];

const PERIOD_PRICES: { key: keyof PlanLike; labelKey: string }[] = [
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
  let unitPrice: { key: keyof PlanLike; labelKey: string } | undefined;
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

  const filtered = useMemo(() => {
    if (!data) return [];
    if (filter === 'all') return data;
    if (filter === 'period')
      return data.filter((p) =>
        RENEWAL_PRICE_KEYS.some((k) => Boolean(p[k.key])),
      );
    return data.filter((p) => Boolean(p.onetime_price));
  }, [data, filter]);

  return (
    <>
      <h2 className="font-weight-normal mb-4 m-3 mx-xl-0 mt-xl-0 mt-4">
        {t('plan.pick_title')}
      </h2>
      <div className="mb-3 font-size-sm mt-3 m-3 mx-xl-0">
        <span className="v2board-plan-tabs border-primary text-primary">
          {/* Original inactive tabs use `N === tabs && "active bg-primary"` → false, so
              React omits the class attribute entirely (umi.js @659916); undefined matches
              that omitted-attribute DOM (an empty class="" would not). */}
          <span
            className={filter === 'all' ? 'active bg-primary' : undefined}
            onClick={() => setFilter('all')}
          >
            {t('plan.filter_all')}
          </span>
          <span
            className={filter === 'period' ? 'active bg-primary' : undefined}
            onClick={() => setFilter('period')}
          >
            {t('plan.filter_period')}
          </span>
          <span
            className={filter === 'traffic' ? 'active bg-primary' : undefined}
            onClick={() => setFilter('traffic')}
          >
            {t('plan.filter_traffic')}
          </span>
        </span>
      </div>

      {isLoading || !data || data.length === 0 ? (
        <div className="spinner-grow text-primary" role="status">
          <span className="sr-only">Loading...</span>
        </div>
      ) : (
        <div className="row">
          {filtered.map((plan) => {
            const unitPrice = getUnitPriceTag(plan);
            const isSoldOut = plan.capacity_limit !== null && plan.capacity_limit <= 0;
            const almostSoldOut =
              plan.capacity_limit !== null &&
              plan.capacity_limit >= 1 &&
              plan.capacity_limit <= 5;

            return (
              // Faithful to the original, which assigns key={Math.random()} to the
              // plan card wrapper on every render (no mount animation, so invisible).
              <div key={Math.random()} className="col-md-12 col-xl-4">
                <a
                  className="block block-link-pop block-rounded m-3 mx-xl-0"
                  ref={legacyHref()}
                  onClick={() => {
                    if (!isSoldOut) navigate(`/plan/${plan.id}`);
                  }}
                >
                  <div className="block-header plan">
                    <h3 className="block-title">{plan.name}</h3>
                    {almostSoldOut && (
                      <span className="v2board-sold-out-tag">{t('plan.almost_sold_out')}</span>
                    )}
                  </div>
                  <div className="block-content bg-gray-light">
                    <div className="py-2">
                      <p className="h1 mb-2">
                        {symbol} {((unitPrice ? (plan[unitPrice.key] as number) : NaN) / 100).toFixed(2)}
                      </p>
                      <p className="h6 text-muted">{unitPrice ? t(unitPrice.labelKey) : ''}</p>
                    </div>
                  </div>
                  <div className="block-content py-3">
                    {plan.content ? <PlanContent content={plan.content} className="mb-3" /> : null}
                    <button
                      type="button"
                      disabled={isSoldOut}
                      className="btn btn-sm btn-alt-primary"
                    >
                      {isSoldOut ? t('plan.sold_out') : t('plan.buy_now')}
                    </button>
                  </div>
                </a>
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}
