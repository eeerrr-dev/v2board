import type { output } from 'zod';
import type { ApiClient } from '../../client';
import { adminListQueryParams, pageSchema, type AdminListQuery } from '../../dialect';
import {
  adminStatSummarySchema,
  adminUserTrafficSchema,
  arraySchema,
  auditLogSchema,
  queueStatsSchema,
  queueWorkloadSchema,
  serverRankSchema,
  statSeriesPointSchema,
  systemLogSchema,
  userRankSchema,
} from '../../contracts';
import type { PageResult, QueryRequestConfig } from './shared';

export type AdminUserTrafficRecord = output<typeof adminUserTrafficSchema>;

export interface AdminUserTrafficQuery {
  user_id: number;
  current?: number;
  pageSize?: number;
}

/** GET /{secure_path}/system/queue-stats — dialect v2 bare object (§6.1, W9). */
export const queueStats = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/system/queue-stats'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: queueStatsSchema,
    ...config,
  });

/** GET /{secure_path}/system/queue-workload — dialect v2 bare array (§6.1, W9). */
export const queueWorkload = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/system/queue-workload'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(queueWorkloadSchema),
    ...config,
  });

/** §7.1 — the GET system/logs filter whitelist (`level` only) and §7.2 sort columns. */
export const SYSTEM_LOG_FILTER_FIELDS = ['level'] as const;
export const SYSTEM_LOG_SORT_FIELDS = ['created_at', 'level'] as const;
export type SystemLogFilterField = (typeof SYSTEM_LOG_FILTER_FIELDS)[number];
export type AdminSystemLogRecord = output<typeof systemLogSchema>;

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
  client.request({
    url: client.resolveAdminPath('/system/logs'),
    method: 'GET',
    dialect: 'v2',
    params: adminListQueryParams(query),
    responseSchema: pageSchema(systemLogSchema),
    ...config,
  });

/** §7.1 — the GET system/audit-logs filter whitelist and §7.2 sort columns. */
export const AUDIT_LOG_FILTER_FIELDS = ['surface', 'actor_email', 'method'] as const;
export const AUDIT_LOG_SORT_FIELDS = ['created_at'] as const;
export type AuditLogFilterField = (typeof AUDIT_LOG_FILTER_FIELDS)[number];
export type AdminAuditLogRecord = output<typeof auditLogSchema>;

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
  client.request({
    url: client.resolveAdminPath('/system/audit-logs'),
    method: 'GET',
    dialect: 'v2',
    params: adminListQueryParams(query),
    responseSchema: pageSchema(auditLogSchema),
    ...config,
  });

/** §6.8 (W14): the `stats/server-rank` + `stats/user-rank` window selector. */
export type StatsRankWindow = 'today' | 'previous';

/** GET /{secure_path}/stats/summary — dialect v2 bare object (§6.8, W14):
 * the three legacy aliases collapsed into one route; money in integer cents. */
export const statSummary = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/stats/summary'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: adminStatSummarySchema,
    ...config,
  });

/** GET /{secure_path}/stats/server-rank `?window=today|previous` — bare array (§6.8, W14). */
export const statServerRank = (
  client: ApiClient,
  window: StatsRankWindow,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath('/stats/server-rank'),
    method: 'GET',
    dialect: 'v2',
    params: { window },
    responseSchema: arraySchema(serverRankSchema),
    ...config,
  });

/** GET /{secure_path}/stats/user-rank `?window=today|previous` — bare array (§6.8, W14). */
export const statUserRank = (
  client: ApiClient,
  window: StatsRankWindow,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath('/stats/user-rank'),
    method: 'GET',
    dialect: 'v2',
    params: { window },
    responseSchema: arraySchema(userRankSchema),
    ...config,
  });

/** GET /{secure_path}/stats/orders — bare `{series, date, value}` array (§6.8, W14):
 * snake_case series slugs, integer-cent money. */
export const statOrder = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/stats/orders'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(statSeriesPointSchema),
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
  const page = await client.request({
    url: client.resolveAdminPath('/stats/user-traffic'),
    method: 'GET',
    dialect: 'v2',
    params: { user_id: query.user_id, page: query.current, per_page: query.pageSize },
    responseSchema: pageSchema(adminUserTrafficSchema),
    ...config,
  });
  return { data: page.items, total: page.total };
};
