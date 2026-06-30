import { user } from '@v2board/api-client';
import {
  keepPreviousData,
  queryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import type { UseQueryResult } from '@tanstack/react-query';
import type { SubscribeInfo, UserInfo } from '@v2board/types';
import dayjs from 'dayjs';
import { formatBytes } from '@v2board/config/format';
import { apiClient } from './api';

export type SaveOrderPayload = Parameters<typeof user.saveOrder>[1];
export type CheckoutOrderPayload = Parameters<typeof user.checkoutOrder>[1];

interface QueryFreshnessOptions {
  enabled?: boolean;
  refetchInterval?: number | false;
  refetchOnMount?: boolean | 'always';
}

declare global {
  interface Window {
    Tawk_API?: { visitor?: { name?: string; email?: string } };
    $crisp?: { push: (command: unknown[]) => void };
  }
}

// The original getUserInfo / getSubscribe sagas report the user to the Tawk and
// Crisp live-chat widgets right after each successful fetch. React Query v5
// removed the per-observer useQuery onSuccess; its canonical replacement is the
// QueryClient-level QueryCache onSuccess, which fires once per successful fetch
// keyed by query. main.tsx wires these reporters there (on userKeys.info /
// userKeys.subscribe), so the queryFns below stay pure while preserving the
// saga's "report after each successful 200" cadence, including refetches. The
// Crisp/Tawk payloads are external-integration contracts — keep their shape.
export function reportUserInfoToChat(info: UserInfo) {
  if (window.Tawk_API) {
    window.Tawk_API.visitor = { name: info.email, email: info.email };
  }
  if (window.$crisp) {
    window.$crisp.push(['set', 'user:email', info.email]);
    window.$crisp.push(['set', 'session:data', [[['Balance', info.balance / 100]]]]);
  }
}

export function reportSubscribeToChat(data: SubscribeInfo) {
  if (!window.$crisp) return;
  // Matches the legacy moment(1e3 * expired_at).format('YYYY-MM-DD'); a null
  // expiry becomes the epoch date exactly as the original does (no '-' fallback
  // here). dayjs formats in local time, byte-identical to the old manual pad.
  const expireTime = dayjs((data.expired_at ?? 0) * 1000).format('YYYY-MM-DD');
  window.$crisp.push([
    'set',
    'session:data',
    [
      [
        ['Plan', data.plan?.name || '-'],
        ['ExpireTime', expireTime],
        ['UsedTraffic', formatBytes(data.u + data.d)],
        ['AllTraffic', formatBytes(data.transfer_enable)],
      ],
    ],
  ]);
}

export const userKeys = {
  info: ['user', 'info'] as const,
  stat: ['user', 'stat'] as const,
  subscribe: ['user', 'subscribe'] as const,
  orders: (status?: number) => ['user', 'orders', status ?? 'all'] as const,
  orderDetail: (tradeNo: string) => ['user', 'orders', 'detail', tradeNo] as const,
  orderStatus: (tradeNo: string) => ['user', 'orders', 'status', tradeNo] as const,
  plans: ['user', 'plans'] as const,
  plan: (id: number | string) => ['user', 'plan', id] as const,
  payments: ['user', 'payments'] as const,
  notices: ['user', 'notices'] as const,
  tickets: ['user', 'tickets'] as const,
  ticketDetail: (id: number | string) => ['user', 'ticket', id] as const,
  invite: ['user', 'invite'] as const,
  inviteDetails: (current?: number, size?: number) =>
    ['user', 'invite', 'details', current ?? '', size ?? ''] as const,
  knowledge: (lang: string, kw?: string) => ['user', 'knowledge', lang, kw ?? ''] as const,
  knowledgeDetail: (id: number | string, lang: string) =>
    ['user', 'knowledge', 'detail', id, lang] as const,
  trafficLog: ['user', 'trafficLog'] as const,
  commConfig: ['user', 'comm'] as const,
  stripePublicKey: (methodId: string) => ['user', 'stripePublicKey', methodId] as const,
  servers: ['user', 'servers'] as const,
  telegramBot: ['user', 'telegram', 'bot'] as const,
};

export function fetchUserInfo() {
  return user.info(apiClient);
}

function fetchSubscribe() {
  return user.getSubscribe(apiClient);
}

export const userQueryOptions = {
  info: () => queryOptions({ queryKey: userKeys.info, queryFn: fetchUserInfo }),
  stat: () => queryOptions({ queryKey: userKeys.stat, queryFn: () => user.getStat(apiClient) }),
  subscribe: () =>
    queryOptions({
      queryKey: userKeys.subscribe,
      queryFn: fetchSubscribe,
    }),
  orders: (status?: number) =>
    queryOptions({
      queryKey: userKeys.orders(status),
      queryFn: () => user.fetchOrders(apiClient, status),
    }),
  orderDetail: (tradeNo: string | undefined) =>
    queryOptions({
      queryKey: userKeys.orderDetail(tradeNo as string),
      queryFn: () => user.orderDetail(apiClient, tradeNo as string),
    }),
  orderStatus: (tradeNo: string | undefined) =>
    queryOptions({
      queryKey: userKeys.orderStatus(tradeNo as string),
      queryFn: () => user.checkOrder(apiClient, tradeNo as string),
    }),
  plans: () =>
    queryOptions({
      queryKey: userKeys.plans,
      queryFn: () => user.fetchPlans(apiClient),
    }),
  plan: (id: number | string | undefined) =>
    queryOptions({
      queryKey: userKeys.plan(id as number | string),
      queryFn: () => user.fetchPlan(apiClient, id as number | string),
    }),
  payments: () =>
    queryOptions({ queryKey: userKeys.payments, queryFn: () => user.getPaymentMethod(apiClient) }),
  notices: () =>
    queryOptions({
      queryKey: userKeys.notices,
      queryFn: () => user.fetchNotices(apiClient),
    }),
  tickets: () =>
    queryOptions({
      queryKey: userKeys.tickets,
      queryFn: () => user.fetchTickets(apiClient),
    }),
  ticketDetail: (id: number | string | undefined) =>
    queryOptions({
      queryKey: userKeys.ticketDetail(id as number | string),
      queryFn: () => user.ticketDetail(apiClient, id as number | string),
    }),
  invite: () =>
    queryOptions({
      queryKey: userKeys.invite,
      queryFn: () => user.fetchInvite(apiClient),
    }),
  inviteDetails: (current?: number, pageSize?: number) =>
    queryOptions({
      queryKey: userKeys.inviteDetails(current, pageSize),
      queryFn: () => user.inviteDetails(apiClient, current, pageSize),
    }),
  knowledge: (language: string, keyword?: string) =>
    queryOptions({
      queryKey: userKeys.knowledge(language, keyword),
      // Pass the query's AbortSignal so a superseded debounced search GET is
      // cancelled instead of running to completion.
      queryFn: ({ signal }) => user.fetchKnowledge(apiClient, language, keyword, { signal }),
    }),
  knowledgeDetail: (id: number | string | undefined, language: string) =>
    queryOptions({
      queryKey: userKeys.knowledgeDetail(id as number | string, language),
      queryFn: ({ signal }) =>
        user.knowledgeDetail(apiClient, id as number | string, language, { signal }),
    }),
  trafficLog: () =>
    queryOptions({ queryKey: userKeys.trafficLog, queryFn: () => user.getTrafficLog(apiClient) }),
  commConfig: () =>
    queryOptions({
      queryKey: userKeys.commConfig,
      queryFn: () => user.commConfig(apiClient),
    }),
  stripePublicKey: (methodId: string | undefined) =>
    queryOptions({
      queryKey: userKeys.stripePublicKey(methodId as string),
      queryFn: () => user.getStripePublicKey(apiClient, Number(methodId)),
    }),
  servers: () =>
    queryOptions({
      queryKey: userKeys.servers,
      queryFn: () => user.fetchServers(apiClient),
    }),
  telegramBot: () =>
    queryOptions({
      queryKey: userKeys.telegramBot,
      queryFn: () => user.getTelegramBotInfo(apiClient),
    }),
};

export const useUserInfo = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.info(),
    ...options,
  });

export const useUserStat = () =>
  useQuery(userQueryOptions.stat());

export const useSubscribe = (options?: QueryFreshnessOptions & { enabled?: boolean }) =>
  useQuery({
    ...userQueryOptions.subscribe(),
    ...options,
  });

export const useOrders = (status?: number) =>
  useQuery(userQueryOptions.orders(status));

export const useOrder = (tradeNo: string | undefined) =>
  useQuery({
    ...userQueryOptions.orderDetail(tradeNo),
    enabled: Boolean(tradeNo),
  });

export const useOrderStatus = (tradeNo: string | undefined, options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.orderStatus(tradeNo),
    enabled: Boolean(tradeNo),
    ...options,
  });

export const usePlans = () =>
  useQuery(userQueryOptions.plans());

export const usePlan = (id: number | string | undefined) =>
  useQuery({
    ...userQueryOptions.plan(id),
    enabled: Boolean(id),
  });

export const usePaymentMethods = (options?: QueryFreshnessOptions & { enabled?: boolean }) =>
  useQuery({
    ...userQueryOptions.payments(),
    // Payment gateways are operator configuration that rarely changes, so avoid
    // refetching them on every checkout mount. (Plans are deliberately NOT given
    // a stale window: their capacity_limit/sold-out state is a live contract.)
    staleTime: 5 * 60_000,
    ...options,
  });

export const useNotices = () =>
  useQuery(userQueryOptions.notices());

export const useTickets = () =>
  useQuery(userQueryOptions.tickets());

export const useTicket = (id: number | string | undefined, options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.ticketDetail(id),
    enabled: Boolean(id),
    ...options,
  });

export const useInvite = () =>
  useQuery(userQueryOptions.invite());

export const useInviteDetails = (current?: number, pageSize?: number) =>
  useQuery({
    ...userQueryOptions.inviteDetails(current, pageSize),
    // Keep the current page visible while the next page request is in flight.
    placeholderData: keepPreviousData,
  });

export const useKnowledge = (language: string, keyword?: string) =>
  useQuery({
    ...userQueryOptions.knowledge(language, keyword),
    // Keep the current result list visible while debounced searches resolve.
    placeholderData: keepPreviousData,
  });

export const useKnowledgeDetail = (id: number | string | undefined, language: string) =>
  useQuery({
    ...userQueryOptions.knowledgeDetail(id, language),
    enabled: id !== undefined,
  });

export const useTrafficLog = () =>
  useQuery(userQueryOptions.trafficLog());

export const useCommConfig = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.commConfig(),
    ...options,
  });

export function useStripePublicKey(
  methodId: string | undefined,
  options?: { enabled?: boolean },
): UseQueryResult<string> {
  return useQuery({
    ...userQueryOptions.stripePublicKey(methodId),
    // The original only fetches the Stripe public key once per method and never
    // refetches or clears it, so cache it forever and gate on a present method.
    enabled: Boolean(methodId) && options?.enabled !== false,
    staleTime: Infinity,
  });
}

export const useServers = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.servers(),
    ...options,
  });

export const useTelegramBotInfo = (enabled: boolean) =>
  useQuery({
    ...userQueryOptions.telegramBot(),
    enabled,
    staleTime: 0,
  });

export function useChangePasswordMutation() {
  return useMutation({
    mutationFn: ({ oldPassword, newPassword }: { oldPassword: string; newPassword: string }) =>
      user.changePassword(apiClient, oldPassword, newPassword),
  });
}

export function useUpdateProfileMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.update>[1]) => user.update(apiClient, payload),
    // Profile edits change the cached user record; invalidate it here so every
    // consumer refreshes instead of each call site wiring its own refetch.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useResetSubscribeMutation() {
  return useMutation({
    mutationFn: () => user.resetSecurity(apiClient),
  });
}

export function useTransferMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (amount: number | string | undefined) => user.transfer(apiClient, amount),
    // Transfer moves commission into the balance shown by the user record.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useRedeemGiftCardMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => user.redeemGiftCard(apiClient, code),
    // A redeemed gift card credits the balance on the user record.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}

export function useCheckCouponMutation() {
  return useMutation({
    mutationFn: ({ code, planId }: { code: string; planId: number | string }) =>
      user.checkCoupon(apiClient, code, planId),
  });
}

export function useSaveOrderMutation() {
  return useMutation({
    mutationFn: (payload: SaveOrderPayload) => user.saveOrder(apiClient, payload),
  });
}

export function useCheckoutOrderMutation() {
  return useMutation({
    mutationFn: (payload: CheckoutOrderPayload) => user.checkoutOrder(apiClient, payload),
  });
}

export function useGenerateInviteMutation() {
  return useMutation({
    mutationFn: () => user.generateInvite(apiClient),
  });
}

export function useWithdrawCommissionMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.withdrawTicket>[1]) =>
      user.withdrawTicket(apiClient, payload),
  });
}

export function useSaveTicketMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.saveTicket>[1]) =>
      user.saveTicket(apiClient, payload),
  });
}

export function useReplyTicketMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.replyTicket>[1]) =>
      user.replyTicket(apiClient, payload),
  });
}

export function useCloseTicketMutation() {
  return useMutation({
    mutationFn: (id: number) => user.closeTicket(apiClient, id),
  });
}

export function useCancelOrderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tradeNo: string) => user.cancelOrder(apiClient, tradeNo),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['user', 'orders'] });
    },
  });
}

export function useNewPeriodMutation() {
  return useMutation({
    mutationFn: () => user.newPeriod(apiClient),
  });
}

export function useUnbindTelegramMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.unbindTelegram(apiClient),
    // Unbinding clears the telegram_id on the user record. The disabled
    // subscribe query is still refetched imperatively at the call site, since
    // invalidation does not refetch a query with enabled:false.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: userKeys.info });
    },
  });
}
