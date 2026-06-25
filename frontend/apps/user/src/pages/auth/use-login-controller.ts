import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { user } from '@v2board/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { apiClient } from '@/lib/api';
import { getAuthData, setAuthData } from '@/lib/auth';
import { i18nGet } from '@/lib/errors';
import { useLoginMutation, useTokenLoginMutation } from '@/lib/guest';
import { fetchUserInfo, userKeys } from '@/lib/queries';

function normalizeRedirectTarget(target: string | null): string {
  if (!target) return '/dashboard';
  return target.startsWith('/') ? target : `/${target}`;
}

export interface LoginController {
  /** Form submit handler — reads the live submitted values and runs the login mutation. */
  submit: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  /** Dismisses the inline error (e.g. once the user edits the form). */
  clearError: () => void;
  isPending: boolean;
  error: string | null;
}

// Authored V2Board — login behavior controller. Owns the auth mutations, the token2Login +
// existing-session bootstrap effect, and the submit flow. The request/redirect contract matches
// the packaged oracle exactly (payload {email,password}, setAuthData, eager user-info fetch,
// normalized redirect, fire-and-forget bootstrap effect with no cleanup flag); only the submit
// *mechanism* is modernized
// to a native <form> submit (see use-login-controller / login behavior tests for the re-pin).
export function useLoginController(): LoginController {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const queryClient = useQueryClient();
  const { mutateAsync, isPending } = useLoginMutation();
  const { mutateAsync: tokenLogin } = useTokenLoginMutation();
  const [error, setError] = useState<string | null>(null);

  const queryRedirect = params.get('redirect');
  const redirect = normalizeRedirectTarget(queryRedirect);
  const verify = params.get('verify');

  const submit = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      // Uncontrolled form — read the live submitted values, matching the old component which read
      // straight off the DOM at submit time rather than from controlled state.
      const form = new FormData(event.currentTarget);
      const email = String(form.get('email') ?? '');
      const password = String(form.get('password') ?? '');
      setError(null);
      try {
        const result = await mutateAsync({ email, password });
        setAuthData(result.auth_data);
        // The saga dispatched user/getUserInfo with `put`, then immediately pushed — it never
        // waited for the user-info request to settle.
        void queryClient
          .fetchQuery({ queryKey: userKeys.info, queryFn: fetchUserInfo })
          .catch(() => undefined);
        navigate(redirect);
      } catch (err) {
        // Transport failures (status 0) surfaced nothing in the oracle, matching the api-client
        // toast model; everything else is shown inline beside the form (the global toast also fires).
        if ((err as { status?: number } | null)?.status === 0) return;
        setError((err as { message?: string } | null)?.message || i18nGet('请求失败'));
      }
    },
    [mutateAsync, navigate, queryClient, redirect],
  );

  useEffect(() => {
    const finishLogin = (authData: string) => {
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
          if (result.is_login) {
            void queryClient
              .fetchQuery({ queryKey: userKeys.info, queryFn: fetchUserInfo })
              .catch(() => undefined);
            navigate(redirect);
          }
        })
        .catch(() => undefined);
    }
  }, [navigate, queryClient, queryRedirect, redirect, tokenLogin, verify]);

  return { submit, clearError: () => setError(null), isPending, error };
}
