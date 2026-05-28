import { user } from '@v2board/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiClient } from './api';

interface QueryFreshnessOptions {
  refetchOnMount?: boolean | 'always';
}

export const userKeys = {
  info: ['user', 'info'] as const,
  stat: ['user', 'stat'] as const,
  subscribe: ['user', 'subscribe'] as const,
  orders: (status?: number) => ['user', 'orders', status ?? 'all'] as const,
  orderDetail: (tradeNo: string) => ['user', 'orders', 'detail', tradeNo] as const,
  plans: ['user', 'plans'] as const,
  plan: (id: number) => ['user', 'plan', id] as const,
  payments: ['user', 'payments'] as const,
  notices: ['user', 'notices'] as const,
  tickets: ['user', 'tickets'] as const,
  ticketDetail: (id: number) => ['user', 'ticket', id] as const,
  invite: ['user', 'invite'] as const,
  inviteDetails: (current: number, size: number) =>
    ['user', 'invite', 'details', current, size] as const,
  knowledge: (lang: string, kw?: string) => ['user', 'knowledge', lang, kw ?? ''] as const,
  knowledgeDetail: (id: number, lang: string) => ['user', 'knowledge', 'detail', id, lang] as const,
  trafficLog: ['user', 'trafficLog'] as const,
  commConfig: ['user', 'comm'] as const,
  sessions: ['user', 'sessions'] as const,
  servers: ['user', 'servers'] as const,
  telegramBot: ['user', 'telegram', 'bot'] as const,
};

export const useUserInfo = (options?: QueryFreshnessOptions) =>
  useQuery({ queryKey: userKeys.info, queryFn: () => user.info(apiClient), ...options });

export const useUserStat = () =>
  useQuery({ queryKey: userKeys.stat, queryFn: () => user.getStat(apiClient) });

export const useSubscribe = (options?: QueryFreshnessOptions) =>
  useQuery({
    queryKey: userKeys.subscribe,
    queryFn: () => user.getSubscribe(apiClient),
    ...options,
  });

export const useOrders = (status?: number) =>
  useQuery({ queryKey: userKeys.orders(status), queryFn: () => user.fetchOrders(apiClient, status) });

export const useOrder = (tradeNo: string | undefined) =>
  useQuery({
    queryKey: userKeys.orderDetail(tradeNo ?? ''),
    queryFn: () => user.orderDetail(apiClient, tradeNo as string),
    enabled: Boolean(tradeNo),
  });

export const usePlans = () =>
  useQuery({ queryKey: userKeys.plans, queryFn: () => user.fetchPlans(apiClient) });

export const usePlan = (id: number | undefined) =>
  useQuery({
    queryKey: userKeys.plan(id ?? 0),
    queryFn: () => user.fetchPlan(apiClient, id as number),
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

export const useTicket = (id: number | undefined) =>
  useQuery({
    queryKey: userKeys.ticketDetail(id ?? 0),
    queryFn: () => user.ticketDetail(apiClient, id as number),
    enabled: Boolean(id),
  });

export const useInvite = () =>
  useQuery({ queryKey: userKeys.invite, queryFn: () => user.fetchInvite(apiClient) });

export const useInviteDetails = (current: number, pageSize: number) =>
  useQuery({
    queryKey: userKeys.inviteDetails(current, pageSize),
    queryFn: () => user.inviteDetails(apiClient, current, pageSize),
  });

export const useKnowledge = (language: string, keyword?: string) =>
  useQuery({
    queryKey: userKeys.knowledge(language, keyword),
    queryFn: () => user.fetchKnowledge(apiClient, language, keyword),
  });

export const useKnowledgeDetail = (id: number | undefined, language: string) =>
  useQuery({
    queryKey: userKeys.knowledgeDetail(id ?? 0, language),
    queryFn: () => user.knowledgeDetail(apiClient, id as number, language),
    enabled: Boolean(id),
  });

export const useTrafficLog = () =>
  useQuery({ queryKey: userKeys.trafficLog, queryFn: () => user.getTrafficLog(apiClient) });

export const useCommConfig = (options?: QueryFreshnessOptions) =>
  useQuery({
    queryKey: userKeys.commConfig,
    queryFn: () => user.commConfig(apiClient),
    ...options,
  });

export const useActiveSessions = () =>
  useQuery({ queryKey: userKeys.sessions, queryFn: () => user.getActiveSession(apiClient) });

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
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.update>[1]) => user.update(apiClient, payload),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.info }),
  });
}

export function useResetSubscribeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.resetSecurity(apiClient),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.subscribe }),
  });
}

export function useTransferMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (amount: number) => user.transfer(apiClient, amount),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.info }),
  });
}

export function useRedeemGiftCardMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => user.redeemGiftCard(apiClient, code),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: userKeys.info });
      queryClient.invalidateQueries({ queryKey: userKeys.subscribe });
    },
  });
}

export function useGenerateInviteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.generateInvite(apiClient),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.invite }),
  });
}

export function useWithdrawCommissionMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.withdrawTicket>[1]) =>
      user.withdrawTicket(apiClient, payload),
  });
}

export function useSaveTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.saveTicket>[1]) =>
      user.saveTicket(apiClient, payload),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.tickets }),
  });
}

export function useReplyTicketMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof user.replyTicket>[1]) =>
      user.replyTicket(apiClient, payload),
  });
}

export function useCloseTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => user.closeTicket(apiClient, id),
    onSuccess: (_, id) => {
      queryClient.invalidateQueries({ queryKey: userKeys.ticketDetail(id) });
      queryClient.invalidateQueries({ queryKey: userKeys.tickets });
    },
  });
}

export function useCancelOrderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tradeNo: string) => user.cancelOrder(apiClient, tradeNo),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['user', 'orders'] }),
  });
}

export function useNewPeriodMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.newPeriod(apiClient),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: userKeys.info });
      queryClient.invalidateQueries({ queryKey: userKeys.subscribe });
    },
  });
}

export function useUnbindTelegramMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => user.unbindTelegram(apiClient),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.info }),
  });
}

export function useRemoveSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => user.removeActiveSession(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: userKeys.sessions }),
  });
}
