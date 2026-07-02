import { useEffect, type BaseSyntheticEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { ApiError, user } from '@v2board/api-client';
import type { AuthData } from '@v2board/types';
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

// A one-time verify token (the backend-emailed `?verify=` handoff) must be redeemed
// exactly once. React 19 StrictMode double-invokes the bootstrap effect in dev
// (mount → cleanup → mount), which would POST token2Login twice and fail the surviving
// call against the already-consumed token. De-dupe by verify value at module scope: the
// first effect run owns the in-flight redemption promise, a doubled run re-attaches to
// it (so the surviving mount still finishes login via its own `active` guard), and the
// entry is released once the request settles so a fresh visit re-redeems. This mirrors
// the file's existing race guards without touching the request or redirect contract.
const pendingVerifyRedemptions = new Map<string, Promise<AuthData | null>>();

function normalizeRedirectTarget(target: string | null): string {
  if (!target) return '/dashboard';
  // Browsers strip tab/newline characters and resolve backslashes as forward
  // slashes when parsing a URL, so "/\\evil.example" or "/\tevil" would slip past
  // a literal "//" guard and resolve cross-origin. Normalize the same way before
  // the protocol-relative check; bare relative targets keep their slash repair.
  const normalized = target
    .replace(/[\t\n\r]/g, '')
    .trim()
    .replace(/\\/g, '/');
  if (normalized.startsWith('//')) return '/dashboard';
  return normalized.startsWith('/') ? normalized : `/${normalized}`;
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
  const form = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: '', password: '' },
  });
  // Server-side login failures live in react-hook-form's reserved `root` error namespace rather
  // than a parallel useState, so the inline alert is the single source of truth for this surface.
  const error = form.formState.errors.root?.serverError?.message ?? null;

  const queryRedirect = params.get('redirect');
  const redirect = normalizeRedirectTarget(queryRedirect);
  const verify = params.get('verify');

  // React Compiler keeps this stable; handleSubmit rewraps it each render anyway.
  const login = async ({ email, password }: LoginFormValues) => {
    form.clearErrors('root.serverError');
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
      form.setError('root.serverError', {
        message: (err instanceof Error && err.message) || i18nGet('请求失败'),
      });
    }
  };
  const submit = form.handleSubmit(login);

  useEffect(() => {
    let active = true;

    const finishLogin = (authData: string) => {
      if (!active) return;
      setAuthData(authData);
      navigate(redirect);
    };

    if (verify) {
      // Redeem this verify token at most once per value (see pendingVerifyRedemptions):
      // the first run creates the request, a StrictMode-doubled run re-attaches to the
      // same promise instead of re-POSTing the one-time token.
      let redemption = pendingVerifyRedemptions.get(verify);
      if (!redemption) {
        redemption = tokenLogin({
          verify,
          ...(queryRedirect !== null ? { redirect: queryRedirect } : {}),
        });
        pendingVerifyRedemptions.set(verify, redemption);
        void redemption
          .catch(() => undefined)
          .finally(() => pendingVerifyRedemptions.delete(verify));
      }
      redemption
        .then((result) => {
          if (result?.auth_data) finishLogin(result.auth_data);
        })
        .catch(() => undefined);
    }

    // Skip the stale-session probe while a verify token is being redeemed:
    // token2Login is minting a fresh auth_data, and a late checkLogin resolving
    // `is_login:false` against the OLD token would wipe the freshly-minted one
    // and silently deauth the user. The two token-writing branches must not race.
    if (!verify && getAuthData()) {
      user.checkLogin(apiClient)
        .then((result) => {
          if (active && result.is_login) {
            void queryClient
              .fetchQuery(userQueryOptions.info())
              .catch(() => undefined);
            navigate(redirect);
          } else if (active) {
            setAuthData(null);
          }
        })
        .catch(() => undefined);
    }

    return () => {
      active = false;
    };
  }, [navigate, queryClient, queryRedirect, redirect, tokenLogin, verify]);

  // RHF only auto-clears root errors on the next submit, so the form-level onInput keeps wiring here
  // to dismiss the alert the moment the user edits a field without resubmitting.
  const clearError = () => form.clearErrors('root.serverError');

  return { registerInput: form.register, submit, clearError, isPending, error };
}
