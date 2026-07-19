import type { ApiRequestConfig } from '../../client';
import type { FilterOp } from '../../dialect';

/**
 * App-internal shared filter clause (`{key, condition, value}`), persisted in
 * cross-page sessionStorage handoffs and drawer state. It is not a wire
 * shape: each list endpoint translates it into the §7 DSL at this boundary
 * (see `userFilterClauses`).
 */
export interface AdminFilter {
  key: string;
  condition: string;
  value: string | number | null;
}

export interface AdminPageQuery {
  current?: number;
  pageSize?: number;
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
  filter?: AdminFilter[];
}

export interface PageResult<T> {
  data: T[];
  total?: number;
}

export type QueryRequestConfig = Pick<ApiRequestConfig, 'signal'>;

/** §7.1 — legacy `{key, condition, value}` conditions folded onto the op set. */
export const LEGACY_FILTER_OPS: Record<string, FilterOp> = {
  '=': 'eq',
  is: 'eq',
  '!=': 'neq',
  '<>': 'neq',
  not: 'neq',
  like: 'like',
  模糊: 'like',
  '>': 'gt',
  '>=': 'gte',
  '<': 'lt',
  '<=': 'lte',
};
