import { useCallback, useEffect, useState, type BaseSyntheticEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { ApiError, user } from '@v2board/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { zodResolver } from '@hookform/resolvers/zod';
import { useForm, type UseFormRegister } from 'react-hook-form';
import { z } from 'zod';
import { apiClient } from '@/lib/api';
import { getAuthData, setAuthData } from '@/lib/auth';
import { i18nGet } from '@/lib/errors';
import { useLoginMutation, useTokenLoginMutation } from '@/lib/guest';
import { userQueryOptions } from '@/lib/queries';

const loginSchema = z.object({
  email: z.string(),
  password: z.string(),
});

type LoginFormValues = z.infer<typeof loginSchema>;

function normalizeRedirectTarget(target: string | null): string {
  if (!target) return '/dashboard';
  if (target.startsWith('//')) return '/dashboard';
  return target.startsWith('/') ? target : `/${target}`;
}

export interface LoginController {
  registerInput: UseFormRegister<LoginFormValues>;
  submit: (event?: BaseSyntheticEvent) => Promise<void>;
  /** Dismisses the inline error (e.g. once the user edits the form). */
  clearError: () => void;
  isPending: boolean;
  error: string | null;
}

// Authored V2Board — login behavior controller. Owns the auth mutations, the token2Login +
// existing-session bootstrap effect, and the submit flow. The request/redirect contract keeps the
// legacy payloads and eager user-info fetch, while the mount bootstrap now ignores stale async
// completions after route/effect cleanup.
export function useLoginController(): LoginController {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const queryClient = useQueryClient();
  const { mutateAsync, isPending } = useLoginMutation();
  const { mutateAsync: tokenLogin } = useTokenLoginMutation();
  const [error, setError] = useState<string | null>(null);
  const form = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: '', password: '' },
  });

  const queryRedirect = params.get('redirect');
  const redirect = normalizeRedirectTarget(queryRedirect);
  const verify = params.get('verify');

  const login = useCallback(
    async ({ email, password }: LoginFormValues) => {
      setError(null);
      try {
        const result = await mutateAsync({ email, password });
        setAuthData(result.auth_data);
        // The saga dispatched user/getUserInfo with `put`, then immediately pushed — it never
        // waited for the user-info request to settle.
        void queryClient
          .fetchQuery(userQueryOptions.info())
          .catch(() => undefined);
        navigate(redirect);
      } catch (err) {
        // The login mutation rejects with ApiError. Transport failures (status 0) surfaced nothing in
        // the oracle (the api-client toast model); everything else is shown inline beside the form
        // (the global toast also fires).
        if (err instanceof ApiError && err.status === 0) return;
        setError((err instanceof Error && err.message) || i18nGet('请求失败'));
      }
    },
    [mutateAsync, navigate, queryClient, redirect],
  );
  const submit = form.handleSubmit(login);

  useEffect(() => {
    let active = true;

    const finishLogin = (authData: string) => {
      if (!active) return;
      setAuthData(authData);
      navigate(redirect);
    };

    if (verify) {
      tokenLogin({
        verify,
        ...(queryRedirect !== null ? { redirect: queryRedirect } : {}),
      })
        .then((result) => {
          if (result?.auth_data) finishLogin(result.auth_data);
        })
        .catch(() => undefined);
    }

    if (getAuthData()) {
      user.checkLogin(apiClient)
        .then((result) => {
          if (active && result.is_login) {
            void queryClient
              .fetchQuery(userQueryOptions.info())
              .catch(() => undefined);
            navigate(redirect);
          }
        })
        .catch(() => undefined);
    }

    return () => {
      active = false;
    };
  }, [navigate, queryClient, queryRedirect, redirect, tokenLogin, verify]);

  const clearError = useCallback(() => setError(null), []);

  return { registerInput: form.register, submit, clearError, isPending, error };
}
