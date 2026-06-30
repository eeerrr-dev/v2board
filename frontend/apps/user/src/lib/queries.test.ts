import { describe, expect, it, vi } from 'vitest';
import type { PlaceholderDataFunction, UseQueryOptions } from '@tanstack/react-query';
import type { KnowledgeCategory, SubscribeInfo, UserInfo } from '@v2board/types';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const queriesSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'queries.ts'), 'utf8');

const useMutation = vi.hoisted(() => vi.fn((options: unknown) => options));
const useQuery = vi.hoisted(() => vi.fn((options: UseQueryOptions) => options));
const invalidateQueries = vi.hoisted(() => vi.fn());

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
  user: {
    fetchKnowledge: vi.fn(),
    knowledgeDetail: vi.fn(),
    getStripePublicKey: vi.fn(),
  },
}));

describe('user query state behavior', () => {
  it('centralizes query definitions behind TanStack Query queryOptions', () => {
    expect(queriesSource).toContain(
      'queryOptions({ queryKey: userKeys.info, queryFn: fetchUserInfo })',
    );
    expect(queriesSource).toContain('export const userQueryOptions = {');
  });

  it('keeps the previous knowledge list while a new search request is pending', async () => {
    const { useKnowledge } = await import('./queries');

    const options = useKnowledge('zh-CN', 'new keyword') as unknown as UseQueryOptions<
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

    const options = useKnowledgeDetail(1, 'zh-CN') as unknown as UseQueryOptions;

    expect(options.gcTime).toBeUndefined();
  });

  it('fetches the Stripe public key as a forever-cached query gated on the selected method', async () => {
    const { useStripePublicKey } = await import('./queries');

    const disabled = useStripePublicKey(undefined) as unknown as UseQueryOptions;
    expect(disabled.enabled).toBe(false);
    expect(disabled.staleTime).toBe(Infinity);

    const enabled = useStripePublicKey('9') as unknown as UseQueryOptions;
    expect(enabled.enabled).toBe(true);
    expect(enabled.queryKey).toEqual(['user', 'stripePublicKey', '9']);
    expect(enabled.staleTime).toBe(Infinity);

    const optedOut = useStripePublicKey('9', { enabled: false }) as unknown as UseQueryOptions;
    expect(optedOut.enabled).toBe(false);

    expect(queriesSource).not.toContain('useStripePublicKeyMutation');
  });

  it('does not expose active-session query state absent from the original user bundle', async () => {
    const queries = await import('./queries');
    const keys = queries.userKeys as unknown as Record<string, unknown>;
    const exports = queries as unknown as Record<string, unknown>;

    expect(keys.sessions).toBeUndefined();
    expect(exports.useActiveSessions).toBeUndefined();
    expect(exports.useRemoveSessionMutation).toBeUndefined();
  });

  it('keeps route-id query keys aligned with the old direct route params', () => {
    expect(queriesSource).toContain('queryKey: userKeys.orderDetail(tradeNo as string)');
    expect(queriesSource).toContain('queryKey: userKeys.plan(id as number | string)');
    expect(queriesSource).toContain('queryKey: userKeys.ticketDetail(id as number | string)');
    expect(queriesSource).toContain('queryKey: userKeys.knowledgeDetail(id as number | string, language)');
    expect(queriesSource).not.toContain("userKeys.orderDetail(tradeNo ?? '')");
    expect(queriesSource).not.toContain("userKeys.plan(id ?? '')");
    expect(queriesSource).not.toContain("userKeys.ticketDetail(id ?? '')");
    expect(queriesSource).not.toContain("userKeys.knowledgeDetail(id ?? '', language)");
  });

  it('keeps the user/info and subscribe queryFns pure (chat reporting moved to QueryCache)', () => {
    expect(queriesSource).toContain('export function fetchUserInfo() {\n  return user.info(apiClient);\n}');
    expect(queriesSource).toContain('return user.getSubscribe(apiClient);');
    // The reporters must not run inside the queryFns anymore — main.tsx wires them
    // through QueryCache onSuccess instead.
    expect(queriesSource).not.toContain('reportUserInfoToChat(info)');
    expect(queriesSource).not.toContain('reportSubscribeToChat(data)');
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
    const { useCancelOrderMutation } = await import('./queries');
    const mutation = useCancelOrderMutation() as unknown as {
      onSuccess: () => void;
    };

    invalidateQueries.mockReset();
    expect(mutation.onSuccess()).toBeUndefined();

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
    const options = invalidateQueries.mock.calls[0]![0] as {
      queryKey: readonly unknown[];
    };
    expect(options.queryKey).toEqual(['user', 'orders']);
    expect(queriesSource).toContain("void queryClient.invalidateQueries({ queryKey: ['user', 'orders'] });");
    expect(queriesSource).toContain("queryKey: ['user', 'orders']");
    expect(queriesSource).not.toContain("query.queryKey[2] !== 'detail'");
  });

  it('invalidates the ticket list from the ticket mutations, not the call sites', async () => {
    const queries = await import('./queries');
    const ticketListMutations = ['useSaveTicketMutation', 'useCloseTicketMutation'] as const;

    for (const name of ticketListMutations) {
      const factory = queries[name] as unknown as () => { onSuccess: () => void };
      const mutation = factory();

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
      const mutation = factory();

      invalidateQueries.mockReset();
      expect(mutation.onSuccess()).toBeUndefined();

      expect(invalidateQueries).toHaveBeenCalledTimes(1);
      const options = invalidateQueries.mock.calls[0]![0] as { queryKey: readonly unknown[] };
      expect(options.queryKey).toEqual(['user', 'info']);
    }
  });

  it('keeps the payment-method query off the per-mount refetch path while leaving plans live', async () => {
    const { usePaymentMethods, usePlans } = await import('./queries');

    const payments = usePaymentMethods() as unknown as UseQueryOptions;
    expect(payments.staleTime).toBe(5 * 60_000);

    const plans = usePlans() as unknown as UseQueryOptions;
    expect(plans.staleTime).toBeUndefined();
  });

  it('owns the order-status 3s self-stopping poll cadence inside useOrderStatus', () => {
    // The stop condition (leave pending status 0, or error) is intrinsic to the
    // /user/order/check poll, so it lives on the hook and the page only toggles
    // enabled. The callback is typed against the concrete number query here.
    expect(queriesSource).toContain(
      "query.state.status === 'error' || (query.state.data ?? 0) !== 0 ? false : 3000",
    );
  });
});
