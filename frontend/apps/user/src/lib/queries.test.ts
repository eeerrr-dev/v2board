import { describe, expect, it, vi } from 'vitest';
import type { PlaceholderDataFunction, UseQueryOptions } from '@tanstack/react-query';
import type { KnowledgeCategory } from '@v2board/types';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const queriesSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'queries.ts'), 'utf8');

const useMutation = vi.hoisted(() => vi.fn((options: unknown) => options));
const useQuery = vi.hoisted(() => vi.fn((options: UseQueryOptions) => options));
const invalidateQueries = vi.hoisted(() => vi.fn());

vi.mock('@tanstack/react-query', () => ({
  queryOptions: (options: unknown) => options,
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

  it('keeps order cancel refresh scoped to list queries like the bundled typo path', async () => {
    const { useCancelOrderMutation } = await import('./queries');
    const mutation = useCancelOrderMutation() as unknown as {
      onSuccess: () => void;
    };

    invalidateQueries.mockReset();
    expect(mutation.onSuccess()).toBeUndefined();

    expect(invalidateQueries).toHaveBeenCalledTimes(1);
    const options = invalidateQueries.mock.calls[0]![0] as {
      predicate: (query: { queryKey: readonly unknown[] }) => boolean;
      queryKey: readonly unknown[];
    };
    expect(options.queryKey).toEqual(['user', 'orders']);
    expect(options.predicate({ queryKey: ['user', 'orders', 'all'] })).toBe(true);
    expect(options.predicate({ queryKey: ['user', 'orders', 'detail', 'TRADE123'] })).toBe(false);
    expect(queriesSource).toContain('void queryClient.invalidateQueries({');
    expect(queriesSource).toContain("queryKey: ['user', 'orders']");
    expect(queriesSource).toContain("query.queryKey[2] !== 'detail'");
  });
});
