import { admin } from '@v2board/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { apiClient } from './api';

export const adminKeys = {
  config: ['admin', 'config'] as const,
  stat: ['admin', 'stat'] as const,
  users: (filters: unknown) => ['admin', 'users', filters] as const,
  orders: (filters: unknown) => ['admin', 'orders', filters] as const,
  plans: ['admin', 'plans'] as const,
  payments: ['admin', 'payments'] as const,
  notices: (filters: unknown) => ['admin', 'notices', filters] as const,
  tickets: (filters: unknown) => ['admin', 'tickets', filters] as const,
  coupons: (filters: unknown) => ['admin', 'coupons', filters] as const,
  giftcards: ['admin', 'giftcards'] as const,
  knowledge: ['admin', 'knowledge'] as const,
  knowledgeCategories: ['admin', 'knowledge', 'categories'] as const,
  serverNodes: ['admin', 'servers', 'nodes'] as const,
  serverGroups: ['admin', 'servers', 'groups'] as const,
  serverRoutes: ['admin', 'servers', 'routes'] as const,
  systemStatus: ['admin', 'system', 'status'] as const,
  queue: ['admin', 'system', 'queue'] as const,
  systemLog: ['admin', 'system', 'log'] as const,
  statOrder: ['admin', 'stat', 'order'] as const,
  statUserToday: ['admin', 'stat', 'userToday'] as const,
  statServerToday: ['admin', 'stat', 'serverToday'] as const,
  paymentMethods: ['admin', 'payment', 'methods'] as const,
  themes: ['admin', 'themes'] as const,
};

export const useStat = () => useQuery({ queryKey: adminKeys.stat, queryFn: () => admin.statSummary(apiClient) });
export const useStatOrder = () =>
  useQuery({ queryKey: adminKeys.statOrder, queryFn: () => admin.statOrder(apiClient) });
export const useStatUserToday = () =>
  useQuery({ queryKey: adminKeys.statUserToday, queryFn: () => admin.statUserTodayRank(apiClient) });
export const useStatServerToday = () =>
  useQuery({
    queryKey: adminKeys.statServerToday,
    queryFn: () => admin.statServerTodayRank(apiClient),
  });

export const useConfig = () =>
  useQuery({ queryKey: adminKeys.config, queryFn: () => admin.fetchConfig(apiClient) });

export const useAdminPlans = () =>
  useQuery({ queryKey: adminKeys.plans, queryFn: () => admin.fetchPlans(apiClient) });

export const useAdminPayments = () =>
  useQuery({ queryKey: adminKeys.payments, queryFn: () => admin.fetchPayments(apiClient) });

export const usePaymentMethods = () =>
  useQuery({ queryKey: adminKeys.paymentMethods, queryFn: () => admin.paymentMethods(apiClient) });

export const useAdminUsers = (query: admin.AdminPageQuery) =>
  useQuery({ queryKey: adminKeys.users(query), queryFn: () => admin.fetchUsers(apiClient, query) });

export const useAdminOrders = (query: admin.AdminPageQuery & { is_commission?: 0 | 1 }) =>
  useQuery({
    queryKey: adminKeys.orders(query),
    queryFn: () => admin.fetchOrders(apiClient, query),
  });

export const useAdminNotices = (query: admin.AdminPageQuery) =>
  useQuery({
    queryKey: adminKeys.notices(query),
    queryFn: () => admin.fetchNotices(apiClient, query),
  });

export const useAdminTickets = (query: admin.AdminPageQuery) =>
  useQuery({
    queryKey: adminKeys.tickets(query),
    queryFn: () => admin.fetchTickets(apiClient, query),
  });

export const useAdminCoupons = (query: admin.AdminPageQuery) =>
  useQuery({
    queryKey: adminKeys.coupons(query),
    queryFn: () => admin.fetchCoupons(apiClient, query),
  });

export const useAdminGiftcards = () =>
  useQuery({ queryKey: adminKeys.giftcards, queryFn: () => admin.fetchGiftcards(apiClient) });

export const useAdminKnowledge = () =>
  useQuery({ queryKey: adminKeys.knowledge, queryFn: () => admin.fetchKnowledge(apiClient) });

export const useAdminKnowledgeCategories = () =>
  useQuery({
    queryKey: adminKeys.knowledgeCategories,
    queryFn: () => admin.knowledgeCategories(apiClient),
  });

export const useServerNodes = () =>
  useQuery({ queryKey: adminKeys.serverNodes, queryFn: () => admin.fetchServerNodes(apiClient) });

export const useServerGroups = () =>
  useQuery({ queryKey: adminKeys.serverGroups, queryFn: () => admin.fetchServerGroups(apiClient) });

export const useServerRoutes = () =>
  useQuery({ queryKey: adminKeys.serverRoutes, queryFn: () => admin.fetchServerRoutes(apiClient) });

export const useSystemStatus = () =>
  useQuery({ queryKey: adminKeys.systemStatus, queryFn: () => admin.systemStatus(apiClient) });

export const useQueueStats = () =>
  useQuery({ queryKey: adminKeys.queue, queryFn: () => admin.queueStats(apiClient) });

export const useSystemLog = () =>
  useQuery({ queryKey: adminKeys.systemLog, queryFn: () => admin.systemLog(apiClient) });

export const useThemes = () =>
  useQuery({ queryKey: adminKeys.themes, queryFn: () => admin.themes(apiClient) });

export function useSavePlanMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.savePlan>[1]) => admin.savePlan(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.plans }),
  });
}

export function useDropPlanMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropPlan(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.plans }),
  });
}

export function useUpdatePlanMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: number; show?: 0 | 1; renew?: 0 | 1 }) =>
      admin.updatePlan(apiClient, vars.id, vars.show, vars.renew),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.plans }),
  });
}

export function useSortPlansMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (ids: number[]) => admin.sortPlans(apiClient, ids),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.plans }),
  });
}

export function useUpdateUserMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.updateUser>[1]) =>
      admin.updateUser(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'users'] }),
  });
}

export function useDeleteUserMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.deleteUser(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'users'] }),
  });
}

export function useResetUserSecretMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.resetUserSecret(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'users'] }),
  });
}

export function useGenerateUserMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.generateUser>[1]) =>
      admin.generateUser(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'users'] }),
  });
}

export function useMarkOrderPaidMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tradeNo: string) => admin.paidOrder(apiClient, tradeNo),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }),
  });
}

export function useCancelOrderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (tradeNo: string) => admin.cancelOrder(apiClient, tradeNo),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }),
  });
}

export function useAssignOrderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.assignOrder>[1]) =>
      admin.assignOrder(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'orders'] }),
  });
}

export function useReplyTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.replyTicket>[1]) =>
      admin.replyTicket(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'tickets'] }),
  });
}

export function useCloseTicketMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.closeTicket(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'tickets'] }),
  });
}

export function useSaveNoticeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveNotice>[1]) =>
      admin.saveNotice(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] }),
  });
}

export function useUpdateNoticeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.updateNotice>[1]) =>
      admin.updateNotice(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] }),
  });
}

export function useDropNoticeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropNotice(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] }),
  });
}

export function useShowNoticeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: number; show: 0 | 1 }) =>
      admin.showNotice(apiClient, vars.id, vars.show),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'notices'] }),
  });
}

export function useSaveConfigMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveConfig>[1]) =>
      admin.saveConfig(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.config }),
  });
}

export function useSavePaymentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.savePayment>[1]) =>
      admin.savePayment(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.payments }),
  });
}

export function useShowPaymentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: number; enable: 0 | 1 }) =>
      admin.showPayment(apiClient, vars.id, vars.enable),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.payments }),
  });
}

export function useDropPaymentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropPayment(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.payments }),
  });
}

export function useGenerateCouponMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.generateCoupon>[1]) =>
      admin.generateCoupon(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'coupons'] }),
  });
}

export function useDropCouponMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropCoupon(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'coupons'] }),
  });
}

export function useShowCouponMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, show }: { id: number; show: 0 | 1 }) => admin.showCoupon(apiClient, id, show),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin', 'coupons'] }),
  });
}

export function useGenerateGiftcardMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.generateGiftcard>[1]) =>
      admin.generateGiftcard(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.giftcards }),
  });
}

export function useDropGiftcardMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropGiftcard(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.giftcards }),
  });
}

export function useSaveKnowledgeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveKnowledge>[1]) =>
      admin.saveKnowledge(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.knowledge }),
  });
}

export function useDropKnowledgeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropKnowledge(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.knowledge }),
  });
}

export function useShowKnowledgeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { id: number; show: 0 | 1 }) =>
      admin.showKnowledge(apiClient, vars.id, vars.show),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.knowledge }),
  });
}

export function useSaveServerGroupMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveServerGroup>[1]) =>
      admin.saveServerGroup(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverGroups }),
  });
}

export function useDropServerGroupMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropServerGroup(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverGroups }),
  });
}

export function useSaveServerRouteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveServerRoute>[1]) =>
      admin.saveServerRoute(apiClient, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverRoutes }),
  });
}

export function useDropServerRouteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) => admin.dropServerRoute(apiClient, id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverRoutes }),
  });
}

export function useDropServerMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { type: admin.ServerTypeName; id: number }) =>
      admin.dropServer(apiClient, vars.type, vars.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverNodes }),
  });
}

export function useCopyServerMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { type: admin.ServerTypeName; id: number }) =>
      admin.copyServer(apiClient, vars.type, vars.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverNodes }),
  });
}

export function useUpdateServerMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (vars: { type: admin.ServerTypeName; id: number; show: 0 | 1 }) =>
      admin.updateServer(apiClient, vars.type, vars.id, vars.show),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: adminKeys.serverNodes }),
  });
}
