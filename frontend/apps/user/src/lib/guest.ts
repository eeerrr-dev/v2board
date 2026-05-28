import { guest, passport } from '@v2board/api-client';
import { useMutation, useQuery } from '@tanstack/react-query';
import { apiClient } from './api';

export const useGuestConfig = () =>
  useQuery({
    queryKey: ['guest', 'config'],
    queryFn: () => guest.config(apiClient),
    staleTime: 0,
    refetchOnMount: 'always',
  });

export const useLoginMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.login>[1]) => passport.login(apiClient, payload),
  });

export const useTokenLoginMutation = () =>
  useMutation({
    mutationFn: (payload: Parameters<typeof passport.token2Login>[1]) =>
      passport.token2Login(apiClient, payload),
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
