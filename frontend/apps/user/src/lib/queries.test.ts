import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import type { PlaceholderDataFunction, UseQueryOptions } from '@tanstack/react-query';
import type { KnowledgeCategory, SubscribeInfo, UserInfo } from '@v2board/types';

// useQuery/useMutation are mocked to echo their options back, but the React
// Compiler compiles these hooks (in tests exactly as in production), so every
// hook call still needs a real React render context for its memo cache.
function callHook<T>(hook: () => T): T {
  return renderHook(hook).result.current;
}

const useMutation = vi.hoisted(() => vi.fn((options: unknown) => options));
const useQuery = vi.hoisted(() => vi.fn((options: UseQueryOptions) => options));
const invalidateQueries = vi.hoisted(() => vi.fn());
const apiUser = vi.hoisted(() => ({
  checkLogin: vi.fn(),
  info: vi.fn(),
  getSubscribe: vi.fn(),
  fetchKnowledge: vi.fn(),
  knowledgeDetail: vi.fn(),
  prepareStripePaymentIntent: vi.fn(),
}));

vi.mock('@tanstack/react-query', () => ({
  queryOptions: (options: unknown) => options,
  keepPreviousData: (previous: unknown) => previous,
  useMutation,
  useQuery,
  useQueryClient: () => ({
    invalidateQueries,
  }),
}));

vi.mock('./api', () => ({
  apiClient: {},
}));

vi.mock('@v2board/api-client', () => ({
  user: apiUser,
}));

type QueryFn = (context: { signal: AbortSignal }) => Promise<unknown>;

describe('user query state behavior', () => {
  it('centralizes the user/info and subscribe queries behind shared queryOptions definitions', async () => {
    const { userQueryOptions, useUserInfo, useSubscribe } = await import('./queries');

    apiUser.info.mockReset().mockResolvedValue({ email: 'user@example.test' });
    apiUser.getSubscribe.mockReset().mockResolvedValue({ subscribe_url: 'https://s.test/sub' });
    const signal = new AbortController().signal;

    const info = userQueryOptions.info() as unknown as UseQueryOptions;
    expect(info.queryKey).toEqual(['user', 'info']);
    await expect((info.queryFn as QueryFn)({ signal })).resolves.toEqual({
      email: 'user@example.test',
    });
    const { apiClient } = await import('./api');
    expect(apiUser.info).toHaveBeenCalledWith(apiClient, { signal });

    const subscribe = userQueryOptions.subscribe() as unknown as UseQueryOptions;
    expect(subscribe.queryKey).toEqual(['user', 'subscribe']);
    await expect((subscribe.queryFn as QueryFn)({ signal })).resolves.toEqual({
      subscribe_url: 'https://s.test/sub',
    });
    expect(apiUser.getSubscribe).toHaveBeenCalledWith(apiClient, { signal });

    // The hooks consume the same centralized definitions instead of redefining keys.
    expect((callHook(() => useUserInfo()) as unknown as UseQueryOptions).queryKey).toEqual(['user', 'info']);
    expect((callHook(() => useSubscribe()) as unknown as UseQueryOptions).queryKey).toEqual(['user', 'subscribe']);
  });

  it('defines the login-session probe with a stable key and forwards TanStack aborts', async () => {
    const { userKeys, userQueryOptions } = await import('./queries');
    const { apiClient } = await import('./api');
    const signal = new AbortController().signal;
    apiUser.checkLogin.mockReset().mockResolvedValue({ is_login: true });

    const options = userQueryOptions.checkLogin() as unknown as UseQueryOptions;

    expect(userKeys.checkLogin).toEqual(['user', 'checkLogin']);
    expect(options.queryKey).toEqual(userKeys.checkLogin);
    expect(options.retry).toBe(false);
    expect(options.staleTime).toBe(0);
    await expect((options.queryFn as QueryFn)({ signal })).resolves.toEqual({ is_login: true });
    expect(apiUser.checkLogin).toHaveBeenCalledWith(apiClient, { signal });
  });

  it('keeps the previous knowledge list while a new search request is pending', async () => {
    const { useKnowledge } = await import('./queries');

    const options = callHook(() => useKnowledge('zh-CN', 'new keyword')) as unknown as UseQueryOptions<
      KnowledgeCategory,
      Error,
      KnowledgeCategory,
      readonly unknown[]
    >;
    const previous: KnowledgeCategory = {
      常见问题: [
        {
          id: 1,
          title: 'Old',
          category: '常见问题',
          sort: 0,
          show: 1,
          updated_at: 1,
        },
      ],
    };

    const placeholderData = options.placeholderData as PlaceholderDataFunction<
      KnowledgeCategory,
      Error,
      KnowledgeCategory,
      readonly unknown[]
    >;

    expect(placeholderData(previous, undefined)).toBe(previous);
  });

  it('lets knowledge detail use the normal TanStack cache lifecycle', async () => {
    const { useKnowledgeDetail } = await import('./queries');

    const options = callHook(() => useKnowledgeDetail(1, 'zh-CN')) as unknown as UseQueryOptions;

    expect(options.gcTime).toBeUndefined();
  });

  it('never reuses a superseded Stripe PaymentIntent cache entry', async () => {
    const { useStripePaymentIntent } = await import('./queries');
    const { apiClient } = await import('./api');
    const signal = new AbortController().signal;
    apiUser.prepareStripePaymentIntent.mockReset().mockResolvedValue({
      public_key: 'pk_test',
      client_secret: 'pi_test_secret',
      amount: 100,
      currency: 'cny',
    });

    const disabled = callHook(() => useStripePaymentIntent(undefined, undefined)) as unknown as UseQueryOptions;
    expect(disabled.enabled).toBe(false);
    expect(disabled.staleTime).toBe(0);
    expect(disabled.gcTime).toBe(0);

    const enabled = callHook(() => useStripePaymentIntent('ORDER123', 9)) as unknown as UseQueryOptions;
    expect(enabled.enabled).toBe(true);
    expect(enabled.queryKey).toEqual(['user', 'stripePaymentIntent', 'ORDER123', 9]);
    expect(enabled.staleTime).toBe(0);
    expect(enabled.gcTime).toBe(0);
    expect(enabled.refetchOnMount).toBe('always');
    await (enabled.queryFn as QueryFn)({ signal });
    expect(apiUser.prepareStripePaymentIntent).toHaveBeenCalledWith(
      apiClient,
      { trade_no: 'ORDER123', method: 9 },
      { signal },
    );

    const optedOut = callHook(() => useStripePaymentIntent('ORDER123', 9, {
      enabled: false,
    })) as unknown as UseQueryOptions;
    expect(optedOut.enabled).toBe(false);
  });

  it('exposes the active-session query keyed under the user namespace', async () => {
    const { userKeys, useActiveSessions } = await import('./queries');

    expect(userKeys.sessions).toEqual(['user', 'sessions']);

    const options = callHook(() => useActiveSessions()) as unknown as UseQueryOptions;
    expect(options.queryKey).toEqual(['user', 'sessions']);
  });

  it('invalidates the session list after a revoke so the removed device disappears', async () => {
    const { useRemoveSessionMutation } = await import('./queries');
    const mutation = callHook(() => useRemoveSessionMutation()) as unknown as { onSuccess: () => void };

    invalidateQueries.mockReset();
    expect(mutation.onSuccess()).toBeUndefined();

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
    const options = invalidateQueries.mock.calls[0]![0] as { queryKey: readonly unknown[] };
    expect(options.queryKey).toEqual(['user', 'sessions']);
  });

  it('keeps route-id query keys aligned with the raw route params and gates fetching on presence', async () => {
    const { userQueryOptions, useOrder, usePlan, useTicket, useKnowledgeDetail } =
      await import('./queries');

    // An unset route id lands a literal `undefined` in the key — never a `?? ''`
    // sentinel, which would be a distinct, request-reaching cache key.
    expect(userQueryOptions.orderDetail(undefined).queryKey).toStrictEqual([
      'user',
      'orders',
      'detail',
      undefined,
    ]);
    expect(userQueryOptions.plan(undefined).queryKey).toStrictEqual(['user', 'plan', undefined]);
    expect(userQueryOptions.ticketDetail(undefined).queryKey).toStrictEqual([
      'user',
      'ticket',
      undefined,
    ]);
    expect(userQueryOptions.knowledgeDetail(undefined, 'zh-CN').queryKey).toStrictEqual([
      'user',
      'knowledge',
      'detail',
      undefined,
      'zh-CN',
    ]);

    // Present ids pass through raw, matching the old direct route params.
    expect(userQueryOptions.orderDetail('T123').queryKey).toStrictEqual([
      'user',
      'orders',
      'detail',
      'T123',
    ]);
    expect(userQueryOptions.plan(7).queryKey).toStrictEqual(['user', 'plan', 7]);
    expect(userQueryOptions.ticketDetail(3).queryKey).toStrictEqual(['user', 'ticket', 3]);

    // The undefined keys stay inert because each wrapper hook gates `enabled`.
    expect((callHook(() => useOrder(undefined)) as unknown as UseQueryOptions).enabled).toBe(false);
    expect((callHook(() => usePlan(undefined)) as unknown as UseQueryOptions).enabled).toBe(false);
    expect((callHook(() => useTicket(undefined)) as unknown as UseQueryOptions).enabled).toBe(false);
    expect((callHook(() => useKnowledgeDetail(undefined, 'zh-CN')) as unknown as UseQueryOptions).enabled).toBe(
      false,
    );
    expect((callHook(() => useOrder('T123')) as unknown as UseQueryOptions).enabled).toBe(true);
  });

  it('keeps the user/info and subscribe queryFns pure (chat reporting moved to QueryCache)', async () => {
    const { userQueryOptions } = await import('./queries');

    const push = vi.fn();
    window.Tawk_API = {};
    window.$crisp = { push };
    apiUser.info.mockReset().mockResolvedValue({ email: 'user@example.test', balance: 100 });
    apiUser.getSubscribe.mockReset().mockResolvedValue({ u: 1, d: 2, transfer_enable: 3 });
    const context = { signal: new AbortController().signal };

    await (userQueryOptions.info().queryFn as QueryFn)(context);
    await (userQueryOptions.subscribe().queryFn as QueryFn)(context);

    // The reporters must not run inside the queryFns anymore — main.tsx wires
    // them through QueryCache onSuccess instead.
    expect(push).not.toHaveBeenCalled();
    expect(window.Tawk_API).toEqual({});
  });

  it('reports the user to Tawk and Crisp after a successful user/info fetch', async () => {
    const { reportUserInfoToChat } = await import('./queries');
    const push = vi.fn();
    window.Tawk_API = {};
    window.$crisp = { push };

    reportUserInfoToChat({ email: 'user@example.test', balance: 1234 } as unknown as UserInfo);

    expect(window.Tawk_API).toEqual({
      visitor: { name: 'user@example.test', email: 'user@example.test' },
    });
    expect(push).toHaveBeenCalledWith(['set', 'user:email', 'user@example.test']);
    expect(push).toHaveBeenCalledWith(['set', 'session:data', [[['Balance', 12.34]]]]);
  });

  it('reports the subscription to Crisp with formatted traffic after a successful fetch', async () => {
    const { reportSubscribeToChat } = await import('./queries');
    const push = vi.fn();
    window.$crisp = { push };

    reportSubscribeToChat({
      expired_at: 1_700_000_000,
      plan: { name: 'Pro' },
      u: 100,
      d: 200,
      transfer_enable: 4096,
    } as unknown as SubscribeInfo);

    expect(push).toHaveBeenCalledTimes(1);
    const rows = push.mock.calls[0]![0][2][0] as [string, unknown][];
    expect(rows).toContainEqual(['Plan', 'Pro']);
    expect(rows).toContainEqual(['UsedTraffic', '300.00 B']);
    expect(rows).toContainEqual(['AllTraffic', '4.00 KB']);
    expect(rows.find(([label]) => label === 'ExpireTime')?.[1]).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('invalidates all order queries after cancel so detail state is not kept stale', async () => {
    const { useCancelOrderMutation, userKeys } = await import('./queries');
    const mutation = callHook(() => useCancelOrderMutation()) as unknown as {
      onSuccess: () => void;
    };

    invalidateQueries.mockReset();
    expect(mutation.onSuccess()).toBeUndefined();

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
    // Exactly the whole orders prefix (list + detail + status) via the userKeys
    // factory — no predicate filtering, and no raw literal a key-shape change
    // could silently miss.
    expect(userKeys.ordersScope).toEqual(['user', 'orders']);
    expect(invalidateQueries).toHaveBeenCalledWith({ queryKey: userKeys.ordersScope });
  });

  it('invalidates the ticket list from the ticket mutations, not the call sites', async () => {
    const queries = await import('./queries');
    const ticketListMutations = ['useSaveTicketMutation', 'useCloseTicketMutation'] as const;

    for (const name of ticketListMutations) {
      const factory = queries[name] as unknown as () => { onSuccess: () => void };
      const mutation = callHook(() => factory());

      invalidateQueries.mockReset();
      expect(mutation.onSuccess()).toBeUndefined();

      expect(invalidateQueries).toHaveBeenCalledTimes(1);
      const options = invalidateQueries.mock.calls[0]![0] as { queryKey: readonly unknown[] };
      expect(options.queryKey).toEqual(['user', 'tickets']);
    }
  });

  it('invalidates the user record from every mutation that changes it, not the call sites', async () => {
    const queries = await import('./queries');
    const userInfoMutations = [
      'useUpdateProfileMutation',
      'useTransferMutation',
      'useRedeemGiftCardMutation',
      'useUnbindTelegramMutation',
    ] as const;

    for (const name of userInfoMutations) {
      const factory = queries[name] as unknown as () => { onSuccess: () => void };
      const mutation = callHook(() => factory());

      invalidateQueries.mockReset();
      expect(mutation.onSuccess()).toBeUndefined();

      expect(invalidateQueries).toHaveBeenCalledTimes(1);
      const options = invalidateQueries.mock.calls[0]![0] as { queryKey: readonly unknown[] };
      expect(options.queryKey).toEqual(['user', 'info']);
    }
  });

  it('invalidates every cached credential projection after reset-subscribe', async () => {
    const { useResetSubscribeMutation } = await import('./queries');
    const mutation = callHook(() => useResetSubscribeMutation()) as unknown as { onSuccess: () => void };

    invalidateQueries.mockReset();
    expect(mutation.onSuccess()).toBeUndefined();
    expect(invalidateQueries).toHaveBeenNthCalledWith(1, { queryKey: ['user', 'info'] });
    expect(invalidateQueries).toHaveBeenNthCalledWith(2, { queryKey: ['user', 'subscribe'] });
  });

  it('keeps the payment-method query off the per-mount refetch path while leaving plans live', async () => {
    const { usePaymentMethods, usePlans } = await import('./queries');

    const payments = callHook(() => usePaymentMethods()) as unknown as UseQueryOptions;
    expect(payments.staleTime).toBe(5 * 60_000);

    const plans = callHook(() => usePlans()) as unknown as UseQueryOptions;
    expect(plans.staleTime).toBeUndefined();
  });

  it('owns the order-status 3s self-stopping poll cadence inside useOrderStatus', async () => {
    const { useOrderStatus } = await import('./queries');

    // The caller only decides whether to poll at all, via enabled.
    const disabled = callHook(() => useOrderStatus(undefined)) as unknown as UseQueryOptions;
    expect(disabled.enabled).toBe(false);

    // The stop condition (leave pending status 0, or error) is intrinsic to the
    // /user/order/check poll, so it lives on the hook.
    const options = callHook(() => useOrderStatus('T123')) as unknown as UseQueryOptions & {
      refetchInterval: (query: { state: { status: string; data?: number } }) => number | false;
    };
    expect(options.enabled).toBe(true);
    expect(options.refetchInterval({ state: { status: 'pending', data: undefined } })).toBe(3000);
    expect(options.refetchInterval({ state: { status: 'success', data: 0 } })).toBe(3000);
    expect(options.refetchInterval({ state: { status: 'success', data: 3 } })).toBe(false);
    expect(options.refetchInterval({ state: { status: 'error', data: 0 } })).toBe(false);
  });
});
