import { admin, INLINE_MUTATION_ERROR_META } from '@v2board/api-client';
import {
  keepPreviousData,
  queryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { apiClient } from './api';

export const adminKeys = {
  config: (key?: string) => ['admin', 'config', key] as const,
  stat: ['admin', 'stat'] as const,
  users: (filters: unknown) => ['admin', 'users', filters] as const,
  user: (id: number | null | undefined) => ['admin', 'user', id] as const,
  orders: (filters: unknown) => ['admin', 'orders', filters] as const,
  order: (id: number | undefined) => ['admin', 'order', id] as const,
  plans: ['admin', 'plans'] as const,
  payments: ['admin', 'payments'] as const,
  notices: (filters: unknown) => ['admin', 'notices', filters] as const,
  tickets: (filters: unknown) => ['admin', 'tickets', filters] as const,
  ticket: (id: number | string | undefined) => ['admin', 'ticket', id] as const,
  coupons: (filters: unknown) => ['admin', 'coupons', filters] as const,
  giftcards: (filters: unknown) => ['admin', 'giftcards', filters] as const,
  knowledge: ['admin', 'knowledge'] as const,
  knowledgeDetail: (id: number | undefined) => ['admin', 'knowledge', 'detail', id] as const,
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
  paymentForm: (payment: string | undefined, id?: number) =>
    ['admin', 'payment', 'form', payment, id] as const,
  emailTemplates: ['admin', 'config', 'emailTemplates'] as const,
};

export const adminQueryOptions = {
  stat: () =>
    queryOptions({
      queryKey: adminKeys.stat,
      queryFn: ({ signal }) => admin.statSummary(apiClient, { signal }),
      staleTime: 30_000,
    }),
  statOrder: () =>
    queryOptions({
      queryKey: adminKeys.statOrder,
      queryFn: ({ signal }) => admin.statOrder(apiClient, { signal }),
      staleTime: 30_000,
    }),
  statUserToday: () =>
    queryOptions({
      queryKey: adminKeys.statUserToday,
      queryFn: ({ signal }) => admin.statUserTodayRank(apiClient, { signal }),
      staleTime: 30_000,
    }),
  statUserLast: () =>
    queryOptions({
      queryKey: adminKeys.statUserLast,
      queryFn: ({ signal }) => admin.statUserLastRank(apiClient, { signal }),
      staleTime: 30_000,
    }),
  statServerToday: () =>
    queryOptions({
      queryKey: adminKeys.statServerToday,
      queryFn: ({ signal }) => admin.statServerTodayRank(apiClient, { signal }),
      staleTime: 30_000,
    }),
  statServerLast: () =>
    queryOptions({
      queryKey: adminKeys.statServerLast,
      queryFn: ({ signal }) => admin.statServerLastRank(apiClient, { signal }),
      staleTime: 30_000,
    }),
  userTraffic: (userId: number | undefined, query: Omit<admin.AdminUserTrafficQuery, 'user_id'>) =>
    queryOptions({
      queryKey: adminKeys.statUserTraffic(userId, query),
      queryFn: ({ signal }) => {
        if (userId == null) throw new Error('User id is required');
        return admin.statUser(apiClient, { user_id: userId, ...query }, { signal });
      },
      placeholderData: keepPreviousData,
    }),
  config: (key?: string) =>
    queryOptions({
      queryKey: adminKeys.config(key),
      queryFn: ({ signal }) => admin.fetchConfig(apiClient, key, { signal }),
      staleTime: 30_000,
    }),
  plans: () =>
    queryOptions({
      queryKey: adminKeys.plans,
      queryFn: ({ signal }) => admin.fetchPlans(apiClient, { signal }),
      staleTime: 30_000,
    }),
  payments: () =>
    queryOptions({
      queryKey: adminKeys.payments,
      queryFn: ({ signal }) => admin.fetchPayments(apiClient, { signal }),
    }),
  paymentMethods: () =>
    queryOptions({
      queryKey: adminKeys.paymentMethods,
      queryFn: ({ signal }) => admin.paymentMethods(apiClient, { signal }),
      staleTime: 5 * 60_000,
    }),
  paymentForm: (payment: string | undefined, id?: number) =>
    queryOptions({
      queryKey: adminKeys.paymentForm(payment, id),
      queryFn: ({ signal }) => {
        if (!payment) throw new Error('Payment method is required');
        return admin.paymentForm(apiClient, payment, id, { signal });
      },
      staleTime: 5 * 60_000,
    }),
  users: (query: admin.AdminPageQuery) =>
    queryOptions({
      queryKey: adminKeys.users(query),
      queryFn: ({ signal }) => admin.fetchUsers(apiClient, query, { signal }),
      placeholderData: keepPreviousData,
    }),
  orders: (query: admin.AdminPageQuery & { is_commission?: 0 | 1 }) =>
    queryOptions({
      queryKey: adminKeys.orders(query),
      queryFn: ({ signal }) => admin.fetchOrders(apiClient, query, { signal }),
      placeholderData: keepPreviousData,
    }),
  order: (id: number | undefined) =>
    queryOptions({
      queryKey: adminKeys.order(id),
      queryFn: ({ signal }) => {
        if (id == null) throw new Error('Order id is required');
        return admin.orderDetail(apiClient, id, { signal });
      },
    }),
  user: (id: number | null | undefined) =>
    queryOptions({
      queryKey: adminKeys.user(id),
      queryFn: ({ signal }) => {
        if (id == null) throw new Error('User id is required');
        return admin.getUserInfoById(apiClient, id, { signal });
      },
    }),
  notices: (query: admin.AdminPageQuery) =>
    queryOptions({
      queryKey: adminKeys.notices(query),
      queryFn: ({ signal }) => admin.fetchNotices(apiClient, query, { signal }),
      placeholderData: keepPreviousData,
    }),
  tickets: (query: admin.AdminPageQuery) =>
    queryOptions({
      queryKey: adminKeys.tickets(query),
      queryFn: ({ signal }) => admin.fetchTickets(apiClient, query, { signal }),
      placeholderData: keepPreviousData,
    }),
  ticket: (id: number | string | undefined) =>
    queryOptions({
      queryKey: adminKeys.ticket(id),
      queryFn: ({ signal }) => {
        if (id == null) throw new Error('Ticket id is required');
        return admin.ticketDetail(apiClient, id, { signal });
      },
      refetchInterval: 5_000,
    }),
  coupons: (query: admin.AdminPageQuery) =>
    queryOptions({
      queryKey: adminKeys.coupons(query),
      queryFn: ({ signal }) => admin.fetchCoupons(apiClient, query, { signal }),
      placeholderData: keepPreviousData,
    }),
  giftcards: (query: admin.AdminPageQuery) =>
    queryOptions({
      queryKey: adminKeys.giftcards(query),
      queryFn: ({ signal }) => admin.fetchGiftcards(apiClient, query, { signal }),
      placeholderData: keepPreviousData,
    }),
  knowledge: () =>
    queryOptions({
      queryKey: adminKeys.knowledge,
      queryFn: ({ signal }) => admin.fetchKnowledge(apiClient, { signal }),
    }),
  knowledgeDetail: (id: number | undefined) =>
    queryOptions({
      queryKey: adminKeys.knowledgeDetail(id),
      queryFn: ({ signal }) => {
        if (id == null) throw new Error('Knowledge id is required');
        return admin.knowledgeDetail(apiClient, id, { signal });
      },
    }),
  knowledgeCategories: () =>
    queryOptions({
      queryKey: adminKeys.knowledgeCategories,
      queryFn: ({ signal }) => admin.knowledgeCategories(apiClient, { signal }),
      staleTime: 5 * 60_000,
    }),
  serverNodes: () =>
    queryOptions({
      queryKey: adminKeys.serverNodes,
      queryFn: ({ signal }) => admin.fetchServerNodes(apiClient, { signal }),
    }),
  serverGroups: () =>
    queryOptions({
      queryKey: adminKeys.serverGroups,
      queryFn: ({ signal }) => admin.fetchServerGroups(apiClient, { signal }),
    }),
  serverRoutes: () =>
    queryOptions({
      queryKey: adminKeys.serverRoutes,
      queryFn: ({ signal }) => admin.fetchServerRoutes(apiClient, { signal }),
    }),
  queueStats: () =>
    queryOptions({
      queryKey: adminKeys.queue,
      queryFn: ({ signal }) => admin.queueStats(apiClient, { signal }),
      refetchInterval: 3_000,
    }),
  queueWorkload: () =>
    queryOptions({
      queryKey: adminKeys.queueWorkload,
      queryFn: ({ signal }) => admin.queueWorkload(apiClient, { signal }),
      refetchInterval: 3_000,
    }),
  emailTemplates: () =>
    queryOptions({
      queryKey: adminKeys.emailTemplates,
      queryFn: ({ signal }) => admin.getEmailTemplate(apiClient, { signal }),
      staleTime: 5 * 60_000,
    }),
};

export const useStat = () => useQuery(adminQueryOptions.stat());
export const useStatOrder = () => useQuery(adminQueryOptions.statOrder());
export const useStatUserToday = () => useQuery(adminQueryOptions.statUserToday());
export const useStatUserLast = () => useQuery(adminQueryOptions.statUserLast());
export const useStatServerToday = () => useQuery(adminQueryOptions.statServerToday());
export const useStatServerLast = () => useQuery(adminQueryOptions.statServerLast());
export const useAdminUserTraffic = (
  userId: number | undefined,
  query: Omit<admin.AdminUserTrafficQuery, 'user_id'>,
  enabled: boolean,
) =>
  useQuery({ ...adminQueryOptions.userTraffic(userId, query), enabled: enabled && userId != null });
export const useConfig = (key?: string) => useQuery(adminQueryOptions.config(key));
export const useAdminPlans = () => useQuery(adminQueryOptions.plans());
export const useAdminPayments = () => useQuery(adminQueryOptions.payments());
export const usePaymentMethods = (enabled = true) =>
  useQuery({ ...adminQueryOptions.paymentMethods(), enabled });
export const usePaymentForm = (
  payment: string | undefined,
  id: number | undefined,
  enabled: boolean,
) =>
  useQuery({
    ...adminQueryOptions.paymentForm(payment, id),
    enabled: enabled && Boolean(payment),
  });
export const useAdminUsers = (query: admin.AdminPageQuery) =>
  useQuery(adminQueryOptions.users(query));
export const useAdminOrders = (query: admin.AdminPageQuery & { is_commission?: 0 | 1 }) =>
  useQuery(adminQueryOptions.orders(query));
export const useAdminOrderDetail = (id?: number) =>
  useQuery({ ...adminQueryOptions.order(id), enabled: id != null });
export const useAdminUserInfo = (id?: number | null) =>
  useQuery({ ...adminQueryOptions.user(id), enabled: id != null });
export const useAdminNotices = (query: admin.AdminPageQuery) =>
  useQuery(adminQueryOptions.notices(query));
export const useAdminTickets = (query: admin.AdminPageQuery) =>
  useQuery(adminQueryOptions.tickets(query));
export const useAdminTicket = (id?: number | string) =>
  useQuery({ ...adminQueryOptions.ticket(id), enabled: id != null });
export const useAdminCoupons = (query: admin.AdminPageQuery) =>
  useQuery(adminQueryOptions.coupons(query));
export const useAdminGiftcards = (query: admin.AdminPageQuery) =>
  useQuery(adminQueryOptions.giftcards(query));
export const useAdminKnowledge = () => useQuery(adminQueryOptions.knowledge());
export const useAdminKnowledgeDetail = (id: number | undefined, open: boolean) =>
  useQuery({
    ...adminQueryOptions.knowledgeDetail(id),
    enabled: open && id != null,
  });
export const useAdminKnowledgeCategories = () => useQuery(adminQueryOptions.knowledgeCategories());
export const useServerNodes = () => useQuery(adminQueryOptions.serverNodes());
export const useServerGroups = () => useQuery(adminQueryOptions.serverGroups());
export const useServerRoutes = () => useQuery(adminQueryOptions.serverRoutes());
export const useQueueStats = () => useQuery(adminQueryOptions.queueStats());
export const useQueueWorkload = () => useQuery(adminQueryOptions.queueWorkload());
export const useEmailTemplates = () => useQuery(adminQueryOptions.emailTemplates());

function useInvalidatingMutation<TVariables, TData>(
  mutationFn: (variables: TVariables) => Promise<TData>,
  invalidate: readonly (readonly unknown[])[],
  meta?: Record<string, unknown>,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn,
    meta,
    onSuccess: async () => {
      await Promise.all(invalidate.map((queryKey) => queryClient.invalidateQueries({ queryKey })));
    },
  });
}

export function useSetTelegramWebhookMutation() {
  return useMutation({
    mutationFn: (telegramBotToken: string) => admin.setTelegramWebhook(apiClient, telegramBotToken),
  });
}

export function useTestSendMailMutation() {
  return useMutation({
    mutationFn: () => admin.testSendMail(apiClient),
  });
}

export function useSavePlanMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.savePlan>[1]) => admin.savePlan(apiClient, data),
    [adminKeys.plans],
  );
}

export function useDropPlanMutation() {
  return useInvalidatingMutation((id: number) => admin.dropPlan(apiClient, id), [adminKeys.plans]);
}

export function useUpdatePlanMutation() {
  return useInvalidatingMutation(
    (vars: { id: number; key: 'show' | 'renew'; value: 0 | 1 }) =>
      admin.updatePlan(apiClient, vars.id, vars.key, vars.value),
    [adminKeys.plans],
  );
}

export function useSortPlansMutation() {
  return useInvalidatingMutation(
    (ids: number[]) => admin.sortPlans(apiClient, ids),
    [adminKeys.plans],
  );
}

export function useUpdateUserMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.updateUser>[1]) => admin.updateUser(apiClient, data),
    [
      ['admin', 'users'],
      ['admin', 'user'],
    ],
  );
}

export function useDeleteUserMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.deleteUser(apiClient, id),
    [['admin', 'users']],
  );
}

export function useResetUserSecretMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.resetUserSecret(apiClient, id),
    [
      ['admin', 'users'],
      ['admin', 'user'],
    ],
  );
}

export function useGenerateUserMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.generateUser>[1]) => admin.generateUser(apiClient, data),
    [['admin', 'users']],
    INLINE_MUTATION_ERROR_META,
  );
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
    meta: INLINE_MUTATION_ERROR_META,
  });
}

export function useBanUsersMutation() {
  return useInvalidatingMutation(
    (filter?: admin.AdminFilter[]) => admin.banUsers(apiClient, filter),
    [['admin', 'users']],
  );
}

export function useDeleteAllUsersMutation() {
  return useInvalidatingMutation(
    (filter?: admin.AdminFilter[]) => admin.deleteAllUsers(apiClient, filter),
    [['admin', 'users']],
  );
}

export function useMarkOrderPaidMutation() {
  return useInvalidatingMutation(
    (tradeNo: string) => admin.paidOrder(apiClient, tradeNo),
    [['admin', 'orders']],
  );
}

export function useCancelOrderMutation() {
  return useInvalidatingMutation(
    (tradeNo: string) => admin.cancelOrder(apiClient, tradeNo),
    [['admin', 'orders']],
  );
}

export function useUpdateOrderMutation() {
  return useInvalidatingMutation(
    (vars: { tradeNo: string; key: 'commission_status' | 'status'; value: string | number }) =>
      admin.updateOrder(apiClient, vars.tradeNo, vars.key, vars.value),
    [
      ['admin', 'orders'],
      ['admin', 'order'],
    ],
  );
}

export function useAssignOrderMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.assignOrder>[1]) => admin.assignOrder(apiClient, data),
    [['admin', 'orders']],
    INLINE_MUTATION_ERROR_META,
  );
}

export function useReplyTicketMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.replyTicket>[1]) => admin.replyTicket(apiClient, data),
    [
      ['admin', 'tickets'],
      ['admin', 'ticket'],
    ],
  );
}

export function useCloseTicketMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.closeTicket(apiClient, id),
    [
      ['admin', 'tickets'],
      ['admin', 'ticket'],
    ],
  );
}

export function useSaveNoticeMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.saveNotice>[1]) => admin.saveNotice(apiClient, data),
    [['admin', 'notices']],
  );
}

export function useDropNoticeMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropNotice(apiClient, id),
    [['admin', 'notices']],
  );
}

export function useShowNoticeMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.showNotice(apiClient, id),
    [['admin', 'notices']],
  );
}

/**
 * A system-config field keeps its local draft on failure and explicitly
 * refetches the authoritative config before clearing that draft on success.
 * Do not invalidate here as well, otherwise every blur would issue two config
 * refetches. Inline metadata prevents a duplicate global toast while the field
 * renders the backend error beside the control.
 */
export function useSaveSystemConfigMutation() {
  return useMutation({
    mutationFn: (data: Parameters<typeof admin.saveConfig>[1]) => admin.saveConfig(apiClient, data),
    meta: INLINE_MUTATION_ERROR_META,
  });
}

export function useSavePaymentMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.savePayment>[1]) => admin.savePayment(apiClient, data),
    [adminKeys.payments],
  );
}

export function useShowPaymentMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.showPayment(apiClient, id),
    [adminKeys.payments],
  );
}

export function useSortPaymentMutation() {
  return useInvalidatingMutation(
    (ids: number[]) => admin.sortPayments(apiClient, ids),
    [adminKeys.payments],
  );
}

export function useDropPaymentMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropPayment(apiClient, id),
    [adminKeys.payments],
  );
}

export function useGenerateCouponMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.generateCoupon>[1]) => admin.generateCoupon(apiClient, data),
    [['admin', 'coupons']],
  );
}

export function useDropCouponMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropCoupon(apiClient, id),
    [['admin', 'coupons']],
  );
}

export function useShowCouponMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.showCoupon(apiClient, id),
    [['admin', 'coupons']],
  );
}

export function useGenerateGiftcardMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.generateGiftcard>[1]) => admin.generateGiftcard(apiClient, data),
    [['admin', 'giftcards']],
  );
}

export function useDropGiftcardMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropGiftcard(apiClient, id),
    [['admin', 'giftcards']],
  );
}

export function useSaveKnowledgeMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.saveKnowledge>[1]) => admin.saveKnowledge(apiClient, data),
    [adminKeys.knowledge],
  );
}

export function useDropKnowledgeMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropKnowledge(apiClient, id),
    [adminKeys.knowledge],
  );
}

export function useShowKnowledgeMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.showKnowledge(apiClient, id),
    [adminKeys.knowledge],
  );
}

export function useSortKnowledgeMutation() {
  return useInvalidatingMutation(
    (ids: number[]) => admin.sortKnowledge(apiClient, ids),
    [adminKeys.knowledge],
  );
}

export function useSaveServerGroupMutation() {
  // Server editors close only from their per-call onSuccess callback. They do
  // not render API errors inline, so intentionally keep the default (global)
  // mutation presentation meta: one MutationCache toast, with the form intact.
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.saveServerGroup>[1]) => admin.saveServerGroup(apiClient, data),
    [adminKeys.serverGroups],
  );
}

export function useDropServerGroupMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropServerGroup(apiClient, id),
    [adminKeys.serverGroups],
  );
}

export function useSaveServerRouteMutation() {
  return useInvalidatingMutation(
    (data: Parameters<typeof admin.saveServerRoute>[1]) => admin.saveServerRoute(apiClient, data),
    [adminKeys.serverRoutes],
  );
}

export function useDropServerRouteMutation() {
  return useInvalidatingMutation(
    (id: number) => admin.dropServerRoute(apiClient, id),
    [adminKeys.serverRoutes],
  );
}

export function useDropServerMutation() {
  return useInvalidatingMutation(
    (vars: { type: admin.ServerTypeName; id: number }) =>
      admin.dropServer(apiClient, vars.type, vars.id),
    [adminKeys.serverNodes],
  );
}

export function useSaveServerMutation() {
  return useInvalidatingMutation(
    (vars: admin.SaveServerRequest) => admin.saveServer(apiClient, vars.type, vars.data),
    [adminKeys.serverNodes],
  );
}

export function useCopyServerMutation() {
  return useInvalidatingMutation(
    (vars: { type: admin.ServerTypeName; id: number }) =>
      admin.copyServer(apiClient, vars.type, vars.id),
    [adminKeys.serverNodes],
  );
}

export function useUpdateServerMutation() {
  return useInvalidatingMutation(
    (vars: { type: admin.ServerTypeName; id: number; key: 'show'; value: 0 | 1 }) =>
      admin.updateServer(apiClient, vars.type, vars.id, vars.key, vars.value),
    [adminKeys.serverNodes],
  );
}

export function useSortServerNodesMutation() {
  return useInvalidatingMutation(
    (payload: Parameters<typeof admin.sortServerNodes>[1]) =>
      admin.sortServerNodes(apiClient, payload),
    [adminKeys.serverNodes],
  );
}
