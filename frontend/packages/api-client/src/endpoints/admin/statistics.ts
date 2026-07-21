import type { InternalApiOperationMap } from '@v2board/types';
import type { ApiClient } from '../../client';
import { adminListQueryParams, type AdminListQuery } from '../../dialect';
import { requestInternal } from '../../internal-operation';
import type { PageResult, QueryRequestConfig } from './shared';

type PageItem<Value> = Value extends { items: Array<infer Item> } ? Item : never;
export type AdminUserTrafficRecord = PageItem<
  InternalApiOperationMap['adminStatsUserTraffic']['response']
>;

export interface AdminUserTrafficQuery {
  user_id: number;
  current?: number;
  pageSize?: number;
}

/** GET /{secure_path}/system/queue-stats — dialect v2 bare object (§6.1, W9). */
export const queueStats = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminSystemQueueStats', {
    ...config,
  });

/** GET /{secure_path}/system/queue-workload — dialect v2 bare array (§6.1, W9). */
export const queueWorkload = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminSystemQueueWorkload', {
    ...config,
  });

/** §7.1 — the GET system/logs filter whitelist (`level` only) and §7.2 sort columns. */
export const SYSTEM_LOG_FILTER_FIELDS = ['level'] as const;
export const SYSTEM_LOG_SORT_FIELDS = ['created_at', 'level'] as const;
export type SystemLogFilterField = (typeof SYSTEM_LOG_FILTER_FIELDS)[number];
export type AdminSystemLogRecord = PageItem<InternalApiOperationMap['adminSystemLogs']['response']>;

/**
 * GET /{secure_path}/system/logs — dialect v2 `{items, total}` page (§6.1,
 * W9) and the §7 filter/sort DSL's first consumer: clauses ride the single
 * JSON `filter` query param, sorting rides enum-validated
 * `sort_by`/`sort_dir`. No modern route parses legacy `filter[i][key]`
 * brackets.
 */
export const fetchSystemLogs = (
  client: ApiClient,
  query: AdminListQuery<SystemLogFilterField> = {},
  config?: QueryRequestConfig,
) =>
  requestInternal(client, 'adminSystemLogs', {
    query: adminListQueryParams(query),
    ...config,
  });

/** §7.1 — the GET system/audit-logs filter whitelist and §7.2 sort columns. */
export const AUDIT_LOG_FILTER_FIELDS = ['surface', 'actor_email', 'method'] as const;
export const AUDIT_LOG_SORT_FIELDS = ['created_at'] as const;
export type AuditLogFilterField = (typeof AUDIT_LOG_FILTER_FIELDS)[number];
export type AdminAuditLogRecord = PageItem<
  InternalApiOperationMap['adminSystemAuditLogsList']['response']
>;

/**
 * GET /{secure_path}/system/audit-logs — the §6.11 append-only operator audit
 * trail as a dialect v2 `{items, total}` page behind the same §8 pagination
 * and §7 filter/sort DSL as system/logs. Admin prefix only (no staff mirror).
 */
export const fetchAuditLogs = (
  client: ApiClient,
  query: AdminListQuery<AuditLogFilterField> = {},
  config?: QueryRequestConfig,
) =>
  requestInternal(client, 'adminSystemAuditLogsList', {
    query: adminListQueryParams(query),
    ...config,
  });

/** §6.8 (W14): the `stats/server-rank` + `stats/user-rank` window selector. */
export type StatsRankWindow = 'today' | 'previous';

/** GET /{secure_path}/stats/summary — dialect v2 bare object (§6.8, W14):
 * the three legacy aliases collapsed into one route; money in integer cents. */
export const statSummary = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminStatsSummary', {
    ...config,
  });

/** GET /{secure_path}/stats/server-rank `?window=today|previous` — bare array (§6.8, W14). */
export const statServerRank = (
  client: ApiClient,
  window: StatsRankWindow,
  config?: QueryRequestConfig,
) =>
  requestInternal(client, 'adminStatsServerRank', {
    query: { window },
    ...config,
  });

/** GET /{secure_path}/stats/user-rank `?window=today|previous` — bare array (§6.8, W14). */
export const statUserRank = (
  client: ApiClient,
  window: StatsRankWindow,
  config?: QueryRequestConfig,
) =>
  requestInternal(client, 'adminStatsUserRank', {
    query: { window },
    ...config,
  });

/** GET /{secure_path}/stats/orders — bare `{series, date, value}` array (§6.8, W14):
 * snake_case series slugs, integer-cent money. */
export const statOrder = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminStatsOrders', {
    ...config,
  });

/**
 * GET /{secure_path}/stats/user-traffic `?user_id=&page=&per_page=` — dialect
 * v2 `{items, total}` page (§6.8, W14): RFC 3339 `record_at`, numeric
 * `server_rate`. The modal keeps its local `{current, pageSize}` state; the
 * §8 wire query is minted here.
 */
export const statUser = async (
  client: ApiClient,
  query: AdminUserTrafficQuery,
  config?: QueryRequestConfig,
): Promise<PageResult<AdminUserTrafficRecord>> => {
  const page = await requestInternal(client, 'adminStatsUserTraffic', {
    query: { user_id: query.user_id, page: query.current, per_page: query.pageSize },
    ...config,
  });
  return { data: page.items, total: page.total };
};
