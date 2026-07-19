import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

// useMutation/useQueryClient are mocked to echo their options back, but the
// React Compiler compiles these hooks in tests exactly as in production, so
// every hook call still needs a real React render context for its memo cache.
function callHook<T>(hook: () => T): T {
  return renderHook(hook).result.current;
}

const useMutation = vi.hoisted(() => vi.fn((options: unknown) => options));
const cancelQueries = vi.hoisted(() => vi.fn());
const getQueriesData = vi.hoisted(() => vi.fn());
const setQueriesData = vi.hoisted(() => vi.fn());
const setQueryData = vi.hoisted(() => vi.fn());
const invalidateQueries = vi.hoisted(() => vi.fn());

vi.mock('@tanstack/react-query', () => ({
  queryOptions: (options: unknown) => options,
  keepPreviousData: Symbol('keepPreviousData'),
  skipToken: Symbol('skipToken'),
  useMutation,
  useQuery: vi.fn((options: unknown) => options),
  useQueryClient: () => ({
    cancelQueries,
    getQueriesData,
    setQueriesData,
    setQueryData,
    invalidateQueries,
  }),
}));

vi.mock('./api', () => ({ apiClient: {} }));
vi.mock('@v2board/api-client', () => ({
  INLINE_MUTATION_ERROR_META: {},
  admin: {},
}));

interface ToggleHandlers<TVars> {
  onMutate: (vars: TVars) => Promise<unknown>;
  onError: (error: Error, vars: TVars, context: unknown) => void;
  onSettled: () => Promise<void>;
}

type Updater = (cached: unknown) => unknown;

// Runs a toggle hook's onMutate against an empty cache and hands back the
// scope filter and updater it passed to setQueriesData, so each test can
// assert the flip logic directly on seeded rows.
async function runOnMutate<TVars>(factory: () => unknown, vars: TVars) {
  const handlers = callHook(factory) as unknown as ToggleHandlers<TVars>;
  cancelQueries.mockClear();
  getQueriesData.mockClear().mockReturnValue([]);
  setQueriesData.mockClear();
  await handlers.onMutate(vars);
  const [scope, updater] = setQueriesData.mock.calls[0]! as [unknown, Updater];
  return { handlers, scope, updater };
}

describe('admin optimistic toggle mutations', () => {
  it('flips only the targeted plan row for the requested show/renew key', async () => {
    const { useUpdatePlanMutation } = await import('./queries');
    const seeded = [
      { id: 1, show: false, renew: false },
      { id: 2, show: false, renew: false },
    ];

    const shows = await runOnMutate(useUpdatePlanMutation, { id: 1, key: 'show', value: true });
    expect(shows.scope).toEqual({ queryKey: ['admin', 'plans'], exact: true });
    expect(shows.updater(seeded)).toEqual([
      { id: 1, show: true, renew: false },
      { id: 2, show: false, renew: false },
    ]);
    expect(shows.updater(undefined)).toBeUndefined();

    const renews = await runOnMutate(useUpdatePlanMutation, { id: 2, key: 'renew', value: true });
    expect(renews.updater(seeded)).toEqual([
      { id: 1, show: false, renew: false },
      { id: 2, show: false, renew: true },
    ]);
  });

  it('flips the coupon row inside every cached filter page, keeping totals', async () => {
    const { useShowCouponMutation } = await import('./queries');
    const { scope, updater } = await runOnMutate(useShowCouponMutation, { id: 7, show: true });

    // Non-exact on purpose: every cached pagination/filter page holds a copy.
    expect(scope).toEqual({ queryKey: ['admin', 'coupons'] });
    expect(
      updater({
        total: 12,
        items: [
          { id: 7, show: false },
          { id: 8, show: false },
        ],
      }),
    ).toEqual({
      total: 12,
      items: [
        { id: 7, show: true },
        { id: 8, show: false },
      ],
    });
  });

  it('matches server nodes on type and id together, never id alone', async () => {
    const { useUpdateServerMutation } = await import('./queries');
    const { scope, updater } = await runOnMutate(useUpdateServerMutation, {
      type: 'vmess',
      id: 1,
      show: true,
    });

    expect(scope).toEqual({ queryKey: ['admin', 'servers', 'nodes'], exact: true });
    expect(
      updater([
        { type: 'shadowsocks', id: 1, show: false },
        { type: 'vmess', id: 1, show: false },
      ]),
    ).toEqual([
      { type: 'shadowsocks', id: 1, show: false },
      { type: 'vmess', id: 1, show: true },
    ]);
  });

  it('keeps the knowledge flip off the detail and category sibling caches', async () => {
    const { useShowKnowledgeMutation } = await import('./queries');
    const { scope, updater } = await runOnMutate(useShowKnowledgeMutation, { id: 3, show: false });

    // exact:true — ['admin','knowledge'] is a key prefix of the detail and
    // categories caches, whose shapes the list updater must never touch.
    expect(scope).toEqual({ queryKey: ['admin', 'knowledge'], exact: true });
    expect(
      updater([
        { id: 3, show: true },
        { id: 4, show: true },
      ]),
    ).toEqual([
      { id: 3, show: false },
      { id: 4, show: true },
    ]);
  });

  it('flips notice and payment rows through the same optimistic path', async () => {
    const { useShowNoticeMutation, useShowPaymentMutation } = await import('./queries');

    const notice = await runOnMutate(useShowNoticeMutation, { id: 5, show: true });
    expect(notice.scope).toEqual({ queryKey: ['admin', 'notices'], exact: true });
    expect(notice.updater([{ id: 5, show: false }])).toEqual([{ id: 5, show: true }]);

    const payment = await runOnMutate(useShowPaymentMutation, { id: 9, enable: false });
    expect(payment.scope).toEqual({ queryKey: ['admin', 'payments'], exact: true });
    expect(payment.updater([{ id: 9, enable: true }])).toEqual([{ id: 9, enable: false }]);
  });

  it('rolls the snapshots back on error and refetches on settlement', async () => {
    const { useUpdatePlanMutation } = await import('./queries');
    const handlers = callHook(useUpdatePlanMutation) as unknown as ToggleHandlers<{
      id: number;
      key: 'show' | 'renew';
      value: boolean;
    }>;
    const vars = { id: 1, key: 'show' as const, value: true };
    const seeded = [{ id: 1, show: false, renew: false }];

    cancelQueries.mockClear();
    getQueriesData.mockClear().mockReturnValue([[['admin', 'plans'], seeded]]);
    setQueriesData.mockClear();
    setQueryData.mockClear();
    invalidateQueries.mockClear();

    const context = await handlers.onMutate(vars);
    expect(cancelQueries).toHaveBeenCalledWith({ queryKey: ['admin', 'plans'], exact: true });

    handlers.onError(new Error('offline'), vars, context);
    expect(setQueryData).toHaveBeenCalledWith(['admin', 'plans'], seeded);

    await handlers.onSettled();
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: ['admin', 'plans'] });
  });
});
