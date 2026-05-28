import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { useCommConfig, usePlans } from '@/lib/queries';
import { PlanContent } from '@/components/plan-content';

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

export default function PlansPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data, isLoading } = usePlans();
  const { data: comm } = useCommConfig();
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
          <span
            className={filter === 'all' ? 'active bg-primary' : ''}
            onClick={() => setFilter('all')}
          >
            {t('plan.filter_all')}
          </span>
          <span
            className={filter === 'period' ? 'active bg-primary' : ''}
            onClick={() => setFilter('period')}
          >
            {t('plan.filter_period')}
          </span>
          <span
            className={filter === 'traffic' ? 'active bg-primary' : ''}
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
            const unitPrice = PERIOD_PRICES.find((p) => plan[p.key] !== null);
            const isSoldOut = plan.capacity_limit !== null && plan.capacity_limit <= 0;
            const almostSoldOut =
              plan.capacity_limit !== null &&
              plan.capacity_limit >= 1 &&
              plan.capacity_limit <= 5;
            if (!unitPrice) return null;

            return (
              <div key={Math.random()} className="col-md-12 col-xl-4">
                <a
                  className="block block-link-pop block-rounded m-3 mx-xl-0"
                  href="javascript:void(0);"
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
                        {symbol} {((plan[unitPrice.key] as number) / 100).toFixed(2)}
                      </p>
                      <p className="h6 text-muted">{t(unitPrice.labelKey)}</p>
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
