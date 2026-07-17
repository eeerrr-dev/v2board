import { INLINE_MUTATION_ERROR_META, guest, passport } from '@v2board/api-client';
import { useMutation, useQuery } from '@tanstack/react-query';
import { apiClient } from './api';

export const useGuestConfig = () =>
  useQuery({
    queryKey: ['guest', 'config'],
    queryFn: ({ signal }) => guest.config(apiClient, { signal }),
    staleTime: 0,
    refetchOnMount: 'always',
  });

export const useLoginMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.login>[1]) =>
      passport.login(apiClient, payload),
    meta: INLINE_MUTATION_ERROR_META,
  });

export const useTokenLoginMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.tokenLogin>[1]) =>
      passport.tokenLogin(apiClient, payload),
  });

export const useRegisterMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.register>[1]) =>
      passport.register(apiClient, payload),
  });

export const useForgetMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.forget>[1]) =>
      passport.forget(apiClient, payload),
  });

export const useSendEmailVerifyMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.sendEmailVerify>[1]) =>
      passport.sendEmailVerify(apiClient, payload),
  });
