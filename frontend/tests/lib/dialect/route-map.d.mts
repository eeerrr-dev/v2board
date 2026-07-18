export interface RouteShape {
  method: string;
  path: string;
  query?: string[];
  bodyKeys?: string[];
  aliases?: string[];
  params?: Record<string, string[]>;
}

export interface RouteEntry {
  id: string;
  legacy: RouteShape;
  modern: RouteShape;
}

export const API_PREFIX: string;
export const WORLDS: readonly string[];
export const SERVER_TYPES: readonly string[];
export const routeMap: readonly RouteEntry[];
export function routeEntry(id: string): RouteEntry;
export function worldRoute(id: string, world: string): RouteShape;
export function resolveRoutePath(
  id: string,
  world: string,
  options?: { securePath?: string; params?: Record<string, string>; query?: Record<string, string> },
): string;
export function matchRoute(
  world: string,
  request: {
    method: string;
    pathname: string;
    searchParams?: URLSearchParams;
    body?: unknown;
    securePath?: string;
  },
): { id: string; params: Record<string, string> } | null;
