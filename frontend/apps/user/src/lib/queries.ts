import { user } from '@v2board/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { SubscribeInfo, UserInfo } from '@v2board/types';
import { formatBytes } from '@v2board/config/format';
import { apiClient } from './api';

interface QueryFreshnessOptions {
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

export const useUserInfo = (options?: QueryFreshnessOptions) =>
  useQuery({
    queryKey: userKeys.info,
    queryFn: fetchUserInfo,
    ...options,
  });

export const useUserStat = () =>
  useQuery({ queryKey: userKeys.stat, queryFn: () => user.getStat(apiClient) });

export const useSubscribe = (options?: QueryFreshnessOptions & { enabled?: boolean }) =>
  useQuery({
    queryKey: userKeys.subscribe,
    queryFn: async () => {
      const data = await user.getSubscribe(apiClient);
      reportSubscribeToChat(data);
      return data;
    },
    ...options,
  });

export const useOrders = (status?: number) =>
  useQuery({ queryKey: userKeys.orders(status), queryFn: () => user.fetchOrders(apiClient, status) });

export const useOrder = (tradeNo: string | undefined) =>
  useQuery({
    queryKey: userKeys.orderDetail(tradeNo as string),
    queryFn: () => user.orderDetail(apiClient, tradeNo as string),
    enabled: Boolean(tradeNo),
  });

export const usePlans = () =>
  useQuery({ queryKey: userKeys.plans, queryFn: () => user.fetchPlans(apiClient) });

export const usePlan = (id: number | string | undefined) =>
  useQuery({
    queryKey: userKeys.plan(id as number | string),
    queryFn: () => user.fetchPlan(apiClient, id as number | string),
    enabled: Boolean(id),
  });

export const usePaymentMethods = (options?: QueryFreshnessOptions & { enabled?: boolean }) =>
  useQuery({
    queryKey: userKeys.payments,
    queryFn: () => user.getPaymentMethod(apiClient),
    ...options,
  });

export const useNotices = () =>
  useQuery({
    queryKey: userKeys.notices,
    queryFn: () => user.fetchNotices(apiClient),
  });

export const useTickets = () =>
  useQuery({ queryKey: userKeys.tickets, queryFn: () => user.fetchTickets(apiClient) });

export const useTicket = (id: number | string | undefined) =>
  useQuery({
    queryKey: userKeys.ticketDetail(id as number | string),
    queryFn: () => user.ticketDetail(apiClient, id as number | string),
    enabled: Boolean(id),
  });

export const useInvite = () =>
  useQuery({ queryKey: userKeys.invite, queryFn: () => user.fetchInvite(apiClient) });

export const useInviteDetails = (current?: number, pageSize?: number) =>
  useQuery({
    queryKey: userKeys.inviteDetails(current, pageSize),
    queryFn: () => user.inviteDetails(apiClient, current, pageSize),
    // Old dva state keeps the previous `invites` array while detailsLoading flips
    // to true for pagination requests; Table then blurs the existing rows.
    placeholderData: (previousData) => previousData,
  });

export const useKnowledge = (language: string, keyword?: string) =>
  useQuery({
    queryKey: userKeys.knowledge(language, keyword),
    queryFn: () => user.fetchKnowledge(apiClient, language, keyword),
    // The original knowledge model has one `knowledges` field; a search request flips
    // fetchLoading but keeps the previous list until a 200 response replaces it.
    placeholderData: (previousData) => previousData,
  });

export const useKnowledgeDetail = (id: number | string | undefined, language: string) =>
  useQuery({
    queryKey: userKeys.knowledgeDetail(id as number | string, language),
    queryFn: () => user.knowledgeDetail(apiClient, id as number | string, language),
    enabled: id !== undefined,
    // The legacy dva model has a single `knowledge` object and hide() clears it;
    // there is no per-article cache to fall back to on a later open/jump.
    gcTime: 0,
  });

export const useTrafficLog = () =>
  useQuery({ queryKey: userKeys.trafficLog, queryFn: () => user.getTrafficLog(apiClient) });

export const useCommConfig = (options?: QueryFreshnessOptions) =>
  useQuery({
    queryKey: userKeys.commConfig,
    queryFn: () => user.commConfig(apiClient),
    ...options,
  });

export const useServers = (options?: QueryFreshnessOptions) =>
  useQuery({
    queryKey: userKeys.servers,
    queryFn: () => user.fetchServers(apiClient),
    ...options,
  });

export const useTelegramBotInfo = (enabled: boolean) =>
  useQuery({
    queryKey: userKeys.telegramBot,
    queryFn: () => user.getTelegramBotInfo(apiClient),
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
