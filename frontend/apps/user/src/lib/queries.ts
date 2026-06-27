import { user } from '@v2board/api-client';
import { queryOptions, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { SubscribeInfo, UserInfo } from '@v2board/types';
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
// Crisp live-chat widgets right after each successful fetch. React Query v5 has
// no useQuery onSuccess, so the same pushes run inside the queryFn (which only
// resolves on a 200, matching the saga's `200 === code` guard).
function reportUserInfoToChat(info: UserInfo) {
  if (window.Tawk_API) {
    window.Tawk_API.visitor = { name: info.email, email: info.email };
  }
  if (window.$crisp) {
    window.$crisp.push(['set', 'user:email', info.email]);
    window.$crisp.push(['set', 'session:data', [[['Balance', info.balance / 100]]]]);
  }
}

function reportSubscribeToChat(data: SubscribeInfo) {
  if (!window.$crisp) return;
  // Matches moment(1e3 * expired_at).format('YYYY-MM-DD'); a null expiry becomes
  // the epoch date exactly as the original does (no '-' fallback here).
  const expireDate = new Date((data.expired_at ?? 0) * 1000);
  const pad = (value: number) => `${value}`.padStart(2, '0');
  const expireTime = `${expireDate.getFullYear()}-${pad(expireDate.getMonth() + 1)}-${pad(expireDate.getDate())}`;
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
  servers: ['user', 'servers'] as const,
  telegramBot: ['user', 'telegram', 'bot'] as const,
};

export async function fetchUserInfo() {
  const info = await user.info(apiClient);
  reportUserInfoToChat(info);
  return info;
}

async function fetchSubscribe() {
  const data = await user.getSubscribe(apiClient);
  reportSubscribeToChat(data);
  return data;
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
      queryFn: () => user.fetchKnowledge(apiClient, language, keyword),
    }),
  knowledgeDetail: (id: number | string | undefined, language: string) =>
    queryOptions({
      queryKey: userKeys.knowledgeDetail(id as number | string, language),
      queryFn: () => user.knowledgeDetail(apiClient, id as number | string, language),
    }),
  trafficLog: () =>
    queryOptions({ queryKey: userKeys.trafficLog, queryFn: () => user.getTrafficLog(apiClient) }),
  commConfig: () =>
    queryOptions({
      queryKey: userKeys.commConfig,
      queryFn: () => user.commConfig(apiClient),
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
    gcTime: 0,
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
    // Old dva state keeps the previous `invites` array while detailsLoading flips
    // to true for pagination requests; Table then blurs the existing rows.
    placeholderData: (previousData) => previousData,
  });

export const useKnowledge = (language: string, keyword?: string) =>
  useQuery({
    ...userQueryOptions.knowledge(language, keyword),
    // The original knowledge model has one `knowledges` field; a search request flips
    // fetchLoading but keeps the previous list until a 200 response replaces it.
    placeholderData: (previousData) => previousData,
  });

export const useKnowledgeDetail = (id: number | string | undefined, language: string) =>
  useQuery({
    ...userQueryOptions.knowledgeDetail(id, language),
    enabled: id !== undefined,
    // The legacy dva model has a single `knowledge` object and hide() clears it;
    // there is no per-article cache to fall back to on a later open/jump.
    gcTime: 0,
  });

export const useTrafficLog = () =>
  useQuery(userQueryOptions.trafficLog());

export const useCommConfig = (options?: QueryFreshnessOptions) =>
  useQuery({
    ...userQueryOptions.commConfig(),
    ...options,
  });

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

export const useInvalidateUser = () => {
  const queryClient = useQueryClient();
  return () => {
    queryClient.invalidateQueries({ queryKey: ['user'] });
  };
};

export function useChangePasswordMutation() {
  return useMutation({
    mutationFn: ({ oldPassword, newPassword }: { oldPassword: string; newPassword: string }) =>
      user.changePassword(apiClient, oldPassword, newPassword),
  });
}

export function useUpdateProfileMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.update>[1]) => user.update(apiClient, payload),
  });
}

export function useResetSubscribeMutation() {
  return useMutation({
    mutationFn: () => user.resetSecurity(apiClient),
  });
}

export function useTransferMutation() {
  return useMutation({
    mutationFn: (amount: number | string | undefined) => user.transfer(apiClient, amount),
  });
}

export function useRedeemGiftCardMutation() {
  return useMutation({
    mutationFn: (code: string) => user.redeemGiftCard(apiClient, code),
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

export function useStripePublicKeyMutation() {
  return useMutation({
    mutationFn: (methodId: number) => user.getStripePublicKey(apiClient, methodId),
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
    // The original cancel saga dispatches `fetch` (refresh the order LIST) then a
    // mistyped `details` action (the effect is named `detail`), so the order DETAIL is
    // intentionally never refreshed — it keeps rendering as pending. Match: invalidate
    // the list queries only, leaving the detail stale.
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: ['user', 'orders'],
        predicate: (query) => query.queryKey[2] !== 'detail',
      });
    },
  });
}

export function useNewPeriodMutation() {
  return useMutation({
    mutationFn: () => user.newPeriod(apiClient),
  });
}

export function useUnbindTelegramMutation() {
  return useMutation({
    mutationFn: () => user.unbindTelegram(apiClient),
  });
}
