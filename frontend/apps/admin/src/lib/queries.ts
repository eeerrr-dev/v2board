import { admin } from '@v2board/api-client';
import { useMutation, useQuery } from '@tanstack/react-query';
import { apiClient } from './api';

export const adminKeys = {
  config: (key?: string) => ['admin', 'config', key] as const,
  stat: ['admin', 'stat'] as const,
  users: (filters: unknown) => ['admin', 'users', filters] as const,
  orders: (filters: unknown) => ['admin', 'orders', filters] as const,
  plans: ['admin', 'plans'] as const,
  payments: ['admin', 'payments'] as const,
  notices: (filters: unknown) => ['admin', 'notices', filters] as const,
  tickets: (filters: unknown) => ['admin', 'tickets', filters] as const,
  coupons: (filters: unknown) => ['admin', 'coupons', filters] as const,
  giftcards: (filters: unknown) => ['admin', 'giftcards', filters] as const,
  knowledge: ['admin', 'knowledge'] as const,
  knowledgeCategories: ['admin', 'knowledge', 'categories'] as const,
  serverNodes: ['admin', 'servers', 'nodes'] as const,
  serverGroups: ['admin', 'servers', 'groups'] as const,
  serverRoutes: ['admin', 'servers', 'routes'] as const,
  queue: ['admin', 'system', 'queue'] as const,
  queueWorkload: ['admin', 'system', 'queueWorkload'] as const,
  statOrder: ['admin', 'stat', 'order'] as const,
  statUserToday: ['admin', 'stat', 'userToday'] as const,
  statUserLast: ['admin', 'stat', 'userLast'] as const,
  statServerToday: ['admin', 'stat', 'serverToday'] as const,
  statServerLast: ['admin', 'stat', 'serverLast'] as const,
  statUserTraffic: (userId: number | undefined, query: unknown) =>
    ['admin', 'stat', 'userTraffic', userId, query] as const,
  paymentMethods: ['admin', 'payment', 'methods'] as const,
  themes: ['admin', 'themes'] as const,
  emailTemplates: ['admin', 'config', 'emailTemplates'] as const,
  themeTemplates: ['admin', 'config', 'themeTemplates'] as const,
};

export const useStat = () =>
  useQuery({
    queryKey: adminKeys.stat,
    queryFn: () => admin.statSummary(apiClient),
  });
// Dashboard chart effects in the bundled admin app render through one-shot
// completion callbacks; unlike `stat/getOverride`, chart payloads are not stored
// in dva state after the page unmounts.
const legacyDashboardChartQueryOptions = { gcTime: 0 } as const;

export const useStatOrder = () =>
  useQuery({
    queryKey: adminKeys.statOrder,
    queryFn: () => admin.statOrder(apiClient),
    ...legacyDashboardChartQueryOptions,
  });
export const useStatUserToday = () =>
  useQuery({
    queryKey: adminKeys.statUserToday,
    queryFn: () => admin.statUserTodayRank(apiClient),
    ...legacyDashboardChartQueryOptions,
  });
export const useStatUserLast = () =>
  useQuery({
    queryKey: adminKeys.statUserLast,
    queryFn: () => admin.statUserLastRank(apiClient),
    ...legacyDashboardChartQueryOptions,
  });
export const useAdminUserTraffic = (
  userId: number | undefined,
  query: Omit<admin.AdminUserTrafficQuery, 'user_id'>,
  enabled: boolean,
) =>
  useQuery({
    queryKey: adminKeys.statUserTraffic(userId, query),
    queryFn: () => admin.statUser(apiClient, { user_id: userId as number, ...query }),
    enabled: enabled && userId != null,
  });
export const useStatServerToday = () =>
  useQuery({
    queryKey: adminKeys.statServerToday,
    queryFn: () => admin.statServerTodayRank(apiClient),
    ...legacyDashboardChartQueryOptions,
  });
export const useStatServerLast = () =>
  useQuery({
    queryKey: adminKeys.statServerLast,
    queryFn: () => admin.statServerLastRank(apiClient),
    ...legacyDashboardChartQueryOptions,
  });

export const useConfig = (key?: string) =>
  useQuery({ queryKey: adminKeys.config(key), queryFn: () => admin.fetchConfig(apiClient, key) });

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

export const useAdminOrderDetail = (id?: number) =>
  useQuery({
    queryKey: ['admin', 'order', id],
    queryFn: () => admin.orderDetail(apiClient, id as number),
    enabled: id != null,
  });

export const useAdminUserInfo = (id?: number | null) =>
  useQuery({
    queryKey: ['admin', 'user', id],
    queryFn: () => admin.getUserInfoById(apiClient, id as number),
    enabled: id != null,
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

export const useAdminTicket = (id?: number | string) =>
  useQuery({
    queryKey: ['admin', 'ticket', id],
    queryFn: () => admin.ticketDetail(apiClient, id as number | string),
    enabled: id != null,
  });

export const useAdminCoupons = (query: admin.AdminPageQuery) =>
  useQuery({
    queryKey: adminKeys.coupons(query),
    queryFn: () => admin.fetchCoupons(apiClient, query),
  });

export const useAdminGiftcards = (query: admin.AdminPageQuery) =>
  useQuery({
    queryKey: adminKeys.giftcards(query),
    queryFn: () => admin.fetchGiftcards(apiClient, query),
  });

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

export const useQueueStats = () =>
  useQuery({
    queryKey: adminKeys.queue,
    queryFn: () => admin.queueStats(apiClient),
    enabled: false,
  });

export const useQueueWorkload = () =>
  useQuery({
    queryKey: adminKeys.queueWorkload,
    queryFn: () => admin.queueWorkload(apiClient),
    enabled: false,
  });

export const useThemes = () =>
  useQuery({ queryKey: adminKeys.themes, queryFn: () => admin.themes(apiClient) });

export const useEmailTemplates = () =>
  useQuery({
    queryKey: adminKeys.emailTemplates,
    queryFn: () => admin.getEmailTemplate(apiClient),
  });

export const useThemeTemplates = () =>
  useQuery({
    queryKey: adminKeys.themeTemplates,
    queryFn: () => admin.getThemeTemplate(apiClient),
  });

export function useThemeConfigMutation() {
  return useMutation({
    mutationFn: (name: string) => admin.themeConfig(apiClient, name),
  });
}

export function useSaveThemeConfigMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveThemeConfig>[1]) =>
      admin.saveThemeConfig(apiClient, data),
  });
}

export function useSetTelegramWebhookMutation() {
  return useMutation({
    mutationFn: () => admin.setTelegramWebhook(apiClient),
  });
}

export function useTestSendMailMutation() {
  return useMutation({
    mutationFn: () => admin.testSendMail(apiClient),
  });
}

export function useSavePlanMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.savePlan>[1]) => admin.savePlan(apiClient, data),
  });
}

export function useDropPlanMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropPlan(apiClient, id),
  });
}

export function useUpdatePlanMutation() {
  return useMutation({
    mutationFn: (vars: { id: number; key: 'show' | 'renew'; value: 0 | 1 }) =>
      admin.updatePlan(apiClient, vars.id, vars.key, vars.value),
  });
}

export function useSortPlansMutation() {
  return useMutation({
    mutationFn: (ids: number[]) => admin.sortPlans(apiClient, ids),
  });
}

export function useUpdateUserMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.updateUser>[1]) => admin.updateUser(apiClient, data),
  });
}

export function useDeleteUserMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.deleteUser(apiClient, id),
  });
}

export function useResetUserSecretMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.resetUserSecret(apiClient, id),
  });
}

export function useGenerateUserMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.generateUser>[1]) =>
      admin.generateUser(apiClient, data),
  });
}

export function useDumpUsersCsvMutation() {
  return useMutation({
    mutationFn: (filter?: admin.AdminFilter[]) => admin.dumpUsersCsv(apiClient, filter),
  });
}

export function useSendMailToUsersMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.sendMailToUsers>[1]) =>
      admin.sendMailToUsers(apiClient, data),
  });
}

export function useBanUsersMutation() {
  return useMutation({
    mutationFn: (filter?: admin.AdminFilter[]) => admin.banUsers(apiClient, filter),
  });
}

export function useDeleteAllUsersMutation() {
  return useMutation({
    mutationFn: (filter?: admin.AdminFilter[]) => admin.deleteAllUsers(apiClient, filter),
  });
}

export function useMarkOrderPaidMutation() {
  return useMutation({
    mutationFn: (tradeNo: string) => admin.paidOrder(apiClient, tradeNo),
  });
}

export function useCancelOrderMutation() {
  return useMutation({
    mutationFn: (tradeNo: string) => admin.cancelOrder(apiClient, tradeNo),
  });
}

export function useUpdateOrderMutation() {
  return useMutation({
    mutationFn: (vars: {
      tradeNo: string;
      key: 'commission_status' | 'status';
      value: string | number;
    }) => admin.updateOrder(apiClient, vars.tradeNo, vars.key, vars.value),
  });
}

export function useAssignOrderMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.assignOrder>[1]) =>
      admin.assignOrder(apiClient, data),
  });
}

export function useReplyTicketMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.replyTicket>[1]) =>
      admin.replyTicket(apiClient, data),
  });
}

export function useCloseTicketMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.closeTicket(apiClient, id),
  });
}

export function useSaveNoticeMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveNotice>[1]) => admin.saveNotice(apiClient, data),
  });
}

export function useDropNoticeMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropNotice(apiClient, id),
  });
}

export function useShowNoticeMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.showNotice(apiClient, id),
  });
}

export function useSaveConfigMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveConfig>[1]) => admin.saveConfig(apiClient, data),
  });
}

export function useSavePaymentMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.savePayment>[1]) =>
      admin.savePayment(apiClient, data),
  });
}

export function useShowPaymentMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.showPayment(apiClient, id),
  });
}

export function useSortPaymentMutation() {
  return useMutation({
    mutationFn: (ids: number[]) => admin.sortPayments(apiClient, ids),
  });
}

export function useDropPaymentMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropPayment(apiClient, id),
  });
}

export function useGenerateCouponMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.generateCoupon>[1]) =>
      admin.generateCoupon(apiClient, data),
  });
}

export function useDropCouponMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropCoupon(apiClient, id),
  });
}

export function useShowCouponMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.showCoupon(apiClient, id),
  });
}

export function useGenerateGiftcardMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.generateGiftcard>[1]) =>
      admin.generateGiftcard(apiClient, data),
  });
}

export function useDropGiftcardMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropGiftcard(apiClient, id),
  });
}

export function useSaveKnowledgeMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveKnowledge>[1]) =>
      admin.saveKnowledge(apiClient, data),
  });
}

export function useDropKnowledgeMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropKnowledge(apiClient, id),
  });
}

export function useShowKnowledgeMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.showKnowledge(apiClient, id),
  });
}

export function useSortKnowledgeMutation() {
  return useMutation({
    mutationFn: (ids: number[]) => admin.sortKnowledge(apiClient, ids),
  });
}

export function useSaveServerGroupMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveServerGroup>[1]) =>
      admin.saveServerGroup(apiClient, data),
  });
}

export function useDropServerGroupMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropServerGroup(apiClient, id),
  });
}

export function useSaveServerRouteMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveServerRoute>[1]) =>
      admin.saveServerRoute(apiClient, data),
  });
}

export function useDropServerRouteMutation() {
  return useMutation({
    mutationFn: (id: number) => admin.dropServerRoute(apiClient, id),
  });
}

export function useDropServerMutation() {
  return useMutation({
    mutationFn: (vars: { type: admin.ServerTypeName; id: number }) =>
      admin.dropServer(apiClient, vars.type, vars.id),
  });
}

export function useCopyServerMutation() {
  return useMutation({
    mutationFn: (vars: { type: admin.ServerTypeName; id: number }) =>
      admin.copyServer(apiClient, vars.type, vars.id),
  });
}

export function useUpdateServerMutation() {
  return useMutation({
    mutationFn: (vars: { type: admin.ServerTypeName; id: number; key: 'show'; value: 0 | 1 }) =>
      admin.updateServer(apiClient, vars.type, vars.id, vars.key, vars.value),
  });
}

export function useSortServerNodesMutation() {
  return useMutation({
    mutationFn: (payload: Parameters<typeof admin.sortServerNodes>[1]) =>
      admin.sortServerNodes(apiClient, payload),
  });
}
