import type { InternalApiAdminPlanItem } from '../generated/internal-api';

export type PlanPeriod =
  | 'month_price'
  | 'quarter_price'
  | 'half_year_price'
  | 'year_price'
  | 'two_year_price'
  | 'three_year_price'
  | 'onetime_price'
  | 'reset_price';

declare const moneyMinorUnit: unique symbol;
declare const moneyMajorUnit: unique symbol;

/** Integer currency minor units as carried by the HTTP contract (for example cents). */
export type MoneyMinor = number & { readonly [moneyMinorUnit]: 'MoneyMinor' };

/** Decimal currency major units used by admin display and editing models. */
export type MoneyMajor = number & { readonly [moneyMajorUnit]: 'MoneyMajor' };

type PlanPrices<TAmount extends number> = Record<PlanPeriod, TAmount | null>;

/**
 * Fields shared by the admin plan wire DTO and its UI-facing domain model.
 * Currency fields deliberately live in the two unit-specific types below.
 */
type AdminPlanBase = Omit<InternalApiAdminPlanItem, PlanPeriod | 'reset_traffic_method'> & {
  reset_traffic_method: 0 | 1 | 2 | 3 | 4 | null;
};

/**
 * GET /{secure_path}/plans wire DTO (docs/api-dialect.md §6.2, W11).
 * Prices are branded integer minor units so they cannot be passed to an admin
 * editor or formatter without an explicit boundary conversion.
 */
export type AdminPlanDto = AdminPlanBase & PlanPrices<MoneyMinor>;

/**
 * Admin plan domain/view model. Prices are decimal major units and therefore
 * cannot accidentally be submitted as the wire DTO's integer minor units.
 */
export type AdminPlanModel = AdminPlanBase & PlanPrices<MoneyMajor>;

/**
 * GET /user/plans and /user/plans/{id} (docs/api-dialect.md §5.5, W4):
 * boolean `show`/`renew` flags and RFC 3339 timestamps. `capacity_limit`
 * keeps the legacy remaining-capacity rewrite (sold out ⇒ ≤ 0).
 */
export interface UserPlan {
  id: number;
  group_id: number;
  transfer_enable: number;
  device_limit: number | null;
  speed_limit: number | null;
  reset_traffic_method: 0 | 1 | 2 | 3 | 4 | null;
  name: string;
  show: boolean;
  sort: number | null;
  renew: boolean;
  content: string | null;
  month_price: number | null;
  quarter_price: number | null;
  half_year_price: number | null;
  year_price: number | null;
  two_year_price: number | null;
  three_year_price: number | null;
  onetime_price: number | null;
  reset_price: number | null;
  capacity_limit: number | null;
  created_at: string;
  updated_at: string;
}
