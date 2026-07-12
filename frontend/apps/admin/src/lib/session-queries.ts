import { queryOptions } from '@tanstack/react-query';
import { user } from '@v2board/api-client';
import { apiClient } from './api';

export const adminSessionKeys = {
  session: ['admin', 'session'] as const,
  userInfo: ['admin', 'user-info'] as const,
};

export const adminSessionQueryOptions = {
  session: () =>
    queryOptions({
      queryKey: adminSessionKeys.session,
      queryFn: ({ signal }) => user.checkLogin(apiClient, { signal }),
      staleTime: 60_000,
    }),
  userInfo: () =>
    queryOptions({
      queryKey: adminSessionKeys.userInfo,
      queryFn: ({ signal }) => user.info(apiClient, { signal }),
      staleTime: 60_000,
    }),
};
